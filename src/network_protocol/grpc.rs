use std::sync::Arc;
use std::{error::Error, io::ErrorKind, net::ToSocketAddrs, pin::Pin};

use futures::{Future, Stream};
use log::info;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::transport::{Identity, ServerTlsConfig};
use tonic::{transport::Server, Response, Status};

use super::server;
use crate::actor;
use crate::config;
use crate::protobuf;

pub struct GrpcServer {
    config: Arc<config::Config>,
}

#[tonic::async_trait]
impl server::Server for GrpcServer {
    async fn run<A, F>(addr: A, config: Arc<config::Config>, shutdown: F) -> anyhow::Result<()>
    where
        A: ToSocketAddrs + Send + 'static,
        std::net::SocketAddr: From<A>,
        F: Future<Output = ()> + Send + 'static,
    {
        info!("Start gRPC server");
        let addr = addr.to_socket_addrs().unwrap().next().unwrap();
        let server = Self {
            config: config.clone(),
        };
        let mut builder = Server::builder();
        if config.tls.enable {
            info!("Enabled TLS for gRPC server");
            let cert = tokio::fs::read(&config.tls.cert_file_path).await?;
            let key = tokio::fs::read(&config.tls.key_file_path).await?;
            let identity = Identity::from_pem(cert, key);
            let tls_config = ServerTlsConfig::new().identity(identity);
            builder = builder.tls_config(tls_config)?;
        }
        builder
            .add_service(protobuf::app::app_server::AppServer::new(server))
            .serve_with_shutdown(addr, shutdown)
            .await?;
        Ok(())
    }
}

#[tonic::async_trait]
impl protobuf::app::app_server::App for GrpcServer {
    type StartStream =
        Pin<Box<dyn Stream<Item = Result<protobuf::app::ServerMessage, Status>> + Send>>;
    async fn start(
        &self,
        req: tonic::Request<tonic::Streaming<protobuf::app::ClientMessage>>,
    ) -> Result<tonic::Response<Self::StartStream>, Status> {
        info!("Connected player");
        let (tx, rx) = mpsc::channel(128);
        let mut in_stream = req.into_inner();
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut player_actor = actor::Player::new(config);
            loop {
                tokio::select! {
                    server_msg = player_actor.recv() => {
                        if let Some(server_msg) = server_msg {
                            if tx.send(Ok(server_msg)).await.is_err() {
                                info!("Disconnected player");
                                return;
                            }
                            continue;
                        }
                        // player_actorがDropしない限りNoneになることはないはず。
                        return;
                    }
                    client_msg = in_stream.next() => {
                        if let Some(client_msg) = client_msg {
                            match client_msg {
                                Ok(client_msg) => {
                                    // player_actorがDropしない限り失敗しないはず。
                                    player_actor.send(client_msg).unwrap();
                                    continue;
                                },
                                Err(err) => {
                                    if let Some(io_err) = match_for_io_error(&err) {
                                        if io_err.kind() == ErrorKind::BrokenPipe {
                                            info!("Disconnected player");
                                            break;
                                        }
                                    }
                                }
                            };
                        }

                        info!("Disconnected player");
                        return;
                    }
                }
            }
        });

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(out_stream) as Self::StartStream))
    }
}

// FYI: https://github.com/hyperium/tonic/blob/9ea03c2c44ad6bf76e4141e6fb40870b9b30ab63/examples/src/streaming/server.rs#L16
fn match_for_io_error(err_status: &Status) -> Option<&std::io::Error> {
    let mut err: &(dyn Error + 'static) = err_status;

    loop {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return Some(io_err);
        }

        // h2::Error do not expose std::io::Error with `source()`
        // https://github.com/hyperium/h2/pull/462
        if let Some(h2_err) = err.downcast_ref::<h2::Error>() {
            if let Some(io_err) = h2_err.get_io() {
                return Some(io_err);
            }
        }

        err = match err.source() {
            Some(err) => err,
            None => return None,
        };
    }
}
