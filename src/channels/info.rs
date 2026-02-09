use crate::{
    broker::ServiceBroker,
    channels::{messages::incoming::InfoMessage, TransportConn},
    config::{Channel, Config},
};

use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Info {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Info Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Info {
    config: Arc<Config>,
    broker: WeakAddr<ServiceBroker>,
    conn: TransportConn,
}

impl Info {
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

    // INFO packets received when a new client connects and broadcasts it's INFO
    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for INFO messages");

        let topic = Channel::Info.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to INFO channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled INFO message"),
                            Err(e) => error!("Unable to handle INFO message: {}", e),
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
                                let info_message: Result<InfoMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match info_message {
                                    Ok(info) => {
                                        send!(broker.handle_info_message(info));
                                        debug!("Successfully handled INFO message");
                                    }
                                    Err(e) => error!("Unable to handle INFO message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to INFO channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let info_message: InfoMessage = self.config.serializer.deserialize(&msg.payload)?;
        send!(self.broker.handle_info_message(info_message));

        Produces::ok(())
    }
}

#[async_trait]
impl Actor for InfoTargeted {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("InfoTargeted Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct InfoTargeted {
    config: Arc<Config>,
    broker: WeakAddr<ServiceBroker>,
    conn: TransportConn,
}

impl InfoTargeted {
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

    // INFO packets received are responses to DISCOVER packet sent by current client
    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for INFO (targeted) messages");

        let topic = Channel::InfoTargeted.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to INFO targeted channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => {
                                debug!("Successfully handled INFO message in response to DISCOVER")
                            }
                            Err(e) => error!(
                                "Unable to handle INFO message in response to DISCOVER: {}",
                                e
                            ),
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
                                let info_message: Result<InfoMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match info_message {
                                    Ok(info) => {
                                        send!(broker.handle_info_message(info));
                                        debug!("Successfully handled INFO message in response to DISCOVER");
                                    }
                                    Err(e) => error!(
                                        "Unable to handle INFO message in response to DISCOVER: {}",
                                        e
                                    ),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to INFO targeted channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let info_message: InfoMessage = self.config.serializer.deserialize(&msg.payload)?;
        send!(self.broker.handle_info_message(info_message));

        Produces::ok(())
    }
}
