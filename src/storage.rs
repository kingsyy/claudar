use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub org_id: String,
    pub session_key: String,
    pub cf_clearance: Option<String>,
    pub last_active_org: Option<String>,
    pub anthropic_device_id: Option<String>,
    pub cf_bm: Option<String>,
    pub ssid: Option<String>,
    // Store full cookie string as fallback
    pub full_cookie_string: Option<String>,
}

impl SessionData {
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            anyhow::bail!("Session file not found. Please run 'claude-notify setup' first.");
        }

        let content = std::fs::read_to_string(path)?;
        let session: SessionData = serde_json::from_str(&content)?;
        Ok(session)
    }

    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Build the full cookie string for HTTP requests
    pub fn cookie_string(&self) -> String {
        // If we have a full cookie string, use it
        if let Some(ref full) = self.full_cookie_string {
            return full.clone();
        }

        // Otherwise, build from individual cookies
        let mut cookies = vec![format!("sessionKey={}", self.session_key)];

        if let Some(ref cf) = self.cf_clearance {
            cookies.push(format!("cf_clearance={}", cf));
        }
        if let Some(ref org) = self.last_active_org {
            cookies.push(format!("lastActiveOrg={}", org));
        }
        if let Some(ref device) = self.anthropic_device_id {
            cookies.push(format!("anthropic-device-id={}", device));
        }
        if let Some(ref bm) = self.cf_bm {
            cookies.push(format!("__cf_bm={}", bm));
        }
        if let Some(ref ssid) = self.ssid {
            cookies.push(format!("__ssid={}", ssid));
        }

        cookies.join("; ")
    }
}
