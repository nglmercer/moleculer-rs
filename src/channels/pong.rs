use crate::{
    channels::{ChannelSupervisor, TransportConn},
    config::{Channel, Config},
};

use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use std::sync::Arc;

#[async_trait]
impl Actor for Pong {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Pong Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

#[allow(dead_code)]
pub(crate) struct Pong {
    config: Arc<Config>,
    conn: TransportConn,
    parent: WeakAddr<ChannelSupervisor>,
}

impl Pong {
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
        info!("Listening for PONG messages");

        let topic = Channel::Pong.channel_to_string(&self.config);

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to PONG channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        // do nothing with incoming pong messages for now
                        let _ = msg;
                        debug!("Successfully handled PONG message");
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
                            while let Some(_msg) = stream.next().await {
                                // do nothing with incoming pong messages for now
                                debug!("Successfully handled PONG message");
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to PONG channel: {}", e);
                        }
                    }
                })
            }
        }
    }
}
