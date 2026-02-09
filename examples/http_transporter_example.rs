//! Simple HTTP Transporter Example
//!
//! This example demonstrates how to use the HTTP transporter in moleculer-rs.
//! It shows how to create an HTTP connection, parse addresses, and work with HTTP messages.

use moleculer::http_transporter::{self, Conn, HttpConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example 1: Using parse_http_address utility
    println!("=== Example 1: Parse HTTP Address ===");
    let (host, port) = http_transporter::parse_http_address("localhost:8080");
    println!("Parsed address: {}:{}", host, port);
    assert_eq!(host, "localhost");
    assert_eq!(port, 8080);

    let (host, port) = http_transporter::parse_http_address("http://localhost:8080");
    println!("Parsed with http:// prefix: {}:{}", host, port);
    assert_eq!(host, "localhost");
    assert_eq!(port, 8080);

    let (host, port) = http_transporter::parse_http_address("8080");
    println!("Parsed port-only: {}:{}", host, port);
    assert_eq!(host, "0.0.0.0");
    assert_eq!(port, 8080);

    // Example 2: Creating HTTP configuration
    println!("\n=== Example 2: HTTP Configuration ===");
    let config = HttpConfig::new("localhost", 8080, "test-node");
    println!("HTTP config base URL: {}", config.base_url());
    assert_eq!(config.base_url(), "http://localhost:8080");

    // Example 3: HTTP Message structures
    println!("\n=== Example 3: HTTP Message Structures ===");
    let message = http_transporter::HttpMessage {
        topic: "test.action".to_string(),
        data: b"hello world".to_vec(),
        sender: Some("sender-node".to_string()),
    };

    println!("Message topic: {}", message.topic);
    println!("Message sender: {:?}", message.sender);

    // Serialize and deserialize
    let serialized = serde_json::to_string(&message)?;
    println!("Serialized: {}", serialized);

    let deserialized: http_transporter::HttpMessage = serde_json::from_str(&serialized)?;
    assert_eq!(message, deserialized);
    println!("Message serialization works!");

    // Example 4: Creating HTTP connection
    println!("\n=== Example 4: HTTP Connection ===");
    let conn = Conn::new("127.0.0.1", 8080, "test-node-1").await?;
    println!("HTTP connection created: {}", conn.config().base_url());

    // Example 5: Message serialization tests
    println!("\n=== Example 5: Message Serialization ===");

    // Test HttpRequest
    let request = http_transporter::HttpRequest {
        topic: "math.add".to_string(),
        data: serde_json::json!({"a": 1, "b": 2}).to_string().into_bytes(),
        reply_to: Some("http://localhost:8080".to_string()),
    };

    let serialized = serde_json::to_string(&request)?;
    println!("Request serialized: {}", serialized);

    // Test HttpResponse
    let response = http_transporter::HttpResponse {
        success: true,
        data: serde_json::json!({"result": 3}).to_string().into_bytes(),
        error: None,
    };

    let serialized = serde_json::to_string(&response)?;
    println!("Response serialized: {}", serialized);

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}
