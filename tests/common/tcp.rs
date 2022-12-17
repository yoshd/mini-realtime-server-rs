use std::net::SocketAddr;

use async_trait::async_trait;
use futures::SinkExt;
use prost::Message as _;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use super::*;

pub struct ClientImpl {
    tx: mpsc::UnboundedSender<protobuf::app::ClientMessage>,
    rx: mpsc::UnboundedReceiver<protobuf::app::ServerMessage>,
}

// TODO: close, 雑なunwrap
impl ClientImpl {
    pub fn new(addr: SocketAddr) -> Self {
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<protobuf::app::ClientMessage>();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let conn = TcpStream::connect(addr).await.unwrap();
            conn.set_nodelay(true).unwrap();
            let (reader, writer) = tokio::io::split(conn);
            let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
            let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
            loop {
                tokio::select! {
                    input_msg = input_rx.recv() => {
                        if let Some(input_msg) = input_msg {
                            framed_writer.send(input_msg.encode_to_vec().into()).await.unwrap();
                        }
                    }
                    output_msg = framed_reader.next() => {
                        if let Some(output_msg) = output_msg {
                            match output_msg {
                                Ok(output_msg) => {
                                    let message: Result<protobuf::app::ServerMessage, prost::DecodeError> = prost::Message::decode(output_msg.as_ref());
                                    output_tx.send(message.unwrap()).unwrap();
                                }
                                Err(err) => {
                                    panic!("Unexpected error: {:?}", err);
                                }
                            };
                        }
                    }
                }
            }
        });

        Self {
            tx: input_tx,
            rx: output_rx,
        }
    }
}

#[async_trait]
impl Client for ClientImpl {
    fn generate() -> Self {
        Self::new(client_addr())
    }

    fn send(
        &self,
        message: protobuf::app::ClientMessage,
    ) -> Result<(), mpsc::error::SendError<protobuf::app::ClientMessage>> {
        self.tx.send(message)
    }

    async fn recv(&mut self) -> Option<protobuf::app::ServerMessage> {
        self.rx.recv().await
    }
}

fn run_server() {
    tokio::spawn(async move {
        network_protocol::server::run::<network_protocol::tcp::ServerImpl, _, _>(
            server_addr(),
            default_config(),
            network_protocol::server::wait_signal(),
        )
        .await
        .unwrap();
    });
}

fn server_addr() -> SocketAddr {
    env::var("GS_TEST_TCP_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8002".to_string())
        .parse::<SocketAddr>()
        .unwrap()
}

fn client_addr() -> SocketAddr {
    server_addr()
}

pub async fn setup() {
    if boot_server() {
        run_server();
        // TODO: 雑にスリープ。実際はReadiness Probe的なチェックが必要かも。
        sleep(Duration::from_millis(1000)).await;
    }
}

pub async fn teardown() {
    // TODO: shutdown
}
