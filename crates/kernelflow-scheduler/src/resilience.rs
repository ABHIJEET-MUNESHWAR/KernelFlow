//! Resilience primitives: timeout + retry + circuit breaker + rate limit.

use std::sync::Arc;
use std::time::Duration;

use governor::{Quota, RateLimiter, clock::DefaultClock, state::{InMemoryState, NotKeyed}};
use kernelflow_core::{KernelError, KernelResult};

#[derive(Debug, Clone)]
pub struct ResilienceConfig {
    pub timeout:           Duration,
    pub max_retries:       u32,
    pub initial_backoff:   Duration,
    pub max_backoff:       Duration,
    pub rps:               u32,
    pub circuit_threshold: u8,   // consecutive failures before open
    pub circuit_cooldown:  Duration,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            timeout:           Duration::from_secs(30),
            max_retries:       3,
            initial_backoff:   Duration::from_millis(100),
            max_backoff:       Duration::from_secs(5),
            rps:               1000,
            circuit_threshold: 5,
            circuit_cooldown:  Duration::from_secs(10),
        }
    }
}

/// Composes timeout / retry / rate-limit / circuit-breaker around a future-returning closure.
pub struct ResilientExecutor {
    cfg:     ResilienceConfig,
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    breaker: tokio::sync::Mutex<CircuitBreaker>,
}

impl ResilientExecutor {
    pub fn new(cfg: ResilienceConfig) -> Self {
        let quota = Quota::per_second(std::num::NonZeroU32::new(cfg.rps.max(1)).unwrap());
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
            breaker: tokio::sync::Mutex::new(CircuitBreaker::new(cfg.circuit_threshold, cfg.circuit_cooldown)),
            cfg,
        }
    }

    pub async fn run<F, Fut, T>(&self, mut f: F) -> KernelResult<T>
    where
        F:   FnMut() -> Fut,
        Fut: std::future::Future<Output = KernelResult<T>>,
    {
        // Rate limit
        if self.limiter.check().is_err() {
            return Err(KernelError::RateLimited);
        }
        // Circuit breaker
        if !self.breaker.lock().await.allow() {
            return Err(KernelError::CircuitOpen);
        }

        let mut attempt: u32 = 0;
        let mut backoff = self.cfg.initial_backoff;
        loop {
            attempt += 1;
            let res = tokio::time::timeout(self.cfg.timeout, f()).await;
            let outcome = match res {
                Ok(Ok(v))  => Ok(v),
                Ok(Err(e)) => Err(e),
                Err(_)     => Err(KernelError::Timeout(self.cfg.timeout)),
            };
            match outcome {
                Ok(v) => {
                    self.breaker.lock().await.on_success();
                    return Ok(v);
                }
                Err(e) if attempt >= self.cfg.max_retries => {
                    self.breaker.lock().await.on_failure();
                    return Err(e);
                }
                Err(e) => {
                    tracing::warn!(?e, attempt, "retrying");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(self.cfg.max_backoff);
                }
            }
        }
    }
}

struct CircuitBreaker {
    threshold:    u8,
    cooldown:     Duration,
    failures:     u8,
    open_until:   Option<std::time::Instant>,
}

impl CircuitBreaker {
    fn new(threshold: u8, cooldown: Duration) -> Self {
        Self { threshold, cooldown, failures: 0, open_until: None }
    }
    fn allow(&mut self) -> bool {
        if let Some(t) = self.open_until {
            if std::time::Instant::now() < t { return false; }
            self.open_until = None;
            self.failures = 0;
        }
        true
    }
    fn on_success(&mut self) { self.failures = 0; }
    fn on_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
        if self.failures >= self.threshold {
            self.open_until = Some(std::time::Instant::now() + self.cooldown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn retries_then_succeeds() {
        let exec = ResilientExecutor::new(ResilienceConfig {
            max_retries: 3, initial_backoff: Duration::from_millis(1), ..Default::default()
        });
        let calls = Arc::new(AtomicU32::new(0));
        let calls_c = calls.clone();
        let r: KernelResult<u32> = exec.run(|| {
            let calls = calls_c.clone();
            async move {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                if n < 2 { Err(KernelError::Network("x".into())) } else { Ok(42) }
            }
        }).await;
        assert_eq!(r.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn timeout_fires() {
        let exec = ResilientExecutor::new(ResilienceConfig {
            timeout: Duration::from_millis(10), max_retries: 1, ..Default::default()
        });
        let r: KernelResult<()> = exec.run(|| async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        }).await;
        assert!(matches!(r, Err(KernelError::Timeout(_))));
    }
}

