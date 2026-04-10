use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use crate::db::user_cooking_profile;
use crate::error::AppError;
use crate::job_store::{JobStore};
use crate::models::user_cooking_profile::{
    CreateIngredientDto, UpdateFillPercentageDto,
    CreateApplianceDto, UpdateApplianceDto,
    CreateCookwareDto, UpdateCookwareDto,
    GlobalPreferencesDto,
};
use crate::models::barcode_import::ImportJobResponse;

use crate::services::barcode_import_orchestrator;
use actix_multipart::Multipart;
use futures_util::StreamExt;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;
use crate::services::serp::SerpClient;


/// GET /api/v1/languages
pub async fn get_languages(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, AppError> {
    let languages = user_cooking_profile::get_languages(&pool).await?;
    Ok(HttpResponse::Ok().json(languages))
}




pub fn extract_user_id(req: &HttpRequest) -> Result<i32, AppError> {
    req.extensions()
        .get::<crate::middleware::auth::AuthenticatedUser>()
        .map(|u| u.user_id)
        .ok_or(AppError::Unauthorized("Missing user identity".into()))
}




/// GET /api/v1/countries
pub async fn get_countries(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, AppError> {
    let countries = user_cooking_profile::get_countries(&pool).await?;
    Ok(HttpResponse::Ok().json(countries))
}





/// GET /api/v1/users/me/cooking-profile
pub async fn get_user_cooking_profile(
    req: HttpRequest,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let profile = user_cooking_profile::get_user_cooking_profile(&pool, user_id).await?;
    Ok(HttpResponse::Ok().json(profile))
}





/// PUT /api/v1/users/me/cooking-profile/global-preferences
pub async fn update_global_preferences(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    body: web::Json<GlobalPreferencesDto>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let updated = user_cooking_profile::upsert_global_preferences(&pool, user_id, &body).await?;
    Ok(HttpResponse::Ok().json(updated))
}






/// GET /api/v1/users/me/cooking-profile/ingredients
pub async fn get_ingredients(
    req: HttpRequest,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let ingredients = user_cooking_profile::get_ingredients(&pool, user_id).await?;
    Ok(HttpResponse::Ok().json(ingredients))
}

/// POST /api/v1/users/me/cooking-profile/ingredients
pub async fn add_ingredient(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    body: web::Json<CreateIngredientDto>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let ingredient = user_cooking_profile::insert_ingredient(
        &pool,
        user_id,
        &body.name,
        body.fill_percentage,
        body.photo_link.as_deref(),
    )
        .await?;
    Ok(HttpResponse::Created().json(ingredient))
}

/// DELETE /api/v1/users/me/cooking-profile/ingredients/:ingredient_id
pub async fn delete_ingredient(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let ingredient_id = path.into_inner();

    let deleted = user_cooking_profile::delete_ingredient(&pool, ingredient_id, user_id).await?;
    if deleted {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::NotFound("Ingredient not found".into()))
    }
}

/// PATCH /api/v1/users/me/cooking-profile/ingredients/:ingredient_id/fill-percentage
pub async fn update_ingredient_fill_percentage(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    body: web::Json<UpdateFillPercentageDto>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let ingredient_id = path.into_inner();

    let updated = user_cooking_profile::update_ingredient_fill_percentage(
        &pool,
        ingredient_id,
        user_id,
        body.fill_percentage,
    )
        .await?;

    match updated {
        Some(row) => Ok(HttpResponse::Ok().json(row)),
        None => Err(AppError::NotFound("Ingredient not found".into())),
    }
}





const MAX_CSV_BYTES: usize = 1 * 1024 * 1024;

/// POST /api/v1/users/me/cooking-profile/ingredients/import
pub async fn import_ingredients_from_csv(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    job_store: web::Data<JobStore>,
    serp_client: web::Data<SerpClient>,
    mut payload: Multipart
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    if job_store.has_active_job(user_id) {
        return Err(AppError::RateLimitError{message:"Wait your last task".to_string(),retry_after_minutes:1});
    }
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut found_file = false;

    while let Some(field_result) = payload.next().await {
        let mut field = field_result
            .map_err(|e| AppError::BadRequest(format!("Multipart error: {e}")))?;

        let content_disposition = field.content_disposition();
        let field_name = content_disposition
            .and_then(|cd| cd.get_name())
            .unwrap_or("");

        if field_name != "file" {
            continue;
        }
        if let Some(mime) = field.content_type() {
            let mime_str = mime.to_string();
            if !mime_str.contains("csv") && !mime_str.contains("text/plain") {
                return Err(AppError::BadRequest(
                    "Only CSV files are accepted".into(),
                ));
            }
        }

        while let Some(chunk) = field.next().await {
            let data = chunk
                .map_err(|e| AppError::BadRequest(format!("Read error: {e}")))?;
            file_bytes.extend_from_slice(&data);
            if file_bytes.len() > MAX_CSV_BYTES {
                return Err(AppError::BadRequest("File exceeds 1 MB limit".into()));
            }
        }

        found_file = true;
        break;
    }

    if !found_file || file_bytes.is_empty() {
        return Err(AppError::BadRequest(
            "No CSV file provided in 'file' field".into(),
        ));
    }
    let ean13_re = regex::Regex::new(r"^\d{13}$").unwrap();

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(file_bytes.as_slice());

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut barcodes: Vec<String> = Vec::new();

    for result in reader.records() {
        let record = result
            .map_err(|e| AppError::BadRequest(format!("CSV row error: {e}")))?;

        for cell in record.iter() {
            let value = cell.trim();
            if ean13_re.is_match(value) && seen.insert(value.to_string()) {
                barcodes.push(value.to_string());
            }
        }
    }

    if barcodes.is_empty() {
        return Err(AppError::BadRequest(
            "No valid EAN-13 barcodes found in CSV".into(),
        ));
    }

    let country_code = crate::db::user_cooking_profile::get_user_country_code(
        &pool, user_id,
    )
        .await?;
    let job_id = job_store.create_job(user_id);

    let pool_clone = pool.get_ref().clone();
    let store_clone = job_store.into_inner();
    let serp_client_clone=serp_client.into_inner();
    tokio::spawn(async move {
        barcode_import_orchestrator::run(
            pool_clone,
            store_clone,
            user_id,
            job_id,
            barcodes,
            serp_client_clone,
            country_code
        )
            .await;
    });

    Ok(HttpResponse::Accepted().json(serde_json::json!({ "import_job_id": job_id })))
}

/// GET /api/v1/users/me/cooking-profile/ingredients/import/:import_job_id
pub async fn get_import_job_status(
    req: HttpRequest,
    job_store: web::Data<JobStore>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let import_job_id = path.into_inner();

    let job = job_store
        .get_job(import_job_id)
        .ok_or(AppError::NotFound("Import job not found".into()))?;

    if job.user_id != user_id {
        return Err(AppError::Forbidden("Access denied".into()));
    }

    Ok(HttpResponse::Ok().json(ImportJobResponse::from_job(&job)))
}






/// GET /api/v1/users/me/cooking-profile/appliances
pub async fn get_appliances(
    req: HttpRequest,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let appliances = user_cooking_profile::get_appliances(&pool, user_id).await?;
    Ok(HttpResponse::Ok().json(appliances))
}

/// POST /api/v1/users/me/cooking-profile/appliances
pub async fn add_appliance(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    body: web::Json<CreateApplianceDto>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let appliance = user_cooking_profile::insert_appliance(
        &pool,
        user_id,
        &body.name,
        body.description.as_deref(),
    )
        .await?;
    Ok(HttpResponse::Created().json(appliance))
}

/// PUT /api/v1/users/me/cooking-profile/appliances/:appliance_id
pub async fn update_appliance(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    body: web::Json<UpdateApplianceDto>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let appliance_id = path.into_inner();

    let updated = user_cooking_profile::update_appliance(
        &pool,
        appliance_id,
        user_id,
        &body.name,
        body.description.as_deref(),
    )
        .await?;

    match updated {
        Some(row) => Ok(HttpResponse::Ok().json(row)),
        None => Err(AppError::NotFound("Appliance not found".into())),
    }
}

/// DELETE /api/v1/users/me/cooking-profile/appliances/:appliance_id
pub async fn delete_appliance(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let appliance_id = path.into_inner();

    let deleted = user_cooking_profile::delete_appliance(&pool, appliance_id, user_id).await?;
    if deleted {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::NotFound("Appliance not found".into()))
    }
}





/// GET /api/v1/users/me/cooking-profile/cookware
pub async fn get_cookware(
    req: HttpRequest,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let cookware = user_cooking_profile::get_cookware(&pool, user_id).await?;
    Ok(HttpResponse::Ok().json(cookware))
}

/// POST /api/v1/users/me/cooking-profile/cookware
pub async fn add_cookware(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    body: web::Json<CreateCookwareDto>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let cookware = user_cooking_profile::insert_cookware(
        &pool,
        user_id,
        &body.name,
        body.description.as_deref(),
    )
        .await?;
    Ok(HttpResponse::Created().json(cookware))
}

/// PUT /api/v1/users/me/cooking-profile/cookware/:cookware_id
pub async fn update_cookware(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    body: web::Json<UpdateCookwareDto>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let cookware_id = path.into_inner();

    let updated = user_cooking_profile::update_cookware(
        &pool,
        cookware_id,
        user_id,
        &body.name,
        body.description.as_deref(),
    )
        .await?;

    match updated {
        Some(row) => Ok(HttpResponse::Ok().json(row)),
        None => Err(AppError::NotFound("Cookware not found".into())),
    }
}

/// DELETE /api/v1/users/me/cooking-profile/cookware/:cookware_id
pub async fn delete_cookware(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let cookware_id = path.into_inner();

    let deleted = user_cooking_profile::delete_cookware(&pool, cookware_id, user_id).await?;
    if deleted {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::NotFound("Cookware not found".into()))
    }
}