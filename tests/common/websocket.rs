use std::net::SocketAddr;

use async_trait::async_trait;
use futures::{pin_mut, SinkExt, StreamExt};
use prost::Message as _;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::*;

pub struct ClientImpl {
    tx: mpsc::UnboundedSender<protobuf::app::ClientMessage>,
    rx: mpsc::UnboundedReceiver<protobuf::app::ServerMessage>,
}

// TODO: close, 雑なunwrap
impl ClientImpl {
    pub fn new(addr: String) -> Self {
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<protobuf::app::ClientMessage>();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let (ws_stream, _) = tokio_tungstenite::connect_async(addr).await.unwrap();
            let (tx, rx) = ws_stream.split();
            pin_mut!(tx, rx);
            loop {
                tokio::select! {
                    input_msg = input_rx.recv() => {
                        if let Some(input_msg) = input_msg {
                            tx.send(Message::binary(input_msg.encode_to_vec())).await.unwrap();
                        }
                    }
                    output_msg = rx.next() => {
                        if let Some(output_msg) = output_msg {
                            match output_msg {
                                Ok(Message::Binary(output_msg)) => {
                                    let message: Result<protobuf::app::ServerMessage, prost::DecodeError> = prost::Message::decode(output_msg.as_ref());
                                    output_tx.send(message.unwrap()).unwrap();
                                }
                                Ok(output_msg) => {
                                    panic!("Unexpected message: {:?}", output_msg);
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
        network_protocol::server::run::<network_protocol::websocket::ServerImpl, _, _>(
            server_addr(),
            default_config(),
            network_protocol::server::wait_signal(),
        )
        .await
        .unwrap();
    });
}

fn server_addr() -> SocketAddr {
    env::var("GS_TEST_WEBSOCKET_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8001".to_string())
        .parse::<SocketAddr>()
        .unwrap()
}

fn client_addr() -> String {
    format!("ws://{}/app", server_addr())
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
