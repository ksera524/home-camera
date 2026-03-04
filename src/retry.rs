use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_ms: 250,
            max_delay_ms: 8_000,
        }
    }
}

impl RetryPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let shift = attempt.min(20);
        let multiplier = 1u64 << shift;
        let uncapped = self.base_delay_ms.saturating_mul(multiplier);
        Duration::from_millis(uncapped.min(self.max_delay_ms))
    }
}
