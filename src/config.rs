use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub thresholds: ThresholdsConfig,
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub instances: Vec<InstanceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub poll_interval_seconds: u64,
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

fn default_timezone() -> String {
    "local".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdsConfig {
    pub five_hour: Vec<u8>,
    pub seven_day: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub sound: bool,
    pub persistent: bool,
    #[serde(default = "default_true")]
    pub notify_threshold_crossings: bool,
    #[serde(default = "default_true")]
    pub notify_predicted_overage: bool,
    #[serde(default = "default_true")]
    pub notify_resets: bool,
    /// Minutes before 5-hour limit reset to send notification (None = disabled)
    #[serde(default)]
    pub minutes_before_five_hour_reset: Option<u64>,
    /// Minutes before 7-day limit reset to send notification (None = disabled)
    #[serde(default)]
    pub minutes_before_seven_day_reset: Option<u64>,
    /// Warn if N minutes remain with M% capacity left for 5-hour limit (None = disabled)
    /// Format: (minutes_remaining, min_capacity_percentage)
    #[serde(default)]
    pub capacity_warning_five_hour: Option<(u64, u8)>,
    /// Warn if N minutes remain with M% capacity left for 7-day limit (None = disabled)
    /// Format: (minutes_remaining, min_capacity_percentage)
    #[serde(default)]
    pub capacity_warning_seven_day: Option<(u64, u8)>,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                poll_interval_seconds: 900, // 15 minutes
                timezone: "local".to_string(),
            },
            thresholds: ThresholdsConfig {
                five_hour: vec![50, 70, 90],
                seven_day: vec![50, 70, 90],
            },
            notifications: NotificationsConfig {
                sound: true,
                persistent: false,
                notify_threshold_crossings: true,
                notify_predicted_overage: true,
                notify_resets: true,
                minutes_before_five_hour_reset: None,
                minutes_before_seven_day_reset: None,
                capacity_warning_five_hour: None,
                capacity_warning_seven_day: None,
            },
            instances: Vec::new(),
        }
    }
}

impl Config {
    pub fn config_dir() -> anyhow::Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?
            .join("claude-notify");
        Ok(dir)
    }

    pub fn config_path() -> anyhow::Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;

        let path = Self::config_path()?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Returns the configured instances, or a single "default" instance if none configured.
    pub fn effective_instances(&self) -> Vec<InstanceConfig> {
        if self.instances.is_empty() {
            vec![InstanceConfig { name: "default".to_string() }]
        } else {
            self.instances.clone()
        }
    }

    /// Whether instances are explicitly configured (vs. using implicit "default").
    pub fn has_instances(&self) -> bool {
        !self.instances.is_empty()
    }

    /// Get the session file path for a given instance name.
    pub fn session_path_for(&self, name: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::config_dir()?;
        let sessions_dir = dir.join("sessions");
        Ok(sessions_dir.join(format!("{}.json", name)))
    }

    /// Get the state file path for a given instance name.
    pub fn state_path_for(&self, name: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::config_dir()?;
        let state_dir = dir.join("state");
        Ok(state_dir.join(format!("{}.json", name)))
    }

    /// Get the Chrome profile directory for a given instance name.
    ///
    /// Using a fixed path (rather than the random temp dirs headless_chrome creates by default)
    /// prevents profile directories from accumulating when the process crashes or is killed
    /// before Drop can run.
    pub fn chrome_profile_path_for(&self, name: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::config_dir()?;
        Ok(dir.join("chrome-profiles").join(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.general.poll_interval_seconds, 900);
        assert_eq!(config.general.timezone, "local");
        assert_eq!(config.thresholds.five_hour, vec![50, 70, 90]);
        assert_eq!(config.thresholds.seven_day, vec![50, 70, 90]);
        assert!(config.notifications.sound);
        assert!(!config.notifications.persistent);
        assert!(config.notifications.notify_threshold_crossings);
        assert!(config.notifications.notify_predicted_overage);
        assert!(config.notifications.notify_resets);
        assert!(config.notifications.minutes_before_five_hour_reset.is_none());
        assert!(config.notifications.capacity_warning_five_hour.is_none());
        assert!(config.instances.is_empty());
    }

    #[test]
    fn effective_instances_empty_returns_default() {
        let config = Config::default();
        let instances = config.effective_instances();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].name, "default");
    }

    #[test]
    fn effective_instances_returns_configured() {
        let mut config = Config::default();
        config.instances = vec![
            InstanceConfig { name: "personal".to_string() },
            InstanceConfig { name: "work".to_string() },
        ];
        let instances = config.effective_instances();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].name, "personal");
        assert_eq!(instances[1].name, "work");
    }

    #[test]
    fn has_instances_false_when_empty() {
        let config = Config::default();
        assert!(!config.has_instances());
    }

    #[test]
    fn has_instances_true_when_configured() {
        let mut config = Config::default();
        config.instances = vec![InstanceConfig { name: "test".to_string() }];
        assert!(config.has_instances());
    }

    #[test]
    fn toml_round_trip() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.general.poll_interval_seconds, config.general.poll_interval_seconds);
        assert_eq!(deserialized.general.timezone, config.general.timezone);
        assert_eq!(deserialized.thresholds.five_hour, config.thresholds.five_hour);
        assert_eq!(deserialized.notifications.sound, config.notifications.sound);
    }

    #[test]
    fn toml_round_trip_with_instances() {
        let mut config = Config::default();
        config.instances = vec![
            InstanceConfig { name: "personal".to_string() },
            InstanceConfig { name: "work".to_string() },
        ];
        config.notifications.minutes_before_five_hour_reset = Some(15);
        config.notifications.capacity_warning_five_hour = Some((30, 20));

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.instances.len(), 2);
        assert_eq!(deserialized.instances[0].name, "personal");
        assert_eq!(deserialized.notifications.minutes_before_five_hour_reset, Some(15));
        assert_eq!(deserialized.notifications.capacity_warning_five_hour, Some((30, 20)));
    }
}
