use std::env;
use std::io::ErrorKind;
use std::path::Path;

use smolfs_core::{Config, InitVolume, MountVolume, SmolFsError, SmolFsHome};
use smolfs_juicefs::{SmolFs, install_managed_storage_backend};

struct MetadataBackend {
    name: &'static str,
    url: String,
}

fn integration_enabled() -> bool {
    env::var("SMOLFS_RUN_INTEGRATION").as_deref() == Ok("1")
}

/// Exercises the full read-only mount lifecycle against a real JuiceFS backend.
///
/// Per `AGENTS.md`, JuiceFS behavior must be covered against both the Redis and
/// SQLite metadata engines, so the lifecycle runs once per backend, both backed
/// by the same S3-compatible object store. Gated behind
/// `SMOLFS_RUN_INTEGRATION=1` because it needs the storage backend binary, the
/// metadata services, and local mount (FUSE) support.
#[test]
fn read_only_mount_rejects_writes_and_flush() {
    if !integration_enabled() {
        eprintln!("skipping read-only mount integration test; set SMOLFS_RUN_INTEGRATION=1");
        return;
    }

    let bucket = env::var("SMOLFS_TEST_S3_BUCKET")
        .unwrap_or_else(|_| "http://127.0.0.1:9000/smolfs-ci".to_string());

    let temp = tempfile::tempdir().expect("create temp SmolFS home");
    let home = SmolFsHome::new(temp.path());

    if let Ok(bin) = env::var("SMOLFS_STORAGE_BACKEND_BIN") {
        Config {
            storage_backend_bin: Some(bin.into()),
        }
        .save(&home)
        .expect("save storage backend path");
    } else {
        install_managed_storage_backend(&home).expect("install managed storage backend");
    }

    let fs = SmolFs::new(home).expect("construct SmolFS service");

    for metadata in metadata_backends(temp.path()) {
        run_lifecycle(&fs, &metadata, &bucket, temp.path());
    }
}

fn run_lifecycle(fs: &SmolFs, metadata: &MetadataBackend, bucket: &str, root: &Path) {
    let name = format!("ro-ci-{}-{}", metadata.name, std::process::id());

    fs.init(InitVolume {
        name: name.clone(),
        dev: false,
        metadata_url: Some(metadata.url.clone()),
        store_url: None,
        storage: Some("s3".to_string()),
        bucket: Some(bucket.to_string()),
    })
    .unwrap_or_else(|err| panic!("init volume with {} metadata: {err}", metadata.name));

    // --- Read-only mount ---
    let ro_dir = root.join(format!("{}-ro", metadata.name));
    fs.mount(MountVolume {
        name: name.clone(),
        path: ro_dir.clone(),
        foreground: false,
        check_storage: false,
        readonly: true,
    })
    .unwrap_or_else(|err| panic!("mount read-only with {} metadata: {err}", metadata.name));

    let status = fs.status(Some(&name)).expect("status after ro mount");
    assert!(
        status.volumes[0].read_only,
        "{}: status should report read_only=true",
        metadata.name
    );

    // A write must be rejected by the filesystem (EROFS).
    let write_err = std::fs::write(ro_dir.join("nope.txt"), b"blocked")
        .expect_err("write to a read-only mount must fail");
    assert!(
        write_err.kind() == ErrorKind::ReadOnlyFilesystem
            || write_err.raw_os_error() == Some(EROFS),
        "{}: expected a read-only filesystem error, got {write_err:?}",
        metadata.name
    );

    // flush must error loudly on a read-only mount.
    let flush_err = fs
        .flush(&name)
        .expect_err("flush on read-only mount must fail");
    assert!(
        matches!(flush_err, SmolFsError::VolumeReadOnly { .. }),
        "{}: expected VolumeReadOnly, got {flush_err:?}",
        metadata.name
    );

    fs.unmount(&name, false).expect("unmount read-only");

    // --- Read-write remount: writes and flush must work again ---
    let rw_dir = root.join(format!("{}-rw", metadata.name));
    fs.mount(MountVolume {
        name: name.clone(),
        path: rw_dir.clone(),
        foreground: false,
        check_storage: false,
        readonly: false,
    })
    .unwrap_or_else(|err| panic!("remount read-write with {} metadata: {err}", metadata.name));

    let status = fs.status(Some(&name)).expect("status after rw mount");
    assert!(
        !status.volumes[0].read_only,
        "{}: status should report read_only=false after rw remount",
        metadata.name
    );

    std::fs::write(rw_dir.join("ok.txt"), b"allowed").expect("write to a read-write mount");
    fs.flush(&name).expect("flush a read-write mount");

    fs.unmount(&name, false).expect("unmount read-write");
}

fn metadata_backends(root: &Path) -> Vec<MetadataBackend> {
    let redis_url = env::var("SMOLFS_TEST_REDIS_METADATA_URL")
        .or_else(|_| env::var("SMOLFS_TEST_METADATA_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_string());
    let sqlite_path = root.join("metadata.sqlite3");
    let sqlite_url = env::var("SMOLFS_TEST_SQLITE_METADATA_URL")
        .unwrap_or_else(|_| format!("sqlite3://{}", sqlite_path.to_string_lossy()));

    vec![
        MetadataBackend {
            name: "redis",
            url: redis_url,
        },
        MetadataBackend {
            name: "sqlite",
            url: sqlite_url,
        },
    ]
}

/// EROFS ("Read-only file system") is errno 30 on both Linux and macOS.
const EROFS: i32 = 30;
