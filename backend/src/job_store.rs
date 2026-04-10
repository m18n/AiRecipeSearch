use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;
use crate::models::barcode_import::{FailedBarcode, ImportedIngredient};



#[derive(Debug, Clone)]
pub enum JobResult {
    RecipeResult {
        markdown_content: String,
        /// Total cost of all third-party API calls (SerpAPI + Gemini + Jina),
        /// rounded to six decimal places.
        cost_usd: f64,
    },
    ImportResult {
        imported: Vec<ImportedIngredient>,
        failed: Vec<FailedBarcode>,
    },
}



#[derive(Debug, Clone)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed(JobResult),
    Failed(String),
    /// Terminal state — set when any upstream API returns HTTP 429.
    RateLimited {
        message: String,
        retry_after_minutes: u32,
    },
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Processing => "processing",
            JobStatus::Completed(_) => "completed",
            JobStatus::Failed(_) => "failed",
            JobStatus::RateLimited { .. } => "rate_limited",
        }
    }
}



#[derive(Debug, Clone)]
pub struct Job {
    pub job_id: Uuid,
    pub user_id: i32,
    pub status: JobStatus,
    pub created_at: Instant,
    /// Completion percentage (0–100). Updated during processing.
    pub progress: u8,
}



const JOB_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Clone)]
pub struct JobStore {
    inner: Arc<Mutex<HashMap<Uuid, Job>>>,
}

impl JobStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    /// Returns `true` if the user already has a job in `Pending` or `Processing` state.
    pub fn has_active_job(&self, user_id: i32) -> bool {
        self.inner
            .lock()
            .expect("job_store lock poisoned")
            .values()
            .any(|job| {
                job.user_id == user_id
                    && matches!(job.status, JobStatus::Pending | JobStatus::Processing)
            })
    }

    /// Creates a new job in `Pending` state, returns its `job_id`.
    pub fn create_job(&self, user_id: i32) -> Uuid {
        let job_id = Uuid::new_v4();
        let job = Job {
            job_id,
            user_id,
            status: JobStatus::Pending,
            created_at: Instant::now(),
            progress: 0,
        };

        self.inner
            .lock()
            .expect("job_store lock poisoned")
            .insert(job_id, job);

        job_id
    }

    /// Updates the progress percentage (clamped to 0–100).
    /// Silently does nothing if `job_id` is not found.
    pub fn set_progress(&self, job_id: Uuid, progress: u8) {
        let mut map = self.inner.lock().expect("job_store lock poisoned");
        if let Some(job) = map.get_mut(&job_id) {
            job.progress = progress.min(100);
        }
    }

    /// Overwrites the status of an existing job.
    /// Silently does nothing if `job_id` is not found.
    pub fn set_status(&self, job_id: Uuid, status: JobStatus) {
        let mut map = self.inner.lock().expect("job_store lock poisoned");
        if let Some(job) = map.get_mut(&job_id) {
            job.status = status;
        }
    }

    /// Returns a snapshot clone of the job, or `None` if not found.
    pub fn get_job(&self, job_id: Uuid) -> Option<Job> {
        self.inner
            .lock()
            .expect("job_store lock poisoned")
            .get(&job_id)
            .cloned()
    }

    /// Removes all jobs whose `created_at` exceeds `JOB_TTL`.
    /// Intended to be called periodically from a background `tokio::spawn` task.
    pub fn cleanup_old_jobs(&self) {
        let mut map = self.inner.lock().expect("job_store lock poisoned");
        map.retain(|_, job| job.created_at.elapsed() < JOB_TTL);
    }
}

impl Default for JobStore {
    fn default() -> Self {
        Self::new()
    }
}