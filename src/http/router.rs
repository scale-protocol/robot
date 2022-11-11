use std::net::SocketAddr;

use log::info;

use axum::{
    self,
    error_handling::HandleErrorLayer,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, Path, TypedHeader,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};
use std::{borrow::Cow, time::Duration};
use tokio::sync::oneshot;
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use crate::bot;
// use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub struct HttpServer {
    shutdown_tx: oneshot::Sender<()>,
}

impl HttpServer {
    pub async fn new(addr: &SocketAddr, mp: bot::machine::SharedStateMap) -> Self {
        let router = router(mp);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });
        info!("start web server ...");
        tokio::spawn(async move {
            server.await.unwrap();
        });
        Self { shutdown_tx }
    }

    pub async fn shutdown(self) {
        info!("send http server shutdown signal");
        let _ = self.shutdown_tx.send(());
    }
}

pub fn router(mp: bot::machine::SharedStateMap) -> Router {
    let app: Router = Router::new()
        .route("/user/info/:pubkey", get(get_user_info))
        .route("/user/positions/:prefix", get(get_user_position_list))
        .route("/ws", get(ws_handler))
        // .layer(
        //     ServiceBuilder::new()
        //         // Handle errors from middleware
        //         .layer(HandleErrorLayer::new(handle_error))
        //         .load_shed()
        //         // .concurrency_limit(1024)
        //         .timeout(Duration::from_secs(3))
        //         .layer(
        //             TraceLayer::new_for_http()
        //                 .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        //         ), // .into_inner(),
        // )
        .layer(Extension(mp));
    app
}

async fn get_user_info(
    Path(key): Path<String>,
    Extension(state): Extension<bot::machine::SharedStateMap>,
) -> impl IntoResponse {
    let result = vec!["xxx"];
    Json(result)
}

async fn get_user_position_list(
    Path(key): Path<String>,
    Extension(state): Extension<bot::machine::SharedStateMap>,
) -> impl IntoResponse {
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
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        println!("`{}` connected", user_agent.as_str());
    }

    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(t) => {
                    println!("client sent str: {:?}", t);
                }
                Message::Binary(_) => {
                    println!("client sent binary data");
                }
                Message::Ping(_) => {
                    println!("socket ping");
                }
                Message::Pong(_) => {
                    println!("socket pong");
                }
                Message::Close(_) => {
                    println!("client disconnected");
                    return;
                }
            }
        } else {
            println!("client disconnected");
            return;
        }
    }

    loop {
        if socket
            .send(Message::Text(String::from("Hi!")))
            .await
            .is_err()
        {
            println!("client disconnected");
            return;
        }
    }
}
