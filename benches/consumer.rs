//! Consumer: 从队列读取时间戳并统计延迟

use std::env;
use std::time::Duration;
use nnxt_rapid::{Address, Reader};

const QUEUE_ADDR: &str = "bench/latency";
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

    let addr = Address::new(QUEUE_ADDR).expect("address");

    println!("Consumer starting...");
    println!("Queue: {}", QUEUE_ADDR);
    println!("Count: {}", count);

    // 等待队列创建
    println!("Waiting for producer...");
    let mut reader = loop {
        match Reader::<u64>::connect(&addr) {
            Ok(r) => break r,
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    println!("Connected!");

    // 读取并计算延迟
    println!("Receiving {} timestamps...", count);
    let mut latencies = Vec::with_capacity(count);
    for _ in 0..count {
        let send_ts = *reader.read();
        let recv_ts = timestamp_ns();
        latencies.push(recv_ts.saturating_sub(send_ts));
    }
    println!("Done!");
    println!();

    // 统计
    compute_stats(&mut latencies);
}

fn compute_stats(latencies: &mut [u64]) {
    latencies.sort_unstable();

    let n = latencies.len();
    let trim_count = n / 1000; // 去掉最大的 0.1%
    let trimmed = &latencies[..n - trim_count];

    // 基本统计
    let min = trimmed[0];
    let max = trimmed[trimmed.len() - 1];
    let sum: u64 = trimmed.iter().sum();
    let avg = sum as f64 / trimmed.len() as f64;

    // 标准差
    let variance: f64 = trimmed.iter()
        .map(|&x| (x as f64 - avg).powi(2))
        .sum::<f64>() / trimmed.len() as f64;
    let stddev = variance.sqrt();

    // 百分位数
    let p50 = trimmed[trimmed.len() * 50 / 100];
    let p90 = trimmed[trimmed.len() * 90 / 100];
    let p99 = trimmed[trimmed.len() * 99 / 100];

    // 输出结果
    println!("Results (trimmed top 0.1%):");
    println!("  Samples: {}", trimmed.len());
    println!("  Min:     {} ns", min);
    println!("  Max:     {} ns", max);
    println!("  Avg:     {:.2} ns", avg);
    println!("  Stddev:  {:.2} ns", stddev);
    println!("  p50:     {} ns", p50);
    println!("  p90:     {} ns", p90);
    println!("  p99:     {} ns", p99);
}
