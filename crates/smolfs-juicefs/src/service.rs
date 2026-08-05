use std::fs;
use std::path::{Path, PathBuf};

use smolfs_core::store::{dev_store, parse_store_url};
use smolfs_core::time::now_rfc3339;
use smolfs_core::validation::validate_volume_name;
use smolfs_core::{
    Config, DoctorReport, InitVolume, MountInfo, MountVolume, Registry, Result, SmolFsError,
    SmolFsHome, StatusReport, Volume, VolumeInfo,
};

use crate::doctor::{doctor, require_mount_support, require_storage_backend_bin};
use crate::juicefs::JuiceFs;

pub struct SmolFs {
    home: SmolFsHome,
    registry: Registry,
    juicefs: JuiceFs,
}

impl SmolFs {
    pub fn from_env() -> Result<Self> {
        let home = SmolFsHome::from_env()?;
        Self::new(home)
    }

    pub fn new(home: SmolFsHome) -> Result<Self> {
        home.ensure_layout()?;
        let config = Config::load(&home)?;
        let bin = require_storage_backend_bin(&home, &config)?;
        Ok(Self {
            registry: Registry::new(home.clone()),
            juicefs: JuiceFs::new(bin),
            home,
        })
    }

    pub fn doctor(&self) -> Result<DoctorReport> {
        doctor(&self.home)
    }

    pub fn init(&self, opts: InitVolume) -> Result<VolumeInfo> {
        validate_volume_name(&opts.name)?;
        let _lock = self.registry.lock()?;
        if self.registry.exists(&opts.name)? {
            return Err(SmolFsError::VolumeExists { name: opts.name });
        }

        let volume = self.build_volume(opts)?;
        self.juicefs.format(&volume)?;
        self.registry.write(&volume)?;
        Ok(VolumeInfo::from(&volume))
    }

    pub fn ensure_volume(&self, opts: InitVolume) -> Result<VolumeInfo> {
        validate_volume_name(&opts.name)?;
        let _lock = self.registry.lock()?;
        if self.registry.exists(&opts.name)? {
            return Ok(VolumeInfo::from(&self.registry.read(&opts.name)?));
        }

        let volume = self.build_volume(opts)?;
        self.juicefs.format(&volume)?;
        self.registry.write(&volume)?;
        Ok(VolumeInfo::from(&volume))
    }

    pub fn mount(&self, opts: MountVolume) -> Result<MountInfo> {
        validate_volume_name(&opts.name)?;
        require_mount_support()?;
        ensure_mountpoint(&opts.path)?;

        let _lock = self.registry.lock()?;
        let mut volume = self.registry.read(&opts.name)?;
        let mountpoint = absolute_path(&opts.path)?;
        let log_path = self.home.volume_log_path(&opts.name);

        let readonly = opts.readonly;
        let command_opts = MountVolume {
            path: mountpoint.clone(),
            ..opts
        };
        self.juicefs.mount(&volume, &command_opts, &log_path)?;
        if readonly {
            probe_mount_readonly(&mountpoint)?;
        } else {
            probe_mount(&mountpoint)?;
        }

        volume.mountpoint = Some(mountpoint.clone());
        volume.read_only = readonly;
        volume.updated_at = now_rfc3339();
        self.registry.write(&volume)?;

        Ok(MountInfo {
            name: volume.name,
            mountpoint,
        })
    }

    pub fn flush(&self, name: &str) -> Result<()> {
        validate_volume_name(name)?;
        let volume = self.registry.read(name)?;
        if volume.read_only {
            return Err(SmolFsError::VolumeReadOnly {
                name: name.to_string(),
            });
        }
        let mountpoint = volume
            .mountpoint
            .ok_or_else(|| SmolFsError::VolumeNotMounted {
                name: name.to_string(),
            })?;
        probe_mount(&mountpoint)?;
        sync_probe(&mountpoint)?;
        Ok(())
    }

    pub fn unmount(&self, name: &str, force: bool) -> Result<()> {
        validate_volume_name(name)?;
        let _lock = self.registry.lock()?;
        let mut volume = self.registry.read(name)?;
        let mountpoint =
            volume
                .mountpoint
                .clone()
                .ok_or_else(|| SmolFsError::VolumeNotMounted {
                    name: name.to_string(),
                })?;
        self.juicefs.unmount(&mountpoint, force)?;
        volume.mountpoint = None;
        volume.read_only = false;
        volume.updated_at = now_rfc3339();
        self.registry.write(&volume)
    }

    pub fn status(&self, name: Option<&str>) -> Result<StatusReport> {
        let volumes = if let Some(name) = name {
            vec![VolumeInfo::from(&self.registry.read(name)?)]
        } else {
            self.registry.list()?.iter().map(VolumeInfo::from).collect()
        };

        Ok(StatusReport { volumes })
    }

    pub fn juicefs_status_raw(&self, name: &str) -> Result<String> {
        let volume = self.registry.read(name)?;
        Ok(self.juicefs.status(&volume)?.stdout)
    }

    fn build_volume(&self, opts: InitVolume) -> Result<Volume> {
        let now = now_rfc3339();

        let (metadata_url, storage, bucket) = if opts.dev {
            let dev_root = self.home.dev_dir().join(&opts.name);
            let metadata = dev_root.join("metadata.db");
            let objects = dev_root.join("objects");
            fs::create_dir_all(&objects).map_err(|source| SmolFsError::IoAt {
                path: objects.clone(),
                source,
            })?;
            let store = dev_store(objects);
            (
                format!("sqlite3://{}", metadata.to_string_lossy()),
                store.storage,
                store.bucket,
            )
        } else {
            let metadata_url = opts.metadata_url.ok_or(SmolFsError::MissingMetadataUrl)?;
            let store = match (opts.store_url, opts.storage, opts.bucket) {
                (Some(store_url), None, None) => parse_store_url(&store_url)?,
                (None, Some(storage), Some(bucket)) => {
                    smolfs_core::store::StoreSpec::new(storage, bucket)
                }
                (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                    return Err(SmolFsError::MissingStore);
                }
                _ => return Err(SmolFsError::MissingStore),
            };
            (metadata_url, store.storage, store.bucket)
        };

        Ok(Volume {
            name: opts.name,
            backend: "juicefs".into(),
            metadata_url,
            storage,
            bucket,
            dev: opts.dev,
            mountpoint: None,
            read_only: false,
            created_at: now.clone(),
            updated_at: now,
        })
    }
}

fn ensure_mountpoint(path: &Path) -> Result<()> {
    if path.exists() && !path.is_dir() {
        return Err(SmolFsError::MountpointNotDirectory {
            path: path.to_path_buf(),
        });
    }
    fs::create_dir_all(path).map_err(|source| SmolFsError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn probe_mount(path: &Path) -> Result<()> {
    let probe = path.join(format!(".smolfs-probe-{}", std::process::id()));
    fs::write(&probe, b"smolfs").map_err(|source| SmolFsError::IoAt {
        path: probe.clone(),
        source,
    })?;
    let contents = fs::read(&probe).map_err(|source| SmolFsError::IoAt {
        path: probe.clone(),
        source,
    })?;
    if contents != b"smolfs" {
        return Err(SmolFsError::Io(std::io::Error::other(
            "mount probe returned unexpected data",
        )));
    }
    fs::remove_file(&probe).map_err(|source| SmolFsError::IoAt {
        path: probe,
        source,
    })
}

fn probe_mount_readonly(path: &Path) -> Result<()> {
    // A read-only mount rejects writes, so verify liveness with a read instead.
    fs::read_dir(path)
        .map(|_| ())
        .map_err(|source| SmolFsError::IoAt {
            path: path.to_path_buf(),
            source,
        })
}

fn sync_probe(path: &Path) -> Result<()> {
    let probe = path.join(".smolfs-flush");
    let file = fs::File::create(&probe).map_err(|source| SmolFsError::IoAt {
        path: probe.clone(),
        source,
    })?;
    file.sync_all()?;
    fs::remove_file(&probe).map_err(|source| SmolFsError::IoAt {
        path: probe,
        source,
    })?;
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("sync").status();
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use smolfs_core::{Registry, SmolFsError, SmolFsHome, Volume};

    use super::SmolFs;

    fn stub_backend(home: &SmolFsHome) {
        home.ensure_layout().unwrap();
        let bin = home.managed_storage_backend_bin();
        std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = std::fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin, perms).unwrap();
    }

    fn mounted_volume(name: &str, read_only: bool) -> Volume {
        Volume {
            name: name.into(),
            backend: "juicefs".into(),
            metadata_url: "sqlite3://demo.db".into(),
            storage: "file".into(),
            bucket: "/tmp/objects".into(),
            dev: true,
            mountpoint: Some("/tmp/does-not-need-to-exist".into()),
            read_only,
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    #[test]
    fn flush_errors_on_read_only_volume() {
        let temp = tempfile::tempdir().unwrap();
        let home = SmolFsHome::new(temp.path());
        stub_backend(&home);
        Registry::new(home.clone())
            .write(&mounted_volume("ro", true))
            .unwrap();

        let fs = SmolFs::new(home).unwrap();
        // The read-only guard fires before any mount I/O, so no real mount is needed.
        let err = fs.flush("ro").unwrap_err();
        assert!(
            matches!(err, SmolFsError::VolumeReadOnly { name } if name == "ro"),
            "expected VolumeReadOnly error"
        );
    }
}
