//! High precision clocks.

use core::time::Duration;
use std::time::Instant;

pub trait Clock {
    fn now_ns(&self) -> u64;
}

#[derive(Debug, Clone)]
pub struct InstantClock {
    start: Instant,
}

impl InstantClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    fn elapsed_ns(&self) -> u64 {
        let duration = Instant::now().duration_since(self.start);
        duration.as_nanos() as u64
    }

    pub fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self.start)
    }
}

impl Default for InstantClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for InstantClock {
    fn now_ns(&self) -> u64 {
        self.elapsed_ns()
    }
}

#[cfg(target_arch = "x86_64")]
#[derive(Debug, Clone, Copy, Default)]
pub struct RdtscClock;

#[cfg(target_arch = "x86_64")]
impl Clock for RdtscClock {
    fn now_ns(&self) -> u64 {
        unsafe { core::arch::x86_64::_rdtsc() }
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct RdtscClock;

#[cfg(not(target_arch = "x86_64"))]
impl Clock for RdtscClock {
    fn now_ns(&self) -> u64 {
        0
    }
}

/// High-precision monotonic clock for cross-process latency measurement.
/// Returns absolute nanoseconds since an unspecified epoch.
/// - Linux: clock_gettime(CLOCK_MONOTONIC_RAW)
/// - Windows: QueryPerformanceCounter
/// - Other: std::time::Instant (fallback)
#[derive(Debug, Clone, Copy, Default)]
pub struct MonotonicClock;

impl MonotonicClock {
    pub fn now_ns() -> u64 {
        monotonic_now_ns()
    }
}

#[cfg(target_os = "linux")]
fn monotonic_now_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: ts is a valid pointer to a timespec struct
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC_RAW, &mut ts);
    }
    (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
}

#[cfg(target_os = "windows")]
fn monotonic_now_ns() -> u64 {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    static FREQUENCY: OnceLock<u64> = OnceLock::new();
    let freq = *FREQUENCY.get_or_init(|| {
        let mut freq: i64 = 0;
        // SAFETY: freq is a valid pointer
        unsafe { QueryPerformanceFrequency(&mut freq) };
        freq as u64
    });

    let mut counter: i64 = 0;
    // SAFETY: counter is a valid pointer
    unsafe { QueryPerformanceCounter(&mut counter) };
    let counter = counter as u64;

    // Convert to nanoseconds: counter * 1_000_000_000 / freq
    // Use 128-bit arithmetic to avoid overflow
    ((counter as u128) * 1_000_000_000 / (freq as u128)) as u64
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn monotonic_now_ns() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    start.elapsed().as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instant_clock_is_monotonic() {
        let clock = InstantClock::new();
        let first = clock.now_ns();
        std::thread::sleep(Duration::from_millis(1));
        let second = clock.now_ns();
        assert!(second >= first);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn rdtsc_clock_returns_non_zero_cycles() {
        let clock = RdtscClock;
        let first = clock.now_ns();
        let second = clock.now_ns();
        assert!(second >= first);
    }
}
