use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub thresholds: ThresholdsConfig,
    pub notifications: NotificationsConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub poll_interval_seconds: u64,
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
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub session_file: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let config_dir = dirs::config_dir()
            .expect("Failed to get config directory")
            .join("claude-notify");

        Self {
            general: GeneralConfig {
                poll_interval_seconds: 900, // 15 minutes
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
            },
            auth: AuthConfig {
                session_file: config_dir.join("session.json"),
            },
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
}
