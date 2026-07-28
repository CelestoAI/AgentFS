use std::path::{Path, PathBuf};

use smolfs_core::{MountVolume, Result, Volume};

use crate::command::{CommandOutput, CommandSpec};

#[derive(Debug, Clone)]
pub struct JuiceFs {
    bin: PathBuf,
}

impl JuiceFs {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self { bin: bin.into() }
    }

    pub fn bin(&self) -> &Path {
        &self.bin
    }

    pub fn format_spec(&self, volume: &Volume) -> CommandSpec {
        CommandSpec::new(&self.bin)
            .arg("format")
            .arg(&volume.metadata_url)
            .arg(&volume.name)
            .arg("--storage")
            .arg(&volume.storage)
            .arg("--bucket")
            .arg(&volume.bucket)
            .arg("--trash-days")
            .arg("0")
            .arg("--no-update")
    }

    pub fn mount_spec(&self, volume: &Volume, opts: &MountVolume, log_path: &Path) -> CommandSpec {
        let mut spec = CommandSpec::new(&self.bin)
            .arg("mount")
            .arg(&volume.metadata_url)
            .arg(opts.path.to_string_lossy())
            .arg("--log")
            .arg(log_path.to_string_lossy());

        if !opts.foreground {
            spec = spec.arg("-d");
        }
        if opts.check_storage {
            spec = spec.arg("--check-storage");
        }
        if opts.readonly {
            spec = spec.arg("--read-only");
        }
        spec
    }

    pub fn status_spec(&self, volume: &Volume) -> CommandSpec {
        CommandSpec::new(&self.bin)
            .arg("status")
            .arg(&volume.metadata_url)
    }

    pub fn unmount_spec(&self, mountpoint: &Path, force: bool) -> CommandSpec {
        let mut spec = CommandSpec::new(&self.bin).arg("umount").arg("--flush");
        if force {
            spec = spec.arg("--force");
        }
        spec.arg(mountpoint.to_string_lossy())
    }

    pub fn format(&self, volume: &Volume) -> Result<CommandOutput> {
        self.format_spec(volume).run()
    }

    pub fn mount(&self, volume: &Volume, opts: &MountVolume, log_path: &Path) -> Result<()> {
        if opts.foreground {
            let mut child = self.mount_spec(volume, opts, log_path).spawn()?;
            let status = child.wait()?;
            if !status.success() {
                return Err(smolfs_core::SmolFsError::CommandFailed {
                    program: self.bin.display().to_string(),
                    args: "mount".to_string(),
                    status: status.to_string(),
                    stdout: String::new(),
                    stderr: String::new(),
                });
            }
            Ok(())
        } else {
            self.mount_spec(volume, opts, log_path).run().map(|_| ())
        }
    }

    pub fn status(&self, volume: &Volume) -> Result<CommandOutput> {
        self.status_spec(volume).run()
    }

    pub fn unmount(&self, mountpoint: &Path, force: bool) -> Result<CommandOutput> {
        self.unmount_spec(mountpoint, force).run()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use smolfs_core::{MountVolume, Volume};

    use super::JuiceFs;

    fn mount_opts(readonly: bool) -> MountVolume {
        MountVolume {
            name: "demo".into(),
            path: PathBuf::from("/tmp/demo"),
            foreground: false,
            check_storage: false,
            readonly,
        }
    }

    fn volume() -> Volume {
        Volume {
            name: "demo".into(),
            backend: "juicefs".into(),
            metadata_url: "sqlite3:///tmp/demo.db".into(),
            storage: "file".into(),
            bucket: "/tmp/objects".into(),
            dev: true,
            mountpoint: None,
            read_only: false,
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    #[test]
    fn builds_format_command() {
        let juicefs = JuiceFs::new("/bin/juicefs");
        let spec = juicefs.format_spec(&volume());
        assert_eq!(spec.args[0], "format");
        assert!(spec.args.contains(&"--storage".to_string()));
        assert!(spec.args.contains(&"--bucket".to_string()));
    }

    #[test]
    fn mount_command_includes_read_only_when_requested() {
        let juicefs = JuiceFs::new("/bin/juicefs");
        let log = PathBuf::from("/tmp/demo.log");
        let spec = juicefs.mount_spec(&volume(), &mount_opts(true), &log);
        assert!(spec.args.contains(&"--read-only".to_string()));
    }

    #[test]
    fn mount_command_omits_read_only_by_default() {
        let juicefs = JuiceFs::new("/bin/juicefs");
        let log = PathBuf::from("/tmp/demo.log");
        let spec = juicefs.mount_spec(&volume(), &mount_opts(false), &log);
        assert!(!spec.args.contains(&"--read-only".to_string()));
    }
}
