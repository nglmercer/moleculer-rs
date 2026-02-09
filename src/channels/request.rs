use crate::{
    broker::ServiceBroker,
    channels::{messages::incoming::RequestMessage, TransportConn},
    config::{self, Channel, Config},
};

use act_zero::*;
use async_trait::async_trait;
use config::DeserializeError;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Request {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Request Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Request {
    config: Arc<Config>,
    broker: WeakAddr<ServiceBroker>,
    conn: TransportConn,
}

impl Request {
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
        info!("Listening for REQUEST messages");

        let topic = Channel::Request.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to REQUEST channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        let request_context: Result<RequestMessage, DeserializeError> =
                            config.serializer.deserialize(&msg.payload);

                        match call!(pid.handle_nats_message(request_context)).await {
                            Ok(_) => debug!("Successfully handled REQUEST message"),
                            Err(e) => error!("Unable to handle REQUEST message: {}", e),
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
                                let request_context: Result<RequestMessage, DeserializeError> =
                                    config.serializer.deserialize(&msg.payload);

                                match request_context {
                                    Ok(ctx) => {
                                        send!(broker.handle_incoming_request(Ok(ctx)));
                                        debug!("Successfully handled REQUEST message");
                                    }
                                    Err(e) => error!("Unable to handle REQUEST message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to REQUEST channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(
        &self,
        request_context: Result<RequestMessage, DeserializeError>,
    ) -> ActorResult<()> {
        send!(self.broker.handle_incoming_request(request_context));
        Produces::ok(())
    }
}
