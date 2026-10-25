use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    #[serde(default)]
    pub name: String,
    pub version: String,
    pub dir: PathBuf,
    pub exe: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub server_url: String,
    #[serde(default)]
    pub apps: BTreeMap<String, Installed>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            server_url: std::env::var("UPDATER_SERVER").unwrap_or_else(|_| "http://localhost:8080".into()),
            apps: BTreeMap::new(),
        }
    }
}

impl Store {
    fn path() -> PathBuf {
        dirs::config_dir().unwrap_or_default().join("AppUpdater").join("state.json")
    }

    pub fn load() -> Self {
        fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn home_dir() -> PathBuf {
        dirs::data_local_dir().unwrap_or_default().join("AppUpdater")
    }

    pub fn install_root(&self) -> PathBuf {
        Self::home_dir().join("apps")
    }

    pub fn register(&mut self, id: &str, version: &str, exe: PathBuf) {
        let dir = exe.parent().map(PathBuf::from).unwrap_or_default();
        self.apps.insert(id.into(), Installed { name: id.into(), version: version.into(), dir, exe: Some(exe) });
    }
}
