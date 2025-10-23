#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]
use std::time::Duration;

#[wstd::test]
async fn simple_example() {
    let res = reqwest::get("https://hyper.rs")
        .await
        .expect("http get example");
    println!("Status: {}", res.status());

    let body = res.text().await.expect("response to utf-8 text");
    println!("Body:\n\n{body}");
}

#[wstd::test]
async fn request_with_timeout() {
    let client = reqwest::Client::new();
    let err = client
        .get("https://hyper.rs")
        .timeout(Duration::from_millis(1))
        .send()
        .await
        .expect_err("Expected error from aborted request");

    assert!(err.is_request());
    assert!(err.is_timeout());
}
