use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

extern crate mini_realtime_server;

pub use mini_realtime_server::*;

pub mod grpc;
pub mod scenario;
pub mod tcp;
pub mod websocket;

#[async_trait]
pub trait Client {
    fn generate() -> Self;

    fn send(
        &self,
        message: protobuf::app::ClientMessage,
    ) -> Result<(), mpsc::error::SendError<protobuf::app::ClientMessage>>;

    async fn recv(&mut self) -> Option<protobuf::app::ServerMessage>;
}

pub fn default_config() -> Arc<config::Config> {
    Arc::new(config::Config {
        auth: config::Auth {
            enable_bearer: true,
            bearer: "bearer".to_string(),
        },
        tls: config::Tls {
            enable: false,
            cert_file_path: "./server.crt".to_string(),
            key_file_path: "./server.key".to_string(),
        },
    })
}

pub fn boot_server() -> bool {
    env::var("GS_TEST_BOOT_SERVER")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap()
}
