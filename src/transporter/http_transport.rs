//! HTTP Transport implementation for moleculer-rs
//!
//! This module provides HTTP-based transport for moleculer-rs using a REST API.
//! It supports both client and server modes for full-duplex communication.

use async_trait::async_trait;
use futures::Stream;
use log::{debug, info};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use warp::Filter;

use super::{Error, MessageStream, ReceivedMessage, Result, Transport};
use crate::config::Config;

/// HTTP Transport implementation
pub struct HttpTransport {
    address: String,
    node_id: String,
    http_client: Option<HttpClient>,
    server_handle: Option<JoinHandle<()>>,
    subscribers: Arc<RwLock<HashMap<String, broadcast::Sender<ReceivedMessage>>>>,
    is_connected: bool,
    port: u16,
    bind_address: String,
}

impl HttpTransport {
    /// Create a new HTTP transport with the given address and node ID
    pub fn new(address: String, node_id: String) -> Self {
        let (bind_address, port) = parse_http_address(&address);

        Self {
            address,
            node_id,
            http_client: None,
            server_handle: None,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            is_connected: false,
            port,
            bind_address,
        }
    }

    /// Start the HTTP server for receiving messages
    async fn start_server(&mut self) -> Result<()> {
        let subscribers = self.subscribers.clone();
        let node_id = self.node_id.clone();

        // Health check endpoint
        let health_route = warp::path!("health").map(move || {
            warp::reply::json(&serde_json::json!({
                "status": "ok",
                "node_id": node_id,
            }))
        });

        // Message publishing endpoint
        let subscribers_clone = subscribers.clone();
        let publish_route = warp::path!("publish")
            .and(warp::post())
            .and(warp::header::optional::<String>("x-sender"))
            .and(warp::body::json())
            .then(move |_sender: Option<String>, msg: HttpMessage| {
                let subscribers = subscribers_clone.clone();
                async move {
                    // Dispatch message to subscribers
                    let subs = subscribers.read().await;
                    if let Some(tx) = subs.get(&msg.topic) {
                        let received = ReceivedMessage::with_reply(
                            msg.topic.clone(),
                            msg.data.clone(),
                            msg.reply_to.unwrap_or_default(),
                        );
                        let _ = tx.send(received);
                    }
                    drop(subs);

                    // Send acknowledgment
                    let response = HttpResponse {
                        success: true,
                        data: vec![],
                        error: None,
                    };
                    warp::reply::with_status(
                        warp::reply::json(&response),
                        warp::http::StatusCode::OK,
                    )
                }
            });

        // Request endpoint (for request/reply pattern)
        let subscribers_clone = subscribers.clone();
        let request_route = warp::path!("request")
            .and(warp::post())
            .and(warp::body::json())
            .then(move |msg: HttpMessage| {
                let subscribers = subscribers_clone.clone();
                async move {
                    let subs = subscribers.read().await;
                    if let Some(tx) = subs.get(&msg.topic) {
                        let received = ReceivedMessage::with_reply(
                            msg.topic.clone(),
                            msg.data.clone(),
                            msg.reply_to.clone().unwrap_or_default(),
                        );
                        let _ = tx.send(received);
                    }
                    drop(subs);

                    // For now, return acknowledgment - actual response comes via reply
                    let response = HttpResponse {
                        success: true,
                        data: vec![],
                        error: None,
                    };
                    warp::reply::with_status(
                        warp::reply::json(&response),
                        warp::http::StatusCode::OK,
                    )
                }
            });

        // Node info endpoint
        let node_id_clone = self.node_id.clone();
        let info_route = warp::path!("info").map(move || {
            warp::reply::json(&serde_json::json!({
                "node_id": node_id_clone,
                "transport": "http",
            }))
        });

        let routes = health_route
            .or(publish_route)
            .or(request_route)
            .or(info_route);

        let server_addr: SocketAddr = format!("{}:{}", self.bind_address, self.port)
            .parse()
            .map_err(|e| Error::InvalidAddress(format!("Invalid server address: {}", e)))?;

        let (_, server) = warp::serve(routes)
            .try_bind_ephemeral(server_addr)
            .map_err(|e| Error::ServerError(format!("Failed to bind server: {}", e)))?;

        let handle = tokio::spawn(async move {
            server.await;
        });

        self.server_handle = Some(handle);
        info!(
            "HTTP transport server started on {}:{}",
            self.bind_address, self.port
        );

        Ok(())
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn connect(&mut self, _config: &Config) -> Result<()> {
        info!(
            "Connecting HTTP transport at {}:{}",
            self.bind_address, self.port
        );

        // Create HTTP client
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Http(format!("Failed to create HTTP client: {}", e)))?;

        self.http_client = Some(http_client);

        // Start the HTTP server
        self.start_server().await?;

        self.is_connected = true;
        info!("HTTP transport connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) {
        if self.is_connected {
            info!("Disconnecting HTTP transport");

            if let Some(handle) = self.server_handle.take() {
                handle.abort();
            }

            self.http_client = None;
            self.is_connected = false;

            info!("HTTP transport disconnected");
        }
    }

    fn is_connected(&self) -> bool {
        self.is_connected
    }

    async fn publish(&self, topic: &str, message: &[u8]) -> Result<()> {
        // For HTTP transport, publishing is done locally to subscribers
        // Remote publishing is handled via send_to_node
        let subscribers = self.subscribers.read().await;
        if let Some(tx) = subscribers.get(topic) {
            let received = ReceivedMessage::new(topic.to_string(), message.to_vec());
            let _ = tx.send(received);
        }
        debug!("Published message to HTTP topic: {}", topic);
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<MessageStream> {
        let mut subscribers = self.subscribers.write().await;

        // Create a new broadcast channel if this is a new topic
        if !subscribers.contains_key(topic) {
            let (tx, _) = broadcast::channel(1024);
            subscribers.insert(topic.to_string(), tx);
        }

        let tx = subscribers.get(topic).unwrap().clone();
        let rx = tx.subscribe();

        debug!("Subscribed to HTTP topic: {}", topic);
        Ok(Box::pin(HttpMessageStream::new(rx)))
    }

    async fn request(&self, topic: &str, message: &[u8], _timeout_ms: u32) -> Result<Vec<u8>> {
        // For HTTP transport, request is handled by sending to a specific node
        // This is a simplified implementation
        let subscribers = self.subscribers.read().await;
        if let Some(tx) = subscribers.get(topic) {
            let received = ReceivedMessage::new(topic.to_string(), message.to_vec());
            let _ = tx.send(received);
        }

        // Return empty response - actual request/reply is handled at higher level
        Ok(vec![])
    }

    fn name(&self) -> &'static str {
        "http"
    }

    fn node_address(&self) -> Option<&str> {
        Some(&self.address)
    }
}

/// HTTP message structure for communication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HttpMessage {
    pub topic: String,
    pub data: Vec<u8>,
    pub sender: Option<String>,
    pub reply_to: Option<String>,
}

impl HttpMessage {
    /// Create a new HTTP message
    pub fn new(topic: String, data: Vec<u8>) -> Self {
        Self {
            topic,
            data,
            sender: None,
            reply_to: None,
        }
    }

    /// Create a new HTTP message with sender
    pub fn with_sender(topic: String, data: Vec<u8>, sender: String) -> Self {
        Self {
            topic,
            data,
            sender: Some(sender),
            reply_to: None,
        }
    }

    /// Create a new HTTP message with reply-to address
    pub fn with_reply(topic: String, data: Vec<u8>, sender: String, reply_to: String) -> Self {
        Self {
            topic,
            data,
            sender: Some(sender),
            reply_to: Some(reply_to),
        }
    }
}

/// HTTP response structure
#[derive(Serialize, Deserialize, Debug)]
pub struct HttpResponse {
    pub success: bool,
    pub data: Vec<u8>,
    pub error: Option<String>,
}

/// HTTP configuration structure
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

/// Wrapper stream that converts broadcast messages to ReceivedMessage
pub struct HttpMessageStream {
    receiver: broadcast::Receiver<ReceivedMessage>,
}

impl HttpMessageStream {
    fn new(receiver: broadcast::Receiver<ReceivedMessage>) -> Self {
        Self { receiver }
    }
}

impl Stream for HttpMessageStream {
    type Item = ReceivedMessage;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Use tokio's broadcast receiver via a futures-compatible approach
        // We need to manually poll the receiver
        match self.receiver.try_recv() {
            Ok(msg) => Poll::Ready(Some(msg)),
            Err(broadcast::error::TryRecvError::Empty) => {
                // Register waker and return Pending
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(broadcast::error::TryRecvError::Closed) => Poll::Ready(None),
            Err(broadcast::error::TryRecvError::Lagged(_)) => {
                // Continue processing even if we lagged
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

/// Parse HTTP address into host and port
///
/// # Examples
///
/// ```
/// use moleculer::transporter::http_transport::parse_http_address;
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

/// HTTP client for sending messages to other nodes
pub struct HttpClientTransport {
    client: HttpClient,
    node_id: String,
}

impl HttpClientTransport {
    /// Create a new HTTP client transport
    pub fn new(node_id: String) -> Result<Self> {
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Http(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, node_id })
    }

    /// Send a message to a specific node
    pub async fn send_to_node(&self, node_url: &str, topic: &str, data: Vec<u8>) -> Result<()> {
        let message = HttpMessage::with_sender(topic.to_string(), data, self.node_id.clone());

        let url = format!("{}/publish", node_url);

        let response = self
            .client
            .post(&url)
            .header("x-sender", &self.node_id)
            .json(&message)
            .send()
            .await
            .map_err(|e| Error::Http(format!("Failed to send message: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Http(format!(
                "Failed to send message, status: {}",
                response.status()
            )));
        }

        debug!("Message sent to {} via HTTP", node_url);
        Ok(())
    }

    /// Send a request to a specific node and wait for a response
    pub async fn request_from_node(
        &self,
        node_url: &str,
        topic: &str,
        data: Vec<u8>,
        reply_to: String,
    ) -> Result<Vec<u8>> {
        let message =
            HttpMessage::with_reply(topic.to_string(), data, self.node_id.clone(), reply_to);

        let url = format!("{}/request", node_url);

        let response = self
            .client
            .post(&url)
            .json(&message)
            .send()
            .await
            .map_err(|e| Error::Http(format!("Failed to send request: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Http(format!(
                "Failed to send request, status: {}",
                response.status()
            )));
        }

        let response_data: HttpResponse = response
            .json()
            .await
            .map_err(|e| Error::Http(format!("Failed to parse response: {}", e)))?;

        if response_data.success {
            Ok(response_data.data)
        } else {
            Err(Error::Http(
                response_data
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
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
        let (host, port) = parse_http_address("localhost");
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_http_message_serialization() {
        let message = HttpMessage::with_sender(
            "test.topic".to_string(),
            b"hello world".to_vec(),
            "sender-node".to_string(),
        );

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: HttpMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(message.topic, deserialized.topic);
        assert_eq!(message.data, deserialized.data);
        assert_eq!(message.sender, deserialized.sender);
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
