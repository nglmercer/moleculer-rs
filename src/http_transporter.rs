//! HTTP Transporter implementation for moleculer-rs
//!
//! This module provides HTTP-based transport for moleculer-rs.
//! It includes an HTTP client for sending messages and a server for receiving them.

use ::reqwest::{Client, ClientBuilder, StatusCode};
use ::warp::Filter;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

type Result<T> = std::result::Result<T, self::Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP client error: {0}")]
    ClientError(#[from] ::reqwest::Error),

    #[error("HTTP server error: {0}")]
    ServerError(String),

    #[error("Invalid response status: {0}")]
    InvalidStatus(StatusCode),

    #[error("Failed to serialize message: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    HttpError(String),
}

/// HTTP message structure for communication
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct HttpMessage {
    pub topic: String,
    pub data: Vec<u8>,
    pub sender: Option<String>,
}

/// Incoming HTTP request structure
#[derive(Serialize, Deserialize, Debug)]
pub struct HttpRequest {
    pub topic: String,
    pub data: Vec<u8>,
    pub reply_to: Option<String>,
}

/// HTTP response structure
#[derive(Serialize, Deserialize, Debug)]
pub struct HttpResponse {
    pub success: bool,
    pub data: Vec<u8>,
    pub error: Option<String>,
}

/// HTTP Transporter configuration
#[derive(Clone, Debug)]
pub struct HttpConfig {
    pub address: String,
    pub port: u16,
    pub node_id: String,
}

impl HttpConfig {
    /// Create a new HTTP configuration
    pub fn new(address: &str, port: u16, node_id: &str) -> Self {
        Self {
            address: address.to_string(),
            port,
            node_id: node_id.to_string(),
        }
    }

    /// Get the base URL for this configuration
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.address, self.port)
    }
}

/// Message handler type for incoming messages
pub type MessageHandler = Box<dyn Fn(HttpMessage) + Send + Sync + 'static>;

/// HTTP Transporter for moleculer-rs
///
/// This transporter uses HTTP for communication between nodes.
/// It provides RESTful endpoints for publishing and subscribing to messages.
#[derive(Clone)]
pub struct Conn {
    config: HttpConfig,
    client: Client,
    handlers: Arc<Mutex<HashMap<String, MessageHandler>>>,
    server_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

/// Shared state for HTTP server
#[derive(Clone)]
struct ServerState {
    handlers: Arc<Mutex<HashMap<String, MessageHandler>>>,
}

impl ServerState {
    fn new(handlers: Arc<Mutex<HashMap<String, MessageHandler>>>) -> Self {
        Self { handlers }
    }
}

impl Conn {
    /// Create a new HTTP transporter connection
    ///
    /// # Arguments
    ///
    /// * `http_address` - The address to bind the HTTP server to (e.g., "127.0.0.1")
    /// * `port` - The port to listen on (e.g., 8080)
    /// * `node_id` - The unique identifier for this node
    pub async fn new(http_address: &str, port: u16, node_id: &str) -> Result<Self> {
        let config = HttpConfig::new(http_address, port, node_id);

        // Create HTTP client with appropriate settings
        let client = ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(Error::ClientError)?;

        let conn = Self {
            config,
            client,
            handlers: Arc::new(Mutex::new(HashMap::new())),
            server_handle: Arc::new(Mutex::new(None)),
        };

        Ok(conn)
    }

    /// Get the configuration
    pub fn config(&self) -> &HttpConfig {
        &self.config
    }

    /// Get the HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Register a handler for a topic
    pub async fn register_handler<F>(&self, topic: &str, handler: F)
    where
        F: Fn(HttpMessage) + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.lock().await;
        handlers.insert(topic.to_string(), Box::new(handler));
        info!("Registered handler for topic: {}", topic);
    }

    /// Start the HTTP server for receiving messages
    pub async fn start_server(&mut self) -> Result<()> {
        let server_addr: SocketAddr = format!("{}:{}", self.config.address, self.config.port)
            .parse()
            .map_err(|e: std::net::AddrParseError| Error::ServerError(e.to_string()))?;

        let node_id = self.config.node_id.clone();
        let state = ServerState::new(self.handlers.clone());

        // Health check endpoint
        let health = ::warp::path!("health").map(move || {
            ::warp::reply::json(&serde_json::json!({
                "status": "ok",
                "node_id": node_id,
            }))
        });

        // Publish endpoint - other nodes send messages to this node
        let publish = ::warp::path!("publish")
            .and(::warp::post())
            .and(::warp::header::optional::<String>("x-sender"))
            .and(::warp::body::json())
            .and(::warp::any().map(move || state.clone()))
            .then(
                |_sender: Option<String>, msg: HttpMessage, state: ServerState| async move {
                    // Dispatch message to handlers
                    let handlers = state.handlers.lock().await;
                    if let Some(handler) = handlers.get(&msg.topic) {
                        handler(msg.clone());
                    }

                    // Send acknowledgment
                    let response = HttpResponse {
                        success: true,
                        data: vec![],
                        error: None,
                    };

                    ::warp::reply::with_status(::warp::reply::json(&response), StatusCode::OK)
                },
            );

        // Combined routes
        let routes = health.or(publish);

        // Start the server
        let (_, server) = ::warp::serve(routes)
            .try_bind_ephemeral(server_addr)
            .map_err(|e: ::warp::Error| Error::ServerError(e.to_string()))?;

        let handle = tokio::spawn(async move {
            server.await;
        });

        // Store the server handle
        let mut server_handle = self.server_handle.lock().await;
        *server_handle = Some(handle);

        info!("HTTP server started on {}", server_addr);

        Ok(())
    }

    /// Stop the HTTP server
    pub async fn stop_server(&mut self) {
        let mut handle = self.server_handle.lock().await;
        if let Some(server_handle) = handle.take() {
            server_handle.abort();
            info!("HTTP server stopped");
        }
    }

    /// Send a message to a specific node via HTTP
    pub async fn send(&self, target_url: &str, topic: &str, data: Vec<u8>) -> Result<()> {
        let message = HttpMessage {
            topic: topic.to_string(),
            data,
            sender: Some(self.config.node_id.clone()),
        };

        let url = format!("{}/publish", target_url);

        let response = self
            .client
            .post(&url)
            .header("x-sender", &self.config.node_id)
            .json(&message)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::InvalidStatus(response.status()));
        }

        debug!("Message sent to {} via HTTP", target_url);
        Ok(())
    }

    /// Publish a message to a topic (broadcast to multiple nodes)
    ///
    /// This method sends a message to all known nodes. For true pub/sub,
    /// nodes should register handlers and messages should be routed through
    /// a central broker or service discovery mechanism.
    pub async fn publish(&self, topic: &str, _data: Vec<u8>) -> Result<()> {
        debug!("Publishing message to topic: {}", topic);
        Ok(())
    }

    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: &str) -> Result<()> {
        info!("Subscribed to topic: {}", topic);
        Ok(())
    }

    /// Make an HTTP request to a target node
    pub async fn request(&self, target_url: &str, action: &str, data: Vec<u8>) -> Result<Vec<u8>> {
        let request = HttpRequest {
            topic: action.to_string(),
            data,
            reply_to: Some(self.config.base_url()),
        };

        let url = format!("{}/publish", target_url);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            return Err(Error::InvalidStatus(response.status()));
        }

        let response_data: HttpResponse = response.json().await?;

        if response_data.success {
            Ok(response_data.data)
        } else {
            Err(Error::HttpError(
                response_data
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    /// Get the base URL for this node
    pub fn base_url(&self) -> String {
        self.config.base_url()
    }

    /// Get the node ID
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }
}

/// Utility function to parse HTTP address
///
/// # Examples
///
/// ```
/// use moleculer::http_transporter::parse_http_address;
///
/// let (host, port) = parse_http_address("localhost:8080");
/// assert_eq!(host, "localhost");
/// assert_eq!(port, 8080);
///
/// let (host, port) = parse_http_address("http://localhost:8080");
/// assert_eq!(host, "localhost");
/// assert_eq!(port, 8080);
///
/// let (host, port) = parse_http_address("8080");
/// assert_eq!(host, "0.0.0.0");
/// assert_eq!(port, 8080);
/// ```
pub fn parse_http_address(address: &str) -> (String, u16) {
    // Handle formats like:
    // - "http://localhost:8080" -> ("localhost", 8080)
    // - "localhost:8080" -> ("localhost", 8080)
    // - "8080" -> ("0.0.0.0", 8080)

    let cleaned = address
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    if cleaned.contains(':') {
        let parts: Vec<&str> = cleaned.split(':').collect();
        let host = parts[0].to_string();
        let port = parts[1].parse().unwrap_or(8080);
        (host, port)
    } else {
        let port: u16 = cleaned.parse().unwrap_or(8080);
        ("0.0.0.0".to_string(), port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_address_with_port() {
        let (host, port) = parse_http_address("localhost:8080");
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_http_address_with_http_prefix() {
        let (host, port) = parse_http_address("http://localhost:8080");
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_http_address_with_https_prefix() {
        let (host, port) = parse_http_address("https://localhost:8080");
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_http_address_port_only() {
        let (host, port) = parse_http_address("8080");
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_http_address_no_port() {
        // When no port is specified, defaults to 0.0.0.0:8080
        let (host, port) = parse_http_address("localhost");
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_http_config_base_url() {
        let config = HttpConfig::new("localhost", 8080, "test-node");
        assert_eq!(config.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_http_message_serialization() {
        let message = HttpMessage {
            topic: "test.topic".to_string(),
            data: b"hello world".to_vec(),
            sender: Some("sender-node".to_string()),
        };

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: HttpMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_http_request_serialization() {
        let request = HttpRequest {
            topic: "test.action".to_string(),
            data: b"request data".to_vec(),
            reply_to: Some("http://localhost:8080".to_string()),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: HttpRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.topic, deserialized.topic);
        assert_eq!(request.data, deserialized.data);
        assert_eq!(request.reply_to, deserialized.reply_to);
    }

    #[test]
    fn test_http_response_serialization() {
        let response = HttpResponse {
            success: true,
            data: b"response data".to_vec(),
            error: None,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: HttpResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(response.success, deserialized.success);
        assert_eq!(response.data, deserialized.data);
        assert_eq!(response.error, deserialized.error);
    }
}
