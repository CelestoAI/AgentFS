# SmolFS Python SDK

SmolFS gives Python agents a workspace folder that can survive after the agent
process stops. You can create a volume, mount it like a normal directory, write
files into it, flush important changes, unmount it, and mount the same files
again later.

The Python package is a native SDK over the same Rust core as the `smolfs`
command. It is useful when an agent runner or automation service wants to manage
workspace volumes without shelling out for every operation.

## Install

Install the Python SDK with `uv`:

```bash
uv add smolfs
```

Mounting volumes also needs SmolFS' managed storage backend on the machine. The
installer sets up both the CLI and backend:

```bash
curl -fsSL https://raw.githubusercontent.com/CelestoAI/smolfs/main/scripts/install.sh | SMOLFS_INSTALL_PYTHON=1 sh
```

If you only need the Python package inside an existing project or virtual
environment, `uv add smolfs` is enough.

## Quickstart

Start with a local development volume:

```python
from pathlib import Path

from smolfs import SmolFS, doctor

report = doctor()
if not report["storage_backend"]["found"] or not report["mount_support"]["found"]:
    raise RuntimeError(f"SmolFS is not ready: {report}")

fs = SmolFS.from_env()
volume = fs.ensure_volume("demo", dev=True)
mount = fs.mount(volume.name, "./workspace")

workspace = Path(mount.mountpoint)
(workspace / "hello.txt").write_text("hello from SmolFS\n")

try:
    fs.flush(volume.name)
finally:
    fs.unmount(volume.name)
```

`dev=True` creates a local-only volume. It is the easiest way to test SmolFS on a
single machine before connecting shared metadata and object storage.

## Cloud Volumes

Cloud volumes use the same API with explicit metadata and object storage
settings:

```python
from smolfs import SmolFS

fs = SmolFS.from_env()
fs.ensure_volume(
    "agent-workspace",
    metadata="redis://localhost:6379/1",
    storage="s3",
    bucket="https://my-bucket.s3.us-east-2.amazonaws.com",
)
```

Keep storage credentials in the environment used by SmolFS. Do not print them in
logs or store them in source files.

## API Overview

- `doctor()` checks whether the machine can create and mount volumes.
- `SmolFS.from_env()` creates a client using `SMOLFS_HOME` and the current
  environment.
- `ensure_volume(...)` creates a volume if it does not exist and returns the
  existing volume if it does.
- `init(...)` creates a new volume.
- `mount(name, path)` mounts a volume at a local directory.
- `flush(name, mountpoint=None)` asks SmolFS to sync important writes.
- `unmount(name, mountpoint=None)` unmounts a mounted volume. Pass
  `mountpoint` when the volume is mounted more than once.
- `status(name=None)` lists known volumes and mountpoints.

## Links

- Repository: https://github.com/CelestoAI/smolfs
- Issues: https://github.com/CelestoAI/smolfs/issues
- CLI installer: https://raw.githubusercontent.com/CelestoAI/smolfs/main/scripts/install.sh
