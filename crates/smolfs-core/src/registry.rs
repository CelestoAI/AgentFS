use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use fs2::FileExt;

use crate::error::{Result, SmolFsError};
use crate::models::Volume;
use crate::paths::SmolFsHome;
use crate::validation::validate_volume_name;

pub struct Registry {
    home: SmolFsHome,
}

pub struct RegistryLock {
    file: File,
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

impl Registry {
    pub fn new(home: SmolFsHome) -> Self {
        Self { home }
    }

    pub fn lock(&self) -> Result<RegistryLock> {
        self.home.ensure_layout()?;
        let path = self.home.lock_path();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|source| SmolFsError::IoAt { path, source })?;
        file.lock_exclusive()?;
        Ok(RegistryLock { file })
    }

    pub fn exists(&self, name: &str) -> Result<bool> {
        validate_volume_name(name)?;
        Ok(self.home.volume_path(name).exists())
    }

    pub fn read(&self, name: &str) -> Result<Volume> {
        validate_volume_name(name)?;
        let path = self.home.volume_path(name);
        if !path.exists() {
            return Err(SmolFsError::VolumeNotFound {
                name: name.to_string(),
            });
        }
        let contents = std::fs::read_to_string(&path).map_err(|source| SmolFsError::IoAt {
            path: path.clone(),
            source,
        })?;
        Ok(toml::from_str(&contents)?)
    }

    pub fn write(&self, volume: &Volume) -> Result<()> {
        validate_volume_name(&volume.name)?;
        self.home.ensure_layout()?;
        let path = self.home.volume_path(&volume.name);
        let contents = toml::to_string_pretty(volume)?;
        write_atomic(&path, contents.as_bytes())
    }

    pub fn list(&self) -> Result<Vec<Volume>> {
        self.home.ensure_layout()?;
        let mut volumes: Vec<Volume> = Vec::new();
        let dir = self.home.volumes_dir();
        for entry in std::fs::read_dir(&dir).map_err(|source| SmolFsError::IoAt {
            path: dir.clone(),
            source,
        })? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let contents = std::fs::read_to_string(&path).map_err(|source| SmolFsError::IoAt {
                path: path.clone(),
                source,
            })?;
            volumes.push(toml::from_str(&contents)?);
        }
        volumes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(volumes)
    }
}

fn write_atomic(path: &PathBuf, contents: &[u8]) -> Result<()> {
    let tmp = path.with_extension("toml.tmp");
    {
        let mut file = File::create(&tmp).map_err(|source| SmolFsError::IoAt {
            path: tmp.clone(),
            source,
        })?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path).map_err(|source| SmolFsError::IoAt {
        path: path.clone(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::Volume;
    use crate::paths::SmolFsHome;

    use super::Registry;

    #[test]
    fn writes_and_reads_volume() {
        let temp = tempfile::tempdir().unwrap();
        let home = SmolFsHome::new(temp.path());
        let registry = Registry::new(home);
        let volume = Volume {
            name: "demo".into(),
            backend: "juicefs".into(),
            metadata_url: "sqlite3://demo.db".into(),
            storage: "file".into(),
            bucket: "/tmp/objects".into(),
            dev: true,
            mountpoint: None,
            read_only: false,
            created_at: "now".into(),
            updated_at: "now".into(),
        };

        registry.write(&volume).unwrap();
        assert_eq!(registry.read("demo").unwrap().name, "demo");
        assert_eq!(registry.list().unwrap().len(), 1);
    }

    #[test]
    fn reads_legacy_volume_without_read_only_field() {
        let temp = tempfile::tempdir().unwrap();
        let home = SmolFsHome::new(temp.path());
        home.ensure_layout().unwrap();
        let path = home.volume_path("legacy");
        // A volume record written before the read_only field existed.
        let legacy = r#"
name = "legacy"
backend = "juicefs"
metadata_url = "sqlite3://legacy.db"
storage = "file"
bucket = "/tmp/objects"
dev = true
created_at = "now"
updated_at = "now"
"#;
        std::fs::write(&path, legacy).unwrap();

        let registry = Registry::new(home);
        let volume = registry.read("legacy").unwrap();
        assert!(!volume.read_only);
    }
}
