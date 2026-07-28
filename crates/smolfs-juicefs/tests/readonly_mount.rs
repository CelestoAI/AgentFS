use std::env;
use std::io::ErrorKind;

use smolfs_core::{Config, InitVolume, MountVolume, SmolFsError, SmolFsHome};
use smolfs_juicefs::{SmolFs, install_managed_storage_backend};

fn integration_enabled() -> bool {
    env::var("SMOLFS_RUN_INTEGRATION").as_deref() == Ok("1")
}

/// Exercises the full read-only lifecycle against a real JuiceFS backend on a
/// local `--dev` volume (SQLite metadata + local object files), so it needs
/// neither MinIO nor Redis. Gated behind `SMOLFS_RUN_INTEGRATION=1` because it
/// requires the storage backend binary and local mount (FUSE) support.
#[test]
fn read_only_mount_rejects_writes_and_flush() {
    if !integration_enabled() {
        eprintln!("skipping read-only mount integration test; set SMOLFS_RUN_INTEGRATION=1");
        return;
    }

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
    let name = format!("ro-ci-{}", std::process::id());

    fs.init(InitVolume {
        name: name.clone(),
        dev: true,
        metadata_url: None,
        store_url: None,
        storage: None,
        bucket: None,
    })
    .expect("init dev volume");

    // --- Read-only mount ---
    let ro_dir = temp.path().join("ro-mount");
    fs.mount(MountVolume {
        name: name.clone(),
        path: ro_dir.clone(),
        foreground: false,
        check_storage: false,
        readonly: true,
    })
    .expect("mount read-only");

    let status = fs.status(Some(&name)).expect("status after ro mount");
    assert!(
        status.volumes[0].read_only,
        "status should report read_only=true"
    );

    // A write must be rejected by the filesystem (EROFS).
    let write_err = std::fs::write(ro_dir.join("nope.txt"), b"blocked")
        .expect_err("write to a read-only mount must fail");
    assert!(
        write_err.kind() == ErrorKind::ReadOnlyFilesystem
            || write_err.raw_os_error() == Some(libc_erofs()),
        "expected a read-only filesystem error, got {write_err:?}"
    );

    // flush must error loudly on a read-only mount.
    let flush_err = fs
        .flush(&name)
        .expect_err("flush on read-only mount must fail");
    assert!(
        matches!(flush_err, SmolFsError::VolumeReadOnly { .. }),
        "expected VolumeReadOnly, got {flush_err:?}"
    );

    fs.unmount(&name, false).expect("unmount read-only");

    // --- Read-write remount: writes and flush must work again ---
    let rw_dir = temp.path().join("rw-mount");
    fs.mount(MountVolume {
        name: name.clone(),
        path: rw_dir.clone(),
        foreground: false,
        check_storage: false,
        readonly: false,
    })
    .expect("remount read-write");

    let status = fs.status(Some(&name)).expect("status after rw mount");
    assert!(
        !status.volumes[0].read_only,
        "status should report read_only=false after rw remount"
    );

    std::fs::write(rw_dir.join("ok.txt"), b"allowed").expect("write to a read-write mount");
    fs.flush(&name).expect("flush a read-write mount");

    fs.unmount(&name, false).expect("unmount read-write");
}

/// EROFS is 30 on both Linux and macOS.
fn libc_erofs() -> i32 {
    30
}
