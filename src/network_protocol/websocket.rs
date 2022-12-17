use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use async_trait::async_trait;
use futures::{pin_mut, Future, SinkExt, StreamExt};
use log::{debug, error, info};
use prost::Message as _;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

use super::server;
use crate::actor;
use crate::config;
use crate::protobuf;

pub struct ServerImpl {}

#[async_trait]
impl server::Server for ServerImpl {
    async fn run<A, F>(addr: A, config: Arc<config::Config>, shutdown: F) -> anyhow::Result<()>
    where
        A: ToSocketAddrs + Send + 'static,
        std::net::SocketAddr: From<A>,
        F: Future<Output = ()> + Send + 'static,
    {
        info!("Start WebSocket server");
        let config_for_routes = config.clone();
        let routes = warp::path("app")
            .and(warp::ws())
            .and(warp::addr::remote())
            .map(move |ws: warp::ws::Ws, addr: Option<SocketAddr>| {
                let config = config_for_routes.clone();
                ws.on_upgrade(move |ws| handle_ws(ws, addr, config))
            });

        if config.tls.enable {
            info!("Enabled TLS for WebSocket server.");
            warp::serve(routes)
                .tls()
                .cert_path(&config.tls.cert_file_path)
                .key_path(&config.tls.key_file_path)
                .bind_with_graceful_shutdown(addr, shutdown)
                .1
                .await;
        } else {
            warp::serve(routes)
                .bind_with_graceful_shutdown(addr, shutdown)
                .1
                .await;
        }
        Ok(())
    }
}

async fn handle_ws(ws: WebSocket, addr: Option<SocketAddr>, config: Arc<config::Config>) {
    let (tx, rx) = ws.split();
    pin_mut!(tx, rx);
    info!("Connected player");
    let mut player_actor = actor::Player::new(config);
    loop {
        tokio::select! {
            server_msg = player_actor.recv() => {
                if let Some(server_msg) = server_msg {
                    tx.send(Message::binary(server_msg.encode_to_vec())).await.unwrap();
                    continue;
                }

                info!("Disconnected player");
                return;
            }
            message = rx.next() => {
                if let Some(message) = message {
                    let message = match message {
                        Ok(message) => message,
                        Err(err) => {
                            error!("message error:{:?}", err);
                            return;
                        }
                    };

                    if message.is_close() {
                        info!("close client: {:?}", addr);
                        tx.close().await.unwrap_or_else(|e| {
                            error!("websocket close error: {:?}", e);
                        });
                        return;
                    }

                    if message.is_ping() {
                        debug!("receive ping");
                        tx.send(Message::pong(message)).await.unwrap_or_else(|e| {
                            error!("websocket pong error: {:?}", e);
                        });
                        continue;
                    }

                    if message.is_binary() {
                        let message: Result<protobuf::app::ClientMessage, prost::DecodeError> =
                            prost::Message::decode(message.as_bytes());

                        match message {
                            Ok(message) => {
                                // player_actorがDropしない限り失敗しないはず。
                                let _ = player_actor.send(message);
                            }
                            Err(err) => {
                                error!("Received invalid message. {:?}", err);
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}
