# SmolFS TypeScript SDK

SmolFS gives TypeScript agents a workspace folder that can survive after the
agent process stops. You can create a volume, mount it like a normal directory,
write files into it, flush important changes, unmount it, and mount the same
files again later.

The npm package is a native Node.js SDK over the same Rust core as the `smolfs`
command. It is useful when an agent runner or automation service wants to manage
workspace volumes from TypeScript without shelling out for every operation.

## Install

Install the SDK from npm:

```bash
npm install @celestoai/smolfs
```

Mounting volumes also needs SmolFS' managed storage backend on the machine. The
installer sets up both the CLI and backend:

```bash
curl -fsSL https://raw.githubusercontent.com/CelestoAI/smolfs/main/scripts/install.sh | sh
```

## Quickstart

Start with a local development volume:

```ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { SmolFS, doctor } from "@celestoai/smolfs";

const report = doctor();
if (!report.storageBackend.found || !report.mountSupport.found) {
  throw new Error(`SmolFS is not ready: ${JSON.stringify(report)}`);
}

const fs = SmolFS.fromEnv();
const volume = fs.ensureVolume({ name: "demo", dev: true });
const mount = fs.mount({ name: volume.name, path: "./workspace" });

mkdirSync(mount.mountpoint, { recursive: true });
writeFileSync(join(mount.mountpoint, "hello.txt"), "hello from SmolFS\n");

try {
  fs.flush(volume.name);
} finally {
  fs.unmount(volume.name);
}
```

`dev: true` creates a local-only volume. It is the easiest way to test SmolFS on
a single machine before connecting shared metadata and object storage.

## Cloud Volumes

Cloud volumes use the same API with explicit metadata and object storage
settings:

```ts
import { SmolFS } from "@celestoai/smolfs";

const fs = SmolFS.fromEnv();
fs.ensureVolume({
  name: "agent-workspace",
  metadata: "redis://localhost:6379/1",
  storage: "s3",
  bucket: "https://my-bucket.s3.us-east-2.amazonaws.com",
});
```

Keep storage credentials in the environment used by SmolFS. Do not print them in
logs or store them in source files.

## API Overview

- `doctor()` checks whether the machine can create and mount volumes.
- `SmolFS.fromEnv()` creates a client using `SMOLFS_HOME` and the current
  environment.
- `ensureVolume(...)` creates a volume if it does not exist and returns the
  existing volume if it does.
- `init(...)` creates a new volume.
- `mount({ name, path })` mounts a volume at a local directory.
- `flush(name, mountpoint?)` asks SmolFS to sync important writes.
- `unmount(name, { force, mountpoint })` unmounts a mounted volume. Pass
  `mountpoint` when the volume is mounted more than once.
- `status(name?)` lists known volumes and mountpoints.

## Links

- Repository: https://github.com/CelestoAI/smolfs
- Issues: https://github.com/CelestoAI/smolfs/issues
- CLI installer: https://raw.githubusercontent.com/CelestoAI/smolfs/main/scripts/install.sh
