use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub platform: String,
    pub notes: String,
    pub filename: String,
    pub sha256: String,
    pub size: u64,
    pub published_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMeta {
    pub name: String,
    pub description: String,
}

impl AppEntry {
    pub fn latest(&self, platform: &str) -> Option<&Release> {
        self.releases
            .iter()
            .filter(|r| r.platform == platform)
            .max_by_key(|r| parse_version(&r.version))
    }
}

pub fn parse_version(v: &str) -> Version {
    Version::parse(v.trim_start_matches('v')).unwrap_or_else(|_| Version::new(0, 0, 0))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

pub fn current_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn is_valid_version(v: &str) -> bool {
    Version::parse(v).is_ok()
}
