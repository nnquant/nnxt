use nnxt_actors::{run, Actor, ActorContext, Event};
use nnxt_gateway::{SimulatedSource, SourceConfig};
use nnxt_master::protocol::{ActorRegistration, QueueInfo, Request, Response};
use nng::{Protocol, Socket};
use nnxt_rapid::{cleanup, Address};
use serde_json::to_vec;
use nnxt_specs::market::InstrumentId;
use nnxt_specs::OrderBook;
use std::str::FromStr;
use std::time::Duration;
use tracing::info;
use nnxt_utils::clock::MonotonicClock;
use nnxt_utils::{broadcast_queue, setup_log};

const MASTER_ADDR: &str = "ipc:///tmp/nnxt/master";

struct Tick;

struct MarketSimActor {
    instrument: InstrumentId,
    queue_addr: String,
    source: SimulatedSource,
    tick_interval: Duration,
    seq: u64,
}

impl Actor for MarketSimActor {
    fn on_start(&mut self, ctx: &mut ActorContext) {
        ctx.write_to::<OrderBook>(&self.queue_addr, 1024)
            .expect("market queue create failed");
        let sender = ctx.external_sender();
        let interval = self.tick_interval;
        std::thread::spawn(move || loop {
            std::thread::sleep(interval);
            if sender.send(Box::new(Tick)).is_err() {
                break;
            }
        });
    }

    fn on_event(&mut self, event: Event, ctx: &mut ActorContext) {
        if let Event::External(_payload) = event {
            let now_ns = MonotonicClock::now_ns();
            let mut book = self.source.generate(self.instrument, now_ns);
            book.timestamp = now_ns;
            ctx.publish(&self.queue_addr, &book)
                .expect("market publish failed");
            self.seq = self.seq.saturating_add(1);
            if self.seq % 100 == 0 {
                info!("market tick event=[MARKET_TICK] seq=[{}]", self.seq);
            }
        }
    }
}

fn main() {
    let _ = setup_log();
    let market_queue = broadcast_queue("market", "market-sim");
    info!("starting market simulator queue=[{}]", market_queue);

    let instrument = InstrumentId::from_str("IF2409").expect("instrument");
    let addr = Address::new(&market_queue).expect("queue addr");
    let _ = cleanup(&addr);

    register_actor(
        MASTER_ADDR,
        "market-sim",
        "market-simulator",
        vec![QueueInfo {
            addr: market_queue.clone(),
            queue_type: "market".to_string(),
        }],
    );

    let source = SimulatedSource::new(SourceConfig::default());
    let tick_interval = source.interval();
    let actor = MarketSimActor {
        instrument,
        queue_addr: market_queue,
        source,
        tick_interval,
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
