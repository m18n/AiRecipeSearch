use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::config::{SERP_RATE_LIMIT_DEFAULT_WAIT_MINUTES, SERP_RATE_LIMIT_PER_SECOND};
use crate::error::AppError;
use crate::models::barcode_import::BarcodeSearchResult;



#[derive(Debug, Deserialize)]
struct BrightResponse {
    #[serde(default)]
    organic: Vec<OrganicItem>,
    #[serde(default)]
    images: Vec<ImageItem>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrganicItem {
    title: Option<String>,
    link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageItem {
    image: Option<String>,
    original_image: Option<String>,
    link: Option<String>,
}

impl ImageItem {
    fn best_url(&self) -> Option<&str> {
        self.image
            .as_deref()
            .or_else(|| self.original_image.as_deref())
            .or_else(|| self.link.as_deref())
    }
}



#[derive(Debug)]
pub struct RecipeSourcesResult {
    pub sources: Vec<RecipeSource>,
}

#[derive(Debug)]
pub struct RecipeSource {
    pub title: String,
    pub link: String,
}



/// Fixed-window counter: allows up to `max_per_second` calls per 1-second window.
/// Exceeding the limit causes a short sleep until the window resets.
struct RateLimiterState {
    /// Fractional tokens currently available.
    tokens: f64,
    /// When the bucket was last refilled.
    last_refill: Instant,
}

struct RateLimiter {
    state: Mutex<RateLimiterState>,
    /// How many tokens per second (also the bucket capacity).
    rate: f64,
}


impl RateLimiter {
    fn new(max_per_second: u32) -> Self {
        let rate = max_per_second as f64;
        Self {
            rate,
            state: Mutex::new(RateLimiterState {
                tokens: rate,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Waits until a token is available, then consumes it.
    async fn acquire(&self) {
        loop {
            let sleep_for = {
                let mut s = self.state.lock().await;
                let elapsed = s.last_refill.elapsed().as_secs_f64();
                s.tokens = (s.tokens + elapsed * self.rate).min(self.rate);
                s.last_refill = Instant::now();

                if s.tokens >= 1.0 {
                    s.tokens -= 1.0;
                    break;
                }
                let secs_needed = (1.0 - s.tokens) / self.rate;
                Duration::from_secs_f64(secs_needed)
            };

            tokio::time::sleep(sleep_for).await;
        }
    }
}



/// Stateful SERP client with built-in rate limiting.
/// Create once (e.g. via `Arc`) and share across requests.
pub struct SerpClient {
    client: Client,
    limiter: Arc<RateLimiter>,
    api_key:String
}

impl SerpClient {
    pub fn new(api_key:String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            api_key,
            limiter: Arc::new(RateLimiter::new(SERP_RATE_LIMIT_PER_SECOND)),
        }
    }

    async fn request(&self, url: &str) -> Result<BrightResponse, AppError> {
        self.limiter.acquire().await;

        let payload = json!({
            "zone": "serp_api2",
            "url": url,
            "format": "raw",
            "data_format": "parsed_light"
        });

        let response = self
            .client
            .post("https://api.brightdata.com/request")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = parse_retry_after_minutes(
                response.headers(),
                SERP_RATE_LIMIT_DEFAULT_WAIT_MINUTES,
            );
            return Err(AppError::RateLimitError {
                message: "BrightData".to_string(),
                retry_after_minutes: retry_after,
            });
        }

        let status = response.status();
        let headers = response.headers().clone();
        let body: BrightResponse = response
            .json()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        if let Some(ref err_msg) = body.error {
            if is_rate_limit_message(err_msg) {
                let retry_after =
                    parse_retry_after_minutes(&headers, SERP_RATE_LIMIT_DEFAULT_WAIT_MINUTES);
                return Err(AppError::RateLimitError {
                    message: "BrightData".to_string(),
                    retry_after_minutes: retry_after,
                });
            }
            return Err(AppError::InternalServerError(format!(
                "BrightData error: {err_msg}"
            )));
        }

        if !status.is_success() {
            return Err(AppError::InternalServerError(format!(
                "BrightData returned status {status}"
            )));
        }

        Ok(body)
    }

    /// Search for recipe web sources by a free-text query.
    pub async fn search_recipe_sources(
        &self,
        query: &str
    ) -> Result<RecipeSourcesResult, AppError> {
        let url = format!(
            "https://www.google.com/search?q={}&num=5&brd_json=1",
            urlencoding::encode(query)
        );

        let body = self.request(&url).await?;

        let sources = body
            .organic
            .into_iter()
            .filter_map(|r| match (r.title, r.link) {
                (Some(title), Some(link)) => Some(RecipeSource { title, link }),
                _ => None,
            })
            .collect();

        Ok(RecipeSourcesResult { sources })
    }

    /// Search for a product by its barcode (organic + image search).
    pub async fn search_by_barcode(
        &self,
        barcode: &str,
        country_code: Option<&str>
    ) -> Result<BarcodeSearchResult, AppError> {
        let mut country="SE";
        if let Some(cc) = country_code {
            country=cc;
        }

        let organic_url = format!(
            "https://www.google.com/search?q={}&gl={}&num=3&brd_json=1",
            urlencoding::encode(&format!("{barcode} food")),country
        );

        let organic_body = self.request(&organic_url).await?;

        let name = organic_body
            .organic
            .iter()
            .find_map(|r| r.title.clone())
            .map(|title| {
                if let Some(pos) = title.find("Open Food Facts") {
                    title[..pos]
                        .trim_end_matches(|c: char| !c.is_alphabetic())
                        .to_owned()
                } else {
                    title
                }
            });
        let mut photo_link = None;

        if let Some(ref product_name) = name {
            let image_url = format!(
                "https://www.google.com/search?q={}&gl={}&num=3&brd_json=1&tbm=isch",
                urlencoding::encode(&format!("{product_name} food")),country
            );

            let image_body = self.request(&image_url).await?;

            photo_link = image_body
                .images
                .iter()
                .find_map(|img| img.best_url().map(str::to_owned));
        }

        Ok(BarcodeSearchResult { name, photo_link })
    }
}





fn parse_retry_after_minutes(headers: &reqwest::header::HeaderMap, default_minutes: u32) -> u32 {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| ((secs + 59) / 60) as u32)
        .unwrap_or(default_minutes)
}

fn is_rate_limit_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("rate") || lower.contains("quota") || lower.contains("limit")
}