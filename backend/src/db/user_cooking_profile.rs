use sqlx::PgPool;
use crate::error::AppError;
use crate::models::user_cooking_profile::{UserCookingProfile, GlobalPreferencesDto, IngredientRow, ApplianceRow, CookwareRow, IngredientDto, CountryRow, GlobalPreferencesResponse, LanguageRow};


pub async fn get_countries(pool: &PgPool) -> Result<Vec<CountryRow>, AppError> {
    let rows = sqlx::query_as!(
        CountryRow,
        r#"SELECT id, code, name FROM countries ORDER BY name ASC"#
    )
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn get_user_cooking_profile(
    pool: &PgPool,
    user_id: i32,
) -> Result<UserCookingProfile, AppError> {
    let global_preferences = get_global_preferences(pool, user_id).await?;
    let ingredients = get_ingredients(pool, user_id).await?;
    let appliances = get_appliances(pool, user_id).await?;
    let cookware = get_cookware(pool, user_id).await?;

    Ok(UserCookingProfile {
        global_preferences,
        ingredients,
        appliances,
        cookware,
    })
}


pub async fn get_languages(pool: &PgPool) -> Result<Vec<LanguageRow>, AppError> {
    let rows = sqlx::query_as!(
        LanguageRow,
        r#"SELECT id, code, name FROM languages ORDER BY name ASC"#
    )
        .fetch_all(pool)
        .await?;

    Ok(rows)
}
pub async fn get_user_country_code(
    pool: &PgPool,
    user_id: i32,
) -> Result<Option<String>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT c.code
        FROM user_global_search_preference p
        JOIN countries c ON c.id = p.country_of_residence_id
        WHERE p.user_id = $1
        "#,
        user_id
    )
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.code))
}
async fn get_global_preferences(
    pool: &PgPool,
    user_id: i32,
) -> Result<Option<GlobalPreferencesResponse>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            p.preference,
            c.id   AS "country_id?: i32",
            c.code AS "country_code?: String",
            c.name AS "country_name?: String",
            l.id   AS "language_id?: i32",
            l.code AS "language_code?: String",
            l.name AS "language_name?: String"
        FROM user_global_search_preference p
        LEFT JOIN countries c ON c.id = p.country_of_residence_id
        LEFT JOIN languages l ON l.id = p.language_id
        WHERE p.user_id = $1
        "#,
        user_id
    )
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| GlobalPreferencesResponse {
        preference: r.preference,
        country: match (r.country_id, r.country_code, r.country_name) {
            (Some(id), Some(code), Some(name)) => Some(CountryRow { id, code, name }),
            _ => None,
        },
        language: match (r.language_id, r.language_code, r.language_name) {
            (Some(id), Some(code), Some(name)) => Some(LanguageRow { id, code, name }),
            _ => None,
        },
    }))
}

pub async fn upsert_global_preferences(
    pool: &PgPool,
    user_id: i32,
    dto: &GlobalPreferencesDto,
) -> Result<GlobalPreferencesResponse, AppError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO user_global_search_preference (user_id, preference, country_of_residence_id, language_id)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_id)
        DO UPDATE SET
            preference              = EXCLUDED.preference,
            country_of_residence_id = EXCLUDED.country_of_residence_id,
            language_id             = EXCLUDED.language_id
        RETURNING preference, country_of_residence_id, language_id
        "#,
        user_id,
        dto.preference.clone() as Option<String>,
        dto.country_of_residence_id as Option<i32>,
        dto.language_id as Option<i32>
    )
        .fetch_one(pool)
        .await?;

    let country = match row.country_of_residence_id {
        Some(cid) => {
            sqlx::query_as!(
                CountryRow,
                r#"SELECT id, code, name FROM countries WHERE id = $1"#,
                cid
            )
                .fetch_optional(pool)
                .await?
        }
        None => None,
    };

    let language = match row.language_id {
        Some(lid) => {
            sqlx::query_as!(
                LanguageRow,
                r#"SELECT id, code, name FROM languages WHERE id = $1"#,
                lid
            )
                .fetch_optional(pool)
                .await?
        }
        None => None,
    };

    Ok(GlobalPreferencesResponse {
        preference: row.preference,
        country,
        language,
    })
}



pub async fn get_ingredients(
    pool: &PgPool,
    user_id: i32,
) -> Result<Vec<IngredientRow>, AppError> {
    let rows = sqlx::query_as!(
        IngredientRow,
        r#"
        SELECT id, name, fill_percentage, photo_link, user_id
        FROM user_ingredient
        WHERE user_id = $1
        ORDER BY id ASC
        "#,
        user_id
    )
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn insert_ingredient(
    pool: &PgPool,
    user_id: i32,
    name: &str,
    fill_percentage: f64,
    photo_link: Option<&str>,
) -> Result<IngredientRow, AppError> {
    let row = sqlx::query_as!(
        IngredientRow,
        r#"
        INSERT INTO user_ingredient (user_id, name, fill_percentage, photo_link)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, fill_percentage, photo_link, user_id
        "#,
        user_id,
        name,
        fill_percentage,
        photo_link
    )
        .fetch_one(pool)
        .await?;

    Ok(row)
}

pub async fn delete_ingredient(
    pool: &PgPool,
    ingredient_id: i32,
    user_id: i32,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM user_ingredient
        WHERE id = $1 AND user_id = $2
        "#,
        ingredient_id,
        user_id
    )
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn update_ingredient_fill_percentage(
    pool: &PgPool,
    ingredient_id: i32,
    user_id: i32,
    fill_percentage: f64,
) -> Result<Option<IngredientRow>, AppError> {
    let row = sqlx::query_as!(
        IngredientRow,
        r#"
        UPDATE user_ingredient
        SET fill_percentage = $1
        WHERE id = $2 AND user_id = $3
        RETURNING id, name, fill_percentage, photo_link, user_id
        "#,
        fill_percentage,
        ingredient_id,
        user_id
    )
        .fetch_optional(pool)
        .await?;

    Ok(row)
}

pub async fn insert_ingredients(
    pool: &PgPool,
    user_id: i32,
    ingredients: Vec<IngredientDto>,
) -> Result<Vec<IngredientRow>, AppError> {
    if ingredients.is_empty() {
        return Ok(vec![]);
    }
    let names: Vec<&str>          = ingredients.iter().map(|i| i.name.as_str()).collect();
    let fill_pcts: Vec<f64>       = ingredients.iter().map(|i| i.fill_percentage).collect();
    let photo_links: Vec<Option<&str>> = ingredients
        .iter()
        .map(|i| i.photo_link.as_deref())
        .collect();

    let rows = sqlx::query_as!(
        IngredientRow,
        r#"
        INSERT INTO user_ingredient (user_id, name, fill_percentage, photo_link)
        SELECT $1, name, fill_percentage, photo_link
        FROM UNNEST($2::text[], $3::float8[], $4::text[])
             AS t(name, fill_percentage, photo_link)
        RETURNING id, name, fill_percentage, photo_link, user_id
        "#,
        user_id,
        &names as &[&str],
        &fill_pcts as &[f64],
        &photo_links as &[Option<&str>]
    )
        .fetch_all(pool)
        .await?;

    Ok(rows)
}



pub async fn get_appliances(
    pool: &PgPool,
    user_id: i32,
) -> Result<Vec<ApplianceRow>, AppError> {
    let rows = sqlx::query_as!(
        ApplianceRow,
        r#"
        SELECT id, name, description, user_id
        FROM user_kitchen_appliances
        WHERE user_id = $1
        ORDER BY id ASC
        "#,
        user_id
    )
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn insert_appliance(
    pool: &PgPool,
    user_id: i32,
    name: &str,
    description: Option<&str>,
) -> Result<ApplianceRow, AppError> {
    let row = sqlx::query_as!(
        ApplianceRow,
        r#"
        INSERT INTO user_kitchen_appliances (user_id, name, description)
        VALUES ($1, $2, $3)
        RETURNING id, name, description, user_id
        "#,
        user_id,
        name,
        description
    )
        .fetch_one(pool)
        .await?;

    Ok(row)
}

pub async fn update_appliance(
    pool: &PgPool,
    appliance_id: i32,
    user_id: i32,
    name: &str,
    description: Option<&str>,
) -> Result<Option<ApplianceRow>, AppError> {
    let row = sqlx::query_as!(
        ApplianceRow,
        r#"
        UPDATE user_kitchen_appliances
        SET name = $1, description = $2
        WHERE id = $3 AND user_id = $4
        RETURNING id, name, description, user_id
        "#,
        name,
        description,
        appliance_id,
        user_id
    )
        .fetch_optional(pool)
        .await?;

    Ok(row)
}

pub async fn delete_appliance(
    pool: &PgPool,
    appliance_id: i32,
    user_id: i32,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM user_kitchen_appliances
        WHERE id = $1 AND user_id = $2
        "#,
        appliance_id,
        user_id
    )
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}



pub async fn get_cookware(
    pool: &PgPool,
    user_id: i32,
) -> Result<Vec<CookwareRow>, AppError> {
    let rows = sqlx::query_as!(
        CookwareRow,
        r#"
        SELECT id, name, description, user_id
        FROM user_cookware
        WHERE user_id = $1
        ORDER BY id ASC
        "#,
        user_id
    )
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn insert_cookware(
    pool: &PgPool,
    user_id: i32,
    name: &str,
    description: Option<&str>,
) -> Result<CookwareRow, AppError> {
    let row = sqlx::query_as!(
        CookwareRow,
        r#"
        INSERT INTO user_cookware (user_id, name, description)
        VALUES ($1, $2, $3)
        RETURNING id, name, description, user_id
        "#,
        user_id,
        name,
        description
    )
        .fetch_one(pool)
        .await?;

    Ok(row)
}

pub async fn update_cookware(
    pool: &PgPool,
    cookware_id: i32,
    user_id: i32,
    name: &str,
    description: Option<&str>,
) -> Result<Option<CookwareRow>, AppError> {
    let row = sqlx::query_as!(
        CookwareRow,
        r#"
        UPDATE user_cookware
        SET name = $1, description = $2
        WHERE id = $3 AND user_id = $4
        RETURNING id, name, description, user_id
        "#,
        name,
        description,
        cookware_id,
        user_id
    )
        .fetch_optional(pool)
        .await?;

    Ok(row)
}

pub async fn delete_cookware(
    pool: &PgPool,
    cookware_id: i32,
    user_id: i32,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM user_cookware
        WHERE id = $1 AND user_id = $2
        "#,
        cookware_id,
        user_id
    )
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}