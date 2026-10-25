/// Token buckets: each client may burst `client_burst` reports and then earns
/// `client_per_hour`, the server as a whole likewise, and `max_clients` bounds
/// what a flood of addresses can take.
#[derive(Clone, Debug)]
pub struct Limits {
    pub client_burst: f64,
    pub client_per_hour: f64,
    pub global_burst: f64,
    pub global_per_hour: f64,
    pub max_clients: usize,
}

impl Default for Limits {
    /// A healthy installer reports at most once per run, and 600 an hour is far
    /// beyond any real user base while still bounding a broken client.
    fn default() -> Self {
        Limits {
            client_burst: 10.0,
            client_per_hour: 10.0,
            global_burst: 600.0,
            global_per_hour: 600.0,
            max_clients: 10_000,
        }
    }
}

struct Bucket {
    tokens: f64,
    updated: std::time::Instant,
}

impl Bucket {
    fn full(capacity: f64, now: std::time::Instant) -> Bucket {
        Bucket {
            tokens: capacity,
            updated: now,
        }
    }

    fn refill(&mut self, now: std::time::Instant, capacity: f64, per_hour: f64) {
        let earned = now.duration_since(self.updated).as_secs_f64() * per_hour / 3_600.0;
        self.tokens = (self.tokens + earned).min(capacity);
        self.updated = now;
    }

    /// Takes a token, or says how many seconds until one is earned.
    fn take(&mut self, now: std::time::Instant, capacity: f64, per_hour: f64) -> Result<(), u64> {
        self.refill(now, capacity, per_hour);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            Err(((1.0 - self.tokens) * 3_600.0 / per_hour).ceil() as u64)
        }
    }
}

pub struct RateLimiter {
    limits: Limits,
    clients: std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, Bucket>>,
    global: std::sync::Mutex<Bucket>,
}

impl RateLimiter {
    pub fn new(limits: Limits) -> RateLimiter {
        let now = std::time::Instant::now();
        RateLimiter {
            global: std::sync::Mutex::new(Bucket::full(limits.global_burst, now)),
            clients: std::sync::Mutex::new(std::collections::HashMap::new()),
            limits,
        }
    }

    /// `Err` carries a Retry-After in seconds. The client's own bucket goes
    /// first, so a client over its limit cannot drain the global one.
    pub fn check(&self, client: std::net::IpAddr, now: std::time::Instant) -> Result<(), u64> {
        let limits = &self.limits;
        {
            let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
            if !clients.contains_key(&client) && clients.len() >= limits.max_clients {
                // A refilled bucket is indistinguishable from a new client.
                clients.retain(|_, bucket| {
                    bucket.refill(now, limits.client_burst, limits.client_per_hour);
                    bucket.tokens < limits.client_burst
                });
                if clients.len() >= limits.max_clients {
                    return Err(60);
                }
            }
            clients
                .entry(client)
                .or_insert_with(|| Bucket::full(limits.client_burst, now))
                .take(now, limits.client_burst, limits.client_per_hour)?;
        }
        self.global.lock().unwrap_or_else(|e| e.into_inner()).take(
            now,
            limits.global_burst,
            limits.global_per_hour,
        )
    }
}

#[cfg(test)]
mod tests {
    fn tight() -> super::Limits {
        super::Limits {
            client_burst: 2.0,
            client_per_hour: 2.0,
            global_burst: 3.0,
            global_per_hour: 3.0,
            max_clients: 2,
        }
    }

    fn ip(text: &str) -> std::net::IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn the_limiter_holds_per_client_and_globally() {
        let limiter = super::RateLimiter::new(tight());
        let now = std::time::Instant::now();
        let a = ip("198.51.100.1");
        let b = ip("198.51.100.2");

        assert!(limiter.check(a, now).is_ok());
        assert!(limiter.check(a, now).is_ok());
        let retry = limiter.check(a, now).unwrap_err();
        assert!((1_700..=1_800).contains(&retry), "{retry}");
        assert!(limiter.check(b, now).is_ok());
        assert!(limiter.check(b, now).is_err(), "the global bucket ran out");

        let an_hour_later = now + std::time::Duration::from_secs(3_600);
        assert!(limiter.check(a, an_hour_later).is_ok());
    }

    #[test]
    fn a_flood_of_addresses_cannot_grow_memory_without_bound() {
        let limiter = super::RateLimiter::new(tight());
        let now = std::time::Instant::now();
        assert!(limiter.check(ip("198.51.100.1"), now).is_ok());
        assert!(limiter.check(ip("198.51.100.2"), now).is_ok());
        assert!(limiter.check(ip("198.51.100.3"), now).is_err());

        let once_refilled = now + std::time::Duration::from_secs(7_200);
        assert!(limiter.check(ip("198.51.100.3"), once_refilled).is_ok());
    }
}
