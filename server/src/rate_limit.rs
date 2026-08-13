use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

/// Fixed-window (60 s) per-IP limiter. In-memory; prune() drops old windows.
pub struct RateLimiter {
    per_min: u32,
    windows: Mutex<HashMap<IpAddr, (i64, u32)>>,
}

impl RateLimiter {
    pub fn new(per_min: u32) -> Self {
        Self {
            per_min,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// True if the request is allowed (consumes a token).
    pub fn check(&self, ip: IpAddr, now: i64) -> bool {
        let mut w = self.windows.lock().unwrap();
        let entry = w.entry(ip).or_insert((now, 0));
        if now - entry.0 >= 60 {
            *entry = (now, 0);
        }
        if entry.1 >= self.per_min {
            return false;
        }
        entry.1 += 1;
        true
    }

    #[cfg(test)]
    pub fn prune(&self, now: i64) {
        self.windows.lock().unwrap().retain(|_, (t, _)| now - *t < 120);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.windows.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn fixed_window_limits_per_minute() {
        let l = RateLimiter::new(3);
        let ip: IpAddr = Ipv4Addr::LOCALHOST.into();
        assert!(l.check(ip, 100));
        assert!(l.check(ip, 100));
        assert!(l.check(ip, 100));
        assert!(!l.check(ip, 100));
        assert!(l.check(ip, 160)); // new window
        assert!(l.check(ip, 161));
        l.prune(400);
        assert_eq!(l.len(), 0);
    }
}
