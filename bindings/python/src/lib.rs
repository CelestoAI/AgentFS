use std::path::PathBuf;

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use smolfs_core::{DoctorReport, InitVolume, MountVolume, SmolFsHome};
use smolfs_juicefs::{SmolFs, doctor as run_doctor};

pyo3::create_exception!(smolfs, SmolFSError, PyException);

#[pyclass(name = "SmolFS")]
struct PySmolFs {
    inner: SmolFs,
}

#[pyclass]
#[derive(Clone)]
struct VolumeInfo {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    metadata_url: String,
    #[pyo3(get)]
    storage: String,
    #[pyo3(get)]
    bucket: String,
    #[pyo3(get)]
    dev: bool,
    #[pyo3(get)]
    mountpoint: Option<String>,
    #[pyo3(get)]
    read_only: bool,
}

#[pyclass]
#[derive(Clone)]
struct MountInfo {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    mountpoint: String,
}

#[pyclass]
#[derive(Clone)]
struct Status {
    #[pyo3(get)]
    volumes: Vec<VolumeInfo>,
}

#[pymethods]
impl PySmolFs {
    #[staticmethod]
    fn from_env() -> PyResult<Self> {
        Ok(Self {
            inner: SmolFs::from_env().map_err(to_py_err)?,
        })
    }

    fn doctor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let report = self.inner.doctor().map_err(to_py_err)?;
        doctor_report_to_py(py, report)
    }

    #[pyo3(signature = (name, *, dev=false, metadata=None, store=None, storage=None, bucket=None))]
    fn init(
        &self,
        name: String,
        dev: bool,
        metadata: Option<String>,
        store: Option<String>,
        storage: Option<String>,
        bucket: Option<String>,
    ) -> PyResult<VolumeInfo> {
        self.inner
            .init(InitVolume {
                name,
                dev,
                metadata_url: metadata,
                store_url: store,
                storage,
                bucket,
            })
            .map(VolumeInfo::from)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (name, *, dev=false, metadata=None, store=None, storage=None, bucket=None))]
    fn ensure_volume(
        &self,
        name: String,
        dev: bool,
        metadata: Option<String>,
        store: Option<String>,
        storage: Option<String>,
        bucket: Option<String>,
    ) -> PyResult<VolumeInfo> {
        self.inner
            .ensure_volume(InitVolume {
                name,
                dev,
                metadata_url: metadata,
                store_url: store,
                storage,
                bucket,
            })
            .map(VolumeInfo::from)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (name, path, *, foreground=false, check_storage=false, read_only=false))]
    fn mount(
        &self,
        name: String,
        path: String,
        foreground: bool,
        check_storage: bool,
        read_only: bool,
    ) -> PyResult<MountInfo> {
        self.inner
            .mount(MountVolume {
                name,
                path: PathBuf::from(path),
                foreground,
                check_storage,
                readonly: read_only,
            })
            .map(MountInfo::from)
            .map_err(to_py_err)
    }

    fn flush(&self, name: String) -> PyResult<()> {
        self.inner.flush(&name).map_err(to_py_err)
    }

    #[pyo3(signature = (name, *, force=false))]
    fn unmount(&self, name: String, force: bool) -> PyResult<()> {
        self.inner.unmount(&name, force).map_err(to_py_err)
    }

    #[pyo3(signature = (name=None))]
    fn status(&self, name: Option<String>) -> PyResult<Status> {
        self.inner
            .status(name.as_deref())
            .map(|status| Status {
                volumes: status.volumes.into_iter().map(VolumeInfo::from).collect(),
            })
            .map_err(to_py_err)
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

#[pyfunction]
fn doctor(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let home = SmolFsHome::from_env().map_err(to_py_err)?;
    let report = run_doctor(&home).map_err(to_py_err)?;
    doctor_report_to_py(py, report)
}

fn doctor_report_to_py(py: Python<'_>, report: DoctorReport) -> PyResult<Bound<'_, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("home", report.home.display().to_string())?;
    dict.set_item("config", report.config.display().to_string())?;
    let storage_backend = PyDict::new(py);
    storage_backend.set_item("found", report.storage_backend.found)?;
    storage_backend.set_item(
        "path",
        report
            .storage_backend
            .path
            .as_ref()
            .map(|path| path.display().to_string()),
    )?;
    storage_backend.set_item("version", report.storage_backend.version)?;
    storage_backend.set_item("managed", report.storage_backend.managed)?;
    dict.set_item("storage_backend", storage_backend)?;
    let mount_support = PyDict::new(py);
    mount_support.set_item("found", report.mount_support.found)?;
    mount_support.set_item("detail", report.mount_support.detail)?;
    mount_support.set_item("fix", report.mount_support.fix)?;
    dict.set_item("mount_support", mount_support)?;
    Ok(dict)
}

fn to_py_err(err: smolfs_core::SmolFsError) -> PyErr {
    SmolFSError::new_err(err.to_string())
}

#[pymodule]
fn _native(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("SmolFSError", py.get_type::<SmolFSError>())?;
    module.add_function(wrap_pyfunction!(doctor, module)?)?;
    module.add_class::<PySmolFs>()?;
    module.add_class::<VolumeInfo>()?;
    module.add_class::<MountInfo>()?;
    module.add_class::<Status>()?;
    Ok(())
}
