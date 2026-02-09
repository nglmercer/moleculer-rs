use crate::{
    broker::ServiceBroker,
    channels::{messages::incoming, messages::outgoing, TransportConn},
    config::{Channel, Config},
};

use super::ChannelSupervisor;
use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Discover {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Discover Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Discover {
    broker: WeakAddr<ServiceBroker>,
    config: Arc<Config>,
    parent: WeakAddr<ChannelSupervisor>,
    conn: TransportConn,
}

impl Discover {
    pub(crate) async fn new(
        broker: WeakAddr<ServiceBroker>,
        parent: WeakAddr<ChannelSupervisor>,
        config: &Arc<Config>,
        conn: &TransportConn,
    ) -> Self {
        Self {
            broker,
            parent,
            conn: conn.clone(),
            config: Arc::clone(config),
        }
    }

    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for DISCOVER messages");

        let topic = Channel::Discover.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to DISCOVER channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled DISCOVER message"),
                            Err(e) => error!("Unable to handle DISCOVER message: {}", e),
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
                                let discover: Result<incoming::DiscoverMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match discover {
                                    Ok(disc) => {
                                        let channel = format!(
                                            "{}.{}",
                                            Channel::Info.channel_to_string(&config),
                                            disc.sender
                                        );
                                        send!(broker.publish_info_to_channel(channel));
                                        debug!("Successfully handled DISCOVER message");
                                    }
                                    Err(e) => error!("Unable to handle DISCOVER message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to DISCOVER channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    pub(crate) async fn broadcast(&self) {
        let msg = outgoing::DiscoverMessage::new(&self.config.node_id);
        send!(self.parent.publish(
            Channel::Discover,
            self.config
                .serializer
                .serialize(msg)
                .expect("should always serialize discover msg")
        ));
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let discover: incoming::DiscoverMessage =
            self.config.serializer.deserialize(&msg.payload)?;

        let channel = format!(
            "{}.{}",
            Channel::Info.channel_to_string(&self.config),
            discover.sender
        );

        send!(self.broker.publish_info_to_channel(channel));

        Produces::ok(())
    }
}

#[async_trait]
impl Actor for DiscoverTargeted {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("DiscoverTargeted Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

// This one shouldn't be used to much, DISCOVER packets are usually sent to the DISCOVER broadcast channel
pub(crate) struct DiscoverTargeted {
    broker: WeakAddr<ServiceBroker>,
    config: Arc<Config>,
    conn: TransportConn,
}

impl DiscoverTargeted {
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
        info!("Listening for DISCOVER (targeted) messages");

        let topic = Channel::DiscoverTargeted.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to DISCOVER targeted channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled DISCOVER (targeted)"),
                            Err(e) => error!("Unable to handle DISCOVER (targeted): {}", e),
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
                                let discover: Result<incoming::DiscoverMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match discover {
                                    Ok(disc) => {
                                        let channel = format!(
                                            "{}.{}",
                                            Channel::Info.channel_to_string(&config),
                                            disc.sender
                                        );
                                        send!(broker.publish_info_to_channel(channel));
                                        debug!("Successfully handled DISCOVER (targeted)");
                                    }
                                    Err(e) => error!("Unable to handle DISCOVER (targeted): {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to DISCOVER targeted channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let discover: incoming::DiscoverMessage =
            self.config.serializer.deserialize(&msg.payload)?;
        let channel = format!(
            "{}.{}",
            Channel::Info.channel_to_string(&self.config),
            discover.sender
        );

        send!(self.broker.publish_info_to_channel(channel));

        Produces::ok(())
    }
}
