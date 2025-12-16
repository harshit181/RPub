use crate::handlers;
use crate::models::AppState;
use axum::{
    Router,
    http::{StatusCode, header},
    routing::{delete, get, patch, post},
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
use tracing::{info, warn};

pub fn create_router(state: Arc<AppState>) -> Router {
    let public_routes = Router::new().route("/opds", get(handlers::opds_handler));

    let protected_routes = Router::new()
        .route("/generate", post(handlers::generate_handler))
        .route("/feeds", get(handlers::list_feeds).post(handlers::add_feed))
        .route("/feeds/import", post(handlers::import_opml))
        .route("/feeds/{id}", delete(handlers::delete_feed))
        .route(
            "/schedules",
            get(handlers::list_schedules).post(handlers::add_schedule),
        )
        .route("/schedules/{id}", delete(handlers::delete_schedule))
        .route("/downloads", get(handlers::list_downloads))
        .route("/cover", post(handlers::upload_cover))
        .route(
            "/email-config",
            get(handlers::get_email_config_handler).post(handlers::update_email_config_handler),
        )
        .route(
            "/read-it-later",
            get(handlers::list_read_it_later).post(handlers::add_read_it_later),
        )
        .route(
            "/read-it-later/{id}",
            delete(handlers::delete_read_it_later).patch(handlers::update_read_it_later_status),
        )
        .route("/read-it-later/deliver", post(handlers::deliver_read_it_later))
        .route("/auth/check", get(|| async { StatusCode::OK }));

    let protected_routes =
        if std::env::var("RPUB_USERNAME").is_ok() && std::env::var("RPUB_PASSWORD").is_ok() {
            info!("Authentication enabled");
            protected_routes.layer(axum::middleware::from_fn(handlers::auth))
        } else {
            warn!("Authentication disabled (RPUB_USERNAME and/or RPUB_PASSWORD not set)");
            protected_routes
        };

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback_service(
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("public, max-age=3600"),
                ))
                .service(ServeDir::new("static")),
        )
        .with_state(state)
}
