//! Producer: 向队列发送时间戳（带节流）

use std::env;
use std::time::Duration;
use nnxt_rapid::{cleanup, Address, Writer};

const QUEUE_ADDR: &str = "bench/latency";
const CAPACITY: usize = 4096;
const DEFAULT_COUNT: usize = 1_000_000;

fn timestamp_ns() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn main() {
    let count: usize = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COUNT);

    // 发送间隔（纳秒），0 表示不限速
    let interval_ns: u64 = env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000); // 默认 1μs 间隔

    let addr = Address::new(QUEUE_ADDR).expect("address");
    let _ = cleanup(&addr);

    println!("Producer starting...");
    println!("Queue: {}", QUEUE_ADDR);
    println!("Count: {}", count);
    println!("Interval: {} ns", interval_ns);

    let mut writer = Writer::<u64>::create(&addr, CAPACITY).expect("writer");

    println!("Waiting for consumer (press Enter)...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();

    println!("Sending...");
    let start = std::time::Instant::now();

    for i in 0..count {
        let ts = timestamp_ns();
        writer.write(ts);

        // 节流
        if interval_ns > 0 {
            let target = Duration::from_nanos(interval_ns * (i as u64 + 1));
            while start.elapsed() < target {
                std::hint::spin_loop();
            }
        }
    }

    let elapsed = start.elapsed();
    println!("Done! Sent {} in {:?}", count, elapsed);
    println!("Rate: {:.2} msg/s", count as f64 / elapsed.as_secs_f64());

    println!("Press Enter to exit...");
    std::io::stdin().read_line(&mut input).ok();
}
