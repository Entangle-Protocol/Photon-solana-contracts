use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_time_ms() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    since_the_epoch.as_secs() * 1000 + since_the_epoch.subsec_millis() as u64
}
