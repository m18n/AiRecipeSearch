use std::sync::Arc;

use futures::future::join_all;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::{
        JINA_COST_PER_PAGE_USD, SERP_COST_PER_SEARCH_USD,
    },
    error::AppError,
    job_store::{JobResult, JobStatus, JobStore},
    models::recipe::{
        ActiveContext, ModelUsage,
       ParsedRecipe,
    },
    models::user_cooking_profile::UserCookingProfile,
    services::{
        jina::JinaClient,
        serp::SerpClient,
    },
};
use crate::models::recipe::PageClassification;
use crate::services::groq::{GroqClient, ModelTier};



#[derive(Default)]
struct CostAccumulator {
    usd: f64,
}

impl CostAccumulator {
    fn add_serp(&mut self) {
        self.usd += SERP_COST_PER_SEARCH_USD;
    }
    fn add_groq(&mut self, cost_usd: f64) {

        self.usd += cost_usd;
    }
    fn add_jina(&mut self) {
        self.usd += JINA_COST_PER_PAGE_USD;
    }
    fn total(&self) -> f64 {
        (self.usd * 1_000_000.0).round() / 1_000_000.0
    }
}



pub async fn run(
    _pool: PgPool,
    job_store: JobStore,
    groq: Arc<GroqClient>,
    _user_id: i32,
    job_id: Uuid,
    active_context: ActiveContext,
    serp_client: Arc<SerpClient>,
    jina_client: Arc<JinaClient>,
    cooking_profile: UserCookingProfile,
) {
    job_store.set_status(job_id, JobStatus::Processing);


    match execute(
        &job_store,
        job_id,
        &groq,
        &active_context,
        &serp_client,
        &jina_client,
        &cooking_profile,
    )
        .await
    {
        Ok(()) => {}
        Err(AppError::RateLimitError {
                message,
                retry_after_minutes,
            }) => {
            job_store.set_status(
                job_id,
                JobStatus::RateLimited {
                    message,
                    retry_after_minutes,
                },
            );
        }
        Err(e) => {
            job_store.set_status(job_id, JobStatus::Failed(e.to_string()));
        }
    }
}



async fn execute(
    job_store: &JobStore,
    job_id: Uuid,
    groq: &GroqClient,
    active_context: &ActiveContext,
    serp_client: &SerpClient,
    jina_client: &JinaClient,
    cooking_profile: &UserCookingProfile,
) -> Result<(), AppError> {
    let mut cost = CostAccumulator::default();


    let preferred_appliance: Option<&crate::models::user_cooking_profile::ApplianceRow> = cooking_profile
                .appliances
                .iter()
                .find(|a| a.id as u32 == active_context.kitchen_appliances_id);
    job_store.set_progress(job_id, 5);
    let queries = build_search_queries(
        groq, active_context, cooking_profile, preferred_appliance, &mut cost,
    ).await?;
    tracing::info!(job_id = %job_id, queries = ?queries, "Generated search queries");
    job_store.set_progress(job_id, 15);
    let links = collect_recipe_links(serp_client, &queries, &mut cost).await?;
    tracing::info!(job_id = %job_id, links = links.len(), "Collected recipe links");

    if links.is_empty() {
        return Err(AppError::InternalServerError(
            "No recipe links found for your query".to_string(),
        ));
    }
    job_store.set_progress(job_id, 35);
    let pages = fetch_pages(jina_client, &links, &mut cost).await;
    tracing::info!(job_id = %job_id, pages = pages.len(), "Fetched page contents");

    if pages.is_empty() {
        return Err(AppError::InternalServerError(
            "Could not fetch any recipe pages".to_string(),
        ));
    }
    job_store.set_progress(job_id, 50);
    let recipes = parse_recipe_pages(groq, pages, &mut cost).await?;
    tracing::info!(job_id = %job_id, recipes = recipes.len(), "Parsed valid recipe pages");

    if recipes.is_empty() {
        return Err(AppError::InternalServerError(
            "No valid single-recipe pages found".to_string(),
        ));
    }
    job_store.set_progress(job_id, 65);
    let recipes = filter_by_appliances(
        groq, recipes, cooking_profile, &mut cost,
    ).await?;
    tracing::info!(job_id = %job_id, recipes = recipes.len(), "After appliance filter");

    if recipes.is_empty() {
        return Err(AppError::InternalServerError(
            "No recipes match your available kitchen tools".to_string(),
        ));
    }
    job_store.set_progress(job_id, 80);
    let best = select_best_recipe(
        groq, recipes, active_context, cooking_profile, preferred_appliance, &mut cost,
    ).await?;
    tracing::info!(job_id = %job_id, title = %best.title, "Selected best recipe");
    job_store.set_progress(job_id, 95);
    let markdown = adapt_recipe_to_markdown(
        groq, &best, active_context, cooking_profile, preferred_appliance, &mut cost,
    ).await?;

    job_store.set_status(
        job_id,
        JobStatus::Completed(JobResult::RecipeResult {
            markdown_content: markdown,
            cost_usd: cost.total(),
        }),
    );
    job_store.set_progress(job_id, 100);

    Ok(())
}


#[derive(serde::Deserialize)]
struct SearchQueriesResponse {
    queries: Vec<String>,
}

async fn build_search_queries(
    groq: &GroqClient,
    active_context: &ActiveContext,
    profile: &UserCookingProfile,
    preferred_appliance: Option<&crate::models::user_cooking_profile::ApplianceRow>,
    cost: &mut CostAccumulator,
) -> Result<Vec<String>, AppError> {
    let ingredients: Vec<&str> = profile
        .ingredients
        .iter()
        .map(|i| i.name.as_str())
        .collect();

    let appliances: Vec<&str> = profile
        .appliances
        .iter()
        .map(|a| a.name.as_str())
        .collect();

    let cookware: Vec<&str> = profile
        .cookware
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    let country = profile
        .global_preferences
        .as_ref()
        .and_then(|p| p.country.as_ref())
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    let preferences = profile
        .global_preferences
        .as_ref()
        .and_then(|p| p.preference.as_deref())
        .unwrap_or("none");

    let language = profile
        .global_preferences
        .as_ref()
        .and_then(|p| p.language.as_ref())
        .map(|l| l.name.as_str())
        .unwrap_or("English");

    let preferred_appliance_hint = match preferred_appliance {
        Some(a) => format!(
            "\nPREFERRED COOKING APPLIANCE: The user specifically wants to cook using \"{}\". \
             All search queries MUST target recipes designed for or compatible with this appliance.\n",
            a.name
        ),
        None => String::new(),
    };

    let prompt = format!(
        r#"You are a recipe search expert. Generate 10 precise Google search queries
that will lead to specific recipe pages (not catalogues).

User request: "{query}"
User preferences: {preferences}
Note: the user request and preferences above are most likely written in {language}.
{preferred_appliance_hint}
Available ingredients (non-exhaustive): {ingredients}
Note: ingredient and product names are most likely from {country}, so labels and brand
names may be written in the local language of that country. Treat them by product type only
(e.g. a Ukrainian-labelled "паста Barilla" → "pasta").
Available appliances: {appliances}
Available cookware: {cookware}
User country: {country}

Rules:
- Each query must target a SINGLE, concrete recipe (e.g. "spaghetti carbonara recipe").
- Write queries in English.
- Take user preferences into account (e.g. dietary restrictions, cuisine style).
- If a preferred appliance is specified, include it in every query (e.g. "air fryer chicken wings recipe").
- IMPORTANT: Ingredient names may include brand names (e.g. "Barilla spaghetti", "Heinz ketchup").
  Treat them by their product type only (e.g. "spaghetti", "ketchup"). Ignore the brand.
- STRICT RULE: Do not include any domain names or website addresses (e.g., "allrecipes.com") in the search queries.
- Return JSON: {{ "queries": ["...", "...", ...] }}
"#,
        query = active_context.query,
        preferences = preferences,
        language = language,
        preferred_appliance_hint = preferred_appliance_hint,
        ingredients = ingredients.join(", "),
        country = country,
        appliances = appliances.join(", "),
        cookware = cookware.join(", "),

    );

    let resp = groq
        .generate::<SearchQueriesResponse>(&prompt, 0.7, ModelTier::Pro)
        .await?;

    cost.add_groq( resp.cost_usd);

    let queries = resp
        .data
        .queries
        .into_iter()
        .take(10)
        .filter(|q| !q.trim().is_empty())
        .collect::<Vec<_>>();

    if queries.is_empty() {
        return Err(AppError::InternalServerError(
            "Groq returned no search queries".to_string(),
        ));
    }

    Ok(queries)
}
/// (source_title, url)
type SourceLink = (String, String);

async fn collect_recipe_links(
    serp: &SerpClient,
    queries: &[String],
    cost: &mut CostAccumulator,
) -> Result<Vec<SourceLink>, AppError> {
    const RESULTS_PER_QUERY: usize = 3;

    for _ in queries {
        cost.add_serp();
    }

    let futures: Vec<_> = queries
        .iter()
        .map(|q| serp.search_recipe_sources(q))
        .collect();

    let results = join_all(futures).await;

    let mut seen_urls = std::collections::HashSet::new();
    let mut links: Vec<SourceLink> = Vec::new();

    for (query, result) in queries.iter().zip(results) {
        match result {
            Err(e @ AppError::RateLimitError { .. }) => return Err(e),
            Err(e) => {
                tracing::warn!(query = %query, error = %e, "SERP search failed, skipping");
            }
            Ok(r) => {
                for source in r.sources.into_iter().take(RESULTS_PER_QUERY) {
                    if seen_urls.insert(source.link.clone()) {
                        links.push((source.title, source.link));
                    }
                }
            }
        }
    }

    Ok(links)
}

/// (source_title, url, markdown_content)
type FetchedPage = (String, String, String, String);

async fn fetch_pages(
    jina: &JinaClient,
    links: &[SourceLink],
    cost: &mut CostAccumulator,
) -> Vec<FetchedPage> {
    for _ in links {
        cost.add_jina();
    }

    let futures: Vec<_> = links
        .iter()
        .map(|(_, url)| jina.fetch_page(url))
        .collect();

    let results = join_all(futures).await;

    links
        .iter()
        .zip(results)
        .filter_map(|((title, url), result)| match result {
            Ok((md, real_url)) => {
                if real_url != *url {
                    tracing::debug!(original = %url, real = %real_url, "Jina followed redirect");
                }
                Some((title.clone(), url.clone(), real_url, md))
            }
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "Jina fetch failed, skipping");
                None
            }
        })
        .collect()
}


struct PageParseResult {
    recipe: Option<ParsedRecipe>,
    usages: Vec<(ModelUsage, f64)>,
    rate_limit_err: Option<AppError>,
}

#[derive(serde::Deserialize)]
struct EquipmentItem {
    name: String,
    description: Option<String>,
}

#[derive(serde::Deserialize)]
struct ParsedRecipeRaw {
    title: String,
    description: Option<String>,
    ingredients: Vec<String>,
    instructions: Vec<String>,
    cooking_time_minutes: Option<u32>,
    servings: Option<u32>,
    difficulty: Option<String>,
    equipment: Vec<EquipmentItem>,
    photo_url: Option<String>,
}

async fn process_single_page(
    groq: &GroqClient,
    source_title: &str,
    source_url: &str,
    markdown: &str,
) -> PageParseResult {
    let truncated = truncate_markdown(markdown, 20_000);
    let mut usages: Vec<(ModelUsage, f64)> = Vec::new();

    let check_prompt = format!(
        r#"Read the following web page content and answer TWO questions:

1. is_actual_recipe: Does this page contain an actual recipe?
   - TRUE  → the page has a list of ingredients AND step-by-step cooking instructions.
   - FALSE → the page is only a description of a dish, a restaurant review, a food article,
             an ingredient glossary, a nutrition guide, or any other non-recipe content.

2. is_single_recipe: Does this page describe exactly ONE specific recipe?
   - TRUE  → one concrete recipe (e.g. "Spaghetti Carbonara").
   - FALSE → a list, catalogue, collection, or index of multiple recipes.

Page content:
---
{truncated}
---

Return JSON:
{{
  "is_actual_recipe": true | false,
  "is_single_recipe": true | false
}}"#
    );

    let check_resp = groq
        .generate::<PageClassification>(&check_prompt, 0.1, ModelTier::Lite)
        .await;

    match check_resp {
        Err(e @ AppError::RateLimitError { .. }) => {
            return PageParseResult {
                recipe: None,
                usages,
                rate_limit_err: Some(e),
            };
        }
        Err(e) => {
            tracing::warn!(url = %source_url, error = %e, "Groq check failed, skipping");
            return PageParseResult {
                recipe: None,
                usages,
                rate_limit_err: None,
            };
        }
        Ok(r) => {
            usages.push((r.usage, r.cost_usd));
            if !r.data.is_actual_recipe {
                tracing::debug!(url = %source_url, "Skipping non-recipe page (dish description or article)");
                return PageParseResult {
                    recipe: None,
                    usages,
                    rate_limit_err: None,
                };
            }
            if !r.data.is_single_recipe {
                tracing::debug!(url = %source_url, "Skipping catalogue page");
                return PageParseResult {
                    recipe: None,
                    usages,
                    rate_limit_err: None,
                };
            }
        }
    }


    let parse_prompt = format!(
        r#"Extract the recipe from the following web page content and return it as JSON.

CRITICAL RULES — you MUST follow these exactly:
- Copy ALL instructions VERBATIM, word for word, exactly as written on the page. Do NOT paraphrase, summarize, shorten, or rewrite anything.
- Copy ALL ingredients VERBATIM, exactly as listed, including quantities and units.
- Copy the title and description VERBATIM.
- Do NOT invent, add, or remove any steps, ingredients, or information.
- If a field is not present on the page, use null.
- For equipment: extract every tool, appliance, or cookware item mentioned. Include any description or usage note exactly as written.

Page content:
---
{truncated}
---

Return this exact JSON shape (use null for missing fields):
{{
  "title": "string — copied verbatim",
  "description": "string | null — copied verbatim",
  "ingredients": ["string — each copied verbatim", ...],
  "instructions": ["string — each step copied verbatim, complete, unshortened", ...],
  "cooking_time_minutes": number | null,
  "servings": number | null,
  "difficulty": "string | null",
  "equipment": [
    {{
      "name": "tool name verbatim",
      "description": "usage note or description verbatim, or null"
    }},
    ...
  ],
  "photo_url": "https://... — full URL of the main dish photo, copied exactly as it appears in the page, or null"
}}"#
    );

    let parse_resp = groq
        .generate::<ParsedRecipeRaw>(&parse_prompt, 0.1, ModelTier::Lite)
        .await;

    match parse_resp {
        Err(e @ AppError::RateLimitError { .. }) => PageParseResult {
            recipe: None,
            usages,
            rate_limit_err: Some(e),
        },
        Err(e) => {
            tracing::warn!(url = %source_url, error = %e, "Groq parse failed, skipping");
            PageParseResult {
                recipe: None,
                usages,
                rate_limit_err: None,
            }
        }
        Ok(r) => {
            usages.push((r.usage, r.cost_usd));
            let raw = r.data;


            let recipe = ParsedRecipe {
                title: raw.title,
                description: raw.description,
                ingredients: raw.ingredients,
                instructions: raw.instructions,
                cooking_time_minutes: raw.cooking_time_minutes,
                servings: raw.servings,
                difficulty: raw.difficulty,
                equipment: raw
                    .equipment
                    .into_iter()
                    .map(|e| crate::models::recipe::EquipmentItem {
                        name: e.name,
                        description: e.description,
                    })
                    .collect(),
                photo_url: raw.photo_url,
                source_url: source_url.to_string(),
                source_title: source_title.to_string(),
            };

            PageParseResult {
                recipe: Some(recipe),
                usages,
                rate_limit_err: None,
            }
        }
    }
}

async fn parse_recipe_pages(
    groq: &GroqClient,
    pages: Vec<FetchedPage>,
    cost: &mut CostAccumulator,
) -> Result<Vec<ParsedRecipe>, AppError> {
    let futures: Vec<_> = pages
        .iter()
        .map(|(title, _original_url, real_url, md)| {
            process_single_page(groq, title, real_url, md)
        })
        .collect();

    let results = join_all(futures).await;

    let mut recipes = Vec::new();
    for result in results {
        for (usage, cost_usd) in &result.usages {
            cost.add_groq( *cost_usd);
        }
        if let Some(e) = result.rate_limit_err {
            return Err(e);
        }
        if let Some(recipe) = result.recipe {
            recipes.push(recipe);
        }
    }

    Ok(recipes)
}



#[derive(serde::Deserialize)]
struct RecipeKeepDecision {
    index: usize,
    keep: bool,
}

#[derive(serde::Deserialize)]
struct BoolFilterResponse {
    recipes: Vec<RecipeKeepDecision>,
}

/*async fn filter_by_ingredients(
    groq: &GroqClient,
    recipes: Vec<ParsedRecipe>,
    profile: &UserCookingProfile,
    cost: &mut CostAccumulator,
) -> Result<Vec<ParsedRecipe>, AppError> {
    if recipes.is_empty() {
        return Ok(recipes);
    }

    let available: Vec<&str> = profile
        .ingredients
        .iter()
        .map(|i| i.name.as_str())
        .collect();

    let recipes_json = recipes_summary_json(&recipes);

    let prompt = format!(
        r#"You are a recipe filter. For each recipe decide if it can be prepared using ONLY
the available ingredients. Common pantry staples (salt, pepper, oil, water) are always available.

IMPORTANT: The available ingredients list may contain brand names (e.g. "Barilla spaghetti",
"Heinz ketchup", "Philadelphia cream cheese"). Always match by PRODUCT TYPE, not brand.
"Barilla spaghetti" counts as "spaghetti", "Heinz ketchup" counts as "ketchup", etc.

Available ingredients: {available}

Recipes (JSON array):
{recipes_json}

For EACH recipe return a keep decision. Do NOT score or rank them — only decide keep: true/false.

Return JSON:
{{
  "recipes": [
    {{ "index": 0, "keep": true }},
    {{ "index": 1, "keep": false }},
    ...
  ]
}}"#,
        available = available.join(", "),
    );

    let resp = groq
        .generate::<BoolFilterResponse>(&prompt, 0.1, ModelTier::Lite)
        .await?;

    cost.add_groq(&resp.usage, resp.cost_usd);

    let keep_set: std::collections::HashSet<usize> = resp
        .data
        .recipes
        .iter()
        .filter(|d| d.keep)
        .map(|d| d.index)
        .collect();

    if keep_set.is_empty() {
        return Ok(recipes);
    }

    Ok(recipes
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep_set.contains(i))
        .map(|(_, r)| r)
        .collect())
}*/


async fn filter_recipes_batch(
    groq: &GroqClient,
    chunk: &[ParsedRecipe],
    appliances: &str,
    cookware: &str,
) -> Result<(Vec<RecipeKeepDecision>, f64), AppError> {
    let recipes_json = recipes_summary_json(chunk);

    let prompt = format!(
        r#"You are a strict kitchen equipment filter. Your ONLY job is to check whether
a recipe can be made with the user's available kitchen APPLIANCES and COOKWARE.

STRICT RULES:
1. ONLY consider physical kitchen appliances (e.g. oven, microwave, air fryer, blender,
   stand mixer, pressure cooker) and cookware (e.g. frying pan, wok, baking tray,
   casserole dish, springform pan).
2. IGNORE everything that is NOT an appliance or cookware:
   - ingredients → ignore
   - cooking techniques or skills → ignore
   - serving ware, plates, glasses → ignore
   - knives, peelers, graters, spoons, spatulas, whisks → ignore (basic hand tools are always available)
   - cutting boards, colanders, mixing bowls → ignore (basic prep tools always available)
3. Mark a recipe keep: FALSE only when it EXPLICITLY requires a specific appliance or
   cookware piece that is NOT in the user's list AND cannot reasonably be substituted
   with what is available.
4. When in doubt → keep: TRUE. Do not reject recipes over minor or substitutable equipment.
5. Do NOT invent equipment requirements — only consider what the recipe explicitly states.

Available appliances: {appliances}
Available cookware: {cookware}

Recipes (JSON array, indices are 0-based within this batch):
{recipes_json}

Return JSON:
{{
  "recipes": [
    {{ "index": 0, "keep": true }},
    {{ "index": 1, "keep": false }},
    ...
  ]
}}"#,
        appliances = appliances,
        cookware = cookware,
    );

    let resp = groq
        .generate::<BoolFilterResponse>(&prompt, 0.1, ModelTier::Pro)
        .await?;

    Ok((resp.data.recipes, resp.cost_usd))
}

async fn filter_by_appliances(
    groq: &GroqClient,
    recipes: Vec<ParsedRecipe>,
    profile: &UserCookingProfile,
    cost: &mut CostAccumulator,
) -> Result<Vec<ParsedRecipe>, AppError> {
    if recipes.is_empty() {
        return Ok(recipes);
    }

    let appliances: Vec<&str> = profile.appliances.iter().map(|a| a.name.as_str()).collect();
    let cookware: Vec<&str> = profile.cookware.iter().map(|c| c.name.as_str()).collect();

    let appliances_str = if appliances.is_empty() { "none specified".to_string() } else { appliances.join(", ") };
    let cookware_str   = if cookware.is_empty()   { "none specified".to_string() } else { cookware.join(", ") };

    const BATCH_SIZE: usize = 10;
    let chunks: Vec<(usize, &[ParsedRecipe])> = recipes
        .chunks(BATCH_SIZE)
        .enumerate()
        .map(|(i, chunk)| (i * BATCH_SIZE, chunk))
        .collect();

    let futures: Vec<_> = chunks
        .iter()
        .map(|(_, chunk)| filter_recipes_batch(groq, chunk, &appliances_str, &cookware_str))
        .collect();

    let batch_results = join_all(futures).await;

    let mut keep_indices = std::collections::HashSet::new();

    for ((offset, _), result) in chunks.iter().zip(batch_results) {
        match result {
            Err(e @ AppError::RateLimitError { .. }) => return Err(e),
            Err(e) => {
                tracing::warn!(offset = offset, error = %e, "Appliance filter batch failed, keeping all in batch");
                for i in 0..BATCH_SIZE {
                    keep_indices.insert(offset + i);
                }
            }
            Ok((decisions, cost_usd)) => {
                cost.add_groq(cost_usd);
                for d in decisions {
                    if d.keep {
                        keep_indices.insert(offset + d.index);
                    }
                }
            }
        }
    }

    Ok(recipes
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep_indices.contains(i))
        .map(|(_, r)| r)
        .collect())
}



#[derive(serde::Deserialize)]
struct BestIndexResponse {
    best_index: usize,
}

async fn select_best_recipe(
    groq: &GroqClient,
    recipes: Vec<ParsedRecipe>,
    active_context: &ActiveContext,
    profile: &UserCookingProfile,
    preferred_appliance: Option<&crate::models::user_cooking_profile::ApplianceRow>,
    cost: &mut CostAccumulator,
) -> Result<ParsedRecipe, AppError> {
    if recipes.len() == 1 {
        return Ok(recipes.into_iter().next().unwrap());
    }

    let preferences = profile
        .global_preferences
        .as_ref()
        .and_then(|p| p.preference.as_deref())
        .unwrap_or("none");

    let language = profile
        .global_preferences
        .as_ref()
        .and_then(|p| p.language.as_ref())
        .map(|l| l.name.as_str())
        .unwrap_or("English");
    let country = profile
        .global_preferences
        .as_ref()
        .and_then(|p| p.country.as_ref())
        .map(|l| l.name.as_str())
        .unwrap_or("English");
    let ingredients: Vec<&str> = profile
        .ingredients
        .iter()
        .map(|i| i.name.as_str())
        .collect();

    let appliances: Vec<&str> = profile
        .appliances
        .iter()
        .map(|a| a.name.as_str())
        .collect();

    let cookware: Vec<&str> = profile
        .cookware
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    let recipes_json = recipes_summary_json(&recipes);
    let preferred_hint = match preferred_appliance {
        Some(a) => format!(
            "\nPREFERRED APPLIANCE: The user specifically wants to cook using \"{}\". \
             Strongly prefer recipes designed for or compatible with this appliance. \
             All else being equal, a recipe that uses \"{}\" should rank highest.\n",
            a.name, a.name
        ),
        None => String::new(),
    };

    let prompt = format!(
        r#"You are a culinary expert. Choose the SINGLE best recipe from the list below
that best matches the user's request and profile.

User request: "{query}"
User preferences: {preferences}
Note: the user request and preferences above are most likely written in {language}.
{preferred_hint}
User available ingredients: {ingredients}
Note: ingredient and product names are most likely from {country}, so labels and brand
names may be written in the local language of that country. Match by product type only —
ignore brand and language of the label.
User available appliances: {appliances}
User available cookware: {cookware}

IMPORTANT: Ingredient names may include brand names. Match by PRODUCT TYPE only.

Candidate recipes (JSON array):
{recipes_json}

Consider all aspects: match to request, available ingredients, appliances/cookware,
dietary preferences. If a preferred appliance is specified, weigh it heavily.

Return JSON: {{ "best_index": <number> }}"#,
        query = active_context.query,
        preferences = preferences,
        language = language,
        country=country,
        preferred_hint = preferred_hint,
        ingredients = ingredients.join(", "),
        appliances  = appliances.join(", "),
        cookware    = cookware.join(", "),
    );

    let resp = groq
        .generate::<BestIndexResponse>(&prompt, 0.3, ModelTier::Pro)
        .await?;

    cost.add_groq( resp.cost_usd);

    let best_index = resp.data.best_index.min(recipes.len() - 1);
    tracing::info!(best_index = best_index, "Best recipe selected");

    Ok(recipes.into_iter().nth(best_index).unwrap())
}



#[derive(serde::Deserialize)]
struct MatchComments {
    query: String,
    ingredients: String,
    equipment: String,
    preferences: String,
    difficulty: String,
    time: String,
}

#[derive(serde::Deserialize)]
struct MatchScores {
    overall: u8,
    query: u8,
    ingredients: u8,
    equipment: u8,
    preferences: u8,
    difficulty: u8,
    time: u8,
    comments: MatchComments,
}

#[derive(serde::Deserialize)]
struct EquipmentStep {
    tool: String,
    action: String,
    mode: String,
}

#[derive(serde::Deserialize)]
struct RecipeStep {
    title: String,
    duration_minutes: Option<u8>,
    instructions: Vec<String>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
struct UiLabels {
    match_section: String,
    overall_match: String,
    criterion: String,
    score: String,
    comment: String,
    criterion_query: String,
    criterion_ingredients: String,
    criterion_equipment: String,
    criterion_preferences: String,
    criterion_difficulty: String,
    criterion_time: String,
    adaptations_section: String,
    ingredients_section: String,
    servings: String,
    servings_unit: String,
    minutes: String,
    equipment_section: String,
    equipment_col_tool: String,
    equipment_col_action: String,
    equipment_col_mode: String,
    steps_section: String,
    step_word: String,
    step_duration: String,
    tips_section: String,
    source_label: String,
}
#[derive(serde::Deserialize)]
struct AdaptedRecipeContent {
    match_scores:       MatchScores,
    adaptations:        Vec<String>,
    tips:               Vec<String>,
    equipment_steps:    Vec<EquipmentStep>,
    steps:              Vec<RecipeStep>,
    ingredients:        Vec<String>,
    servings:           Option<u32>,
    total_time_minutes: Option<u32>,
    description:        Option<String>,
}

#[derive(serde::Deserialize)]
struct AdaptedRecipeStructured {
    match_scores: MatchScores,
    adaptations: Vec<String>,
    tips: Vec<String>,
    equipment_steps: Vec<EquipmentStep>,
    steps: Vec<RecipeStep>,
    ingredients: Vec<String>,
    servings: Option<u32>,
    total_time_minutes: Option<u32>,
    description: Option<String>,
    ui_labels: UiLabels
}

async fn adapt_recipe_to_markdown(
    groq: &GroqClient,
    recipe: &ParsedRecipe,
    active_context: &ActiveContext,
    profile: &UserCookingProfile,
    preferred_appliance: Option<&crate::models::user_cooking_profile::ApplianceRow>,
    cost: &mut CostAccumulator,
) -> Result<String, AppError> {
    let available_ingredients: Vec<&str> = profile.ingredients.iter().map(|i| i.name.as_str()).collect();

    let appliances_detail: Vec<String> = profile.appliances.iter().map(|a| {
        if let Some(desc) = &a.description { format!("{} ({})", a.name, desc) } else { a.name.clone() }
    }).collect();

    let cookware_detail: Vec<String> = profile.cookware.iter().map(|c| {
        if let Some(desc) = &c.description { format!("{} ({})", c.name, desc) } else { c.name.clone() }
    }).collect();

    let country = profile.global_preferences.as_ref()
        .and_then(|p| p.country.as_ref()).map(|c| c.name.as_str()).unwrap_or("Unknown");
    let preferences = profile.global_preferences.as_ref()
        .and_then(|p| p.preference.as_deref()).unwrap_or("none");
    let language = profile.global_preferences.as_ref()
        .and_then(|p| p.language.as_ref()).map(|l| l.name.as_str()).unwrap_or("English");

    let recipe_json = serde_json::to_string_pretty(recipe).unwrap_or_default();

    let preferred_appliance_hint = match preferred_appliance {
        Some(a) => format!("\nPreferred appliance: \"{}\". Mention it briefly where relevant.\n", a.name),
        None => String::new(),
    };
    let content_prompt = format!(
        r#"You are a professional recipe editor. Adapt the recipe below for the user and return structured JSON.

OUTPUT LANGUAGE: Write ALL text fields in {language}. Translate everything fully if not English.

RULES:
- Make MINIMAL changes — only what is strictly necessary.
- Do NOT invent new steps or ingredients.
- Preserve the original cooking technique and character of the dish.
- Copy instructions VERBATIM where no adaptation is needed.
- For equipment_steps: only list tools actually used. Specify exact mode/power/temperature from appliance descriptions.
- For adaptations: only list real changes made. If nothing changed, return an empty array.
- For tips: practical tips only, no marketing language.
- For match_scores: score 0-100 per criterion.
- Match ingredients by PRODUCT TYPE only — ignore brand names.
- Ingredient/product names may be in local language of {country} — apply product-type matching regardless.
- For description: translate the original recipe description to {language}. If null, return null.

User request: "{query}"
User country: {country}
User preferences: {preferences}
User available ingredients: {available}
{preferred_appliance_hint}
User appliances (with descriptions): {appliances}
User cookware (with descriptions): {cookware}

Recipe JSON:
{recipe_json}

Return this exact JSON shape:
{{
  "description": "translated recipe description, or null",
  "match_scores": {{
    "overall": <0-100>,
    "query": <0-100>,
    "ingredients": <0-100>,
    "equipment": <0-100>,
    "preferences": <0-100>,
    "difficulty": <0-100>,
    "time": <0-100>,
    "comments": {{
      "query": "one sentence explanation",
      "ingredients": "one sentence explanation",
      "equipment": "one sentence explanation",
      "preferences": "one sentence explanation",
      "difficulty": "one sentence explanation",
      "time": "one sentence explanation"
    }}
  }},
  "adaptations": ["what was changed and why", ...],
  "tips": ["practical tip", ...],
  "ingredients": ["200g pasta", ...],
  "servings": <number | null>,
  "total_time_minutes": <number | null>,
  "equipment_steps": [
    {{
      "tool": "tool name",
      "action": "what it is used for",
      "mode": "exact mode/power/temperature, or — if not applicable"
    }},
    ...
  ],
  "steps": [
    {{
      "title": "Step title",
      "duration_minutes": <number | null>,
      "instructions": ["Full instruction sentence", ...],
      "warnings": ["⚠️ Critical warning if any", ...]
    }},
    ...
  ]
}}"#,
        query               = active_context.query,
        language            = language,
        country             = country,
        preferences         = preferences,
        preferred_appliance_hint = preferred_appliance_hint,
        available           = available_ingredients.join(", "),
        appliances          = appliances_detail.join(", "),
        cookware            = cookware_detail.join(", "),
    );
    let labels_prompt = format!(
        r#"Translate all values in the following JSON to {language}.
Keep keys exactly as-is. Keep emoji as-is. Return only valid JSON.

{{
  "match_section": "📊 Match with your request",
  "overall_match": "Overall match",
  "criterion": "Criterion",
  "score": "Score",
  "comment": "Comment",
  "criterion_query": "Query",
  "criterion_ingredients": "Ingredients",
  "criterion_equipment": "Equipment",
  "criterion_preferences": "Preferences",
  "criterion_difficulty": "Difficulty",
  "criterion_time": "Time",
  "adaptations_section": "✏️ What was adapted",
  "ingredients_section": "🛒 Ingredients",
  "servings": "for",
  "servings_unit": "servings",
  "minutes": "min",
  "equipment_section": "🍳 Equipment and modes",
  "equipment_col_tool": "Tool",
  "equipment_col_action": "What to do",
  "equipment_col_mode": "Mode / Setting",
  "steps_section": "📋 Preparation",
  "step_word": "Step",
  "step_duration": "min",
  "tips_section": "💡 Tips",
  "source_label": "Source"
}}"#,
        language = language,
    );
    let (content_result, labels_result) = futures::future::join(
        groq.generate::<AdaptedRecipeContent>(&content_prompt, 0.5, ModelTier::Pro),
        groq.generate::<UiLabels>(&labels_prompt, 0.1, ModelTier::Pro),
    ).await;

    let content_resp = content_result?;
    let labels_resp  = labels_result?;

    cost.add_groq(content_resp.cost_usd);
    cost.add_groq(labels_resp.cost_usd);

    let adapted = AdaptedRecipeStructured {
        match_scores:       content_resp.data.match_scores,
        adaptations:        content_resp.data.adaptations,
        tips:               content_resp.data.tips,
        equipment_steps:    content_resp.data.equipment_steps,
        steps:              content_resp.data.steps,
        ingredients:        content_resp.data.ingredients,
        servings:           content_resp.data.servings,
        total_time_minutes: content_resp.data.total_time_minutes,
        description:        content_resp.data.description,
        ui_labels:          labels_resp.data,
    };

    Ok(render_markdown(recipe, &adapted))
}

fn render_markdown(recipe: &ParsedRecipe, data: &AdaptedRecipeStructured) -> String {
    let mut md = String::new();
    let l = &data.ui_labels;
    if let Some(url) = &recipe.photo_url {
        md.push_str(&format!("![{}]({})\n\n", recipe.title, url));
    }
    md.push_str(&format!("# {}\n\n", recipe.title));
    if let Some(desc) = &data.description {
        md.push_str(&format!("> {}\n\n", desc));
    }


    md.push_str("---\n\n");
    md.push_str(&format!("## {}\n\n", l.match_section));
    md.push_str(&format!(
        "**{}: {}%**\n\n",
        l.overall_match, data.match_scores.overall
    ));

    md.push_str(&format!("| {} | {} | {} |\n", l.criterion, l.score, l.comment));
    md.push_str("|----------|--------|----------|\n");

    let c = &data.match_scores.comments;
    md.push_str(&format!("| {} | {}% | {} |\n", l.criterion_query,       data.match_scores.query,       c.query));
    md.push_str(&format!("| {} | {}% | {} |\n", l.criterion_ingredients, data.match_scores.ingredients, c.ingredients));
    md.push_str(&format!("| {} | {}% | {} |\n", l.criterion_equipment,   data.match_scores.equipment,   c.equipment));
    md.push_str(&format!("| {} | {}% | {} |\n", l.criterion_preferences, data.match_scores.preferences, c.preferences));
    md.push_str(&format!("| {} | {}% | {} |\n", l.criterion_difficulty,  data.match_scores.difficulty,  c.difficulty));
    md.push_str(&format!("| {} | {}% | {} |\n", l.criterion_time,        data.match_scores.time,        c.time));

    md.push_str("\n---\n\n");
    if !data.adaptations.is_empty() {
        md.push_str(&format!("## {}\n\n", l.adaptations_section));
        for adaptation in &data.adaptations {
            md.push_str(&format!("- {}\n", adaptation));
        }
        md.push_str("\n---\n\n");
    }
    md.push_str(&format!("## {}\n\n", l.ingredients_section));

    let meta: Vec<String> = [
        data.servings.map(|s| format!("{} {} {}", l.servings, s, l.servings_unit)),
        data.total_time_minutes.map(|t| format!("~{} {}", t, l.minutes)),
    ]
        .into_iter()
        .flatten()
        .collect();

    if !meta.is_empty() {
        md.push_str(&format!("*{}*\n\n", meta.join(" · ")));
    }

    for ingredient in &data.ingredients {
        md.push_str(&format!("- {}\n", ingredient));
    }

    md.push_str("\n---\n\n");
    if !data.equipment_steps.is_empty() {
        md.push_str(&format!("## {}\n\n", l.equipment_section));
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            l.equipment_col_tool, l.equipment_col_action, l.equipment_col_mode
        ));
        md.push_str("|------------|-----------|----------------------|\n");
        for eq in &data.equipment_steps {
            md.push_str(&format!("| {} | {} | {} |\n", eq.tool, eq.action, eq.mode));
        }
        md.push_str("\n---\n\n");
    }
    md.push_str(&format!("## {}\n\n", l.steps_section));

    for (i, step) in data.steps.iter().enumerate() {
        let step_header = match step.duration_minutes {
            Some(d) => format!(
                "### {} {} — {} ({} {})\n\n",
                l.step_word,
                i + 1,
                step.title,
                d,
                l.step_duration
            ),
            None => format!("### {} {} — {}\n\n", l.step_word, i + 1, step.title),
        };
        md.push_str(&step_header);

        for instruction in &step.instructions {
            md.push_str(&format!("{}\n\n", instruction));
        }

        for warning in &step.warnings {
            md.push_str(&format!("> {}\n\n", warning));
        }
    }

    md.push_str("---\n\n");
    if !data.tips.is_empty() {
        md.push_str(&format!("## {}\n\n", l.tips_section));
        for tip in &data.tips {
            md.push_str(&format!("- {}\n", tip));
        }
        md.push_str("\n---\n\n");
    }
    md.push_str(&format!(
        "**{}:** [{}]({})\n",
        l.source_label, recipe.source_title, recipe.source_url
    ));

    md
}



fn recipes_summary_json(recipes: &[ParsedRecipe]) -> String {
    #[derive(serde::Serialize)]
    struct Summary<'a> {
        index: usize,
        title: &'a str,
        ingredients: &'a [String],
        instructions_count: usize,
        cooking_time_minutes: Option<u32>,
        difficulty: Option<&'a str>,
    }

    let summaries: Vec<Summary> = recipes
        .iter()
        .enumerate()
        .map(|(i, r)| Summary {
            index: i,
            title: &r.title,
            ingredients: &r.ingredients,
            instructions_count: r.instructions.len(),
            cooking_time_minutes: r.cooking_time_minutes,
            difficulty: r.difficulty.as_deref(),
        })
        .collect();

    serde_json::to_string_pretty(&summaries).unwrap_or_default()
}

fn truncate_markdown(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        return text;
    }
    let safe_boundary = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i < max_chars)
        .last()
        .unwrap_or(0);

    match text[..safe_boundary].rfind('\n') {
        Some(pos) => &text[..pos],
        None => &text[..safe_boundary],
    }
}