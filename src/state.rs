use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorState {
    pub five_hour: LimitState,
    pub seven_day: LimitState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitState {
    /// The current reset timestamp for this limit
    pub current_reset_at: Option<DateTime<Utc>>,

    /// Thresholds that have been notified for the current period
    pub notified_thresholds: HashSet<u8>,

    /// Timestamp of the last predicted overage warning
    pub last_overage_warning: Option<DateTime<Utc>>,

    /// Whether we've notified about the most recent reset
    pub notified_reset: bool,

    /// Whether we've notified about the upcoming reset (minutes before)
    #[serde(default)]
    pub notified_upcoming_reset: bool,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            five_hour: LimitState::default(),
            seven_day: LimitState::default(),
        }
    }
}

impl Default for LimitState {
    fn default() -> Self {
        Self {
            current_reset_at: None,
            notified_thresholds: HashSet::new(),
            last_overage_warning: None,
            notified_reset: false,
            notified_upcoming_reset: false,
        }
    }
}

impl MonitorState {
    /// Get the path to the state file
    pub fn state_path() -> anyhow::Result<PathBuf> {
        let dir = crate::config::Config::config_dir()?;
        Ok(dir.join("monitor_state.json"))
    }

    /// Load state from disk, or create default if doesn't exist
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::state_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let mut state: MonitorState = serde_json::from_str(&content)?;

        // Validate and clean potentially stale state
        state.validate_and_clean();

        Ok(state)
    }

    /// Validate state and clean up stale or corrupted data
    ///
    /// This prevents false notifications from:
    /// - Very old reset times (service offline for extended period)
    /// - Corrupted state files
    /// - Clock drift or system time changes
    fn validate_and_clean(&mut self) {
        let now = Utc::now();

        // Validate 5-hour limit state
        // If stored reset time is more than 2 periods (10 hours) in the past, reset the state
        if let Some(reset_at) = self.five_hour.current_reset_at {
            if reset_at < now - chrono::Duration::hours(10) {
                tracing::warn!(
                    "5-hour limit reset time is very old ({}), resetting state to prevent false notifications",
                    reset_at
                );
                self.five_hour = LimitState::default();
            }
        }

        // Validate 7-day limit state
        // If stored reset time is more than 2 periods (14 days) in the past, reset the state
        if let Some(reset_at) = self.seven_day.current_reset_at {
            if reset_at < now - chrono::Duration::days(14) {
                tracing::warn!(
                    "7-day limit reset time is very old ({}), resetting state to prevent false notifications",
                    reset_at
                );
                self.seven_day = LimitState::default();
            }
        }

        // Validate overage warning timestamps
        if let Some(last_warning) = self.five_hour.last_overage_warning {
            if last_warning < now - chrono::Duration::days(7) {
                tracing::debug!("Clearing stale overage warning timestamp for 5-hour limit");
                self.five_hour.last_overage_warning = None;
            }
        }

        if let Some(last_warning) = self.seven_day.last_overage_warning {
            if last_warning < now - chrono::Duration::days(30) {
                tracing::debug!("Clearing stale overage warning timestamp for 7-day limit");
                self.seven_day.last_overage_warning = None;
            }
        }
    }

    /// Save state to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = crate::config::Config::config_dir()?;
        std::fs::create_dir_all(&dir)?;

        let path = Self::state_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check if a reset has occurred and update state accordingly
    ///
    /// Reset detection logic:
    /// - If current time has passed the stored reset time, a reset has occurred
    /// - Always update the stored reset time to the new value from the API
    /// - This simple approach avoids false positives from timestamp variations
    pub fn check_and_handle_reset(
        &mut self,
        limit_type: LimitType,
        new_reset_at: DateTime<Utc>,
    ) -> bool {
        let state = match limit_type {
            LimitType::FiveHour => &mut self.five_hour,
            LimitType::SevenDay => &mut self.seven_day,
        };

        // If we don't have a reset time yet, this is the first run
        // Store the reset time without notifying (we don't know if a reset just occurred)
        if state.current_reset_at.is_none() {
            state.current_reset_at = Some(new_reset_at);
            // Don't mark as notified - if a reset just happened, we want to notify
            // The notification will be suppressed if we're still before the reset time
            return false;
        }

        let stored_reset_at = state.current_reset_at.unwrap();
        let now = Utc::now();

        // A reset has occurred if:
        // 1. Current time has passed the stored reset time (now >= stored_reset_at)
        // 2. We haven't already notified about this reset
        let reset_occurred = now >= stored_reset_at && !state.notified_reset;

        // Always update to the new reset time from the API
        // This handles API adjustments and ensures we track the correct next reset
        state.current_reset_at = Some(new_reset_at);

        if reset_occurred {
            // Reset occurred - clear all notification state for this limit
            state.notified_thresholds.clear();
            state.last_overage_warning = None;
            state.notified_reset = false; // Will be set to true by caller after notification
            state.notified_upcoming_reset = false;
            tracing::debug!(
                "Reset detected for {} limit: stored_time={}, current_time={}, new_time={}",
                limit_type.as_str(),
                stored_reset_at,
                now,
                new_reset_at
            );
            return true;
        }

        false
    }

    /// Mark a threshold as notified
    pub fn mark_threshold_notified(&mut self, limit_type: LimitType, threshold: u8) {
        let state = match limit_type {
            LimitType::FiveHour => &mut self.five_hour,
            LimitType::SevenDay => &mut self.seven_day,
        };
        state.notified_thresholds.insert(threshold);
    }

    /// Check if a threshold has been notified
    pub fn is_threshold_notified(&self, limit_type: LimitType, threshold: u8) -> bool {
        let state = match limit_type {
            LimitType::FiveHour => &self.five_hour,
            LimitType::SevenDay => &self.seven_day,
        };
        state.notified_thresholds.contains(&threshold)
    }

    /// Mark that an overage warning was sent
    pub fn mark_overage_warned(&mut self, limit_type: LimitType) {
        let state = match limit_type {
            LimitType::FiveHour => &mut self.five_hour,
            LimitType::SevenDay => &mut self.seven_day,
        };
        state.last_overage_warning = Some(Utc::now());
    }

    /// Check if we should send an overage warning (not sent in last hour)
    pub fn should_warn_overage(&self, limit_type: LimitType) -> bool {
        let state = match limit_type {
            LimitType::FiveHour => &self.five_hour,
            LimitType::SevenDay => &self.seven_day,
        };

        match state.last_overage_warning {
            None => true,
            Some(last) => {
                let elapsed = Utc::now().signed_duration_since(last);
                elapsed.num_hours() >= 1
            }
        }
    }

    /// Mark that a reset notification was sent
    pub fn mark_reset_notified(&mut self, limit_type: LimitType) {
        let state = match limit_type {
            LimitType::FiveHour => &mut self.five_hour,
            LimitType::SevenDay => &mut self.seven_day,
        };
        state.notified_reset = true;
    }

    /// Check if reset notification has been sent
    pub fn is_reset_notified(&self, limit_type: LimitType) -> bool {
        let state = match limit_type {
            LimitType::FiveHour => &self.five_hour,
            LimitType::SevenDay => &self.seven_day,
        };
        state.notified_reset
    }

    /// Mark that an upcoming reset notification was sent
    pub fn mark_upcoming_reset_notified(&mut self, limit_type: LimitType) {
        let state = match limit_type {
            LimitType::FiveHour => &mut self.five_hour,
            LimitType::SevenDay => &mut self.seven_day,
        };
        state.notified_upcoming_reset = true;
    }

    /// Check if upcoming reset notification has been sent
    pub fn is_upcoming_reset_notified(&self, limit_type: LimitType) -> bool {
        let state = match limit_type {
            LimitType::FiveHour => &self.five_hour,
            LimitType::SevenDay => &self.seven_day,
        };
        state.notified_upcoming_reset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitType {
    FiveHour,
    SevenDay,
}

impl LimitType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LimitType::FiveHour => "5-hour",
            LimitType::SevenDay => "7-day",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_reset_detection_first_run() {
        let mut state = MonitorState::default();
        let future_reset = Utc::now() + Duration::hours(5);

        // First run should not trigger reset notification
        let reset_occurred = state.check_and_handle_reset(LimitType::FiveHour, future_reset);
        assert!(!reset_occurred, "First run should not trigger reset");
        assert_eq!(state.five_hour.current_reset_at, Some(future_reset));
        assert!(!state.five_hour.notified_reset, "Should not be marked as notified");
    }

    #[test]
    fn test_reset_detection_time_passed() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set up initial state with reset time in the past
        let past_reset = now - Duration::hours(1);
        state.five_hour.current_reset_at = Some(past_reset);
        state.five_hour.notified_reset = false;

        // New reset time from API (next reset)
        let next_reset = now + Duration::hours(4);

        // Should detect reset
        let reset_occurred = state.check_and_handle_reset(LimitType::FiveHour, next_reset);
        assert!(reset_occurred, "Should detect reset when time has passed");
        assert_eq!(state.five_hour.current_reset_at, Some(next_reset));
        assert!(state.five_hour.notified_thresholds.is_empty(), "Thresholds should be cleared");
        assert!(state.five_hour.last_overage_warning.is_none(), "Overage warning should be cleared");
        assert!(!state.five_hour.notified_upcoming_reset, "Upcoming reset flag should be cleared");
    }

    #[test]
    fn test_reset_detection_no_false_positive() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set up initial state with future reset time
        let future_reset = now + Duration::hours(3);
        state.five_hour.current_reset_at = Some(future_reset);
        state.five_hour.notified_reset = false;

        // API returns slightly different reset time (e.g., API adjusted by 30 seconds)
        let adjusted_reset = future_reset + Duration::seconds(30);

        // Should NOT detect reset (time hasn't passed yet)
        let reset_occurred = state.check_and_handle_reset(LimitType::FiveHour, adjusted_reset);
        assert!(!reset_occurred, "Should not falsely detect reset for small timestamp changes");
        assert_eq!(state.five_hour.current_reset_at, Some(adjusted_reset), "Should update to new timestamp");
    }

    #[test]
    fn test_reset_detection_already_notified() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set up state where reset time has passed but we already notified
        let past_reset = now - Duration::minutes(30);
        state.five_hour.current_reset_at = Some(past_reset);
        state.five_hour.notified_reset = true; // Already notified

        let next_reset = now + Duration::hours(4);

        // Should NOT trigger notification again
        let reset_occurred = state.check_and_handle_reset(LimitType::FiveHour, next_reset);
        assert!(!reset_occurred, "Should not trigger reset if already notified");
    }

    #[test]
    fn test_reset_detection_multiple_resets_missed() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Service was offline for 15 hours, missing 3 resets
        let very_old_reset = now - Duration::hours(15);
        state.five_hour.current_reset_at = Some(very_old_reset);
        state.five_hour.notified_reset = false;

        // API returns the next upcoming reset
        let next_reset = now + Duration::hours(2);

        // Should detect reset (doesn't matter how many were missed, just notify once)
        let reset_occurred = state.check_and_handle_reset(LimitType::FiveHour, next_reset);
        assert!(reset_occurred, "Should detect reset even if multiple resets were missed");
        assert_eq!(state.five_hour.current_reset_at, Some(next_reset));
    }

    #[test]
    fn test_reset_clears_notification_state() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set up state with various notifications sent
        state.five_hour.current_reset_at = Some(now - Duration::hours(1));
        state.five_hour.notified_thresholds.insert(50);
        state.five_hour.notified_thresholds.insert(70);
        state.five_hour.last_overage_warning = Some(now - Duration::minutes(30));
        state.five_hour.notified_upcoming_reset = true;

        let next_reset = now + Duration::hours(4);

        // Trigger reset
        let reset_occurred = state.check_and_handle_reset(LimitType::FiveHour, next_reset);
        assert!(reset_occurred);

        // Verify all notification state was cleared
        assert!(state.five_hour.notified_thresholds.is_empty(), "Thresholds should be cleared");
        assert!(state.five_hour.last_overage_warning.is_none(), "Overage warning should be cleared");
        assert!(!state.five_hour.notified_upcoming_reset, "Upcoming reset flag should be cleared");
        assert!(!state.five_hour.notified_reset, "Reset notification flag should be cleared");
    }

    #[test]
    fn test_state_validation_stale_five_hour() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set very old reset time (12 hours ago, > 10 hour threshold)
        state.five_hour.current_reset_at = Some(now - Duration::hours(12));
        state.five_hour.notified_thresholds.insert(50);
        state.five_hour.notified_thresholds.insert(70);

        // Validate and clean
        state.validate_and_clean();

        // Should have reset the state
        assert!(state.five_hour.current_reset_at.is_none(), "Very old reset time should be cleared");
        assert!(state.five_hour.notified_thresholds.is_empty(), "Thresholds should be cleared");
    }

    #[test]
    fn test_state_validation_stale_seven_day() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set very old reset time (20 days ago, > 14 day threshold)
        state.seven_day.current_reset_at = Some(now - Duration::days(20));
        state.seven_day.notified_thresholds.insert(90);

        // Validate and clean
        state.validate_and_clean();

        // Should have reset the state
        assert!(state.seven_day.current_reset_at.is_none(), "Very old reset time should be cleared");
        assert!(state.seven_day.notified_thresholds.is_empty(), "Thresholds should be cleared");
    }

    #[test]
    fn test_state_validation_recent_timestamps_preserved() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set recent reset times (within valid range)
        let recent_five_hour = now + Duration::hours(2);
        let recent_seven_day = now + Duration::days(3);

        state.five_hour.current_reset_at = Some(recent_five_hour);
        state.seven_day.current_reset_at = Some(recent_seven_day);

        // Validate and clean
        state.validate_and_clean();

        // Should preserve recent timestamps
        assert_eq!(state.five_hour.current_reset_at, Some(recent_five_hour));
        assert_eq!(state.seven_day.current_reset_at, Some(recent_seven_day));
    }

    #[test]
    fn test_state_validation_clears_stale_overage_warnings() {
        let mut state = MonitorState::default();
        let now = Utc::now();

        // Set old overage warning (8 days ago for 5-hour, 35 days ago for 7-day)
        state.five_hour.last_overage_warning = Some(now - Duration::days(8));
        state.seven_day.last_overage_warning = Some(now - Duration::days(35));

        // Validate and clean
        state.validate_and_clean();

        // Should clear stale warnings
        assert!(state.five_hour.last_overage_warning.is_none(), "Stale 5-hour overage warning should be cleared");
        assert!(state.seven_day.last_overage_warning.is_none(), "Stale 7-day overage warning should be cleared");
    }

    #[test]
    fn test_threshold_notification_tracking() {
        let mut state = MonitorState::default();

        // Mark thresholds as notified
        state.mark_threshold_notified(LimitType::FiveHour, 50);
        state.mark_threshold_notified(LimitType::FiveHour, 70);

        // Check tracking
        assert!(state.is_threshold_notified(LimitType::FiveHour, 50));
        assert!(state.is_threshold_notified(LimitType::FiveHour, 70));
        assert!(!state.is_threshold_notified(LimitType::FiveHour, 90));
    }

    #[test]
    fn test_overage_warning_cooldown() {
        let mut state = MonitorState::default();

        // Initially should allow warning
        assert!(state.should_warn_overage(LimitType::FiveHour));

        // Mark as warned
        state.mark_overage_warned(LimitType::FiveHour);

        // Should not allow warning immediately after
        assert!(!state.should_warn_overage(LimitType::FiveHour));

        // Manually set to 61 minutes ago (should allow warning again)
        state.five_hour.last_overage_warning = Some(Utc::now() - Duration::minutes(61));
        assert!(state.should_warn_overage(LimitType::FiveHour));
    }

    #[test]
    fn test_reset_notification_tracking() {
        let mut state = MonitorState::default();

        // Initially not notified
        assert!(!state.is_reset_notified(LimitType::FiveHour));

        // Mark as notified
        state.mark_reset_notified(LimitType::FiveHour);

        // Should be marked
        assert!(state.is_reset_notified(LimitType::FiveHour));

        // Seven-day should be independent
        assert!(!state.is_reset_notified(LimitType::SevenDay));
    }

    #[test]
    fn test_upcoming_reset_notification_tracking() {
        let mut state = MonitorState::default();

        // Initially not notified
        assert!(!state.is_upcoming_reset_notified(LimitType::SevenDay));

        // Mark as notified
        state.mark_upcoming_reset_notified(LimitType::SevenDay);

        // Should be marked
        assert!(state.is_upcoming_reset_notified(LimitType::SevenDay));

        // Five-hour should be independent
        assert!(!state.is_upcoming_reset_notified(LimitType::FiveHour));
    }
}
