#![allow(private_interfaces)]

//! Transporter trait abstraction for pluggable transport layers.
//!
//! This module defines the `Transporter` trait that all transport implementations
//! must implement. This allows moleculer-rs to support different transport layers
//! like NATS, TCP, and HTTP through a unified interface.

use async_trait::async_trait;
use thiserror::Error;

use crate::config::{Channel, Config};

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
}

/// A message received from a subscription
pub struct ReceivedMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub reply_to: Option<String>,
}

/// Stream of received messages (trait object)
pub type SubscribeStream = std::pin::Pin<Box<dyn futures::Stream<Item = ReceivedMessage> + Send>>;

/// Trait that defines the interface for all transport implementations.
///
/// This trait provides a unified interface for publishing messages,
/// subscribing to topics, and making request/reply calls.
#[async_trait]
pub trait Transporter: Send + Sync {
    /// Connect to the transport layer
    async fn connect(&mut self, config: &Config) -> Result<()>;

    /// Disconnect from the transport layer
    async fn disconnect(&mut self);

    /// Publish a message to a channel
    async fn publish(&self, channel: &Channel, config: &Config, message: &[u8]) -> Result<()>;

    /// Subscribe to a channel and return a stream of messages
    async fn subscribe(&self, channel: &Channel, config: &Config) -> Result<SubscribeStream>;

    /// Make a request and wait for a response
    async fn request(&self, channel: &Channel, config: &Config, message: &[u8]) -> Result<Vec<u8>>;

    /// Get the transporter name for logging/debugging
    fn name(&self) -> &'static str;
}
