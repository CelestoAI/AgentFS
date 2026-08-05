use std::path::PathBuf;

use napi::{Error, Result};
use napi_derive::napi;
use smolfs_core::{InitVolume, MountVolume, SmolFsError, SmolFsHome};
use smolfs_juicefs::{SmolFs as InnerSmolFs, doctor as run_doctor};

#[napi(object)]
pub struct InitVolumeOptions {
    pub name: String,
    pub dev: Option<bool>,
    pub metadata: Option<String>,
    pub store: Option<String>,
    pub storage: Option<String>,
    pub bucket: Option<String>,
}

#[napi(object)]
pub struct MountVolumeOptions {
    pub name: String,
    pub path: String,
    pub check_storage: Option<bool>,
    pub read_only: Option<bool>,
}

#[napi(object)]
pub struct UnmountOptions {
    pub force: Option<bool>,
}

#[napi(object)]
pub struct VolumeInfo {
    pub name: String,
    pub metadata_url: String,
    pub storage: String,
    pub bucket: String,
    pub dev: bool,
    pub mountpoint: Option<String>,
    pub read_only: bool,
}

#[napi(object)]
pub struct MountInfo {
    pub name: String,
    pub mountpoint: String,
}

#[napi(object)]
pub struct Status {
    pub volumes: Vec<VolumeInfo>,
}

#[napi(object)]
pub struct BinaryReport {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub managed: bool,
}

#[napi(object)]
pub struct MountSupportReport {
    pub found: bool,
    pub detail: String,
    pub fix: Option<String>,
}

#[napi(object)]
pub struct DoctorReport {
    pub home: String,
    pub config: String,
    pub storage_backend: BinaryReport,
    pub mount_support: MountSupportReport,
}

#[napi(js_name = "SmolFS")]
pub struct SmolFs {
    inner: InnerSmolFs,
}

#[napi]
impl SmolFs {
    #[napi(constructor)]
    pub fn new() -> Result<Self> {
        Self::from_env()
    }

    #[napi(factory)]
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            inner: InnerSmolFs::from_env().map_err(to_napi_err)?,
        })
    }

    #[napi]
    pub fn doctor(&self) -> Result<DoctorReport> {
        self.inner
            .doctor()
            .map(DoctorReport::from)
            .map_err(to_napi_err)
    }

    #[napi]
    pub fn init(&self, options: InitVolumeOptions) -> Result<VolumeInfo> {
        self.inner
            .init(options.into())
            .map(VolumeInfo::from)
            .map_err(to_napi_err)
    }

    #[napi]
    pub fn ensure_volume(&self, options: InitVolumeOptions) -> Result<VolumeInfo> {
        self.inner
            .ensure_volume(options.into())
            .map(VolumeInfo::from)
            .map_err(to_napi_err)
    }

    #[napi]
    pub fn mount(&self, options: MountVolumeOptions) -> Result<MountInfo> {
        self.inner
            .mount(options.into())
            .map(MountInfo::from)
            .map_err(to_napi_err)
    }

    #[napi]
    pub fn flush(&self, name: String) -> Result<()> {
        self.inner.flush(&name).map_err(to_napi_err)
    }

    #[napi]
    pub fn unmount(&self, name: String, options: Option<UnmountOptions>) -> Result<()> {
        let force = options.and_then(|options| options.force).unwrap_or(false);
        self.inner.unmount(&name, force).map_err(to_napi_err)
    }

    #[napi]
    pub fn status(&self, name: Option<String>) -> Result<Status> {
        self.inner
            .status(name.as_deref())
            .map(|status| Status {
                volumes: status.volumes.into_iter().map(VolumeInfo::from).collect(),
            })
            .map_err(to_napi_err)
    }
}

#[napi]
pub fn doctor() -> Result<DoctorReport> {
    let home = SmolFsHome::from_env().map_err(to_napi_err)?;
    run_doctor(&home)
        .map(DoctorReport::from)
        .map_err(to_napi_err)
}

impl From<InitVolumeOptions> for InitVolume {
    fn from(value: InitVolumeOptions) -> Self {
        Self {
            name: value.name,
            dev: value.dev.unwrap_or(false),
            metadata_url: value.metadata,
            store_url: value.store,
            storage: value.storage,
            bucket: value.bucket,
        }
    }
}

impl From<MountVolumeOptions> for MountVolume {
    fn from(value: MountVolumeOptions) -> Self {
        Self {
            name: value.name,
            path: PathBuf::from(value.path),
            foreground: false,
            check_storage: value.check_storage.unwrap_or(false),
            readonly: value.read_only.unwrap_or(false),
        }
    }
}

impl From<smolfs_core::VolumeInfo> for VolumeInfo {
    fn from(value: smolfs_core::VolumeInfo) -> Self {
        Self {
            name: value.name,
            metadata_url: value.metadata_url,
            storage: value.storage,
            bucket: value.bucket,
            dev: value.dev,
            mountpoint: value.mountpoint.map(|path| path.display().to_string()),
            read_only: value.read_only,
        }
    }
}

impl From<smolfs_core::MountInfo> for MountInfo {
    fn from(value: smolfs_core::MountInfo) -> Self {
        Self {
            name: value.name,
            mountpoint: value.mountpoint.display().to_string(),
        }
    }
}

impl From<smolfs_core::DoctorReport> for DoctorReport {
    fn from(value: smolfs_core::DoctorReport) -> Self {
        Self {
            home: value.home.display().to_string(),
            config: value.config.display().to_string(),
            storage_backend: BinaryReport {
                found: value.storage_backend.found,
                path: value
                    .storage_backend
                    .path
                    .map(|path| path.display().to_string()),
                version: value.storage_backend.version,
                managed: value.storage_backend.managed,
            },
            mount_support: MountSupportReport {
                found: value.mount_support.found,
                detail: value.mount_support.detail,
                fix: value.mount_support.fix,
            },
        }
    }
}

fn to_napi_err(err: smolfs_core::SmolFsError) -> Error {
    match err {
        SmolFsError::CommandFailed { status, .. } => {
            Error::from_reason(format!("SmolFS storage backend exited with {status}"))
        }
        err => Error::from_reason(err.to_string()),
    }
}
