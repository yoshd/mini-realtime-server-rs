use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use env_logger;
use log::info;

mod actor;
mod config;
mod entity;
mod protobuf;
mod network_protocol;

use network_protocol::*;

#[tokio::main]
async fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let args = Args::parse();
    let addr = args.address.parse::<SocketAddr>().unwrap();
    let config = Arc::new(config::Config {
        auth: config::Auth {
            enable_bearer: args.enable_auth_bearer,
            bearer: args.auth_bearer,
        },
        tls: config::Tls {
            enable: args.enable_tls,
            cert_file_path: args.tls_cert_file_path,
            key_file_path: args.tls_key_file_path,
        },
    });
    info!("Start server. address={:?}", args.address);
    match args.protocol.as_str() {
        "websocket" => {
            server::run::<websocket::WebSocketServer, _, _>(addr, config, server::wait_signal())
                .await
                .unwrap()
        }
        "grpc" => server::run::<grpc::GrpcServer, _, _>(addr, config, server::wait_signal())
            .await
            .unwrap(),
        "tcp" => server::run::<tcp::TcpServer, _, _>(addr, config, server::wait_signal())
            .await
            .unwrap(),
        _ => panic!("invalid protocol"),
    };
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'p', long = "protocol", default_value = "websocket")]
    protocol: String,

    #[clap(short = 'a', long = "addr", default_value = "127.0.0.1:8000")]
    address: String,

    #[clap(long = "enable-auth-bearer", action = clap::ArgAction::Set, default_value = "true")]
    enable_auth_bearer: bool,

    #[clap(long = "auth-bearer", default_value = "test")]
    auth_bearer: String,

    #[clap(long = "enable-tls", action = clap::ArgAction::Set, default_value = "false")]
    enable_tls: bool,

    #[clap(
        long = "tls-cert-file-path",
        default_value = "./server.crt"
    )]
    tls_cert_file_path: String,

    #[clap(
        long = "tls-key-file-path",
        default_value = "./server.key"
    )]
    tls_key_file_path: String,
}
