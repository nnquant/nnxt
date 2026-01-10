//! Message protocol between actors and master.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueInfo {
    pub addr: String,
    pub queue_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorRegistration {
    pub actor_id: String,
    pub actor_type: String,
    pub queues: Vec<QueueInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorSnapshot {
    pub actor_id: String,
    pub actor_type: String,
    pub queues: Vec<QueueInfo>,
    pub status: HealthStatus,
    pub last_heartbeat_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Request {
    Register { actor: ActorRegistration },
    RegisterQueue { actor_id: String, queue: QueueInfo, target_actor: Option<String> },
    Unregister { actor_id: String },
    Heartbeat { actor_id: String },
    LookupQueue { queue_addr: String },
    FindQueues { queue_type: String },
    FindQueuesByActor { actor_type: String, queue_type: String },
    ConnectTrade { target_type: String, actor_id: String },
    ListActors,
    SendCommand { target_actor: String, command: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Response {
    Ok,
    Error { message: String },
    ActorList { actors: Vec<ActorSnapshot> },
    QueueInfo { owner: String, queue_type: String },
    QueueList { queues: Vec<QueueInfo> },
    ConnectTrade {
        trade_gateway_id: String,
        order_event_queue: String,
        trade_event_queue: String,
    },
}
