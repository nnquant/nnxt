//! Master client for queue discovery.

use std::time::Duration;

use nnxt_master::protocol::{ActorRegistration, QueueInfo, Request, Response};
use nng::options::{Options, RecvTimeout, SendTimeout};
use nng::{Protocol, Socket};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum MasterClientError {
    Transport(nng::Error),
    Protocol(String),
    UnexpectedResponse,
    QueueNotFound { queue_type: String },
    Timeout,
}

pub struct MasterClient {
    socket: Socket,
}

impl MasterClient {
    pub fn new(addr: &str) -> Result<Self, nng::Error> {
        let socket = Socket::new(Protocol::Req0)?;
        socket.set_opt::<RecvTimeout>(Some(DEFAULT_TIMEOUT))?;
        socket.set_opt::<SendTimeout>(Some(DEFAULT_TIMEOUT))?;
        socket.dial(addr)?;
        Ok(Self { socket })
    }

    pub fn lookup_queue(&mut self, queue_addr: &str) -> Result<(String, String), MasterClientError> {
        let request = Request::LookupQueue {
            queue_addr: queue_addr.to_string(),
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::QueueInfo { owner, queue_type } => Ok((owner, queue_type)),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }

    pub fn register_actor(
        &mut self,
        actor_id: &str,
        actor_type: &str,
        queues: Vec<QueueInfo>,
    ) -> Result<(), MasterClientError> {
        let request = Request::Register {
            actor: ActorRegistration {
                actor_id: actor_id.to_string(),
                actor_type: actor_type.to_string(),
                queues,
            },
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::Ok => Ok(()),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }

    pub fn find_queues(
        &mut self,
        queue_type: &str,
    ) -> Result<Vec<nnxt_master::protocol::QueueInfo>, MasterClientError> {
        let request = Request::FindQueues {
            queue_type: queue_type.to_string(),
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::QueueList { queues } => Ok(queues),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }

    pub fn find_queues_by_actor(
        &mut self,
        actor_type: &str,
        queue_type: &str,
    ) -> Result<Vec<nnxt_master::protocol::QueueInfo>, MasterClientError> {
        let request = Request::FindQueuesByActor {
            actor_type: actor_type.to_string(),
            queue_type: queue_type.to_string(),
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::QueueList { queues } => Ok(queues),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }

    pub fn connect_trade(
        &mut self,
        target_type: &str,
        actor_id: &str,
    ) -> Result<(String, String, String), MasterClientError> {
        let request = Request::ConnectTrade {
            target_type: target_type.to_string(),
            actor_id: actor_id.to_string(),
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::ConnectTrade {
                trade_gateway_id,
                order_event_queue,
                trade_event_queue,
            } => Ok((trade_gateway_id, order_event_queue, trade_event_queue)),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }

    pub fn register_queue(
        &mut self,
        actor_id: &str,
        queue: nnxt_master::protocol::QueueInfo,
        target_actor: Option<String>,
    ) -> Result<(), MasterClientError> {
        let request = Request::RegisterQueue {
            actor_id: actor_id.to_string(),
            queue,
            target_actor,
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::Ok => Ok(()),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }

    pub fn heartbeat(&mut self, actor_id: &str) -> Result<(), MasterClientError> {
        let request = Request::Heartbeat {
            actor_id: actor_id.to_string(),
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| MasterClientError::Transport(err))?;

        let msg = self
            .socket
            .recv()
            .map_err(MasterClientError::Transport)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| MasterClientError::Protocol(err.to_string()))?;

        match response {
            Response::Ok => Ok(()),
            Response::Error { message } => Err(MasterClientError::Protocol(message)),
            _ => Err(MasterClientError::UnexpectedResponse),
        }
    }
}
