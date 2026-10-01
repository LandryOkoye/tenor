//! A tiny sliding-window rate limiter.
//!
//! Panta enforces per-account budgets (see docs/panta-notes.md); we stay
//! comfortably under them client-side so a burst of basket quotes can never
//! trip a 429. One limiter per endpoint family, exactly mirroring Panta's
//! documented families.
//!
//! Deliberately simple: a deque of `Instant`s (recently allowed requests).
//! `acquire()` sleeps until a slot frees up in the window.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::sync::Mutex;

#[derive(Debug)]
pub struct RateLimiter {
    /// How many requests are allowed…
    max: usize,
    /// …per sliding window of this length.
    window: Duration,
    /// Timestamps of the requests we already allowed inside the window.
    /// tokio's Instant (not std's) so paused-clock tests behave identically.
    hits: Mutex<VecDeque<tokio::time::Instant>>,
}

impl RateLimiter {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: Mutex::new(VecDeque::new()),
        }
    }

    /// Wait until this caller may make one request.
    pub async fn acquire(&self) {
        loop {
            // How long (if at all) do we need to wait for a free slot?
            let wait_for = {
                let mut hits = self.hits.lock().await;
                let now = tokio::time::Instant::now();

                // Drop timestamps that slid out of the window.
                while let Some(oldest) = hits.front() {
                    if now.duration_since(*oldest) >= self.window {
                        hits.pop_front();
                    } else {
                        break;
                    }
                }

                if hits.len() < self.max {
                    hits.push_back(now); // slot taken
                    None
                } else {
                    // Window is full: wake up exactly when the oldest hit expires.
                    Some(self.window - now.duration_since(hits[0]))
                }
            };

            match wait_for {
                None => return,                         // slot acquired
                Some(d) => tokio::time::sleep(d).await, // wait and retry
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn limits_bursts_to_max_per_window() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));

        // First two calls pass instantly.
        let t0 = tokio::time::Instant::now();
        limiter.acquire().await;
        limiter.acquire().await;
        assert_eq!(t0.elapsed(), Duration::ZERO);

        // Third call must wait until the first hit leaves the window (~100 ms).
        limiter.acquire().await;
        assert!(t0.elapsed() >= Duration::from_millis(100));
        assert!(t0.elapsed() < Duration::from_millis(200));

        // And the window slides: after waiting, more calls pass quickly.
        let t1 = tokio::time::Instant::now();
        limiter.acquire().await; // slot freed by the first call aging out
        assert!(t1.elapsed() < Duration::from_millis(50));
    }
}
