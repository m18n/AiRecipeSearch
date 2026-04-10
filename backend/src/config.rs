use std::time::Duration;


pub const SERP_COST_PER_SEARCH_USD: f64 = 0.0015;

pub const JINA_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const JINA_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const JINA_COST_PER_PAGE_USD: f64 = 0.0;
pub const GROQ_RATE_LIMIT_DEFAULT_WAIT_MINUTES: u32 = 1;

pub const SERP_RATE_LIMIT_DEFAULT_WAIT_MINUTES: u32 = 5;
pub const SERP_RATE_LIMIT_PER_SECOND: u32 = 100;
pub const JINA_RATE_LIMIT_DEFAULT_WAIT_MINUTES: u32 = 5;
pub const JINA_RATE_LIMIT_PER_MINUTES: u32 = 500;

pub const PASSWORD_INIT_TOKEN_TTL_HOURS: i64 = 72;


pub const CORS_ALLOWED_ORIGINS: &[&str] = &["http://localhost:5173", "http://localhost:8080"];
pub const CORS_ALLOWED_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
pub const CORS_ALLOWED_HEADERS: &[&str] = &["Authorization", "Content-Type", "Accept"];


#[derive(Debug, Clone)]
pub struct GroqModelProfile {
    pub name: String,
    /// Requests per minute
    pub rpm_limit: u32,
    /// Tokens per minute
    pub tpm_limit: u32,
    /// Default wait when rate-limited (minutes)
    pub default_wait_minutes: u32,
    /// USD per 1 000 input tokens
    pub input_cost_per_1k: f64,
    /// USD per 1 000 output tokens
    pub output_cost_per_1k: f64,
    /// Enable chain-of-thought ("default" | "turbo" | "none")
    pub reasoning_effort: Option<String>,
}

impl GroqModelProfile {
    /// Build a lite (fast/cheap) profile from env, with sensible defaults.
    pub fn lite_from_env() -> Self {
        Self {
            name: std::env::var("MODEL_LITE")
                .unwrap_or_else(|_| "llama3-8b-8192".into()),
            rpm_limit: parse_u32_env("GROQ_LITE_RPM", 30),
            tpm_limit: parse_u32_env("GROQ_LITE_TPM", 14_400),
            default_wait_minutes: parse_u32_env("GROQ_LITE_WAIT_MIN", 1),
            input_cost_per_1k: parse_f64_env("GROQ_LITE_INPUT_COST_1K", 0.0_f64),
            output_cost_per_1k: parse_f64_env("GROQ_LITE_OUTPUT_COST_1K", 0.0_f64),
            reasoning_effort: None,
        }
    }

    /// Build a pro (powerful/thinking) profile from env, with sensible defaults.
    pub fn pro_from_env() -> Self {
        Self {
            name: std::env::var("MODEL_PRO")
                .unwrap_or_else(|_| "deepseek-r1-distill-llama-70b".into()),
            rpm_limit: parse_u32_env("GROQ_PRO_RPM", 30),
            tpm_limit: parse_u32_env("GROQ_PRO_TPM", 6_000),
            default_wait_minutes: parse_u32_env("GROQ_PRO_WAIT_MIN", 1),
            input_cost_per_1k: parse_f64_env("GROQ_PRO_INPUT_COST_1K", 0.0_f64),
            output_cost_per_1k: parse_f64_env("GROQ_PRO_OUTPUT_COST_1K", 0.0_f64),
            reasoning_effort: std::env::var("GROQ_PRO_REASONING_EFFORT")
                .ok()
                .filter(|s| !s.is_empty()),
        }
    }
}



pub fn parse_u32_env(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn parse_f64_env(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub db_pool_size: u32,
    pub admin_user_id: i32,

    pub port: u16,
    pub frontend_dist: String,
    pub jwt_access_secret: String,
    pub jwt_access_expiration: Duration,
    pub jwt_refresh_secret: String,
    pub jwt_refresh_expiration: Duration,
    pub serp_api_key: String,
    pub jina_api_key: String,

    pub groq_api_key: String,
    pub groq_lite: GroqModelProfile,
    pub groq_pro: GroqModelProfile,
    pub app_base_url: String,

}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            database_url: require("DATABASE_URL")?,
            db_pool_size: std::env::var("DB_POOL_SIZE")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .unwrap_or(10),

            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .unwrap_or(8080),
            frontend_dist: std::env::var("FRONTEND_DIST")
                .unwrap_or_else(|_| "./frontend/dist".into()),
            jwt_access_secret: require("JWT_ACCESS_SECRET")?,
            jwt_access_expiration: parse_duration_secs(
                &std::env::var("JWT_ACCESS_EXPIRATION").unwrap_or_else(|_| "900".into()),
                900,
            ),
            jwt_refresh_secret: require("JWT_REFRESH_SECRET")?,
            jwt_refresh_expiration: parse_duration_secs(
                &std::env::var("JWT_REFRESH_EXPIRATION").unwrap_or_else(|_| "2592000".into()),
                2_592_000,
            ),
            serp_api_key: require("SERP_API_KEY")?,
            jina_api_key: require("JINA_API_KEY")?,
            groq_api_key: require("GROQ_API_KEY")?,
            groq_lite: GroqModelProfile::lite_from_env(),
            groq_pro: GroqModelProfile::pro_from_env(),
            admin_user_id: std::env::var("ADMIN_USER_ID")
                .unwrap_or_else(|_| "1".into())
                .parse()
                .unwrap_or(1),

            app_base_url: std::env::var("APP_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".into()),
        })
    }
}



/// Returns the env var value or a descriptive error.
fn require(key: &str) -> Result<String, Box<dyn std::error::Error>> {
    std::env::var(key).map_err(|_| format!("Missing required env var: {key}").into())
}

/// Parses a seconds string into a `Duration`, falling back to `default_secs`.
fn parse_duration_secs(value: &str, default_secs: u64) -> Duration {
    Duration::from_secs(value.parse().unwrap_or(default_secs))
}