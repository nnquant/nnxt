//! Control plane coordinator for nnxt actors.

pub mod protocol;
pub mod registry;
pub mod server;

pub use protocol::{ActorRegistration, ActorSnapshot, HealthStatus, QueueInfo, Request, Response};
pub use registry::{ActorRegistry, RegistryError};
pub use server::MasterServer;
