use std::collections::VecDeque;
use std::time::{Duration, Instant};


pub struct RequestLimiter {
    capacity: usize,
    used: VecDeque<Instant>,
    interval: Duration,
}

impl RequestLimiter {
    pub fn new(capacity: usize, interval: Duration) -> Self {
        Self {
            capacity,
            used: Default::default(),
            interval,
        }
    }

    pub fn try(&mut self) -> bool {
        let now = Instant::now();

        remove_outdated(&mut self.used, now);

        if self.used.len() < self.capacity {
            self.used.push_back(now + self.interval);
            return true
        }

        false
    }
}

fn remove_outdated(used: &mut VecDeque<Instant>, now: Instant) {
    while let Some(time) = used.pop_front() {
        if now < time {
            used.push_front(time);
            break
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep_ms;

    #[test]
    fn simple_success() {
        let mut limiter = RequestLimiter::new(1, Duration::from_secs(1));

        assert!(limiter.try());
        assert!(!limiter.try());
    }

    #[test]
    fn wait() {
        let mut limiter = RequestLimiter::new(1, Duration::from_millis(100));

        assert!(limiter.try());
        assert!(!limiter.try());
        sleep_ms(100);
        assert!(limiter.try());
    }
}
