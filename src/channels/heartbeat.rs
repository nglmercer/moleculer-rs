use crate::{
    broker::ServiceBroker,
    channels::{
        messages::{incoming, outgoing},
        TransportConn,
    },
    config::{Channel, Config},
};

use super::ChannelSupervisor;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::{sync::Arc, time::Duration};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

pub(crate) struct Heartbeat {
    pid: Addr<Self>,
    config: Arc<Config>,
    timer: Timer,
    conn: TransportConn,
    parent: Addr<ChannelSupervisor>,
    broker: WeakAddr<ServiceBroker>,
    heartbeat_interval: u32,
    system: sysinfo::System,
}

#[async_trait]
impl Actor for Heartbeat {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));

        // Start the timer
        self.timer.set_timeout_for_strong(
            pid_clone.clone(),
            Duration::from_secs(self.heartbeat_interval as u64),
        );

        self.pid = pid_clone;

        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Heartbeat Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

#[async_trait]
impl Tick for Heartbeat {
    async fn tick(&mut self) -> ActorResult<()> {
        self.system
            .refresh_cpu_specifics(CpuRefreshKind::everything().without_frequency());

        if self.timer.tick() {
            self.timer.set_timeout_for_strong(
                self.pid.clone(),
                Duration::from_secs(self.heartbeat_interval as u64),
            );
            let _ = self.send_heartbeat().await;
        }
        Produces::ok(())
    }
}

impl Heartbeat {
    pub(crate) async fn new(
        parent: WeakAddr<ChannelSupervisor>,
        broker: WeakAddr<ServiceBroker>,
        config: &Arc<Config>,
        conn: &TransportConn,
    ) -> Self {
        Self {
            pid: Addr::detached(),
            config: Arc::clone(config),
            parent: parent.upgrade(),
            broker,
            conn: conn.clone(),
            heartbeat_interval: config.heartbeat_interval,
            timer: Timer::default(),
            system: System::new_with_specifics(
                RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
            ),
        }
    }

    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for HEARTBEAT messages");

        let topic = Channel::Heartbeat.channel_to_string(&self.config);
        let config = self.config.clone();
        let broker = self.broker.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to HEARTBEAT channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled HEARTBEAT message"),
                            Err(e) => error!("Unable to handle HEARTBEAT message: {}", e),
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
                                let heartbeat: Result<incoming::HeartbeatMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match heartbeat {
                                    Ok(hb) => {
                                        send!(broker.handle_heartbeat_message(hb));
                                        debug!("Successfully handled HEARTBEAT message");
                                    }
                                    Err(e) => error!("Unable to handle HEARTBEAT message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to HEARTBEAT channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn handle_nats_message(&self, msg: async_nats::Message) -> ActorResult<()> {
        let heartbeat: incoming::HeartbeatMessage =
            self.config.serializer.deserialize(&msg.payload)?;

        send!(self.broker.handle_heartbeat_message(heartbeat));

        Produces::ok(())
    }

    async fn send_heartbeat(&self) -> ActorResult<()> {
        let msg =
            outgoing::HeartbeatMessage::new(&self.config.node_id, self.system.global_cpu_usage());

        send!(self
            .parent
            .publish(Channel::Heartbeat, self.config.serializer.serialize(msg)?));

        Produces::ok(())
    }
}
