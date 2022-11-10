use axum::{
    error_handling::HandleErrorLayer,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};
use std::{borrow::Cow, time::Duration};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;

pub fn router() -> Router {
    let app: Router = Router::new()
        .route("/user/info/:pubkey", get(get_user_info))
        .route("/user/positions/", get(get_user_position_list))
        .layer(
            ServiceBuilder::new()
                // Handle errors from middleware
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                // .concurrency_limit(1024)
                .timeout(Duration::from_secs(3))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        );
    app
}

async fn get_user_info() -> impl IntoResponse {
    let result = vec!["xxx"];
    Json(result)
}

async fn get_user_position_list() -> impl IntoResponse {
    let result = vec!["yy"];
    Json(result)
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {}", error)),
    )
}
