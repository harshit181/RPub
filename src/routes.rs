use crate::handlers;
use crate::models::AppState;
use axum::{
    http::{header, StatusCode},
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
use tracing::{info, warn};
use crate::handlers::{auth_handler, config_handler, domain_override_handler, download_handler, email_handler, feed_handler, read_it_later_handler, schedule_handler};

pub fn create_router(state: Arc<AppState>) -> Router {
    let public_routes = Router::new()
        .route("/opds", get(handlers::opds_handler))
        .route("/downloads/latest_rss.epub", get(download_handler::download_latest_rss))
        .route("/downloads/latest_readlater.epub", get(download_handler::download_latest_readlater));

    let protected_routes = Router::new()
        .route("/generate", post(download_handler::generate_epub_adhoc))
        .route("/feeds", get(feed_handler::list_feeds).post(feed_handler::add_feed))
        .route("/feeds/import", post(feed_handler::import_opml))
        .route("/feeds/{id}", delete(feed_handler::delete_feed))
        .route("/feeds/{id}/processor", get(feed_handler::get_feed_processor).put(feed_handler::update_feed_processor))
        .route(
            "/schedules",
            get(schedule_handler::list_schedules).post(schedule_handler::add_schedule),
        )
        .route("/schedules/{id}", delete(schedule_handler::delete_schedule))
        .route("/downloads", get(download_handler::list_downloads))
        .route("/cover", post(handlers::upload_cover))
        .route(
            "/email-config",
            get(email_handler::get_email_config_handler).post(email_handler::update_email_config_handler),
        )
        .route(
            "/general-config",
            get(config_handler::get_general_config).post(config_handler::update_general_config),
        )
        .route(
            "/read-it-later",
            get(read_it_later_handler::list_read_it_later).post(read_it_later_handler::add_read_it_later),
        )
        .route(
            "/read-it-later/{id}",
            delete(read_it_later_handler::delete_read_it_later).patch(read_it_later_handler::update_read_it_later_status),
        )
        .route("/read-it-later/deliver", post(read_it_later_handler::deliver_read_it_later))
        .route(
            "/domain-overrides",
            get(domain_override_handler::list_domain_overrides).post(domain_override_handler::add_domain_override),
        )
        .route("/domain-overrides/{id}", delete(domain_override_handler::delete_domain_override))
        .route("/auth/check", get(|| async { StatusCode::OK }));

    let protected_routes =
        if std::env::var("RPUB_USERNAME").is_ok() && std::env::var("RPUB_PASSWORD").is_ok() {
            info!("Authentication enabled");
            protected_routes.layer(axum::middleware::from_fn(auth_handler::auth))
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
