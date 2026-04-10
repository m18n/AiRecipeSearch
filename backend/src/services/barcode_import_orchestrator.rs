use std::sync::Arc;
use futures::future::join_all;
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use uuid::Uuid;

use crate::{
    db::user_cooking_profile::insert_ingredients,
    error::AppError,
    job_store::{JobResult, JobStatus, JobStore},
    models::user_cooking_profile::IngredientDto,
};
use sqlx::PgPool;
use crate::config::Config;
use crate::models::barcode_import::{FailedBarcode, ImportedIngredient};
use crate::services::serp::SerpClient;

enum TaskOutcome {
    Imported(ImportedIngredient),
    Failed(FailedBarcode),
    RateLimited { message: String, retry_after_minutes: u32 },
}

pub async fn run(
    db: PgPool,
    job_store: Arc<JobStore>,
    user_id: i32,
    job_id: Uuid,
    barcodes: Vec<String>,
    serp_client: Arc<SerpClient>,
    country_code: Option<String>,
) {
    job_store.set_status(job_id, JobStatus::Processing);
    if barcodes.is_empty() {
        job_store.set_status(
            job_id,
            JobStatus::Completed(JobResult::ImportResult {
                imported: vec![],
                failed: vec![],
            }),
        );
        return;
    }
    let total = barcodes.len();
    let mut completed = 0usize;
    let mut futures: FuturesUnordered<_> = barcodes.into_iter().map(|barcode| {
        let client = Arc::clone(&serp_client);
        let country = country_code.clone();
        async move {
            match client.search_by_barcode(&barcode, country.as_deref()).await {
                Ok(result) => match result.name {
                    Some(name) => TaskOutcome::Imported(ImportedIngredient {
                        barcode,
                        name,
                        photo_link: result.photo_link,
                    }),
                    None => TaskOutcome::Failed(FailedBarcode {
                        barcode,
                        reason: "No product found for this barcode".to_string(),
                    }),
                },
                Err(AppError::RateLimitError { message, retry_after_minutes }) => {
                    TaskOutcome::RateLimited { message, retry_after_minutes }
                }
                Err(e) => TaskOutcome::Failed(FailedBarcode {
                    barcode,
                    reason: e.to_string(),
                }),
            }
        }
    }).collect();

    let mut imported: Vec<ImportedIngredient> = Vec::new();
    let mut failed: Vec<FailedBarcode> = Vec::new();

    while let Some(outcome) = futures.next().await {
        completed += 1;
        let progress = ((completed * 90) / total) as u8;
        job_store.set_progress(job_id, progress);

        match outcome {
            TaskOutcome::Imported(i) => imported.push(i),
            TaskOutcome::Failed(f) => failed.push(f),
            TaskOutcome::RateLimited { message, retry_after_minutes } => {
                tracing::warn!(
                    job_id = %job_id,
                    message = %message,
                    retry_after_minutes,
                    "Barcode import job hit rate limit — marking as RateLimited"
                );
                job_store.set_status(
                    job_id,
                    JobStatus::RateLimited { message, retry_after_minutes },
                );
                return;
            }
        }
    }

    if !imported.is_empty() {
        let ingredient_dtos: Vec<IngredientDto> = imported
            .iter()
            .map(|i| IngredientDto {
                name: i.name.clone(),
                fill_percentage: 1.0,
                photo_link: i.photo_link.clone(),
            })
            .collect();

        if let Err(e) = insert_ingredients(&db, user_id, ingredient_dtos).await {
            tracing::error!(
                job_id = %job_id,
                user_id,
                "Failed to persist imported ingredients: {:?}", e
            );
            job_store.set_status(
                job_id,
                JobStatus::Failed(format!(
                    "Import succeeded but failed to save to database: {}",
                    e
                )),
            );
            return;
        }
    }

    tracing::info!(
        job_id = %job_id,
        imported = imported.len(),
        failed = failed.len(),
        "Barcode import job completed"
    );

    job_store.set_progress(job_id, 100);
    job_store.set_status(
        job_id,
        JobStatus::Completed(JobResult::ImportResult { imported, failed }),
    );
}