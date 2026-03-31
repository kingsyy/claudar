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

    /// Convert session cookies to name-value pairs for browser injection
    pub fn cookie_pairs(&self) -> Vec<(String, String)> {
        let mut cookies = vec![("sessionKey".to_string(), self.session_key.clone())];

        if let Some(ref cf) = self.cf_clearance {
            cookies.push(("cf_clearance".to_string(), cf.clone()));
        }
        if let Some(ref org) = self.last_active_org {
            cookies.push(("lastActiveOrg".to_string(), org.clone()));
        }
        if let Some(ref device) = self.anthropic_device_id {
            cookies.push(("anthropic-device-id".to_string(), device.clone()));
        }
        if let Some(ref bm) = self.cf_bm {
            cookies.push(("__cf_bm".to_string(), bm.clone()));
        }
        if let Some(ref ssid) = self.ssid {
            cookies.push(("__ssid".to_string(), ssid.clone()));
        }

        cookies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(all_fields: bool) -> SessionData {
        SessionData {
            org_id: "org-123".to_string(),
            session_key: "sk-abc".to_string(),
            cf_clearance: if all_fields { Some("cf-clear".to_string()) } else { None },
            last_active_org: if all_fields { Some("org-456".to_string()) } else { None },
            anthropic_device_id: if all_fields { Some("dev-789".to_string()) } else { None },
            cf_bm: if all_fields { Some("bm-val".to_string()) } else { None },
            ssid: if all_fields { Some("ssid-val".to_string()) } else { None },
            full_cookie_string: None,
        }
    }

    #[test]
    fn cookie_pairs_required_only() {
        let session = make_session(false);
        let pairs = session.cookie_pairs();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("sessionKey".to_string(), "sk-abc".to_string()));
    }

    #[test]
    fn cookie_pairs_all_fields() {
        let session = make_session(true);
        let pairs = session.cookie_pairs();
        assert_eq!(pairs.len(), 6);

        let names: Vec<&str> = pairs.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"sessionKey"));
        assert!(names.contains(&"cf_clearance"));
        assert!(names.contains(&"lastActiveOrg"));
        assert!(names.contains(&"anthropic-device-id"));
        assert!(names.contains(&"__cf_bm"));
        assert!(names.contains(&"__ssid"));
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-session.json");

        let session = make_session(true);
        session.save(&path.to_path_buf()).unwrap();

        let loaded = SessionData::load(&path.to_path_buf()).unwrap();
        assert_eq!(loaded.org_id, "org-123");
        assert_eq!(loaded.session_key, "sk-abc");
        assert_eq!(loaded.cf_clearance, Some("cf-clear".to_string()));
        assert_eq!(loaded.anthropic_device_id, Some("dev-789".to_string()));
    }

    #[test]
    fn load_nonexistent_file_errors() {
        let path = PathBuf::from("/tmp/nonexistent-claude-notify-test.json");
        assert!(SessionData::load(&path).is_err());
    }
}
