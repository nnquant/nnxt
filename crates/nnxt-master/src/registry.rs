//! Actor registry and service discovery.

use std::collections::HashMap;

use crate::protocol::{ActorRegistration, ActorSnapshot, HealthStatus, QueueInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    ActorNotFound { actor_id: String },
    QueueNotFound { queue_addr: String },
    QueueAlreadyRegistered { queue_addr: String, owner: String },
    ActorTypeNotFound { actor_type: String },
    QueueTypeNotFound { actor_id: String, queue_type: String },
}

#[derive(Debug, Clone)]
pub struct ActorRegistry {
    actors: HashMap<String, ActorRecord>,
    queues: HashMap<String, QueueOwner>,
}

#[derive(Debug, Clone)]
struct ActorRecord {
    registration: ActorRegistration,
    last_heartbeat_ns: u64,
    status: HealthStatus,
}

#[derive(Debug, Clone)]
struct QueueOwner {
    owner: String,
    queue_type: String,
}

impl ActorRegistry {
    pub fn new() -> Self {
        Self {
            actors: HashMap::new(),
            queues: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        actor: ActorRegistration,
        now_ns: u64,
    ) -> Result<(), RegistryError> {
        let actor_id = actor.actor_id.clone();
        if let Some(existing) = self.actors.get(&actor_id) {
            for queue in &existing.registration.queues {
                self.queues.remove(&queue.addr);
            }
        }

        for queue in &actor.queues {
            if let Some(owner) = self.queues.get(&queue.addr) {
                return Err(RegistryError::QueueAlreadyRegistered {
                    queue_addr: queue.addr.clone(),
                    owner: owner.owner.clone(),
                });
            }
        }

        for queue in &actor.queues {
            self.queues.insert(
                queue.addr.clone(),
                QueueOwner {
                    owner: actor_id.clone(),
                    queue_type: queue.queue_type.clone(),
                },
            );
        }

        self.actors.insert(
            actor_id,
            ActorRecord {
                registration: actor,
                last_heartbeat_ns: now_ns,
                status: HealthStatus::Healthy,
            },
        );

        Ok(())
    }

    pub fn register_queue(
        &mut self,
        actor_id: &str,
        queue: QueueInfo,
    ) -> Result<(), RegistryError> {
        let record = self
            .actors
            .get_mut(actor_id)
            .ok_or_else(|| RegistryError::ActorNotFound {
                actor_id: actor_id.to_string(),
            })?;

        if let Some(owner) = self.queues.get(&queue.addr) {
            return Err(RegistryError::QueueAlreadyRegistered {
                queue_addr: queue.addr.clone(),
                owner: owner.owner.clone(),
            });
        }

        self.queues.insert(
            queue.addr.clone(),
            QueueOwner {
                owner: actor_id.to_string(),
                queue_type: queue.queue_type.clone(),
            },
        );
        record.registration.queues.push(queue);
        Ok(())
    }

    pub fn unregister(&mut self, actor_id: &str) -> Result<(), RegistryError> {
        let record = self
            .actors
            .remove(actor_id)
            .ok_or_else(|| RegistryError::ActorNotFound {
                actor_id: actor_id.to_string(),
            })?;

        for queue in record.registration.queues {
            self.queues.remove(&queue.addr);
        }

        Ok(())
    }

    pub fn lookup_queue(&self, queue_addr: &str) -> Result<(String, String), RegistryError> {
        self.queues
            .get(queue_addr)
            .map(|owner| (owner.owner.clone(), owner.queue_type.clone()))
            .ok_or_else(|| RegistryError::QueueNotFound {
                queue_addr: queue_addr.to_string(),
            })
    }

    pub fn find_queues_by_type(&self, queue_type: &str) -> Vec<QueueInfo> {
        self.queues
            .iter()
            .filter(|(_, owner)| owner.queue_type == queue_type)
            .map(|(addr, owner)| QueueInfo {
                addr: addr.clone(),
                queue_type: owner.queue_type.clone(),
            })
            .collect()
    }

    pub fn list_actors(&self) -> Vec<ActorSnapshot> {
        self.actors
            .values()
            .map(|record| ActorSnapshot {
                actor_id: record.registration.actor_id.clone(),
                actor_type: record.registration.actor_type.clone(),
                queues: record.registration.queues.clone(),
                status: record.status.clone(),
                last_heartbeat_ns: record.last_heartbeat_ns,
            })
            .collect()
    }

    pub fn find_actor_by_type(&self, actor_type: &str) -> Result<String, RegistryError> {
        self.actors
            .values()
            .find(|record| record.registration.actor_type == actor_type)
            .map(|record| record.registration.actor_id.clone())
            .ok_or_else(|| RegistryError::ActorTypeNotFound {
                actor_type: actor_type.to_string(),
            })
    }

    pub fn find_queue_by_owner_type(
        &self,
        actor_id: &str,
        queue_type: &str,
    ) -> Result<String, RegistryError> {
        self.queues
            .iter()
            .find(|(_, owner)| owner.owner == actor_id && owner.queue_type == queue_type)
            .map(|(addr, _)| addr.clone())
            .ok_or_else(|| RegistryError::QueueTypeNotFound {
                actor_id: actor_id.to_string(),
                queue_type: queue_type.to_string(),
            })
    }

    pub fn record_heartbeat(&mut self, actor_id: &str, now_ns: u64) -> Result<(), RegistryError> {
        let record = self
            .actors
            .get_mut(actor_id)
            .ok_or_else(|| RegistryError::ActorNotFound {
                actor_id: actor_id.to_string(),
            })?;
        record.last_heartbeat_ns = now_ns;
        record.status = HealthStatus::Healthy;
        Ok(())
    }

    pub fn update_health(&mut self, now_ns: u64, timeout_ns: u64) {
        for record in self.actors.values_mut() {
            if now_ns.saturating_sub(record.last_heartbeat_ns) > timeout_ns {
                record.status = HealthStatus::Unhealthy;
            }
        }
    }

    pub fn control_queue(&self, actor_id: &str) -> Result<Option<String>, RegistryError> {
        let record = self
            .actors
            .get(actor_id)
            .ok_or_else(|| RegistryError::ActorNotFound {
                actor_id: actor_id.to_string(),
            })?;

        let control_addr = record
            .registration
            .queues
            .iter()
            .find(|queue| queue.queue_type == "control")
            .map(|queue| queue.addr.clone());

        Ok(control_addr)
    }
}

impl Default for ActorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::QueueInfo;

    fn sample_actor(actor_id: &str) -> ActorRegistration {
        ActorRegistration {
            actor_id: actor_id.to_string(),
            actor_type: "sample".to_string(),
            queues: vec![QueueInfo {
                addr: format!("ipc:///tmp/{}", actor_id),
                queue_type: "control".to_string(),
            }],
        }
    }

    #[test]
    fn register_and_lookup_queue() {
        let mut registry = ActorRegistry::new();
        registry.register(sample_actor("actor-1"), 1).expect("register");

        let (owner, queue_type) = registry
            .lookup_queue("ipc:///tmp/actor-1")
            .expect("queue");
        assert_eq!(owner, "actor-1");
        assert_eq!(queue_type, "control");
    }

    #[test]
    fn unregister_removes_queues() {
        let mut registry = ActorRegistry::new();
        registry.register(sample_actor("actor-1"), 1).expect("register");
        registry.unregister("actor-1").expect("unregister");

        let err = registry
            .lookup_queue("ipc:///tmp/actor-1")
            .expect_err("queue removed");
        assert_eq!(
            err,
            RegistryError::QueueNotFound {
                queue_addr: "ipc:///tmp/actor-1".to_string()
            }
        );
    }

    #[test]
    fn update_health_marks_unhealthy() {
        let mut registry = ActorRegistry::new();
        registry.register(sample_actor("actor-1"), 1).expect("register");
        registry.update_health(200, 50);

        let actor = registry.list_actors().pop().expect("actor");
        assert_eq!(actor.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn find_queues_by_type_returns_all() {
        let mut registry = ActorRegistry::new();
        registry.register(sample_actor("actor-1"), 1).expect("register");
        registry
            .register(ActorRegistration {
                actor_id: "actor-2".to_string(),
                actor_type: "sample".to_string(),
                queues: vec![QueueInfo {
                    addr: "ipc:///tmp/actor-2".to_string(),
                    queue_type: "market".to_string(),
                }],
            }, 1)
            .expect("register");

        let queues = registry.find_queues_by_type("control");
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].addr, "ipc:///tmp/actor-1");

        let queues = registry.find_queues_by_type("market");
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].addr, "ipc:///tmp/actor-2");
    }

    #[test]
    fn register_queue_appends() {
        let mut registry = ActorRegistry::new();
        registry.register(sample_actor("actor-1"), 1).expect("register");
        let queue = QueueInfo {
            addr: "ipc:///tmp/extra".to_string(),
            queue_type: "extra".to_string(),
        };
        registry.register_queue("actor-1", queue).expect("queue");
        let queues = registry.find_queues_by_type("extra");
        assert_eq!(queues.len(), 1);
    }
}
