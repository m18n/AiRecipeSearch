use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tiktoken_rs::{cl100k_base, o200k_base};
use tokio::sync::{watch, Mutex};
use tokio::time::timeout;

use crate::config::{Config, GroqModelProfile, GROQ_RATE_LIMIT_DEFAULT_WAIT_MINUTES};
use crate::error::AppError;
use crate::models::recipe::ModelUsage;

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const GENERATE_TIMEOUT: Duration     = Duration::from_secs(150);

#[derive(Debug, Clone, Copy)]
pub enum ModelTier {
    Lite,
    Pro,
}



struct ModelState {
    profile: GroqModelProfile,
    limiter: SlidingWindowLimiter,
    tpm: TpmLimiter,
    tpm_tx: watch::Sender<u32>,
    blackout_until: Option<Instant>,
}

impl ModelState {
    fn new(profile: GroqModelProfile) -> Self {
        let limiter = SlidingWindowLimiter::new(profile.rpm_limit, profile.default_wait_minutes);
        let tpm = TpmLimiter::new(profile.tpm_limit);
        let (tpm_tx, _) = watch::channel(0u32);
        Self { profile, limiter, tpm, tpm_tx, blackout_until: None }
    }

    /// Повертає Some(залишок), якщо зараз blackout.
    fn check_blackout(&self) -> Option<Duration> {
        self.blackout_until.and_then(|until| {
            let now = Instant::now();
            if until > now { Some(until - now) } else { None }
        })
    }

    /// Виставляємо blackout на `minutes` хвилин (але не менше поточного значення).
    fn set_blackout(&mut self, minutes: u32) {
        let deadline = Instant::now() + Duration::from_secs(minutes as u64 * 60);
        let current = self.blackout_until.unwrap_or(Instant::now());
        if deadline > current {
            self.blackout_until = Some(deadline);
        }
    }
}



struct SlidingWindowLimiter {
    window: VecDeque<(Instant, u64)>,
    limit: u32,
    default_wait_minutes: u32,
    next_id: u64,
}

impl SlidingWindowLimiter {
    fn new(limit: u32, default_wait_minutes: u32) -> Self {
        Self {
            window: VecDeque::new(),
            limit,
            default_wait_minutes,
            next_id: 0,
        }
    }

    /// Check without committing — returns wait duration if over limit.
    fn check(&mut self, source: &str) -> Result<Option<Duration>, AppError> {
        const WINDOW: Duration = Duration::from_secs(60);
        self.evict();

        if (self.window.len() as u32) < self.limit {
            return Ok(None);
        }
        if let Some((oldest, _)) = self.window.front() {
            let elapsed = oldest.elapsed();
            if elapsed < WINDOW {
                return Ok(Some(WINDOW - elapsed));
            }
        }
        Err(AppError::RateLimitError {
            message: source.to_string(),
            retry_after_minutes: self.default_wait_minutes,
        })
    }

    /// Commit after a successful check() — returns reservation id.
    fn commit(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.window.push_back((Instant::now(), id));
        id
    }

    /// Cancel a previously committed slot by id.
    fn cancel(&mut self, id: u64) {
        if let Some(pos) = self.window.iter().position(|(_, i)| *i == id) {
            self.window.remove(pos);
        }
    }

    fn evict(&mut self) {
        const WINDOW: Duration = Duration::from_secs(60);
        while self.window.front().map_or(false, |(t, _)| t.elapsed() >= WINDOW) {
            self.window.pop_front();
        }
    }
}



struct TpmLimiter {
    /// (timestamp, tokens, reservation_id)
    window: VecDeque<(Instant, u32, u64)>,
    limit: u32,
    next_id: u64,
}

impl TpmLimiter {
    fn new(limit: u32) -> Self {
        Self {
            window: VecDeque::new(),
            limit,
            next_id: 0,
        }
    }

    fn cancel_reservation(&mut self, id: u64) {
        if let Some(pos) = self.window.iter().position(|(_, _, i)| *i == id) {
            self.window.remove(pos);
        }
    }

    fn evict(&mut self) {
        const WINDOW: Duration = Duration::from_secs(60);
        while self
            .window
            .front()
            .map_or(false, |(t, _, _)| t.elapsed() >= WINDOW)
        {
            self.window.pop_front();
        }
    }

    fn current_tokens(&self) -> u32 {
        self.window.iter().map(|(_, t, _)| *t).sum()
    }

    /// Returns (wait_duration, reservation_id).
    /// On rejection: (Some(d), None).
    /// On success:   (None,    Some(id)).
    fn try_reserve(&mut self, tokens: u32) -> (Option<Duration>, Option<u64>) {
        const WINDOW: Duration = Duration::from_secs(60);
        self.evict();

        let tokens_with_buffer = (tokens as f32 * 1.15).ceil() as u32;

        if self.current_tokens() + tokens_with_buffer <= self.limit {
            let id = self.next_id;
            self.next_id += 1;
            self.window.push_back((Instant::now(), tokens_with_buffer, id));
            return (None, Some(id));
        }

        let mut freed: u32 = 0;
        let needed = (self.current_tokens() + tokens_with_buffer)
            .saturating_sub(self.limit);
        for (ts, tok, _) in &self.window {
            freed += tok;
            if freed >= needed {
                let elapsed = ts.elapsed();
                if elapsed < WINDOW {
                    return (Some(WINDOW - elapsed), None);
                }
                break;
            }
        }

        (Some(WINDOW), None)
    }

    fn adjust_reservation(&mut self, id: u64, actual: u32) {
        if let Some(entry) = self.window.iter_mut().find(|(_, _, i)| *i == id) {
            entry.1 = actual;
        }
    }
}



fn estimate_tokens(text: &str, model: &str) -> u32 {
    if model.starts_with("openai/") {
        return match o200k_base() {
            Ok(bpe) => bpe.encode_ordinary(text).len() as u32,
            Err(_) => match cl100k_base() {
                Ok(bpe) => bpe.encode_ordinary(text).len() as u32,
                Err(_) => ((text.chars().count() as f32) / 4.0).ceil() as u32,
            },
        };
    }

    if model.starts_with("meta-llama/llama-4") {
        return match o200k_base() {
            Ok(bpe) => bpe.encode_ordinary(text).len() as u32,
            Err(_) => ((text.chars().count() as f32) / 3.5).ceil() as u32,
        };
    }

    ((text.chars().count() as f32) / 4.0).ceil() as u32
}

fn estimate_request_tokens(body: &GroqRequest) -> u32 {
    let input_tokens: u32 = body
        .messages
        .iter()
        .map(|m| estimate_tokens(&m.content, &body.model) + 4)
        .sum();

    let expected_output = body.max_tokens;
    input_tokens + expected_output
}



#[derive(Serialize)]
struct GroqRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}



#[derive(Deserialize, Debug)]
struct GroqApiResponse {
    choices: Option<Vec<Choice>>,
    usage: Option<GroqUsage>,
    error: Option<GroqApiError>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Option<ChoiceMessage>,
    #[serde(rename = "finish_reason")]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GroqUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
struct GroqApiError {
    message: Option<String>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    error_type: Option<String>,
    code: Option<serde_json::Value>,
}



pub struct GroqTypedResponse<T> {
    pub data: T,
    pub usage: ModelUsage,
    pub cost_usd: f64,
}



#[derive(Clone)]
pub struct GroqClient {
    http: Client,
    api_key: String,
    lite: Arc<Mutex<ModelState>>,
    pro: Arc<Mutex<ModelState>>,
}

impl GroqClient {
    pub fn new(config: &Config) -> Self {
        Self {
            http: Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_REQUEST_TIMEOUT)
                .build()
                .expect("Failed to build HTTP client"),
            api_key: config.groq_api_key.clone(),
            lite: Arc::new(Mutex::new(ModelState::new(config.groq_lite.clone()))),
            pro: Arc::new(Mutex::new(ModelState::new(config.groq_pro.clone()))),
        }
    }

    /// Returns (rpm_id, tpm_id) once both gates pass.
    async fn acquire_slot(
        &self,
        tier: ModelTier,
        estimated_tokens: u32,
    ) -> Result<(u64, u64), AppError> {
        let mut rx = self.state(tier).lock().await.tpm_tx.subscribe();

        loop {
            let outcome = {
                let mut s = self.state(tier).lock().await;
                if let Some(wait) = s.check_blackout() {
                    Err(wait)
                }
                else {
                    match s.limiter.check("Groq")? {
                        Some(d) => Err(d),
                        None => {
                            match s.tpm.try_reserve(estimated_tokens) {
                                (None, Some(tpm_id)) => {
                                    let rpm_id = s.limiter.commit();
                                    Ok((rpm_id, tpm_id))
                                }
                                (wait, None) => Err(wait.unwrap_or(Duration::from_secs(60))),
                                _ => unreachable!(),
                            }
                        }
                    }
                }
            };

            match outcome {
                Ok(ids) => return Ok(ids),
                Err(d) => {
                    tokio::select! {
                        _ = tokio::time::sleep(d) => {},
                        _ = rx.changed() => {},
                    }
                }
            }
        }
    }

    pub async fn generate<T: DeserializeOwned>(
        &self,
        prompt: &str,
        temperature: f32,
        tier: ModelTier,
    ) -> Result<GroqTypedResponse<T>, AppError> {
        timeout(GENERATE_TIMEOUT, self.generate_inner(prompt, temperature, tier))
            .await
            .map_err(|_| AppError::InternalServerError(
                format!("Groq request timed out after {}s", GENERATE_TIMEOUT.as_secs())
            ))?
    }

    async fn generate_inner<T: DeserializeOwned>(
        &self,
        prompt: &str,
        temperature: f32,
        tier: ModelTier,
    ) -> Result<GroqTypedResponse<T>, AppError> {
        let (model_name, reasoning_effort) = {
            let state = self.state(tier).lock().await;
            (
                state.profile.name.clone(),
                state.profile.reasoning_effort.clone(),
            )
        };

        let body = GroqRequest {
            model: model_name,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: "You are a structured data assistant. \
                              Respond with valid JSON only — no markdown fences, \
                              no prose, no text outside the JSON object."
                        .to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: prompt.to_string(),
                },
            ],
            temperature,
            max_tokens: 8192,
            response_format: Some(ResponseFormat { kind: "json_object" }),
            reasoning_effort,
        };

        let estimated_tokens = estimate_request_tokens(&body);
        let (rpm_id, tpm_id) = self.acquire_slot(tier, estimated_tokens).await?;

        let (raw, usage) = match self.send_and_extract(&body).await {
            Ok(result) => result,
            Err(e) => {
                if let AppError::RateLimitError { retry_after_minutes, .. } = &e {
                    let mut state = self.state(tier).lock().await;
                    state.limiter.cancel(rpm_id);
                    state.tpm.cancel_reservation(tpm_id);
                    state.set_blackout(*retry_after_minutes);
                    let _ = state.tpm_tx.send(state.tpm.current_tokens());
                }
                return Err(e);
            }
        };

        let actual_total = usage.input_tokens + usage.output_tokens;
        let cost_usd = {
            let mut state = self.state(tier).lock().await;
            state.tpm.adjust_reservation(tpm_id, actual_total);

            let current = state.tpm.current_tokens();
            let _ = state.tpm_tx.send(current);

            let p = &state.profile;
            (usage.input_tokens as f64 / 1_000.0) * p.input_cost_per_1k
                + (usage.output_tokens as f64 / 1_000.0) * p.output_cost_per_1k
        };

        let data: T = serde_json::from_str(&raw).map_err(|e| {
            AppError::InternalServerError(format!(
                "Groq JSON deserialize error: {e}\nRaw response:\n{raw}"
            ))
        })?;

        Ok(GroqTypedResponse { data, usage, cost_usd })
    }

    fn state(&self, tier: ModelTier) -> &Arc<Mutex<ModelState>> {
        match tier {
            ModelTier::Lite => &self.lite,
            ModelTier::Pro => &self.pro,
        }
    }

    async fn send_and_extract(
        &self,
        body: &GroqRequest,
    ) -> Result<(String, ModelUsage), AppError> {
        let response = self
            .http
            .post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_secs = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(60.0);

            let retry_after_minutes = (retry_secs / 60.0).ceil() as u32;

            return Err(AppError::RateLimitError {
                message: "Groq".to_string(),
                retry_after_minutes: retry_after_minutes.max(1),
            });
        }

        let api_resp: GroqApiResponse = response
            .json()
            .await
            .map_err(|e| AppError::InternalServerError(format!("Groq parse error: {e}")))?;

        if let Some(ref err) = api_resp.error {
            let is_rate_limit = err
                .code
                .as_ref()
                .and_then(|c| c.as_str())
                .map(|s| s.contains("rate_limit") || s.contains("quota"))
                .unwrap_or(false)
                || err
                .message
                .as_deref()
                .map(is_rate_limit_message)
                .unwrap_or(false);

            if is_rate_limit {
                return Err(AppError::RateLimitError {
                    message: "Groq".to_string(),
                    retry_after_minutes: GROQ_RATE_LIMIT_DEFAULT_WAIT_MINUTES,
                });
            }

            return Err(AppError::InternalServerError(
                err.message
                    .clone()
                    .unwrap_or_else(|| "Unknown Groq error".to_string()),
            ));
        }

        let text = api_resp
            .choices
            .as_deref()
            .and_then(|c| c.first())
            .and_then(|c| c.message.as_ref())
            .and_then(|m| m.content.clone())
            .ok_or_else(|| {
                AppError::InternalServerError("Groq returned no content".to_string())
            })?;

        let usage = ModelUsage {
            input_tokens: api_resp
                .usage
                .as_ref()
                .and_then(|u| u.prompt_tokens)
                .unwrap_or(0),
            output_tokens: api_resp
                .usage
                .as_ref()
                .and_then(|u| u.completion_tokens)
                .unwrap_or(0),
        };

        Ok((text, usage))
    }
}



fn is_rate_limit_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("rate limit")
        || lower.contains("quota")
        || lower.contains("too many requests")
        || lower.contains("resource exhausted")
}