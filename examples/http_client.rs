//! HTTP Transporter Client Example
//!
//! This example demonstrates an HTTP client that sends requests to the server.
//!
//! Run this example after starting the server:
//! cargo run --example http_server
//!
//! Then in another terminal:
//! cargo run --example http_client localhost:8080

use moleculer::http_transporter::{self, Conn};
use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[derive(Serialize)]
struct MathRequest {
    a: i32,
    b: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== HTTP Transporter Client Example ===\n");

    // Get server URL from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let server_url = args.get(1).map(|s| s.as_str()).unwrap_or("localhost:8080");

    // Parse server URL
    let (host, port) = http_transporter::parse_http_address(server_url);
    println!("Connecting to server at {}:{}", host, port);

    // Create target URL
    let target_url = format!("http://{}:{}", host, port);

    // Create HTTP connection
    let conn = Conn::new(&host, port, "client-node").await?;
    println!("Client connection created: {}\n", conn.base_url());

    // Send some test requests
    println!("Sending math.add requests...\n");

    let request_count = Arc::new(AtomicUsize::new(0));

    // Send multiple requests
    for i in 1..=5 {
        let count = request_count.fetch_add(1, Ordering::SeqCst);

        let request = MathRequest { a: i, b: i * 2 };
        let request_data = serde_json::to_vec(&request)?;

        println!("[Client] Sending request #{}: {} + {}", count, i, i * 2);

        match conn.send(&target_url, "math.add", request_data).await {
            Ok(_) => {
                println!("[Client] Request #{} sent successfully", count);
            }
            Err(e) => {
                println!("[Client] Failed to send request #{}: {}", count, e);
            }
        }

        // Small delay between requests
        sleep(Duration::from_millis(500)).await;
    }

    println!("\n[Client] All requests sent!");
    println!(
        "[Client] Total requests sent: {}",
        request_count.load(Ordering::SeqCst)
    );

    Ok(())
}
