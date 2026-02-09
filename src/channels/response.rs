use crate::{
    channels::{messages::incoming::ResponseMessage, TransportConn},
    config::{Channel, Config},
};

use act_zero::runtimes::tokio::{spawn_actor, Timer};
use act_zero::timer::Tick;
use act_zero::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use log::{debug, error, info};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::oneshot::Sender;

type RequestId = String;

#[async_trait]
impl Actor for Response {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        let pid_clone = pid.clone();
        send!(pid_clone.listen(pid));
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("Response Actor Error: {:?}", error);

        // do not stop on actor error
        false
    }
}

pub(crate) struct Response {
    config: Arc<Config>,
    waiters: HashMap<RequestId, Addr<ResponseWaiter>>,
    conn: TransportConn,
}

impl Response {
    pub(crate) async fn new(config: &Arc<Config>, conn: &TransportConn) -> Self {
        Self {
            conn: conn.clone(),
            config: Arc::clone(config),
            waiters: HashMap::new(),
        }
    }

    pub(crate) async fn start_response_waiter(
        &mut self,
        timeout: i32,
        node_name: String,
        request_id: RequestId,
        tx: Sender<Value>,
    ) {
        let response_waiter_pid = spawn_actor(ResponseWaiter::new(
            timeout,
            request_id.clone(),
            node_name,
            tx,
        ));

        self.waiters.insert(request_id, response_waiter_pid);
    }

    pub(crate) async fn listen(&mut self, pid: Addr<Self>) {
        info!("Listening for RESPONSE messages");

        let topic = Channel::Response.channel_to_string(&self.config);
        let config = self.config.clone();

        match &self.conn {
            TransportConn::Nats(nats_conn) => {
                let mut channel = nats_conn
                    .subscribe(&topic)
                    .await
                    .expect("Should subscribe to RESPONSE channel");

                pid.clone().send_fut(async move {
                    while let Some(msg) = channel.next().await {
                        match call!(pid.handle_nats_message(msg)).await {
                            Ok(_) => debug!("Successfully handled RESPONSE message"),
                            Err(e) => error!("Unable to handle RESPONSE message: {}", e),
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
                                let response: Result<ResponseMessage, _> =
                                    config.serializer.deserialize(&msg.payload);

                                match response {
                                    Ok(resp) => {
                                        let response_id = resp.id.clone();
                                        // We need to send this to the waiter
                                        // For now, just log it
                                        debug!(
                                            "Successfully handled RESPONSE message with id: {}",
                                            response_id
                                        );
                                    }
                                    Err(e) => error!("Unable to handle RESPONSE message: {}", e),
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to subscribe to RESPONSE channel: {}", e);
                        }
                    }
                })
            }
        }
    }

    async fn timeout_reached(&mut self, request_id: String) {
        self.waiters.remove(&request_id);
    }

    async fn handle_nats_message(&mut self, msg: async_nats::Message) -> ActorResult<()> {
        let response: ResponseMessage = self.config.serializer.deserialize(&msg.payload)?;
        let response_id = response.id.clone();

        if let Some(response_waiter) = self.waiters.get(&response_id) {
            let response_waiter = response_waiter.clone();

            // wether send_response succeeds or fails we should remove it from hashmap
            let _ = call!(response_waiter.send_response(response)).await;
            self.waiters.remove(&response_id);
        }

        Produces::ok(())
    }
}

#[async_trait]
impl Actor for ResponseWaiter {
    async fn started(&mut self, pid: Addr<Self>) -> ActorResult<()> {
        self.pid = pid.downgrade();

        // Start the timer
        self.timer
            .set_timeout_for_weak(pid.downgrade(), Duration::from_millis(self.timeout as u64));

        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        error!("ResponseWaiter Actor Error: {:?}", error);

        // stop actor on actor error
        true
    }
}

#[async_trait]
impl Tick for ResponseWaiter {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.parent.timeout_reached(self.request_id.clone()))
        }
        Produces::ok(())
    }
}

struct ResponseWaiter {
    parent: WeakAddr<Response>,
    pid: WeakAddr<Self>,
    request_id: RequestId,

    timeout: i32,
    node_name: String,
    tx: Option<Sender<Value>>,

    timer: Timer,
}

impl ResponseWaiter {
    fn new(timeout: i32, request_id: RequestId, node_name: String, tx: Sender<Value>) -> Self {
        Self {
            parent: WeakAddr::detached(),
            pid: WeakAddr::detached(),

            request_id,
            timeout,
            node_name,
            tx: Some(tx),

            timer: Timer::default(),
        }
    }

    async fn send_response(&mut self, response: ResponseMessage) -> ActorResult<()> {
        if self.node_name != response.sender {
            // something went wrong here, should handle this error better
            error!("Node name does not match sender")
        }

        // take the tx from actor state and replace it with a none
        let tx = std::mem::take(&mut self.tx).unwrap();

        tx.send(response.data).unwrap();
        Produces::ok(())
    }
}
