use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub out_dir: PathBuf,
    pub workers: u32,
    pub default_originals: bool,
    pub default_exts: String,
    /// "system" (segue Windows), "light" o "dark".
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        let downloads = dirs::download_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"));
        Self { out_dir: downloads.join("archive"), workers: 3, default_originals: true, default_exts: String::new(), theme: "system".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum JobStatus {
    Queued,
    Downloading,
    Retrying { attempt: u32, wait_s: u64 },
    Paused,
    Done,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub item_id: String,
    pub name: String,
    pub size: u64,
    pub dest: PathBuf,
    pub status: JobStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewFile {
    pub item_id: String,
    pub name: String,
    pub size: u64,
}
