use std::net::SocketAddr;

use super::router;
use axum;
use log::info;

use tokio::sync::oneshot;
// use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub struct HttpServer {
    shutdown_tx: oneshot::Sender<()>,
}

impl HttpServer {
    pub async fn new(addr: &SocketAddr) -> Self {
        let router = router::router();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });
        info!("start web server ...");
        tokio::spawn(server);
        Self { shutdown_tx }
    }

    pub async fn shutdown(self) {
        info!("send http server shutdown signal");
        let _ = self.shutdown_tx.send(());
    }
}
