use async_trait::async_trait;
use tokio::sync::mpsc;

use super::*;

pub struct ClientImpl {
    tx: mpsc::UnboundedSender<protobuf::app::ClientMessage>,
    rx: mpsc::UnboundedReceiver<protobuf::app::ServerMessage>,
}

// TODO: close, 雑なunwrap
impl ClientImpl {
    pub fn new(addr: String) -> Self {
        let (input_tx, mut input_rx) = mpsc::unbounded_channel();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut client = protobuf::app::app_client::AppClient::connect(addr)
                .await
                .unwrap();
            let stream = async_stream::stream! {
                while let Some(message) = input_rx.recv().await {
                    yield message;
                }
            };
            let mut inbound = client.start(stream).await.unwrap().into_inner();
            while let Some(message) = inbound.message().await.unwrap() {
                output_tx.send(message).unwrap();
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
        network_protocol::server::run::<network_protocol::grpc::ServerImpl, _, _>(
            server_addr(),
            default_config(),
            network_protocol::server::wait_signal(),
        )
        .await
        .unwrap();
    });
}

fn server_addr() -> SocketAddr {
    env::var("GS_TEST_GRPC_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8000".to_string())
        .parse::<SocketAddr>()
        .unwrap()
}

fn client_addr() -> String {
    format!("http://{}", server_addr())
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
