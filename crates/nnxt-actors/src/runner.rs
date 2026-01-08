//! Actor 运行器。

use crate::context::ActorContext;
use crate::event::Event;
use crate::reactor::Reactor;
use crate::Actor;

/// 运行 Actor 的事件循环
pub fn run<A>(mut actor: A)
where
    A: Actor,
{
    let mut reactor = Reactor::new();
    let external_tx = reactor.external_sender();
    let mut ctx = ActorContext::new(
        reactor.rapid_sources_handle(),
        reactor.control_handle(),
        external_tx,
    );
    actor.on_start(&mut ctx);

    loop {
        if let Some(event) = reactor.poll() {
            if matches!(event, Event::Shutdown) {
                actor.on_event(event, &mut ctx);
                actor.on_stop();
                break;
            }
            actor.on_event(event, &mut ctx);
        } else {
            std::hint::spin_loop();
        }
    }
}
