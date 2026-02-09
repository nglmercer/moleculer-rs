//! HTTP Standalone Example for moleculer-rs
//!
//! This example demonstrates a self-contained HTTP service that can:
//! - Handle actions (request/reply pattern)
//! - Handle events
//! - Call its own services to demonstrate functionality
//!
//! # Running
//!
//! ```bash
//! cargo run --example http_standalone
//! ```

use std::error::Error;

use moleculer::{
    config::{ConfigBuilder, Transporter},
    service::{ActionBuilder, EventBuilder, Service},
    ActionContext, EventContext, ServiceBroker,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("🚀 Starting HTTP Standalone Example");
    println!("====================================");

    // Build config with HTTP transporter
    let config = ConfigBuilder::default()
        .node_id("http-standalone-node")
        .transporter(Transporter::http("0.0.0.0:8080"))
        .log_level(log::Level::Info)
        .build();

    println!("📡 HTTP Transport configured on 0.0.0.0:8080");

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

    // Create service broker
    let service_broker = ServiceBroker::new(config)
        .add_service(math_service)
        .add_service(greeter_service);

    println!();
    println!("⏳ Waiting for broker to initialize...");
    sleep(Duration::from_secs(3)).await;

    println!();
    println!("🚀 Starting self-tests...");
    println!();

    // Clone broker for async task
    let broker_for_call1 = service_broker.clone();
    let broker_for_call2 = service_broker.clone();
    let broker_for_emit = service_broker.clone();
    let broker_for_broadcast = service_broker.clone();

    // Spawn a task to make internal calls
    tokio::spawn(async move {
        // Wait for broker to be fully ready
        sleep(Duration::from_secs(2)).await;

        // Test 1: Call math.add (internal call)
        println!("📝 Test 1: Calling math.add with a=10, b=20");
        match broker_for_call1
            .call("math.add", serde_json::json!({"a": 10, "b": 20}))
            .await
        {
            Ok(result) => {
                println!(
                    "✅ Result: {}",
                    serde_json::to_string_pretty(&result).unwrap()
                );
            }
            Err(e) => {
                println!("❌ Error calling math.add: {}", e);
            }
        }
        println!();

        // Test 2: Call math.multiply (internal call)
        println!("📝 Test 2: Calling math.multiply with a=7, b=8");
        match broker_for_call2
            .call("math.multiply", serde_json::json!({"a": 7, "b": 8}))
            .await
        {
            Ok(result) => {
                println!(
                    "✅ Result: {}",
                    serde_json::to_string_pretty(&result).unwrap()
                );
            }
            Err(e) => {
                println!("❌ Error calling math.multiply: {}", e);
            }
        }
        println!();

        // Test 3: Emit a greet event
        println!("📝 Test 3: Emitting greet event");
        broker_for_emit.emit(
            "greet",
            serde_json::json!({
                "message": "Hello from standalone!",
                "from": "http-standalone-node"
            }),
        );
        println!("✅ Event emitted");
        println!();

        // Test 4: Broadcast a greet event
        println!("📝 Test 4: Broadcasting greet event");
        broker_for_broadcast.broadcast(
            "greet",
            serde_json::json!({
                "message": "Broadcast hello!",
                "from": "http-standalone-node"
            }),
        );
        println!("✅ Event broadcasted");
        println!();

        println!("🎉 All tests completed!");
        println!("Press Ctrl+C to exit.");
    });

    println!("✅ Server ready! Listening on http://0.0.0.0:8080");
    println!("   Endpoints:");
    println!("   - GET  /health - Health check");
    println!("   - GET  /info   - Node information");
    println!("   - POST /publish - Publish message");
    println!("   - POST /request - Request/reply");

    // Start the service broker
    service_broker.start().await;

    Ok(())
}

/// Handler for math.add action
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
