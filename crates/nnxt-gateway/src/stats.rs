//! Latency statistics.

#[derive(Debug, Default, Clone)]
pub struct LatencyStats {
    count: u64,
    total_ns: u128,
    max_ns: u64,
    last_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencySnapshot {
    pub count: u64,
    pub avg_ns: u64,
    pub max_ns: u64,
    pub last_ns: u64,
}

impl LatencyStats {
    pub fn record(&mut self, latency_ns: u64) {
        self.count += 1;
        self.total_ns += latency_ns as u128;
        self.last_ns = latency_ns;
        if latency_ns > self.max_ns {
            self.max_ns = latency_ns;
        }
    }

    pub fn snapshot(&self) -> LatencySnapshot {
        let avg_ns = if self.count == 0 {
            0
        } else {
            (self.total_ns / self.count as u128) as u64
        };
        LatencySnapshot {
            count: self.count,
            avg_ns,
            max_ns: self.max_ns,
            last_ns: self.last_ns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_snapshot() {
        let mut stats = LatencyStats::default();
        stats.record(10);
        stats.record(30);
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.count, 2);
        assert_eq!(snapshot.avg_ns, 20);
        assert_eq!(snapshot.max_ns, 30);
        assert_eq!(snapshot.last_ns, 30);
    }
}
