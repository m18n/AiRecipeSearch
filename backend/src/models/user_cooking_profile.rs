use serde::{Deserialize, Serialize};
use validator::Validate;




#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LanguageRow {
    pub id: i32,
    pub code: String,
    pub name: String,
}





#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CountryRow {
    pub id: i32,
    pub code: String,
    pub name: String,
}





#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IngredientRow {
    pub id: i32,
    pub name: String,
    pub fill_percentage: f64,
    pub photo_link: Option<String>,
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApplianceRow {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CookwareRow {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub user_id: i32,
}






/// Full composite user cooking profile assembled from four tables:
/// `user_global_search_preference`, `user_ingredient`,
/// `user_kitchen_appliances`, `user_cookware`.
/// NOT a single DB table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCookingProfile {
    pub global_preferences: Option<GlobalPreferencesResponse>,
    pub ingredients: Vec<IngredientRow>,
    pub appliances: Vec<ApplianceRow>,
    pub cookware: Vec<CookwareRow>,
}





/// Used for both reading and updating `user_global_search_preference`.
/// The `preference` column stores the full preferences payload as text
/// (e.g., serialized JSON or a plain string).

/// Used for both reading and updating `user_global_search_preference`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GlobalPreferencesDto {
    pub preference: Option<String>,
    pub country_of_residence_id: Option<i32>,
    pub language_id: Option<i32>,
}
/// Response enriched with full country details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPreferencesResponse {
    pub preference: Option<String>,
    pub country: Option<CountryRow>,
    pub language: Option<LanguageRow>,
}





/// Input for `POST /api/v1/users/me/cooking-profile/ingredients`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateIngredientDto {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: String,

    #[validate(range(min = 0.0, max = 1.0, message = "fill_percentage must be between 0.0 and 1.0"))]
    pub fill_percentage: f64,

    pub photo_link: Option<String>,
}

/// Input for `PATCH /api/v1/users/me/cooking-profile/ingredients/:id/fill-percentage`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateFillPercentageDto {
    #[validate(range(min = 0.0, max = 1.0, message = "fill_percentage must be between 0.0 and 1.0"))]
    pub fill_percentage: f64,
}

/// Used by the barcode import orchestrator for batch insertion.
/// `fill_percentage` defaults to `1.0` (100%) on import.
#[derive(Debug, Clone)]
pub struct IngredientDto {
    pub name: String,
    pub fill_percentage: f64,
    pub photo_link: Option<String>,
}





/// Input for `POST /api/v1/users/me/cooking-profile/appliances`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateApplianceDto {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: String,

    pub description: Option<String>,
}

/// Input for `PUT /api/v1/users/me/cooking-profile/appliances/:id`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateApplianceDto {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: String,

    pub description: Option<String>,
}





/// Input for `POST /api/v1/users/me/cooking-profile/cookware`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateCookwareDto {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: String,

    pub description: Option<String>,
}

/// Input for `PUT /api/v1/users/me/cooking-profile/cookware/:id`.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateCookwareDto {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: String,

    pub description: Option<String>,
}