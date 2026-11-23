use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "VolumeRecord")]
pub struct Volume {
    pub name: String,
    pub backend: String,
    pub metadata_url: String,
    pub storage: String,
    pub bucket: String,
    pub dev: bool,
    pub created_at: String,
    pub updated_at: String,
    /// Every live mount of this volume tracked by this registry.
    pub mounts: Vec<MountRecord>,
}

impl Volume {
    pub fn mount_at(&self, mountpoint: &Path) -> Option<&MountRecord> {
        self.mounts
            .iter()
            .find(|mount| mount.mountpoint == mountpoint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountRecord {
    pub mountpoint: PathBuf,
    #[serde(default)]
    pub read_only: bool,
}

/// On-disk volume record. Records written before multiple mounts were tracked
/// store a single `mountpoint` and `read_only` pair instead of `mounts`.
#[derive(Deserialize)]
struct VolumeRecord {
    name: String,
    backend: String,
    metadata_url: String,
    storage: String,
    bucket: String,
    dev: bool,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    mounts: Vec<MountRecord>,
    #[serde(default)]
    mountpoint: Option<PathBuf>,
    #[serde(default)]
    read_only: bool,
}

impl From<VolumeRecord> for Volume {
    fn from(record: VolumeRecord) -> Self {
        let mut mounts = record.mounts;
        if mounts.is_empty() {
            mounts.extend(record.mountpoint.map(|mountpoint| MountRecord {
                mountpoint,
                read_only: record.read_only,
            }));
        }
        Self {
            name: record.name,
            backend: record.backend,
            metadata_url: record.metadata_url,
            storage: record.storage,
            bucket: record.bucket,
            dev: record.dev,
            created_at: record.created_at,
            updated_at: record.updated_at,
            mounts,
        }
    }
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
    /// The most recent mount, kept for callers that expect a single mount.
    /// Use `mounts` to see every mount.
    pub mountpoint: Option<PathBuf>,
    pub read_only: bool,
    pub mounts: Vec<MountRecord>,
}

impl From<&Volume> for VolumeInfo {
    fn from(volume: &Volume) -> Self {
        Self {
            name: volume.name.clone(),
            metadata_url: volume.metadata_url.clone(),
            storage: volume.storage.clone(),
            bucket: volume.bucket.clone(),
            dev: volume.dev,
            mountpoint: volume.mounts.last().map(|mount| mount.mountpoint.clone()),
            read_only: volume.mounts.last().is_some_and(|mount| mount.read_only),
            mounts: volume.mounts.clone(),
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
    use super::{MountRecord, Volume, VolumeInfo};

    fn volume(mounts: Vec<MountRecord>) -> Volume {
        Volume {
            name: "demo".into(),
            backend: "juicefs".into(),
            metadata_url: "sqlite3://demo.db".into(),
            storage: "file".into(),
            bucket: "/tmp/objects".into(),
            dev: true,
            created_at: "now".into(),
            updated_at: "now".into(),
            mounts,
        }
    }

    fn mount(path: &str, read_only: bool) -> MountRecord {
        MountRecord {
            mountpoint: path.into(),
            read_only,
        }
    }

    #[test]
    fn volume_info_carries_read_only() {
        assert!(VolumeInfo::from(&volume(vec![mount("/tmp/ro", true)])).read_only);
        assert!(!VolumeInfo::from(&volume(vec![mount("/tmp/rw", false)])).read_only);
        assert!(!VolumeInfo::from(&volume(Vec::new())).read_only);
    }

    #[test]
    fn volume_info_lists_every_mount() {
        let info = VolumeInfo::from(&volume(vec![
            mount("/tmp/ro", true),
            mount("/tmp/rw", false),
        ]));
        assert_eq!(
            info.mounts,
            vec![mount("/tmp/ro", true), mount("/tmp/rw", false)]
        );
        assert_eq!(info.mountpoint, Some("/tmp/rw".into()));
    }

    #[test]
    fn migrates_legacy_single_mountpoint_record() {
        let legacy = r#"
name = "demo"
backend = "juicefs"
metadata_url = "sqlite3://demo.db"
storage = "file"
bucket = "/tmp/objects"
dev = true
mountpoint = "/tmp/ro"
read_only = true
created_at = "now"
updated_at = "now"
"#;
        let volume: Volume = toml::from_str(legacy).unwrap();
        assert_eq!(volume.mounts, vec![mount("/tmp/ro", true)]);
    }

    #[test]
    fn legacy_unmounted_record_has_no_mounts() {
        let legacy = r#"
name = "demo"
backend = "juicefs"
metadata_url = "sqlite3://demo.db"
storage = "file"
bucket = "/tmp/objects"
dev = true
read_only = false
created_at = "now"
updated_at = "now"
"#;
        let volume: Volume = toml::from_str(legacy).unwrap();
        assert!(volume.mounts.is_empty());
    }
}
