use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub thresholds: ThresholdsConfig,
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub instances: Vec<InstanceConfig>,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub web: WebConfig,
}

/// Optional read-only HTTP dashboard, e.g. for checking usage from a phone over Tailscale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    /// Whether to serve the dashboard (default: false — opt-in).
    #[serde(default)]
    pub enabled: bool,

    /// Address to bind to. Defaults to loopback-only; set this to a Tailscale IP
    /// (e.g. `tailscale ip -4`) to expose it on that network. Never defaults to
    /// `0.0.0.0` — exposure must be a deliberate choice.
    #[serde(default = "default_web_bind")]
    pub bind: String,

    #[serde(default = "default_web_port")]
    pub port: u16,

    /// Whether to also serve `/api/agent`, a minimal machine-readable endpoint
    /// meant for other agents/tools to poll (default: false — opt-in, and only
    /// takes effect while `enabled` is also true).
    #[serde(default)]
    pub agent_api_enabled: bool,
}

fn default_web_bind() -> String {
    "127.0.0.1".to_string()
}

fn default_web_port() -> u16 {
    4317
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_web_bind(),
            port: default_web_port(),
            agent_api_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Whether to record a usage snapshot after each successful poll (default: false)
    #[serde(default)]
    pub enabled: bool,

    /// Maximum number of records to retain per instance (default: 2016 ≈ 14 days at 15-min polls)
    #[serde(default = "default_history_retention")]
    pub max_records: usize,
}

fn default_history_retention() -> usize {
    2016
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_records: default_history_retention(),
        }
    }
}

/// Which service an instance monitors. Serialised in kebab-case (`claude-web`,
/// `openai-web`) so `config.toml` stays readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    /// Claude.ai via session cookies — the original and only provider before 0.4.6.
    #[default]
    ClaudeWeb,
    /// ChatGPT via a session cookie exchanged for a bearer token.
    OpenaiWeb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub name: String,
    /// Absent in configs written before multi-provider support; those instances
    /// are all Claude, which is what `Provider::default()` yields.
    #[serde(default)]
    pub provider: Provider,
}

impl InstanceConfig {
    /// A Claude instance — the shape every call site wanted before providers existed.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), provider: Provider::ClaudeWeb }
    }

    pub fn with_provider(name: impl Into<String>, provider: Provider) -> Self {
        Self { name: name.into(), provider }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub poll_interval_seconds: u64,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Show a menu-bar / system-tray icon with at-a-glance usage (default: false).
    #[serde(default)]
    pub show_tray_icon: bool,
    /// When launched at login (autostart), start hidden in the background instead
    /// of opening the window. Has no effect on manual launches (default: false).
    #[serde(default)]
    pub start_minimized: bool,
    /// Show each usage bar's pace delta (usage% minus time-elapsed%) alongside
    /// the existing now/peak marks (default: false).
    #[serde(default)]
    pub show_pace_delta: bool,
}

fn default_timezone() -> String {
    "local".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdsConfig {
    pub five_hour: Vec<u8>,
    pub seven_day: Vec<u8>,
}

impl ThresholdsConfig {
    /// Validate a single limit's threshold list: 1-5 entries, each 0-100,
    /// strictly increasing (i.e. sorted ascending with no duplicates).
    fn validate_list(label: &str, values: &[u8]) -> Result<(), String> {
        if values.is_empty() {
            return Err(format!("{label} thresholds: at least 1 threshold is required"));
        }
        if values.len() > 5 {
            return Err(format!("{label} thresholds: at most 5 thresholds are allowed"));
        }
        if values.iter().any(|&v| v > 100) {
            return Err(format!("{label} thresholds: values must be between 0 and 100"));
        }
        if values.windows(2).any(|w| w[0] >= w[1]) {
            return Err(format!(
                "{label} thresholds: values must be sorted ascending with no duplicates"
            ));
        }
        Ok(())
    }

    /// Validate both limits' threshold lists.
    pub fn validate(&self) -> Result<(), String> {
        Self::validate_list("5-hour", &self.five_hour)?;
        Self::validate_list("7-day", &self.seven_day)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub sound: bool,
    /// Name of the system sound to play (e.g. "Glass", "Ping")
    #[serde(default = "default_sound_name")]
    pub sound_name: String,
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

fn default_sound_name() -> String {
    "Glass".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                poll_interval_seconds: 900, // 15 minutes
                timezone: "local".to_string(),
                show_tray_icon: false,
                start_minimized: false,
                show_pace_delta: false,
            },
            thresholds: ThresholdsConfig {
                five_hour: vec![50, 75, 90, 100],
                seven_day: vec![50, 75, 90, 100],
            },
            notifications: NotificationsConfig {
                sound: true,
                sound_name: default_sound_name(),
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
            history: HistoryConfig::default(),
            web: WebConfig::default(),
        }
    }
}

impl Config {
    pub fn config_dir() -> anyhow::Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?
            .join("claudar");
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
            vec![InstanceConfig::new("default")]
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

    /// Get the history file path for a given instance name.
    pub fn history_path_for(&self, name: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::config_dir()?;
        Ok(dir.join("history").join(format!("{}.jsonl", name)))
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
    fn thresholds_validate_default_config() {
        assert!(Config::default().thresholds.validate().is_ok());
    }

    #[test]
    fn thresholds_validate_rejects_empty_list() {
        let thresholds = ThresholdsConfig { five_hour: vec![], seven_day: vec![50] };
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn thresholds_validate_rejects_more_than_five() {
        let thresholds = ThresholdsConfig { five_hour: vec![10, 20, 30, 40, 50, 60], seven_day: vec![50] };
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn thresholds_validate_accepts_exactly_five() {
        let thresholds = ThresholdsConfig { five_hour: vec![10, 20, 30, 40, 50], seven_day: vec![50] };
        assert!(thresholds.validate().is_ok());
    }

    #[test]
    fn thresholds_validate_rejects_value_over_100() {
        let thresholds = ThresholdsConfig { five_hour: vec![50, 101], seven_day: vec![50] };
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn thresholds_validate_rejects_unsorted() {
        let thresholds = ThresholdsConfig { five_hour: vec![90, 50], seven_day: vec![50] };
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn thresholds_validate_rejects_duplicates() {
        let thresholds = ThresholdsConfig { five_hour: vec![50, 50, 90], seven_day: vec![50] };
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn thresholds_validate_accepts_single_value() {
        let thresholds = ThresholdsConfig { five_hour: vec![100], seven_day: vec![0] };
        assert!(thresholds.validate().is_ok());
    }

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.general.poll_interval_seconds, 900);
        assert_eq!(config.general.timezone, "local");
        assert_eq!(config.thresholds.five_hour, vec![50, 75, 90, 100]);
        assert_eq!(config.thresholds.seven_day, vec![50, 75, 90, 100]);
        assert!(config.notifications.sound);
        assert_eq!(config.notifications.sound_name, "Glass");
        assert!(!config.notifications.persistent);
        assert!(config.notifications.notify_threshold_crossings);
        assert!(config.notifications.notify_predicted_overage);
        assert!(config.notifications.notify_resets);
        assert!(config.notifications.minutes_before_five_hour_reset.is_none());
        assert!(config.notifications.capacity_warning_five_hour.is_none());
        assert!(config.instances.is_empty());
        assert!(!config.history.enabled);
        assert_eq!(config.history.max_records, 2016);
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
            InstanceConfig::new("personal"),
            InstanceConfig::new("work"),
        ];
        let instances = config.effective_instances();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].name, "personal");
        assert_eq!(instances[1].name, "work");
    }

    /// Configs written before providers existed have no `provider` key; loading
    /// one must not fail and must not silently repoint the account at ChatGPT.
    #[test]
    fn instance_without_provider_key_loads_as_claude() {
        let toml = r#"
            [general]
            poll_interval_seconds = 900
            [thresholds]
            five_hour = [50, 90]
            seven_day = [50, 90]
            [notifications]
            sound = true
            persistent = false
            [[instances]]
            name = "personal"
        "#;
        let config: Config = toml::from_str(toml).expect("pre-provider config must still load");
        assert_eq!(config.instances[0].provider, Provider::ClaudeWeb);
    }

    #[test]
    fn provider_round_trips_as_kebab_case() {
        let instance = InstanceConfig::with_provider("chatgpt", Provider::OpenaiWeb);
        let encoded = toml::to_string(&instance).unwrap();
        assert!(encoded.contains(r#"provider = "openai-web""#), "got: {encoded}");
        let decoded: InstanceConfig = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.provider, Provider::OpenaiWeb);
    }

    #[test]
    fn has_instances_false_when_empty() {
        let config = Config::default();
        assert!(!config.has_instances());
    }

    #[test]
    fn has_instances_true_when_configured() {
        let mut config = Config::default();
        config.instances = vec![InstanceConfig::new("test")];
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
        assert_eq!(deserialized.notifications.sound_name, config.notifications.sound_name);
    }

    #[test]
    fn history_config_defaults() {
        let config = Config::default();
        assert!(!config.history.enabled);
        assert_eq!(config.history.max_records, 2016);
    }

    #[test]
    fn history_config_round_trip() {
        let mut config = Config::default();
        config.history.enabled = true;
        config.history.max_records = 500;
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert!(deserialized.history.enabled);
        assert_eq!(deserialized.history.max_records, 500);
    }

    #[test]
    fn history_config_deserializes_without_section() {
        // Existing configs without [history] section should default correctly
        let toml_str = r#"
[general]
poll_interval_seconds = 900
timezone = "local"

[thresholds]
five_hour = [50, 75, 90, 100]
seven_day = [50, 75, 90, 100]

[notifications]
sound = true
persistent = false
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.history.enabled);
        assert_eq!(config.history.max_records, 2016);
    }

    #[test]
    fn toml_round_trip_with_instances() {
        let mut config = Config::default();
        config.instances = vec![
            InstanceConfig::new("personal"),
            InstanceConfig::new("work"),
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
