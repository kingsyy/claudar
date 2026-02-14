use notify_rust::{Notification, Timeout};

#[cfg(test)]
use std::sync::{Arc, Mutex};

/// Trait for sending notifications, allowing for mocking in tests
pub trait NotificationSender: Send + Sync {
    fn send(&self, summary: &str, body: &str, timeout: Timeout) -> anyhow::Result<()>;
}

/// Real notification sender using notify-rust
pub struct RealNotificationSender;

impl NotificationSender for RealNotificationSender {
    fn send(&self, summary: &str, body: &str, timeout: Timeout) -> anyhow::Result<()> {
        Notification::new()
            .summary(summary)
            .body(body)
            .timeout(timeout)
            .show()?;
        Ok(())
    }
}

/// Mock notification sender for testing
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MockNotificationSender {
    pub sent_notifications: Arc<Mutex<Vec<SentNotification>>>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub struct SentNotification {
    pub summary: String,
    pub body: String,
    pub timeout_ms: Option<u32>, // None for Timeout::Never
}

#[cfg(test)]
impl MockNotificationSender {
    pub fn new() -> Self {
        Self {
            sent_notifications: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_sent(&self) -> Vec<SentNotification> {
        self.sent_notifications.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.sent_notifications.lock().unwrap().clear();
    }

    pub fn count(&self) -> usize {
        self.sent_notifications.lock().unwrap().len()
    }
}

#[cfg(test)]
impl NotificationSender for MockNotificationSender {
    fn send(&self, summary: &str, body: &str, timeout: Timeout) -> anyhow::Result<()> {
        let timeout_ms = match timeout {
            Timeout::Milliseconds(ms) => Some(ms),
            Timeout::Never => None,
            _ => Some(5000), // Default for other variants
        };

        let notification = SentNotification {
            summary: summary.to_string(),
            body: body.to_string(),
            timeout_ms,
        };

        self.sent_notifications.lock().unwrap().push(notification);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_sender_records_notifications() {
        let mock = MockNotificationSender::new();

        mock.send("Test Summary", "Test Body", Timeout::Milliseconds(5000))
            .unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].summary, "Test Summary");
        assert_eq!(sent[0].body, "Test Body");
        assert_eq!(sent[0].timeout_ms, Some(5000));
    }

    #[test]
    fn test_mock_sender_handles_never_timeout() {
        let mock = MockNotificationSender::new();

        mock.send("Test", "Body", Timeout::Never).unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent[0].timeout_ms, None);
    }

    #[test]
    fn test_mock_sender_clear() {
        let mock = MockNotificationSender::new();

        mock.send("Test", "Body", Timeout::Milliseconds(1000)).unwrap();
        assert_eq!(mock.count(), 1);

        mock.clear();
        assert_eq!(mock.count(), 0);
    }
}
