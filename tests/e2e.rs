mod common;
use common::*;
use futures::FutureExt;

#[tokio::test]
async fn e2e_grpc() {
    grpc::setup().await;
    let result = std::panic::AssertUnwindSafe(async {
        scenario::run_e2e_all::<grpc::ClientImpl>().await;
    })
    .catch_unwind()
    .await;
    grpc::teardown().await;
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

#[tokio::test]
async fn e2e_websocket() {
    websocket::setup().await;
    let result = std::panic::AssertUnwindSafe(async {
        scenario::run_e2e_all::<websocket::ClientImpl>().await;
    })
    .catch_unwind()
    .await;
    websocket::teardown().await;
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

#[tokio::test]
async fn e2e_tcp() {
    tcp::setup().await;
    let result = std::panic::AssertUnwindSafe(async {
        scenario::run_e2e_all::<tcp::ClientImpl>().await;
    })
    .catch_unwind()
    .await;
    tcp::teardown().await;
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}
