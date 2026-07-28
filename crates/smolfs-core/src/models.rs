use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub backend: String,
    pub metadata_url: String,
    pub storage: String,
    pub bucket: String,
    pub dev: bool,
    pub mountpoint: Option<PathBuf>,
    #[serde(default)]
    pub read_only: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct InitVolume {
    pub name: String,
    pub dev: bool,
    pub metadata_url: Option<String>,
    pub store_url: Option<String>,
    pub storage: Option<String>,
    pub bucket: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MountVolume {
    pub name: String,
    pub path: PathBuf,
    pub foreground: bool,
    pub check_storage: bool,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub name: String,
    pub mountpoint: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub metadata_url: String,
    pub storage: String,
    pub bucket: String,
    pub dev: bool,
    pub mountpoint: Option<PathBuf>,
    pub read_only: bool,
}

impl From<&Volume> for VolumeInfo {
    fn from(volume: &Volume) -> Self {
        Self {
            name: volume.name.clone(),
            metadata_url: volume.metadata_url.clone(),
            storage: volume.storage.clone(),
            bucket: volume.bucket.clone(),
            dev: volume.dev,
            mountpoint: volume.mountpoint.clone(),
            read_only: volume.read_only,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub home: PathBuf,
    pub config: PathBuf,
    pub storage_backend: BinaryReport,
    pub mount_support: MountSupportReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryReport {
    pub found: bool,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountSupportReport {
    pub found: bool,
    pub detail: String,
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub volumes: Vec<VolumeInfo>,
}

#[cfg(test)]
mod tests {
    use super::{Volume, VolumeInfo};

    fn volume(read_only: bool) -> Volume {
        Volume {
            name: "demo".into(),
            backend: "juicefs".into(),
            metadata_url: "sqlite3://demo.db".into(),
            storage: "file".into(),
            bucket: "/tmp/objects".into(),
            dev: true,
            mountpoint: None,
            read_only,
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    #[test]
    fn volume_info_carries_read_only() {
        assert!(VolumeInfo::from(&volume(true)).read_only);
        assert!(!VolumeInfo::from(&volume(false)).read_only);
    }
}
