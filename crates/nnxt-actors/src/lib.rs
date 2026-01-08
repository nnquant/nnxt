//! actors: Actor 抽象层，封装 rapid 队列通信能力。

mod context;
mod error;
mod event;
mod reactor;
mod runner;

pub use context::ActorContext;
pub use error::Error;
pub use event::Event;
pub use reactor::{ControlHandle, EventSource, Reactor};
pub use runner::run;

/// Actor 生命周期 trait
pub trait Actor: Send + 'static {
    fn on_start(&mut self, _ctx: &mut ActorContext) {}
    fn on_event(&mut self, _event: Event, _ctx: &mut ActorContext) {}
    fn on_stop(&mut self) {}
}

/// 消息处理 trait
pub trait Handler<M> {
    fn handle(&mut self, msg: &M, ctx: &mut ActorContext);
}
