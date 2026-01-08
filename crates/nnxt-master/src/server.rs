//! Master server handling actor requests.

use crate::protocol::{Request, Response};
use crate::registry::{ActorRegistry, RegistryError};
use nng::options::{Options, RecvTimeout};
use nng::{Message, Protocol, Socket};
use std::time::Duration;
use tracing::{debug, info, warn};
use nnxt_utils::clock::{Clock, InstantClock};
use nnxt_utils::setup_signal;

pub struct MasterServer {
    socket: Socket,
    registry: ActorRegistry,
    clock: Box<dyn Clock + Send + Sync>,
    heartbeat_timeout_ns: u64,
}

impl MasterServer {
    pub fn new(addr: &str, heartbeat_timeout_ns: u64) -> Result<Self, nng::Error> {
        let socket = Socket::new(Protocol::Rep0)?;
        socket.listen(addr)?;
        Ok(Self {
            socket,
            registry: ActorRegistry::new(),
            clock: Box::new(InstantClock::new()),
            heartbeat_timeout_ns,
        })
    }

    pub fn with_clock(
        addr: &str,
        heartbeat_timeout_ns: u64,
        clock: Box<dyn Clock + Send + Sync>,
    ) -> Result<Self, nng::Error> {
        let socket = Socket::new(Protocol::Rep0)?;
        socket.listen(addr)?;
        Ok(Self {
            socket,
            registry: ActorRegistry::new(),
            clock,
            heartbeat_timeout_ns,
        })
    }

    pub fn serve_once(&mut self) -> Result<(), nng::Error> {
        let msg = self.socket.recv()?;
        let response = self.handle_message(&msg);
        let payload = serde_json::to_vec(&response).unwrap_or_else(|err| {
            serde_json::to_vec(&Response::Error {
                message: format!("response serialization failed error=[{}]", err),
            })
            .unwrap_or_default()
        });
        self.socket.send(payload.as_slice())?;
        Ok(())
    }

    pub fn serve(&mut self) -> Result<(), nng::Error> {
        loop {
            self.serve_once()?;
        }
    }

    pub fn run(&mut self) -> Result<(), nng::Error> {
        let shutdown = setup_signal();
        self.socket.set_opt::<RecvTimeout>(Some(Duration::from_millis(200)))?;

        loop {
            if shutdown.is_shutdown() {
                break;
            }
            let msg = match self.socket.recv() {
                Ok(msg) => msg,
                Err(nng::Error::TimedOut) => continue,
                Err(err) => return Err(err),
            };
            let response = self.handle_message(&msg);
            let payload = serde_json::to_vec(&response).unwrap_or_else(|err| {
                serde_json::to_vec(&Response::Error {
                    message: format!("response serialization failed error=[{}]", err),
                })
                .unwrap_or_default()
            });
            self.socket.send(payload.as_slice())?;
        }
        Ok(())
    }

    fn handle_message(&mut self, msg: &Message) -> Response {
        let request = match serde_json::from_slice::<Request>(msg.as_slice()) {
            Ok(request) => request,
            Err(err) => {
                warn!("invalid request event=[INVALID_REQUEST] error=[{}]", err);
                return Response::Error {
                    message: format!("invalid request error=[{}]", err),
                }
            }
        };

        let now_ns = self.clock.now_ns();
        self.registry
            .update_health(now_ns, self.heartbeat_timeout_ns);

        self.handle_request(request, now_ns)
    }

    fn handle_request(&mut self, request: Request, now_ns: u64) -> Response {
        match request {
            Request::Register { actor } => match self.registry.register(actor.clone(), now_ns) {
                Ok(()) => {
                    info!(
                        "registered actor event=[ACTOR_REGISTERED] actor_id=[{}] actor_type=[{}] queues=[{}]",
                        actor.actor_id,
                        actor.actor_type,
                        actor.queues.len()
                    );
                    Response::Ok
                }
                Err(err) => error_response(err),
            },
            Request::Unregister { actor_id } => match self.registry.unregister(&actor_id) {
                Ok(()) => {
                    info!(
                        "unregistered actor event=[ACTOR_UNREGISTERED] actor_id=[{}]",
                        actor_id
                    );
                    Response::Ok
                }
                Err(err) => error_response(err),
            },
            Request::RegisterQueue {
                actor_id,
                queue,
                target_actor,
            } => match self.registry.register_queue(&actor_id, queue.clone()) {
                Ok(()) => {
                    info!(
                        "queue registered event=[QUEUE_REGISTERED] actor_id=[{}] queue_type=[{}] addr=[{}]",
                        actor_id,
                        queue.queue_type,
                        queue.addr
                    );
                    if let Some(target_actor) = target_actor {
                        let command = format!("add_action_queue:{}", queue.addr);
                        let _ = self.forward_command(&target_actor, &command);
                    }
                    Response::Ok
                }
                Err(err) => error_response(err),
            },
            Request::Heartbeat { actor_id } => match self.registry.record_heartbeat(&actor_id, now_ns)
            {
                Ok(()) => {
                    debug!(
                        "heartbeat updated event=[ACTOR_HEARTBEAT] actor_id=[{}]",
                        actor_id
                    );
                    Response::Ok
                }
                Err(err) => error_response(err),
            },
            Request::LookupQueue { queue_addr } => match self.registry.lookup_queue(&queue_addr) {
                Ok((owner, queue_type)) => {
                    info!(
                        "queue lookup event=[QUEUE_LOOKUP] queue_addr=[{}] owner=[{}] queue_type=[{}]",
                        queue_addr, owner, queue_type
                    );
                    Response::QueueInfo { owner, queue_type }
                }
                Err(err) => error_response(err),
            },
            Request::FindQueues { queue_type } => {
                let queues = self.registry.find_queues_by_type(&queue_type);
                info!(
                    "queue find event=[QUEUE_FIND] queue_type=[{}] count=[{}]",
                    queue_type,
                    queues.len()
                );
                Response::QueueList { queues }
            }
            Request::ConnectTrade {
                target_type,
                actor_id: _,
            } => match self.registry.find_actor_by_type(&target_type) {
                Ok(trade_gateway_id) => {
                    let order_event_queue = match self
                        .registry
                        .find_queue_by_owner_type(&trade_gateway_id, "order-event")
                    {
                        Ok(addr) => addr,
                        Err(err) => return error_response(err),
                    };
                    let trade_event_queue = match self
                        .registry
                        .find_queue_by_owner_type(&trade_gateway_id, "trade-event")
                    {
                        Ok(addr) => addr,
                        Err(err) => return error_response(err),
                    };
                    Response::ConnectTrade {
                        trade_gateway_id,
                        order_event_queue,
                        trade_event_queue,
                    }
                }
                Err(err) => error_response(err),
            },
            Request::ListActors => {
                let actors = self.registry.list_actors();
                debug!(
                    "list actors event=[ACTOR_LIST] count=[{}]",
                    actors.len()
                );
                Response::ActorList { actors }
            }
            Request::SendCommand {
                target_actor,
                command,
            } => {
                info!(
                    "send command event=[COMMAND_FORWARD] target_actor=[{}] command_size=[{}]",
                    target_actor,
                    command.len()
                );
                self.forward_command(&target_actor, &command)
            }
        }
    }

    fn forward_command(&self, target_actor: &str, command: &str) -> Response {
        let control_addr = match self.registry.control_queue(target_actor) {
            Ok(addr) => addr,
            Err(err) => return error_response(err),
        };

        let Some(control_addr) = control_addr else {
            return Response::Error {
                message: format!("missing control queue actor_id=[{}]", target_actor),
            };
        };

        let socket = match Socket::new(Protocol::Req0) {
            Ok(socket) => socket,
            Err(err) => {
                return Response::Error {
                    message: format!("command socket create failed error=[{}]", err),
                }
            }
        };

        if let Err(err) = socket.dial(&control_addr) {
            return Response::Error {
                message: format!("command dial failed addr=[{}] error=[{}]", control_addr, err),
            };
        }

        if let Err((_msg, err)) = socket.send(command.as_bytes()) {
            return Response::Error {
                message: format!(
                    "command send failed actor_id=[{}] error=[{}]",
                    target_actor, err
                ),
            };
        }

        Response::Ok
    }
}

fn error_response(error: RegistryError) -> Response {
    warn!("registry error event=[REGISTRY_ERROR] error=[{:?}]", error);
    Response::Error {
        message: format!("registry error error=[{:?}]", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ActorRegistration, QueueInfo, Request};

    struct FixedClock {
        now: u64,
    }

    impl Clock for FixedClock {
        fn now_ns(&self) -> u64 {
            self.now
        }
    }

    #[test]
    fn handle_register_and_list() {
        let mut server = MasterServer {
            socket: Socket::new(Protocol::Rep0).expect("socket"),
            registry: ActorRegistry::new(),
            clock: Box::new(FixedClock { now: 10 }),
            heartbeat_timeout_ns: 100,
        };

        let actor = ActorRegistration {
            actor_id: "actor-1".to_string(),
            actor_type: "sample".to_string(),
            queues: vec![QueueInfo {
                addr: "ipc:///tmp/actor-1".to_string(),
                queue_type: "control".to_string(),
            }],
        };

        let response = server.handle_request(Request::Register { actor }, 10);
        assert_eq!(response, Response::Ok);

        let response = server.handle_request(Request::ListActors, 10);
        match response {
            Response::ActorList { actors } => {
                assert_eq!(actors.len(), 1);
                assert_eq!(actors[0].status, crate::protocol::HealthStatus::Healthy);
            }
            _ => panic!("expected actor list"),
        }
    }
}
