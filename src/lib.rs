pub mod auth;
pub mod cron;
pub mod db;
pub mod error;
pub mod handlers;
pub mod models;
pub mod openapi;
pub mod ratelimit;
pub mod state;
pub mod stats;

use axum::middleware;
use axum::routing::{get, post, put};
use axum::Router;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;
use crate::state::AppState;

/// Build the application router without binding a socket.
///
/// Keeping construction separate from `main` lets the HTTP contract be tested
/// in-process and ensures public documentation routes are not accidentally
/// wrapped by the subscription authentication layer.
pub fn build_app(state: AppState) -> Router {
    let protected = Router::new()
        .route(
            "/subscriptions/",
            post(handlers::create_subscription).get(handlers::list_subscriptions),
        )
        .route(
            "/subscriptions/{id}",
            put(handlers::update_subscription).delete(handlers::delete_subscription),
        )
        .route(
            "/subscriptions/{id}/cancel",
            put(handlers::cancel_subscription),
        )
        .route(
            "/subscriptions/{id}/restore",
            put(handlers::restore_subscription),
        )
        .route("/subscriptions/stats", get(handlers::get_stats))
        .route("/subscriptions/upcoming", get(handlers::get_upcoming_bills))
        .route(
            "/subscriptions/expired",
            get(handlers::get_expired_subscriptions),
        )
        .route(
            "/subscriptions",
            post(handlers::create_subscription).get(handlers::list_subscriptions),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::auth_middleware,
        ));

    Router::new()
        .route("/", get(handlers::root))
        .merge(protected)
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::rate_limit_middleware,
        ))
        .with_state(state)
}
