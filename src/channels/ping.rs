use crate::{
    channels::{
        messages::{incoming::PingMessage, outgoing::PongMessage},
        ChannelSupervisor, TransportConn,
    },
    config::{Channel, Config},
};

use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Ping {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Ping Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Ping {
    config: Arc<Config>,
    conn: TransportConn,
    parent: WeakAddr<ChannelSupervisor>,
}

impl Ping {
    pub(crate) async fn new(
        parent: WeakAddr<ChannelSupervisor>,
        config: &Arc<Config>,
        conn: &TransportConn,
    ) -> Self {
        Self {
            parent,
            conn: conn.clone(),
            config: Arc::clone(config),
        }
    }

    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for PING messages");

        let topic = Channel::Ping.channel_to_string(&self.config);
        let config = self.config.clone();
        let parent = self.parent.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to PING channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled PING message"),
                            Err(e) => error!("Unable to handle PING message: {}", e),
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
                                let ping_message: Result<PingMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match ping_message {
                                    Ok(ping) => {
                                        let channel = format!(
                                            "{}.{}",
                                            Channel::PongPrefix.channel_to_string(&config),
                                            &ping.sender
                                        );
                                        let pong_message: PongMessage =
                                            (ping, config.node_id.as_str()).into();

                                        send!(parent.publish_to_channel(
                                            channel,
                                            config.serializer.serialize(pong_message).unwrap()
                                        ));
                                        debug!("Successfully handled PING message");
                                    }
                                    Err(e) => error!("Unable to handle PING message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to PING channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let ping_message: PingMessage = self.config.serializer.deserialize(&msg.payload)?;
        let channel = format!(
            "{}.{}",
            Channel::PongPrefix.channel_to_string(&self.config),
            &ping_message.sender
        );

        let pong_message: PongMessage = (ping_message, self.config.node_id.as_str()).into();

        send!(self
            .parent
            .publish_to_channel(channel, self.config.serializer.serialize(pong_message)?));

        Produces::ok(())
    }
}

#[async_trait]
impl Actor for PingTargeted {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("PingTargeted Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct PingTargeted {
    config: Arc<Config>,
    conn: TransportConn,
    parent: WeakAddr<ChannelSupervisor>,
}

impl PingTargeted {
    pub(crate) async fn new(
        parent: WeakAddr<ChannelSupervisor>,
        config: &Arc<Config>,
        conn: &TransportConn,
    ) -> Self {
        Self {
            parent,
            conn: conn.clone(),
            config: Arc::clone(config),
        }
    }

    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for PING (targeted) messages");

        let topic = Channel::PingTargeted.channel_to_string(&self.config);
        let config = self.config.clone();
        let parent = self.parent.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to PING targeted channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled PING message"),
                            Err(e) => error!("Unable to handle PING message: {}", e),
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
                                let ping_message: Result<PingMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match ping_message {
                                    Ok(ping) => {
                                        let channel = format!(
                                            "{}.{}",
                                            Channel::PongPrefix.channel_to_string(&config),
                                            &ping.sender
                                        );
                                        let pong_message: PongMessage =
                                            (ping, config.node_id.as_str()).into();

                                        send!(parent.publish_to_channel(
                                            channel,
                                            config.serializer.serialize(pong_message).unwrap()
                                        ));
                                        debug!("Successfully handled PING message");
                                    }
                                    Err(e) => error!("Unable to handle PING message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to PING targeted channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let ping_message: PingMessage = self.config.serializer.deserialize(&msg.payload)?;
        let channel = format!(
            "{}.{}",
            Channel::PongPrefix.channel_to_string(&self.config),
            &ping_message.sender
        );

        let pong_message: PongMessage = (ping_message, self.config.node_id.as_str()).into();

        send!(self
            .parent
            .publish_to_channel(channel, self.config.serializer.serialize(pong_message)?));

        Produces::ok(())
    }
}
