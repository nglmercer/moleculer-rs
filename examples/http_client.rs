//! HTTP Client Example for moleculer-rs
//!
//! This example demonstrates how to make HTTP requests to a moleculer
//! HTTP server using a simple HTTP client (reqwest).
//!
//! # Running
//!
//! First, start the server in one terminal:
//! ```bash
//! cargo run --example http_server
//! ```
//!
//! Then, run the client in another terminal:
//! ```bash
//! cargo run --example http_client
//! ```

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize)]
struct HttpMessage {
    topic: String,
    data: Vec<u8>,
    sender: Option<String>,
    reply_to: Option<String>,
}

#[derive(Deserialize, Debug)]
struct HealthResponse {
    status: String,
    node_id: String,
}

#[derive(Deserialize, Debug)]
struct InfoResponse {
    node_id: String,
    transport: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    println!("🔗 HTTP Client Example");
    println!("======================");
    println!();

    let server_url = "http://localhost:8080";
    println!("🎯 Target server: {}", server_url);
    println!();

    // Create HTTP client
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    // Test 1: Health check
    println!("📝 Test 1: Health check");
    match client.get(format!("{}/health", server_url)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let health: HealthResponse = response.json().await?;
                println!("✅ Health check passed:");
                println!("   Status: {}", health.status);
                println!("   Node ID: {}", health.node_id);
            } else {
                println!("❌ Health check failed with status: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to connect to server: {}", e);
            println!("   Make sure the server is running: cargo run --example http_server");
        }
    }
    println!();

    // Test 2: Get node info
    println!("📝 Test 2: Get node info");
    match client.get(format!("{}/info", server_url)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let info: InfoResponse = response.json().await?;
                println!("✅ Node info received:");
                println!("   Node ID: {}", info.node_id);
                println!("   Transport: {}", info.transport);
            } else {
                println!("❌ Info request failed with status: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to get info: {}", e);
        }
    }
    println!();

    // Test 3: Publish a message
    println!("📝 Test 3: Publish a message");
    let message = HttpMessage {
        topic: "MOL.INFO".to_string(),
        data: b"test data".to_vec(),
        sender: Some("http-client".to_string()),
        reply_to: None,
    };

    match client
        .post(format!("{}/publish", server_url))
        .json(&message)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                println!("✅ Message published successfully");
            } else {
                println!("❌ Publish failed with status: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to publish: {}", e);
        }
    }
    println!();

    // Test 4: Send a request
    println!("📝 Test 4: Send a request");
    let request = HttpMessage {
        topic: "MOL.REQUEST.http-server-node".to_string(),
        data: b"request data".to_vec(),
        sender: Some("http-client".to_string()),
        reply_to: Some("MOL.RESPONSE.http-client".to_string()),
    };

    match client
        .post(format!("{}/request", server_url))
        .json(&request)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                println!("✅ Request sent successfully");
            } else {
                println!("❌ Request failed with status: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to send request: {}", e);
        }
    }
    println!();

    println!("🎉 All tests completed!");

    Ok(())
}
