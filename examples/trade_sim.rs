use nnxt_actors::{run, Actor, ActorContext, Event};
use nnxt_gateway::{TradeGateway, TradeGatewayEvent, TradeSimulator};
use nnxt_master::protocol::{ActorRegistration, QueueInfo, Request, Response};
use nng::{Protocol, Socket};
use nnxt_rapid::{cleanup, Address};
use serde_json::to_vec;
use nnxt_specs::{OrderEvent, TradeEvent};
use std::collections::HashSet;
use tracing::info;
use nnxt_utils::{broadcast_queue, setup_log};

const MASTER_ADDR: &str = "ipc:///tmp/nnxt/master";
const CONTROL_ADDR: &str = "ipc:///tmp/nnxt/trade-sim-control";

struct TradeSimActor {
    order_event_addr: String,
    trade_event_addr: String,
    action_sources: HashSet<usize>,
    simulator: TradeSimulator,
    seq: u64,
}

impl Actor for TradeSimActor {
    fn on_start(&mut self, ctx: &mut ActorContext) {
        let order_addr = Address::new(&self.order_event_addr).expect("order addr");
        let trade_addr = Address::new(&self.trade_event_addr).expect("trade addr");
        let _ = cleanup(&order_addr);
        let _ = cleanup(&trade_addr);

        ctx.write_to::<OrderEvent>(&self.order_event_addr, 1024)
            .expect("order queue create failed");
        ctx.write_to::<TradeEvent>(&self.trade_event_addr, 1024)
            .expect("trade queue create failed");

        let control = Socket::new(Protocol::Rep0).expect("control socket");
        control.listen(CONTROL_ADDR).expect("control listen");
        ctx.set_control_socket(control)
            .expect("control socket attach failed");
    }

    fn on_event(&mut self, event: Event, ctx: &mut ActorContext) {
        match event {
            Event::Control { message } => {
                let command = String::from_utf8_lossy(&message);
                if let Some(addr) = command.strip_prefix("add_action_queue:") {
                    let source_id = connect_action_reader(ctx, addr);
                    self.action_sources.insert(source_id);
                    info!(
                        "trade action event=[ACTION_QUEUE_ADDED] addr=[{}] source_id=[{}]",
                        addr, source_id
                    );
                }
                ctx.reply_control(b"ok")
                    .expect("control reply failed");
            }
            Event::Data { source_id, ptr } => {
                if !self.action_sources.contains(&source_id) {
                    return;
                }
                // SAFETY: 该 source_id 只注册 Action 类型
                let action = unsafe { *(ptr as *const nnxt_strategy::Action) };
                self.seq = self.seq.saturating_add(1);
                if self.seq % 10 == 0 {
                    info!(
                        "trade action event=[ACTION_RECEIVED] seq=[{}] kind=[{:?}]",
                        self.seq, action.kind
                    );
                }
                self.simulator.send_order(&action).expect("send order");
                for event in self.simulator.poll_events() {
                    match event {
                        TradeGatewayEvent::Order(order_event) => {
                            ctx.publish(&self.order_event_addr, &order_event)
                                .expect("order publish failed");
                            info!(
                                "trade event event=[ORDER_EVENT] order_id=[{}] status=[{:?}]",
                                order_event.order_id, order_event.status
                            );
                        }
                        TradeGatewayEvent::Trade(trade_event) => {
                            ctx.publish(&self.trade_event_addr, &trade_event)
                                .expect("trade publish failed");
                            info!(
                                "trade event event=[TRADE_EVENT] order_id=[{}] trade_id=[{}]",
                                trade_event.order_id, trade_event.trade_id
                            );
                        }
                    }
                }
            }
            Event::Shutdown => {
                info!("trade simulator event=[SHUTDOWN]");
            }
            _ => {}
        }
    }
}

fn main() {
    let _ = setup_log();
    let order_event_queue = broadcast_queue("order-event", "trade-sim");
    let trade_event_queue = broadcast_queue("trade-event", "trade-sim");
    info!(
        "starting trade simulator order_event_queue=[{}] trade_event_queue=[{}] control_queue=[{}]",
        order_event_queue, trade_event_queue, CONTROL_ADDR
    );

    register_actor(
        MASTER_ADDR,
        "trade-sim",
        "trade-simulator",
        vec![
            QueueInfo {
                addr: order_event_queue.clone(),
                queue_type: "order-event".to_string(),
            },
            QueueInfo {
                addr: trade_event_queue.clone(),
                queue_type: "trade-event".to_string(),
            },
            QueueInfo {
                addr: CONTROL_ADDR.to_string(),
                queue_type: "control".to_string(),
            },
        ],
    );

    let actor = TradeSimActor {
        order_event_addr: order_event_queue,
        trade_event_addr: trade_event_queue,
        action_sources: HashSet::new(),
        simulator: TradeSimulator::new(),
        seq: 0,
    };

    run(actor);
}

fn register_actor(
    master_addr: &str,
    actor_id: &str,
    actor_type: &str,
    queues: Vec<QueueInfo>,
) {
    let socket = Socket::new(Protocol::Req0).expect("master socket");
    socket.dial(master_addr).expect("master dial");
    let request = Request::Register {
        actor: ActorRegistration {
            actor_id: actor_id.to_string(),
            actor_type: actor_type.to_string(),
            queues,
        },
    };
    let payload = to_vec(&request).expect("serialize");
    socket.send(payload.as_slice()).expect("send");
    let msg = socket.recv().expect("recv");
    let response: Response = serde_json::from_slice(msg.as_slice()).expect("response");
    if let Response::Error { message } = response {
        panic!("register failed {}", message);
    }
}

fn connect_action_reader(ctx: &mut ActorContext, addr: &str) -> usize {
    loop {
        match ctx.read_from::<nnxt_strategy::Action>(addr) {
            Ok(source_id) => return source_id,
            Err(nnxt_actors::Error::RapidError(nnxt_rapid::Error::NotFound)) => {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(err) => panic!("action queue connect failed: {:?}", err),
        }
    }
}
