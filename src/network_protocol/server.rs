use std::{net::ToSocketAddrs, sync::Arc};

use async_trait::async_trait;
use futures::Future;
use tokio::signal::unix::{signal, SignalKind};

use crate::config;

#[async_trait]
pub trait Server {
    async fn run<A, F>(addr: A, config: Arc<config::Config>, shutdown: F) -> anyhow::Result<()>
    where
        A: ToSocketAddrs + Send + 'static,
        std::net::SocketAddr: From<A>,
        F: Future<Output = ()> + Send + 'static;
}

pub async fn run<S, A, F>(addr: A, config: Arc<config::Config>, shutdown: F) -> anyhow::Result<()>
where
    S: Server,
    A: ToSocketAddrs + Send + 'static,
    std::net::SocketAddr: From<A>,
    F: Future<Output = ()> + Send + 'static,
{
    S::run(addr, config, shutdown).await
}

pub async fn wait_signal() {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    };
}
