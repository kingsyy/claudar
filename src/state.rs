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
        let state: MonitorState = serde_json::from_str(&content)?;
        Ok(state)
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
        // Set the reset time and mark as already notified to prevent false notifications
        if state.current_reset_at.is_none() {
            state.current_reset_at = Some(new_reset_at);
            state.notified_reset = true;
            return false;
        }

        // Check if the reset time has changed (indicating a reset occurred)
        // Use a tolerance of 1 second to handle minor timestamp variations
        let current = state.current_reset_at.unwrap();
        let time_diff = (new_reset_at.timestamp() - current.timestamp()).abs();

        if time_diff > 1 {
            // Reset occurred - clear notification state
            state.current_reset_at = Some(new_reset_at);
            state.notified_thresholds.clear();
            state.last_overage_warning = None;
            state.notified_reset = false;
            state.notified_upcoming_reset = false;
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
