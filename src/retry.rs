use std::time::Duration;
use rand::Rng;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 2000, // 2 seconds
            max_delay_ms: 10000, // 10 seconds
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with custom max retries
    #[cfg(test)]
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }
}

/// Retry a fallible operation with exponential backoff and jitter
pub fn retry_with_backoff<F, T, E>(
    mut operation: F,
    config: &RetryConfig,
    verbose: bool,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
{
    let mut attempt = 0;

    loop {
        attempt += 1;

        match operation() {
            Ok(result) => {
                if attempt > 1 && verbose {
                    tracing::info!("Operation succeeded after {} attempts", attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                if attempt > config.max_retries {
                    tracing::error!("Operation failed after {} attempts: {}", attempt, e);
                    return Err(e);
                }

                // Calculate delay with exponential backoff
                let exponential_delay = config.base_delay_ms * 2_u64.pow(attempt - 1);
                let delay_ms = exponential_delay.min(config.max_delay_ms);

                // Add jitter (±20% randomness)
                let jitter_range = (delay_ms as f64 * 0.2) as u64;
                let jitter = rand::thread_rng().gen_range(0..=jitter_range * 2);
                let final_delay_ms = delay_ms.saturating_sub(jitter_range).saturating_add(jitter);

                tracing::warn!(
                    "Operation failed (attempt {}/{}): {}. Retrying in {}ms...",
                    attempt,
                    config.max_retries + 1,
                    e,
                    final_delay_ms
                );

                if verbose {
                    eprintln!(
                        "⚠️  Attempt {}/{} failed: {}. Retrying in {:.1}s...",
                        attempt,
                        config.max_retries + 1,
                        e,
                        final_delay_ms as f64 / 1000.0
                    );
                }

                std::thread::sleep(Duration::from_millis(final_delay_ms));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_succeeds_immediately() {
        let config = RetryConfig::default();
        let mut call_count = 0;

        let result = retry_with_backoff(
            || {
                call_count += 1;
                Ok::<i32, String>(42)
            },
            &config,
            false,
        );

        assert_eq!(result, Ok(42));
        assert_eq!(call_count, 1);
    }

    #[test]
    fn test_retry_succeeds_after_failures() {
        let config = RetryConfig::default();
        let mut call_count = 0;

        let result = retry_with_backoff(
            || {
                call_count += 1;
                if call_count < 3 {
                    Err("Temporary error".to_string())
                } else {
                    Ok::<i32, String>(42)
                }
            },
            &config,
            false,
        );

        assert_eq!(result, Ok(42));
        assert_eq!(call_count, 3);
    }

    #[test]
    fn test_retry_exhausts_attempts() {
        let config = RetryConfig::with_max_retries(2);
        let mut call_count = 0;

        let result = retry_with_backoff(
            || {
                call_count += 1;
                Err::<i32, String>("Persistent error".to_string())
            },
            &config,
            false,
        );

        assert_eq!(result, Err("Persistent error".to_string()));
        assert_eq!(call_count, 3); // 1 initial + 2 retries
    }
}
