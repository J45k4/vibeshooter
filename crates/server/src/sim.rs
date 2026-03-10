use std::time::Duration;

pub const TICK_RATE_HZ: u64 = 60;
pub const SNAPSHOT_RATE_HZ: u64 = 20;

pub fn tick_duration() -> Duration {
    Duration::from_secs_f64(1.0 / TICK_RATE_HZ as f64)
}

pub fn snapshot_interval_ticks() -> u64 {
    TICK_RATE_HZ / SNAPSHOT_RATE_HZ
}
