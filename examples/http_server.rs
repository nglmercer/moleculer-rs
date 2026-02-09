//! HTTP Server Example for moleculer-rs
//!
//! This example demonstrates how to create a moleculer service broker
//! using HTTP transport. It creates a simple math service that can
//! handle actions (request/reply pattern).
//!
//! # Running
//!
//! ```bash
//! cargo run --example http_server
//! ```
//!
//! # Testing with curl
//!
//! Once the server is running, you can test it with curl:
//!
//! ```bash
//! # Health check
//! curl http://localhost:8080/health
//!
//! # Node info
//! curl http://localhost:8080/info
//!
//! # Publish a message to a topic
//! curl -X POST http://localhost:8080/publish \
//!   -H "Content-Type: application/json" \
//!   -d '{"topic": "test", "data": [1,2,3], "sender": "curl", "reply_to": null}'
//! ```

use std::error::Error;

use moleculer::{
    config::{ConfigBuilder, Transporter},
    service::{ActionBuilder, EventBuilder, Service},
    ActionContext, EventContext, ServiceBroker,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("🚀 Starting HTTP Server Example");
    println!("================================");

    // Build config with HTTP transporter
    // The server will listen on 0.0.0.0:8080
    let config = ConfigBuilder::default()
        .node_id("http-server-node")
        .transporter(Transporter::http("0.0.0.0:8080"))
        .log_level(log::Level::Info)
        .build();

    println!("📡 HTTP Transport configured on 0.0.0.0:8080");
    println!("   Endpoints:");
    println!("   - GET  /health - Health check");
    println!("   - GET  /info   - Node information");
    println!("   - POST /publish - Publish message");
    println!("   - POST /request - Request/reply");

    // Create math service with actions
    let math_add_action = ActionBuilder::new("math.add")
        .add_callback(math_add)
        .build();

    let math_multiply_action = ActionBuilder::new("math.multiply")
        .add_callback(math_multiply)
        .build();

    let math_service = Service::new("math")
        .add_action(math_add_action)
        .add_action(math_multiply_action);

    println!("📦 Service 'math' registered with actions:");
    println!("   - math.add (adds two numbers)");
    println!("   - math.multiply (multiplies two numbers)");

    // Create greeter service with events
    let greet_event = EventBuilder::new("greet")
        .add_callback(handle_greet)
        .build();

    let greeter_service = Service::new("greeter").add_event(greet_event);

    println!("📦 Service 'greeter' registered with events:");
    println!("   - greet (handles greeting events)");

    // Create service broker with services
    let service_broker = ServiceBroker::new(config)
        .add_service(math_service)
        .add_service(greeter_service);

    println!();
    println!("✅ Server ready! Press Ctrl+C to stop.");
    println!();
    println!("💡 Test with curl:");
    println!("   curl http://localhost:8080/health");
    println!("   curl http://localhost:8080/info");

    // Start the service broker (this will run forever)
    service_broker.start().await;

    Ok(())
}

/// Handler for math.add action
/// Adds two numbers and returns the result
fn math_add(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
    let params: MathParams = serde_json::from_value(ctx.params.clone())?;

    let result = MathResult {
        result: params.a + params.b,
        operation: "add".to_string(),
        a: params.a,
        b: params.b,
    };

    println!(
        "➕ math.add: {} + {} = {}",
        params.a, params.b, result.result
    );

    ctx.reply(serde_json::to_value(result)?);

    Ok(())
}

/// Handler for math.multiply action
/// Multiplies two numbers and returns the result
fn math_multiply(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
    let params: MathParams = serde_json::from_value(ctx.params.clone())?;

    let result = MathResult {
        result: params.a * params.b,
        operation: "multiply".to_string(),
        a: params.a,
        b: params.b,
    };

    println!(
        "✖️  math.multiply: {} * {} = {}",
        params.a, params.b, result.result
    );

    ctx.reply(serde_json::to_value(result)?);

    Ok(())
}

/// Handler for greet event
fn handle_greet(ctx: EventContext) -> Result<(), Box<dyn Error>> {
    let greeting: GreetMessage = serde_json::from_value(ctx.params)?;

    println!(
        "👋 Received greeting: '{}' from {}",
        greeting.message, greeting.from
    );

    Ok(())
}

// Data structures

#[derive(Deserialize)]
struct MathParams {
    a: i32,
    b: i32,
}

#[derive(Serialize)]
struct MathResult {
    result: i32,
    operation: String,
    a: i32,
    b: i32,
}

#[derive(Deserialize)]
struct GreetMessage {
    message: String,
    from: String,
}
