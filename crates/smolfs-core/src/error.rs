use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SmolFsError {
    #[error("invalid volume name {name:?}; use only letters, numbers, '.', '_' and '-'")]
    InvalidVolumeName { name: String },

    #[error("volume {name:?} already exists")]
    VolumeExists { name: String },

    #[error("volume {name:?} does not exist")]
    VolumeNotFound { name: String },

    #[error(
        "SmolFS storage backend is not installed; run `smolfs doctor --install` or rerun the SmolFS installer"
    )]
    MissingStorageBackend,

    #[error("SmolFS cannot auto-install the storage backend on {platform}")]
    UnsupportedStorageBackendPlatform { platform: String },

    #[error("could not install the SmolFS storage backend: {reason}")]
    StorageBackendInstallFailed { reason: String },

    #[error(
        "unsupported store URL {store:?}; use file://, s3://, gs://, or pass --storage and --bucket"
    )]
    UnsupportedStoreUrl { store: String },

    #[error("missing metadata URL; pass --metadata or use --dev")]
    MissingMetadataUrl,

    #[error("missing object storage config; pass --store or --storage and --bucket")]
    MissingStore,

    #[error("mountpoint {path} exists and is not a directory")]
    MountpointNotDirectory { path: PathBuf },

    #[error("volume {name:?} is not mounted")]
    VolumeNotMounted { name: String },

    #[error("volume {name:?} is mounted read-only")]
    VolumeReadOnly { name: String },

    #[error("local mount support is unavailable: {detail}\nFix: {fix}")]
    MountSupportUnavailable { detail: String, fix: String },

    #[error("SmolFS storage backend command failed ({status})")]
    CommandFailed {
        program: String,
        args: String,
        status: String,
        stdout: String,
        stderr: String,
    },

    #[error("I/O error at {path}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
}

pub type Result<T> = std::result::Result<T, SmolFsError>;
