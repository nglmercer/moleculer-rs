use crate::{
    broker::ServiceBroker,
    channels::{messages::incoming::DisconnectMessage, TransportConn},
    config::{Channel, Config},
};

use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Disconnect {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Disconnect Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Disconnect {
    broker: WeakAddr<ServiceBroker>,
    config: Arc<Config>,
    conn: TransportConn,
}

impl Disconnect {
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
        info!("Listening for DISCONNECT messages");

        let topic = Channel::Disconnect.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to DISCONNECT channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled DISCONNECT message"),
                            Err(e) => error!("Unable to handle DISCONNECT message: {}", e),
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
                                let disconnect_msg: Result<DisconnectMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match disconnect_msg {
                                    Ok(disc) => {
                                        send!(broker.handle_disconnect_message(disc));
                                        debug!("Successfully handled DISCONNECT message");
                                    }
                                    Err(e) => error!("Unable to handle DISCONNECT message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to DISCONNECT channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let disconnect_msg: DisconnectMessage = self.config.serializer.deserialize(&msg.payload)?;

        send!(self.broker.handle_disconnect_message(disconnect_msg));

        Produces::ok(())
    }
}
