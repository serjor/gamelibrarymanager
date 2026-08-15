//! Every provider cuts the requests it takes and answers 429 over the line.
//! With a library of a thousand games that is not a detail: it is the difference
//! between finishing a pass and stopping halfway.
//!
//! One limiter for every provider, with the window each one publishes. IGDB
//! allows 4 requests per second, ITAD allows 1000 every five minutes, and the
//! rule that keeps both honest is the same one.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{Instant, sleep_until};

pub struct RateLimiter {
    window: Duration,
    capacity: usize,
    recent: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    /// `capacity` requests for each `window`.
    pub fn new(capacity: usize, window: Duration) -> Self {
        Self {
            window,
            capacity,
            recent: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Waits only the necessary time to stay in the limit. You call it before
    /// each request.
    pub async fn acquire(&self) {
        loop {
            let wait_until = {
                let mut recent = self.recent.lock().await;
                let now = Instant::now();
                while recent
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= self.window)
                {
                    recent.pop_front();
                }

                if recent.len() < self.capacity {
                    recent.push_back(now);
                    return;
                }
                // The oldest one shows when a slot becomes free.
                recent.front().map(|t| *t + self.window)
            };

            match wait_until {
                Some(deadline) => sleep_until(deadline).await,
                None => return,
            }
        }
    }
}
