use actix_web::{web, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::{
    error::AppError,
    job_store::{JobStore},
    models::recipe::{ActiveContext},
    services::recipe_orchestrator,
};
use crate::config::Config;
use crate::db::user_cooking_profile;
use crate::handlers::cooking_profile::extract_user_id;
use crate::models::recipe::RecipeJobResponse;
use crate::services::groq::GroqClient;
use crate::services::jina::JinaClient;
use crate::services::serp::SerpClient;

pub async fn search_recipe(
    req: HttpRequest,
    active_context: web::Json<ActiveContext>,
    job_store: web::Data<JobStore>,
    db_pool: web::Data<sqlx::PgPool>,
    groq:web::Data<GroqClient>,
    serp_client: web::Data<SerpClient>,
    jina_client: web::Data<JinaClient>,
    config: web::Data<Config>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;

    if job_store.has_active_job(user_id) {
        return Err(AppError::RateLimitError{message:"Wait your last task".to_string(),retry_after_minutes:1});
    }
    let cooking_profile = user_cooking_profile::get_user_cooking_profile(
        db_pool.as_ref(),
        user_id,
    )
        .await?;
    let job_id = job_store.create_job(user_id);

    let store_clone = job_store.clone();
    let pool_clone = db_pool.clone();

    let active_context_clone = active_context.clone();
    let config_clone = config.into_inner();
    let serp_clone=serp_client.into_inner();
    let jina_clone=jina_client.into_inner();
    let groq_clone=groq.into_inner();
    tokio::spawn(async move {
        recipe_orchestrator::run(
            pool_clone.as_ref().clone(),
            store_clone.as_ref().clone(),
            groq_clone,
            user_id,
            job_id,
            active_context_clone,
            serp_clone,
            jina_clone.clone(),
            cooking_profile


        )
            .await;
    });

    Ok(HttpResponse::Accepted().json(serde_json::json!({ "job_id": job_id })))
}

pub async fn get_job_status(
    req: HttpRequest,
    path: web::Path<Uuid>,
    job_store: web::Data<JobStore>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let job_id = path.into_inner();

    let job = job_store
        .get_job(job_id)
        .ok_or_else(|| AppError::NotFound(format!("Job '{job_id}' not found")))?;

    if job.user_id != user_id {
        return Err(AppError::Forbidden("You do not have access to this job".to_string()));
    }
    
    Ok(HttpResponse::Ok().json(RecipeJobResponse::from_job(&job)))
}