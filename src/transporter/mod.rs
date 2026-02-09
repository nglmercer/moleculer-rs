#![allow(private_interfaces)]

//! Transporter trait abstraction for pluggable transport layers.
//!
//! This module defines the `Transport` trait that all transport implementations
//! must implement. This allows moleculer-rs to support different transport layers
//! like NATS and HTTP through a unified interface.

use async_trait::async_trait;
use futures::Stream;
use thiserror::Error;

use crate::config::Config;

/// Result type for transporter operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during transporter operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Publish error: {0}")]
    Publish(String),

    #[error("Subscribe error: {0}")]
    Subscribe(String),

    #[error("Request error: {0}")]
    Request(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Transport not connected")]
    NotConnected,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Server error: {0}")]
    ServerError(String),
}

/// A message received from a subscription
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub reply_to: Option<String>,
}

impl ReceivedMessage {
    /// Create a new received message
    pub fn new(topic: String, payload: Vec<u8>) -> Self {
        Self {
            topic,
            payload,
            reply_to: None,
        }
    }

    /// Create a new received message with a reply-to address
    pub fn with_reply(topic: String, payload: Vec<u8>, reply_to: String) -> Self {
        Self {
            topic,
            payload,
            reply_to: Some(reply_to),
        }
    }
}

/// Stream of received messages (trait object)
pub type MessageStream = std::pin::Pin<Box<dyn Stream<Item = ReceivedMessage> + Send>>;

/// Trait that defines the interface for all transport implementations.
///
/// This trait provides a unified interface for publishing messages,
/// subscribing to topics, and making request/reply calls.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to the transport layer
    async fn connect(&mut self, config: &Config) -> Result<()>;

    /// Disconnect from the transport layer
    async fn disconnect(&mut self);

    /// Check if the transport is connected
    fn is_connected(&self) -> bool;

    /// Publish a message to a topic
    async fn publish(&self, topic: &str, message: &[u8]) -> Result<()>;

    /// Subscribe to a topic and return a stream of messages
    async fn subscribe(&self, topic: &str) -> Result<MessageStream>;

    /// Make a request and wait for a response (for request/reply pattern)
    async fn request(&self, topic: &str, message: &[u8], timeout_ms: u32) -> Result<Vec<u8>>;

    /// Get the transporter name for logging/debugging
    fn name(&self) -> &'static str;

    /// Get the node address for this transport (used for HTTP)
    fn node_address(&self) -> Option<&str> {
        None
    }
}

/// Factory for creating transport instances based on configuration
pub fn create_transport(config: &Config) -> Box<dyn Transport> {
    use crate::config::Transporter;

    match &config.transporter {
        Transporter::Nats(address) => Box::new(NatsTransport::new(address.clone())),
        Transporter::Http(address) => {
            Box::new(HttpTransport::new(address.clone(), config.node_id.clone()))
        }
    }
}

// Re-export the specific implementations
pub mod http_transport;
pub mod nats_transport;

pub use http_transport::HttpTransport;
pub use nats_transport::NatsTransport;
