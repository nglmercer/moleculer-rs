use crate::{
    broker::ServiceBroker,
    channels::{messages::incoming::EventMessage, TransportConn},
    config::{self, Channel, Config},
};

use act_zero::*;
use async_trait::async_trait;
use config::DeserializeError;
use futures::StreamExt;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Event {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Event Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Event {
    config: Arc<Config>,
    broker: WeakAddr<ServiceBroker>,
    conn: TransportConn,
}

impl Event {
    pub(crate) async fn new(
        broker: WeakAddr<ServiceBroker>,
        config: &Arc<Config>,
        conn: &TransportConn,
    ) -> Self {
        Self {
            broker,
            conn: conn.clone(),
            config: Arc::clone(config),
        }
    }

    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for EVENT messages");

        let topic = Channel::Event.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        // For NATS, use the existing subscription mechanism
        // For HTTP, we use a broadcast channel approach
        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to EVENT channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        let event_context: Result<EventMessage, DeserializeError> =
                            config.serializer.deserialize(&msg.payload);

                        match call!(pid.handle_nats_message(event_context)).await {
                            Ok(_) => debug!("Successfully handled EVENT message"),
                            Err(e) => error!("Unable to handle EVENT message: {}", e),
                        }
                    }
                })
            }
            TransportConn::Http(transport) => {
                let transport = transport.clone();

                pid.clone().send_fut(async move {
                    let transport_guard = transport.lock().await;
                    match transport_guard.subscribe(&topic).await {
                        Ok(mut stream) => {
                            drop(transport_guard);
                            while let Some(msg) = stream.next().await {
                                let event_context: Result<EventMessage, DeserializeError> =
                                    config.serializer.deserialize(&msg.payload);

                                match event_context {
                                    Ok(ctx) => {
                                        send!(broker.handle_incoming_event(Ok(ctx)));
                                        debug!("Successfully handled EVENT message");
                                    }
                                    Err(e) => error!("Unable to handle EVENT message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to EVENT channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(
        &self,
        event_context: Result<EventMessage, DeserializeError>,
    ) -> ActorResult<()> {
        send!(self.broker.handle_incoming_event(event_context));
        Produces::ok(())
    }
}
