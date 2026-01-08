//! 延迟基准测试：Producer-Consumer 模式
//!
//! 测量 rapid 队列的端到端延迟，统计：
//! - 平均值、标准差、最大、最小
//! - p50/p90/p99（去掉最大的 0.1%）

use nnxt_rapid::{cleanup, Address, Reader, Writer};

const QUEUE_ADDR: &str = "bench/latency";
const CAPACITY: usize = 4096;
const WARMUP_COUNT: usize = 10_000;
const SAMPLE_COUNT: usize = 1_000_000;

fn timestamp_ns() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn main() {
    let addr = Address::new(QUEUE_ADDR).expect("address");
    let _ = cleanup(&addr);

    println!("Latency Benchmark: Producer-Consumer");
    println!("=====================================");
    println!("Queue: {}", QUEUE_ADDR);
    println!("Capacity: {}", CAPACITY);
    println!("Warmup: {} samples", WARMUP_COUNT);
    println!("Measure: {} samples", SAMPLE_COUNT);
    println!();

    // 创建队列
    let mut writer = Writer::<u64>::create(&addr, CAPACITY).expect("writer");
    let mut reader = Reader::<u64>::connect(&addr).expect("reader");

    // 预热
    print!("Warming up... ");
    for _ in 0..WARMUP_COUNT {
        let ts = timestamp_ns();
        writer.write(ts);
        let _ = reader.read();
    }
    println!("done");

    // 测量
    print!("Measuring... ");
    let mut latencies = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let send_ts = timestamp_ns();
        writer.write(send_ts);
        let recv_ts = *reader.read();
        let now = timestamp_ns();
        latencies.push(now.saturating_sub(recv_ts));
    }
    println!("done");
    println!();

    // 统计计算
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
