use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum BurnStatus {
    #[allow(dead_code)]
    Disconnected,
    Ready,
    Burning(f64),
    Success,
    Error(String),
}

impl BurnStatus {
    #[allow(dead_code)]
    pub fn display(&self) -> String {
        match self {
            BurnStatus::Disconnected => "Disconnected".into(),
            BurnStatus::Ready => "Ready".into(),
            BurnStatus::Burning(pct) => format!("Burning {:.0}%", pct * 100.0),
            BurnStatus::Success => "Success".into(),
            BurnStatus::Error(e) => format!("Error: {e}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceState {
    pub hub_id: String,
    #[allow(dead_code)]
    pub serial: String,
    pub status: BurnStatus,
    pub cancel_tx: Option<mpsc::Sender<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnConfig {
    pub image_path: Option<PathBuf>,
    pub parts: String,
    pub wipe: bool,
    pub reset: bool,
    pub erase_bootloader: bool,
    pub soc: String,
    pub password_path: Option<PathBuf>,
    pub efuse_path: Option<PathBuf>,
    pub adnl: bool,
}

impl Default for BurnConfig {
    fn default() -> Self {
        Self {
            image_path: None,
            parts: "all".into(),
            wipe: false,
            reset: true,
            erase_bootloader: false,
            soc: "gxl".into(),
            password_path: None,
            efuse_path: None,
            adnl: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub name: String,
    #[allow(dead_code)]
    pub file: String,
    #[allow(dead_code)]
    pub file_type: String,
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub partitions: Vec<PartitionInfo>,
    #[allow(dead_code)]
    pub tmp_dir: PathBuf,
    pub soc: String,
    #[allow(dead_code)]
    pub secured: bool,
}
