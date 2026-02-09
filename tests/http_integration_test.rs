//! HTTP Integration Tests for moleculer-rs
//!
//! These tests verify that the HTTP transport works correctly
//! for both server and client operations.

use moleculer::{
    config::{ConfigBuilder, Transporter},
    service::{ActionBuilder, EventBuilder, Service},
    ActionContext, EventContext, ServiceBroker,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

// Test data structures
#[derive(Deserialize)]
struct AddParams {
    a: i32,
    b: i32,
}

#[derive(Serialize)]
struct AddResult {
    result: i32,
}

/// Test that HTTP transport can be created and connected
#[tokio::test]
async fn test_http_transport_creation() {
    let port = 9000;
    let config = ConfigBuilder::default()
        .node_id("test-creation-node")
        .transporter(Transporter::http(format!("0.0.0.0:{}", port)))
        .build();

    let _service_broker = ServiceBroker::new(config);
}

/// Test that a service with an action can be registered
#[tokio::test]
async fn test_service_registration() {
    let port = 9001;
    let config = ConfigBuilder::default()
        .node_id("test-registration-node")
        .transporter(Transporter::http(format!("0.0.0.0:{}", port)))
        .build();

    let add_action = ActionBuilder::new("testAdd")
        .add_callback(|ctx: ActionContext| {
            let params: AddParams = serde_json::from_value(ctx.params.clone())?;
            let result = AddResult {
                result: params.a + params.b,
            };
            ctx.reply(serde_json::to_value(result)?);
            Ok(())
        })
        .build();

    let math_service = Service::new("testMath").add_action(add_action);

    let _service_broker = ServiceBroker::new(config).add_service(math_service);
}

/// Test that an event handler can be registered
#[tokio::test]
async fn test_event_registration() {
    let port = 9002;
    let config = ConfigBuilder::default()
        .node_id("test-event-node")
        .transporter(Transporter::http(format!("0.0.0.0:{}", port)))
        .build();

    fn event_handler(_ctx: EventContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    let test_event = EventBuilder::new("testEvent")
        .add_callback(event_handler)
        .build();

    let test_service = Service::new("testService").add_event(test_event);

    let _service_broker = ServiceBroker::new(config).add_service(test_service);
}

/// Test HTTP transport address parsing
#[test]
fn test_http_address_parsing() {
    use moleculer::http_transporter::parse_http_address;

    // Test with host:port
    let (host, port) = parse_http_address("localhost:8080");
    assert_eq!(host, "localhost");
    assert_eq!(port, 8080);

    // Test with http:// prefix
    let (host, port) = parse_http_address("http://localhost:9090");
    assert_eq!(host, "localhost");
    assert_eq!(port, 9090);

    // Test with just port number
    let (host, port) = parse_http_address("3000");
    assert_eq!(host, "0.0.0.0");
    assert_eq!(port, 3000);
}

/// Test multiple services can be registered on the same broker
#[tokio::test]
async fn test_multiple_services() {
    let port = 9003;
    let config = ConfigBuilder::default()
        .node_id("test-multi-node")
        .transporter(Transporter::http(format!("0.0.0.0:{}", port)))
        .build();

    // First service
    fn service1_handler(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
        ctx.reply(serde_json::json!({"service": 1}));
        Ok(())
    }

    let service1_action = ActionBuilder::new("action1")
        .add_callback(service1_handler)
        .build();

    let service1 = Service::new("service1").add_action(service1_action);

    // Second service
    fn service2_handler(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
        ctx.reply(serde_json::json!({"service": 2}));
        Ok(())
    }

    let service2_action = ActionBuilder::new("action2")
        .add_callback(service2_handler)
        .build();

    let service2 = Service::new("service2").add_action(service2_action);

    let _service_broker = ServiceBroker::new(config)
        .add_service(service1)
        .add_service(service2);
}

/// Test that emit and broadcast don't panic
#[tokio::test]
async fn test_emit_and_broadcast() {
    let port = 9004;
    let config = ConfigBuilder::default()
        .node_id("test-emit-node")
        .transporter(Transporter::http(format!("0.0.0.0:{}", port)))
        .build();

    fn test_event_handler(_ctx: EventContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    let test_event = EventBuilder::new("testEvent")
        .add_callback(test_event_handler)
        .build();

    let test_service = Service::new("testService").add_event(test_event);

    let service_broker = ServiceBroker::new(config).add_service(test_service);

    // These should not panic
    service_broker.emit("testEvent", serde_json::json!({"test": 1}));
    service_broker.broadcast("testEvent", serde_json::json!({"test": 2}));
}

/// Test HTTP transporter configuration
#[tokio::test]
async fn test_http_transporter_config() {
    let _config = ConfigBuilder::default()
        .node_id("test-config-node")
        .transporter(Transporter::http("192.168.1.1:7000"))
        .build();
}

/// Test that the HTTP transport can handle the Transporter enum
#[test]
fn test_transporter_enum() {
    let http_transport = Transporter::http("localhost:8080");
    let nats_transport = Transporter::nats("nats://localhost:4222");

    // Both should be valid transporters
    match http_transport {
        Transporter::Http(addr) => assert_eq!(addr, "localhost:8080"),
        _ => panic!("Expected HTTP transporter"),
    }

    match nats_transport {
        Transporter::Nats(addr) => assert_eq!(addr, "nats://localhost:4222"),
        _ => panic!("Expected NATS transporter"),
    }
}

/// Test service with multiple actions and events
#[tokio::test]
async fn test_complex_service() {
    let port = 9005;
    let config = ConfigBuilder::default()
        .node_id("test-complex-node")
        .transporter(Transporter::http(format!("0.0.0.0:{}", port)))
        .build();

    fn action1_handler(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
        ctx.reply(serde_json::json!({"result": 1}));
        Ok(())
    }

    fn action2_handler(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
        ctx.reply(serde_json::json!({"result": 2}));
        Ok(())
    }

    fn event1_handler(_ctx: EventContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn event2_handler(_ctx: EventContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    let action1 = ActionBuilder::new("action1")
        .add_callback(action1_handler)
        .build();

    let action2 = ActionBuilder::new("action2")
        .add_callback(action2_handler)
        .build();

    let event1 = EventBuilder::new("event1")
        .add_callback(event1_handler)
        .build();

    let event2 = EventBuilder::new("event2")
        .add_callback(event2_handler)
        .build();

    let complex_service = Service::new("complexService")
        .add_action(action1)
        .add_action(action2)
        .add_event(event1)
        .add_event(event2);

    let _service_broker = ServiceBroker::new(config).add_service(complex_service);
}

/// Test that ConfigBuilder works correctly with all HTTP options
#[test]
fn test_config_builder_with_http() {
    let _config = ConfigBuilder::default()
        .node_id("test-builder-node")
        .transporter(Transporter::http("0.0.0.0:8888"))
        .log_level(log::Level::Debug)
        .build();
}

/// Test HTTP message structure
#[test]
fn test_http_message_structure() {
    use moleculer::http_transporter::HttpMessage;

    // Create a simple message
    let msg = HttpMessage::new("test.topic".to_string(), vec![1, 2, 3, 4]);
    assert_eq!(msg.topic, "test.topic");
    assert_eq!(msg.data, vec![1, 2, 3, 4]);
    assert!(msg.sender.is_none());
    assert!(msg.reply_to.is_none());

    // Create a message with sender
    let msg_with_sender = HttpMessage::with_sender(
        "another.topic".to_string(),
        vec![5, 6, 7, 8],
        "node-1".to_string(),
    );
    assert_eq!(msg_with_sender.topic, "another.topic");
    assert_eq!(msg_with_sender.sender, Some("node-1".to_string()));

    // Create a message with reply-to
    let msg_with_reply = HttpMessage::with_reply(
        "request.topic".to_string(),
        vec![9, 10],
        "node-1".to_string(),
        "reply.topic".to_string(),
    );
    assert_eq!(msg_with_reply.reply_to, Some("reply.topic".to_string()));
}

/// Test HTTP config structure
#[test]
fn test_http_config_structure() {
    use moleculer::http_transporter::HttpConfig;

    let config = HttpConfig::new("localhost", 8080, "test-node");

    assert_eq!(config.address, "localhost");
    assert_eq!(config.port, 8080);
    assert_eq!(config.node_id, "test-node");
    assert_eq!(config.base_url(), "http://localhost:8080");
}

/// Test HTTP response structure
#[test]
fn test_http_response_structure() {
    use moleculer::http_transporter::HttpResponse;

    let response = HttpResponse {
        success: true,
        data: vec![1, 2, 3],
        error: None,
    };

    assert!(response.success);
    assert_eq!(response.data, vec![1, 2, 3]);
    assert!(response.error.is_none());

    let error_response = HttpResponse {
        success: false,
        data: vec![],
        error: Some("Something went wrong".to_string()),
    };

    assert!(!error_response.success);
    assert!(error_response.error.is_some());
}
