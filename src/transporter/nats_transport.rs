//! NATS Transport implementation

use async_nats::{Client, Subject, Subscriber};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use log::{debug, error, info, warn};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::time::{timeout, Duration};

use super::{Error, MessageStream, ReceivedMessage, Result, Transport};
use crate::config::Config;

/// NATS Transport implementation
pub struct NatsTransport {
    address: String,
    client: Option<Client>,
}

impl NatsTransport {
    /// Create a new NATS transport with the given address
    pub fn new(address: String) -> Self {
        Self {
            address,
            client: None,
        }
    }
}

#[async_trait]
impl Transport for NatsTransport {
    async fn connect(&mut self, _config: &Config) -> Result<()> {
        info!("Connecting to NATS at {}", self.address);

        let client = async_nats::connect(&self.address)
            .await
            .map_err(|e| Error::Connection(format!("NATS connection failed: {:?}", e.kind())))?;

        self.client = Some(client);
        info!("Successfully connected to NATS");
        Ok(())
    }

    async fn disconnect(&mut self) {
        if self.client.is_some() {
            info!("Disconnecting from NATS");
            self.client = None;
        }
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn publish(&self, topic: &str, message: &[u8]) -> Result<()> {
        let client = self.client.as_ref().ok_or(Error::NotConnected)?;

        let subject = Subject::from(topic);
        let payload = Bytes::from(message.to_vec());

        let mut retries: i8 = 0;
        let mut result = client.publish(subject.clone(), payload.clone()).await;

        while result.is_err() {
            retries += 1;
            let error_message = format!("Failed to publish to NATS, retry {} times", retries);

            if retries < 5 {
                warn!("{}", error_message);
            } else {
                error!("{}", error_message);
            }

            result = client.publish(subject.clone(), payload.clone()).await;
        }

        debug!("Published message to NATS topic: {}", topic);
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<MessageStream> {
        let client = self.client.as_ref().ok_or(Error::NotConnected)?;

        let subject = Subject::from(topic);
        let subscriber = client
            .subscribe(subject.clone())
            .await
            .map_err(|e| Error::Subscribe(format!("Failed to subscribe to {}: {}", topic, e)))?;

        debug!("Subscribed to NATS topic: {}", topic);
        Ok(Box::pin(NatsMessageStream::new(subscriber)))
    }

    async fn request(&self, topic: &str, message: &[u8], timeout_ms: u32) -> Result<Vec<u8>> {
        let client = self.client.as_ref().ok_or(Error::NotConnected)?;

        let subject = Subject::from(topic);
        let payload = Bytes::from(message.to_vec());

        let response = timeout(
            Duration::from_millis(timeout_ms as u64),
            client.request(subject, payload),
        )
        .await
        .map_err(|_| Error::Request("Request timeout".to_string()))?
        .map_err(|e| Error::Request(format!("Request failed: {}", e)))?;

        Ok(response.payload.to_vec())
    }

    fn name(&self) -> &'static str {
        "nats"
    }
}

/// Wrapper stream that converts NATS messages to ReceivedMessage
pub struct NatsMessageStream {
    subscriber: Subscriber,
}

impl NatsMessageStream {
    fn new(subscriber: Subscriber) -> Self {
        Self { subscriber }
    }
}

impl Stream for NatsMessageStream {
    type Item = ReceivedMessage;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.subscriber.poll_next_unpin(cx) {
            Poll::Ready(Some(msg)) => {
                let received = ReceivedMessage {
                    topic: msg.subject.to_string(),
                    payload: msg.payload.to_vec(),
                    reply_to: msg.reply.map(|s| s.to_string()),
                };
                Poll::Ready(Some(received))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
