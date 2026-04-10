use actix_web::web;

use crate::handlers::{auth, cooking_profile, recipes};
use crate::middleware::auth::AuthMiddleware;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/countries", web::get().to(cooking_profile::get_countries))
            .route("/languages", web::get().to(cooking_profile::get_languages))
            .service(
                web::scope("/auth")
                    .route("/login", web::post().to(auth::login))
                    .route("/refresh", web::post().to(auth::refresh))
                    .route("/set-password", web::post().to(auth::set_password))
                    .service(
                        web::scope("")
                            .wrap(AuthMiddleware)
                            .route("/logout", web::post().to(auth::logout)),
                    ),
            )
            .service(
                web::scope("/admin")
                    .wrap(AuthMiddleware)
                    .route(
                        "/users/{user_id}/password-init-link",
                        web::post().to(auth::generate_password_init_link),
                    ),
            )
            .service(
                web::scope("")
                    .wrap(AuthMiddleware)
                    .service(
                        web::scope("/recipes")
                            .route("/search", web::post().to(recipes::search_recipe))
                            .route(
                                "/jobs/{job_id}",
                                web::get().to(recipes::get_job_status),
                            ),
                    )
                    .service(
                        web::scope("/users/me/cooking-profile")
                            .route("", web::get().to(cooking_profile::get_user_cooking_profile))
                            .route(
                                "/global-preferences",
                                web::put().to(cooking_profile::update_global_preferences),
                            )
                            .service(
                                web::scope("/ingredients")
                                    .route(
                                        "",
                                        web::get().to(cooking_profile::get_ingredients),
                                    )
                                    .route(
                                        "",
                                        web::post().to(cooking_profile::add_ingredient),
                                    )

                                    .route(
                                        "/import",
                                        web::post()
                                            .to(cooking_profile::import_ingredients_from_csv),
                                    )
                                    .route(
                                        "/import/{import_job_id}",
                                        web::get()
                                            .to(cooking_profile::get_import_job_status),
                                    )
                                    .route(
                                        "/{ingredient_id}",
                                        web::delete().to(cooking_profile::delete_ingredient),
                                    )
                                    .route(
                                        "/{ingredient_id}/fill-percentage",
                                        web::patch().to(
                                            cooking_profile::update_ingredient_fill_percentage,
                                        ),
                                    ),
                            )
                            .service(
                                web::scope("/appliances")
                                    .route(
                                        "",
                                        web::get().to(cooking_profile::get_appliances),
                                    )
                                    .route(
                                        "",
                                        web::post().to(cooking_profile::add_appliance),
                                    )
                                    .route(
                                        "/{appliance_id}",
                                        web::put().to(cooking_profile::update_appliance),
                                    )
                                    .route(
                                        "/{appliance_id}",
                                        web::delete().to(cooking_profile::delete_appliance),
                                    ),
                            )
                            .service(
                                web::scope("/cookware")
                                    .route(
                                        "",
                                        web::get().to(cooking_profile::get_cookware),
                                    )
                                    .route(
                                        "",
                                        web::post().to(cooking_profile::add_cookware),
                                    )
                                    .route(
                                        "/{cookware_id}",
                                        web::put().to(cooking_profile::update_cookware),
                                    )
                                    .route(
                                        "/{cookware_id}",
                                        web::delete().to(cooking_profile::delete_cookware),
                                    ),
                            ),
                    ),
            ),
    );
}