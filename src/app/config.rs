use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub recent_images: Vec<String>,
    pub window_width: i32,
    pub window_height: i32,
    pub last_erase_flash: bool,
    pub last_erase_bootloader: bool,
    pub last_reset_after_burn: bool,
    pub last_parts: String,
    pub last_soc: String,
    pub last_password_path: String,
    pub last_efuse_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            recent_images: Vec::new(),
            window_width: 960,
            window_height: 720,
            last_erase_flash: false,
            last_erase_bootloader: false,
            last_reset_after_burn: true,
            last_parts: "all".into(),
            last_soc: "gxl".into(),
            last_password_path: String::new(),
            last_efuse_path: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, s);
        }
    }

    pub fn add_recent_image(&mut self, path: &str) {
        self.recent_images.retain(|p| p != path);
        self.recent_images.insert(0, path.into());
        self.recent_images.truncate(10);
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    base.join("aml-flash-tool").join("config.json")
}
