use std::thread;
use std::time::Duration;

/// Pauses the current thread.
pub fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

/// Counts to n, one step at a time.
pub fn count_slowly(n: u32) -> u32 {
    let mut count = 0;
    while count < n {
        sleep_ms(1);
        count += 1;
    }
    count
}
