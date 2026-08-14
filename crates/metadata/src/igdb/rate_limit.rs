//! IGDB corta a 4 peticiones por segundo y devuelve 429. Con una biblioteca de
//! mil juegos eso no es un detalle: es la diferencia entre sincronizar y que se
//! caiga a la mitad.

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
    /// `capacity` peticiones por `window`.
    pub fn new(capacity: usize, window: Duration) -> Self {
        Self {
            window,
            capacity,
            recent: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Espera lo justo para no pasarse. Se llama antes de cada petición.
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
                // La más antigua marca cuándo se libera un hueco.
                recent.front().map(|t| *t + self.window)
            };

            match wait_until {
                Some(deadline) => sleep_until(deadline).await,
                None => return,
            }
        }
    }
}
