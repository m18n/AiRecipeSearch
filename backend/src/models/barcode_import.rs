use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::job_store::{Job, JobResult, JobStatus};

/// Result of a single barcode lookup via SerpAPI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarcodeSearchResult {
    pub name: Option<String>,
    pub photo_link: Option<String>,
}

/// A successfully recognized product from a barcode lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedIngredient {
    pub barcode: String,
    pub name: String,
    pub photo_link: Option<String>,
}

/// A barcode for which no product could be found or an error occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedBarcode {
    pub barcode: String,
    pub reason: String,
}

/// The final aggregated result of a completed import job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJobResult {
    pub imported: Vec<ImportedIngredient>,
    pub failed: Vec<FailedBarcode>,
}

/// Response body returned by `GET /ingredients/import/:import_job_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJobResponse {
    pub import_job_id: Uuid,
    /// One of: "pending", "processing", "completed", "failed", "rate_limited".
    pub status: String,
    /// 0–100, присутній поки job не завершено.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Populated only when `status == "completed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ImportJobResult>,
    /// Populated only when `status == "rate_limited"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_minutes: Option<u32>,
    /// Populated when `status == "failed"` or `"rate_limited"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ImportJobResponse {
    pub fn from_job(job: &Job) -> Self {
        Self {
            import_job_id: job.job_id,
            status: job.status.as_str().to_string(),
            progress: Some(job.progress),
            result: match &job.status {
                JobStatus::Completed(JobResult::ImportResult { imported, failed }) => {
                    Some(ImportJobResult {
                        imported: imported.clone(),
                        failed: failed.clone(),
                    })
                }
                _ => None,
            },
            retry_after_minutes: match &job.status {
                JobStatus::RateLimited { retry_after_minutes, .. } => Some(*retry_after_minutes),
                _ => None,
            },
            error: match &job.status {
                JobStatus::Failed(reason) => Some(reason.clone()),
                JobStatus::RateLimited { message, .. } => Some(message.clone()),
                _ => None,
            },
        }
    }
}