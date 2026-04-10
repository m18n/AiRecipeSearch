mod config;
mod error;
mod routes;
mod db;
mod handlers;
mod models;
mod services;
mod job_store;
mod middleware;

use actix_files::{Files, NamedFile};
use actix_web::{App, HttpServer, middleware::Logger, web, ResponseError};
use actix_cors::Cors;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::job_store::JobStore;
use crate::services::groq::GroqClient;
use crate::services::jina::JinaClient;
use crate::services::serp::SerpClient;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,sqlx=warn,actix_web=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
    let config = Config::from_env()
        .expect("Failed to load configuration");

    tracing::info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(config.db_pool_size)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Migrations applied successfully");
    let pool = web::Data::new(pool);
    let job_store = web::Data::new(JobStore::new());
    let port = config.port;
    let frontend_dist = config.frontend_dist.clone();
    let config = web::Data::new(config);
    let groq=web::Data::new(GroqClient::new(&config));
    let serp=web::Data::new(SerpClient::new(config.serp_api_key.clone()));
    let jina=web::Data::new(JinaClient::new(config.jina_api_key.clone()));

    HttpServer::new(move || {
        let app_base_url=config.app_base_url.clone();
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, _req_head| {
                origin.as_bytes().starts_with(b"http://localhost")
                    || origin.as_bytes().starts_with(b"http://127.0.0.1")
                    || origin.as_bytes().starts_with(app_base_url.as_bytes())
            })
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        let frontend = frontend_dist.clone();

        App::new()
            .app_data(pool.clone())
            .app_data(job_store.clone())
            .app_data(config.clone())
            .app_data(groq.clone())
            .app_data(serp.clone())
            .app_data(jina.clone())
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(
                web::JsonConfig::default()
                    .error_handler(|err, _req| {
                        let response = crate::error::AppError::from(err).error_response();
                        actix_web::error::InternalError::from_response(
                            "Invalid JSON",
                            response,
                        )
                            .into()
                    }),
            )
            .service(
                web::scope("/api")
                    .configure(routes::configure)
            )
            .service(
                Files::new("/", &frontend)
                    .index_file("index.html")
                    .default_handler({
                        let frontend = frontend.clone();
                        web::to(move || {
                            let index = format!("{}/index.html", frontend);
                            async move {
                                NamedFile::open_async(index).await
                            }
                        })
                    }),
            )
    })
        .bind(("0.0.0.0", port))?
        .run()
        .await
}