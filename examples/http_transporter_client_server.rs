#![allow(dead_code, unused_imports)]

//! HTTP Transporter Combined Client-Server Example
//!
//! This example demonstrates a complete HTTP client-server setup in a single process.
//! The server listens for messages and the client sends requests to it.
//!
//! Run with: cargo run --example http_transporter_client_server

use moleculer::http_transporter::{Conn, HttpMessage};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Simple math request structure
#[derive(Serialize, Deserialize, Debug)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== HTTP Transporter Combined Client-Server Example ===\n");

    // Initialize logging
    env_logger::init();

    // Server configuration
    let server_host = "127.0.0.1";
    let server_port = 8080;
    let server_url = format!("http://{}:{}", server_host, server_port);

    // Create server connection
    let mut server_conn = Conn::new(server_host, server_port, "server-node").await?;
    println!("Server connection created: {}", server_conn.base_url());

    // Register handler for "math.add" topic on server
    let message_count = Arc::new(AtomicUsize::new(0));
    let count_clone = message_count.clone();

    server_conn
        .register_handler("math.add", move |msg: HttpMessage| {
            let count = count_clone.fetch_add(1, Ordering::SeqCst);
            println!("[Server] Received math.add request #{}", count);

            // Parse the request
            if let Ok(request) = serde_json::from_slice::<MathRequest>(&msg.data) {
                let result = request.a + request.b;
                println!(
                    "[Server] Computing: {} + {} = {} (sender: {:?})",
                    request.a, request.b, result, msg.sender
                );
            } else {
                println!("[Server] Failed to parse request data");
            }
            println!();
        })
        .await;

    // Start the HTTP server
    server_conn.start_server().await?;
    println!("\n[Server] Listening on {}", server_url);
    println!("[Server] Health check: {}/health", server_url);
    println!("[Server] Publish endpoint: POST {}/publish", server_url);

    // Give the server time to start
    sleep(Duration::from_millis(500)).await;

    // Create client connection
    let client_conn = Conn::new(server_host, server_port + 1, "client-node").await?;
    println!("\n[Client] Connection created: {}", client_conn.base_url());

    // Send some test requests
    println!("\n[Client] Sending math.add requests...\n");

    let request_count = Arc::new(AtomicUsize::new(0));

    // Send multiple requests
    for i in 1..=5 {
        let count = request_count.fetch_add(1, Ordering::SeqCst);

        let request = MathRequest { a: i, b: i * 2 };
        let request_data = serde_json::to_vec(&request)?;

        println!("[Client] Sending request #{}: {} + {}", count, i, i * 2);

        match client_conn
            .send(&server_url, "math.add", request_data)
            .await
        {
            Ok(_) => {
                println!("[Client] Request #{} sent successfully", count);
            }
            Err(e) => {
                println!("[Client] Failed to send request #{}: {}", count, e);
            }
        }

        // Small delay between requests
        sleep(Duration::from_millis(300)).await;
    }

    // Wait for all messages to be processed
    sleep(Duration::from_millis(1000)).await;

    println!("\n=== Summary ===");
    println!(
        "[Server] Total messages received: {}",
        message_count.load(Ordering::SeqCst)
    );
    println!(
        "[Client] Total requests sent: {}",
        request_count.load(Ordering::SeqCst)
    );

    // Stop the server
    println!("\n[Server] Shutting down...");
    server_conn.stop_server().await;
    println!("[Server] Stopped");

    Ok(())
}
