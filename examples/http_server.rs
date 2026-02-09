#![allow(dead_code, unused_imports)]

//! HTTP Transporter Client-Server Example
//!
//! This example demonstrates a complete HTTP client-server communication
//! using the moleculer-rs HTTP transporter.
//!
//! Run this example with two terminals:
//! Terminal 1 (Server): cargo run --example http_server
//! Terminal 2 (Client): cargo run --example http_client localhost:8080
//!
//! Or run both in one terminal:
//! cargo run --example http_transporter_client_server

use moleculer::http_transporter::{Conn, HttpConfig, HttpMessage};
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Simple math request structure
#[derive(Deserialize, Debug)]
struct MathRequest {
    a: i32,
    b: i32,
}

/// Response structure
#[derive(serde::Serialize)]
struct MathResponse {
    result: i32,
    from: String,
}

/// Server example - listens for messages and responds
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== HTTP Transporter Server Example ===\n");

    // Initialize logging
    env_logger::init();

    // Create server configuration
    let config = HttpConfig::new("127.0.0.1", 8080, "server-node");
    println!("Server configuration: {}", config.base_url());

    // Create HTTP connection
    let mut conn = Conn::new("127.0.0.1", 8080, "server-node").await?;
    println!("HTTP connection created\n");

    // Register handler for "math.add" topic
    let message_count = Arc::new(AtomicUsize::new(0));
    let count_clone = message_count.clone();

    conn.register_handler("math.add", move |msg: HttpMessage| {
        let count = count_clone.fetch_add(1, Ordering::SeqCst);
        println!("[Server] Received math.add request #{}", count);

        // Parse the request
        if let Ok(request) = serde_json::from_slice::<MathRequest>(&msg.data) {
            let result = request.a + request.b;
            println!(
                "[Server] Computing: {} + {} = {}",
                request.a, request.b, result
            );
            println!("[Server] Sender: {:?}", msg.sender);
        } else {
            println!("[Server] Failed to parse request data");
        }
        println!();
    })
    .await;

    // Start the HTTP server
    conn.start_server().await?;
    println!("[Server] Listening on http://127.0.0.1:8080");
    println!("[Server] Health check: http://127.0.0.1:8080/health");
    println!("[Server] Publish endpoint: POST http://127.0.0.1:8080/publish");
    println!("\nPress Ctrl+C to stop the server\n");

    // Keep the server running until Ctrl+C (SIGINT)
    // Note: Signal handling requires tokio with "signal" feature
    // For simplicity, we'll just run indefinitely until the process is killed
    // or use a simple loop that can be broken
    loop {
        sleep(Duration::from_secs(1)).await;
    }
}
