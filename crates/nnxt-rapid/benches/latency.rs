use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nnxt_rapid::{cleanup, Address, Reader, Writer};

fn bench_latency(c: &mut Criterion) {
    let path = format!("bench/latency/{}", std::process::id());
    let addr = Address::new(&path).expect("address");
    let _ = cleanup(&addr);

    let mut writer = Writer::<u64>::create(&addr, 1024).expect("writer");
    let mut reader = Reader::<u64>::connect(&addr).expect("reader");

    c.bench_function("rapid_write_read", |b| {
        let mut value = 0u64;
        b.iter(|| {
            value = value.wrapping_add(1);
            writer.write(black_box(value));
            let read = reader.read_timeout(Duration::from_secs(1)).expect("read");
            black_box(*read);
        })
    });
}

criterion_group!(benches, bench_latency);
criterion_main!(benches);
