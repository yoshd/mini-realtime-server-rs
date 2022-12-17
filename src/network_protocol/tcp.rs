use std::net::ToSocketAddrs;
use std::path::Path;
use std::{net::SocketAddr, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use futures::Future;
use futures::SinkExt;
use log::{error, info};
use prost::Message as _;
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls;
use tokio_rustls::rustls::{Certificate, PrivateKey};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

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
        info!("Start TCP server");
        let addr = addr.to_socket_addrs()?.next().unwrap();

        let mut tls_acceptor = None;
        if config.tls.enable {
            info!("Enabled TLS for TCP server");
            let certs = load_certs(&config.tls.cert_file_path)?;
            let key = load_key(&config.tls.key_file_path)?;
            let tls_config = rustls::ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_single_cert(certs, key)?;
            tls_acceptor = Some(TlsAcceptor::from(Arc::new(tls_config)));
        }

        let listener = TcpListener::bind(addr).await?;
        let (internal_shutdown_tx, mut internal_shutdown_rx) = tokio::sync::oneshot::channel();
        let listener_join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((client, addr)) => {
                                let config = config.clone();
                                let tls_acceptor = tls_acceptor.clone();
                                tokio::spawn(async move {
                                    handle_tcp_connection(client, tls_acceptor, addr, config).await;
                                });
                            }
                            Err(err) => {
                                error!("Connection error. {:?}", err);
                            }
                        };
                    }
                    _ = &mut internal_shutdown_rx => {
                        // TODO: 全員抜けるまで待つ。
                        return;
                    }
                }
            }
        });

        shutdown.await;
        info!("shutdown TCP server");
        internal_shutdown_tx
            .send(())
            .map_err(|_| anyhow!("internal shutdown error"))?;
        tokio::join!(listener_join_handle).0?;
        Ok(())
    }
}

async fn handle_tcp_connection(
    stream: TcpStream,
    acceptor: Option<TlsAcceptor>,
    addr: SocketAddr,
    config: Arc<config::Config>,
) {
    let result = TcpStreamType::from(stream, acceptor).await;
    match result {
        Ok(stream_type) => {
            match stream_type {
                TcpStreamType::Plain(stream) => {
                    let (reader, writer) = tokio::io::split(stream);
                    handle_rw_stream(reader, writer, addr, config).await;
                }
                TcpStreamType::Tls(stream) => {
                    let (reader, writer) = tokio::io::split(stream);
                    handle_rw_stream(reader, writer, addr, config).await;
                }
            };
        }
        Err(err) => {
            error!("Connection error. {:?}", err);
        }
    }
}

async fn handle_rw_stream(
    reader: ReadHalf<impl AsyncRead>,
    writer: WriteHalf<impl AsyncWrite>,
    addr: SocketAddr,
    config: Arc<config::Config>,
) {
    let mut frame_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
    let mut player_actor = actor::Player::new(config);
    loop {
        tokio::select! {
            server_msg = player_actor.recv() => {
                if let Some(server_msg) = server_msg {
                    framed_writer.send(server_msg.encode_to_vec().into()).await.unwrap();
                    continue;
                }

                info!("Disconnected player");
                return;
            }
            frame = frame_reader.next() => {
                if let Some(frame) = frame {
                    match frame {
                        Ok(message) => {
                            info!("received: addr={:?}, data={:?}", addr, message);
                            let message: Result<protobuf::app::ClientMessage, prost::DecodeError> =
                                prost::Message::decode(message);

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
                        Err(err) => {
                            error!("Received invalid message. addr={:?}, error={:?}", addr, err);
                            return;
                        }
                    }
                }
            }
        }
    }
}

enum TcpStreamType {
    Tls(Box<TlsStream<TcpStream>>),
    Plain(Box<TcpStream>),
}

impl TcpStreamType {
    pub async fn from(stream: TcpStream, acceptor: Option<TlsAcceptor>) -> anyhow::Result<Self> {
        match acceptor {
            Some(tls_acceptor) => Ok(Self::Tls(Box::new(tls_acceptor.accept(stream).await?))),
            None => Ok(Self::Plain(Box::new(stream))),
        }
    }
}

pub fn load_certs(path: impl AsRef<Path>) -> anyhow::Result<Vec<Certificate>> {
    let fd = std::fs::File::open(path)?;
    let mut buf = std::io::BufReader::new(&fd);
    let certs = rustls_pemfile::certs(&mut buf)?
        .into_iter()
        .map(Certificate)
        .collect();
    Ok(certs)
}

pub fn load_key(path: impl AsRef<Path>) -> anyhow::Result<PrivateKey> {
    let fd = std::fs::File::open(path)?;
    let mut buf = std::io::BufReader::new(&fd);
    rustls_pemfile::pkcs8_private_keys(&mut buf)?
        .into_iter()
        .map(PrivateKey)
        .next()
        .ok_or_else(|| anyhow!("failed to load private key"))
}
