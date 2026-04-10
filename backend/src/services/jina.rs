use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::Client;
use tokio::sync::Mutex;
use tracing::instrument;

use crate::{
    config::{JINA_RATE_LIMIT_DEFAULT_WAIT_MINUTES, JINA_RATE_LIMIT_PER_MINUTES},
    error::AppError,
};
use crate::config::{JINA_CONNECT_TIMEOUT_SECS, JINA_REQUEST_TIMEOUT_SECS};

const JINA_READER_BASE_URL: &str = "https://r.jina.ai/";
const WINDOW: Duration = Duration::from_secs(60);



/// Thread-safe Jina Reader client with a sliding-window rate limiter.
/// Guarantees ≤ `JINA_RATE_LIMIT_PER_MINUTES` requests per 60-second window
/// while maximising throughput (no unnecessary waiting).
#[derive(Clone, Debug)]
pub struct JinaClient {
    client:        Client,
    api_key:       String,
    /// Timestamps of the last N dispatched requests (front = oldest).
    request_times: Arc<Mutex<VecDeque<Instant>>>,
}

impl JinaClient {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(JINA_REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(JINA_CONNECT_TIMEOUT_SECS))
            .build()
            .expect("Failed to build Jina HTTP client");

        Self {
            client,
            api_key,
            request_times: Arc::new(Mutex::new(VecDeque::with_capacity(
                JINA_RATE_LIMIT_PER_MINUTES as usize,
            ))),
        }
    }

    /// Acquires a "slot" in the current 60-second window.
    /// If the window is full, sleeps until the oldest request expires.
    async fn acquire_slot(&self) {
        let limit = JINA_RATE_LIMIT_PER_MINUTES as usize;

        loop {
            let mut times = self.request_times.lock().await;
            let now = Instant::now();
            while times.front().map_or(false, |&t| now.duration_since(t) >= WINDOW) {
                times.pop_front();
            }

            if times.len() < limit {
                times.push_back(now);
                return;
            }
            let oldest   = *times.front().expect("non-empty");
            let wait_for = WINDOW.saturating_sub(now.duration_since(oldest)) + Duration::from_millis(1);

            drop(times);

            tracing::debug!(
                wait_ms = wait_for.as_millis(),
                "Jina rate-limit: window full, sleeping"
            );
            tokio::time::sleep(wait_for).await;
        }
    }

    /// Fetches the Markdown content of a web page via the Jina Reader API.
    /// Blocks until a rate-limit slot is free, then dispatches the request.
    /// On HTTP 429 returns `AppError::RateLimitError { source: "Jina", retry_after_minutes }`.
    #[instrument(skip(self), fields(url = %url))]
    pub async fn fetch_page(&self, url: &str) -> Result<(String,String), AppError> {
        self.acquire_slot().await;

        let jina_url = format!("{}{}", JINA_READER_BASE_URL, url);

        let mut request = self
            .client
            .get(&jina_url)
            .header("X-With-Generated-Alt", "true")
            .header("x-respond-with", "markdown");

        if !self.api_key.is_empty() {
            request = request.header(
                "Authorization",
                format!("Bearer {}", self.api_key),
            );
        }

        let response = request.send().await.map_err(|e| {
            tracing::error!("Jina Reader request failed: {e}");
            AppError::InternalServerError(format!("Jina Reader request error: {e}"))
        })?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after_minutes = parse_retry_after_minutes(&response);
            tracing::warn!(
                retry_after_minutes,
                "Jina Reader returned 429 – rate limited"
            );
            return Err(AppError::RateLimitError {
                message: "Jina".to_string(),
                retry_after_minutes,
            });
        }
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Jina API Auth/Quota Error");
            return Err(AppError::InternalServerError(format!(
                "Jina Reader authentication/quota error: {body}"
            )));
        }
        if status == reqwest::StatusCode::BAD_REQUEST
            || status == reqwest::StatusCode::NOT_FOUND
        {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!(status = %status, body = %body, url = %url, "Target URL error");
            return Err(AppError::InternalServerError(format!(
                "Invalid target URL or page not found: {body}"
            )));
        }
        if status.as_u16() == 524 || status == reqwest::StatusCode::GATEWAY_TIMEOUT {
            tracing::warn!(url = %url, "Jina timed out while fetching target");
            return Err(AppError::InternalServerError(
                "Target website took too long to respond".to_string(),
            ));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Jina Reader error");
            return Err(AppError::InternalServerError(format!(
                "Jina Reader error {status}: {body}"
            )));
        }
        let text = response.text().await.map_err(|e| {
            tracing::error!("Failed to read body: {e}");
            AppError::InternalServerError(format!("Failed to read response: {e}"))
        })?;

        if text.trim().is_empty() {
            tracing::warn!(url, "Jina returned empty body");
            return Err(AppError::InternalServerError("Empty content".to_string()));
        }

        if is_garbage_content(&text) {
            tracing::warn!(url, "Jina returned anti-bot or error page content");
            return Err(AppError::InternalServerError(
                "Target website blocked the scraper (Captcha / Access Denied)".to_string(),
            ));
        }

        let real_url = extract_url_source(&text).unwrap_or_else(|| url.to_string());

        Ok((text, real_url))
    }
}



fn parse_retry_after_minutes(response: &reqwest::Response) -> u32 {
    if let Some(value) = response.headers().get(reqwest::header::RETRY_AFTER) {
        if let Ok(s) = value.to_str() {
            if let Ok(seconds) = s.trim().parse::<u64>() {
                return ((seconds + 59) / 60).max(1) as u32;
            }
        }
    }
    JINA_RATE_LIMIT_DEFAULT_WAIT_MINUTES
}

fn is_garbage_content(text: &str) -> bool {
    let text_lower = text.to_lowercase();

    let bad_titles = [
        "title: just a moment...",
        "title: attention required!",
        "title: access denied",
        "title: 403 forbidden",
        "title: 404 not found",
        "title: are you a robot?",
        "title: robot check",
    ];
    if bad_titles.iter().any(|t| text_lower.contains(t)) {
        return true;
    }

    let bot_signatures = [
        "enable javascript and cookies to continue",
        "checking your browser before accessing",
        "please verify you are a human",
        "cf-browser-verification",
        "why have i been blocked?",
        "please enable js and disable any ad blocker",
    ];
    if text.len() < 2500 && bot_signatures.iter().any(|s| text_lower.contains(s)) {
        return true;
    }

    false
}
fn extract_url_source(text: &str) -> Option<String> {
    text.lines()
        .take(10)
        .find(|line| line.to_lowercase().starts_with("url source:"))
        .and_then(|line| line.splitn(2, ':').nth(1))
        .map(|url| url.trim().to_string())
        .filter(|url| url.starts_with("http"))
}