---
name: bluefin-server
description: >
  Build, verify, and maintain the FSDK-based bluefin-server bootc image, including
  compilation elements and offline Cargo setups.
  Use when modifying server-image configs, troubleshooting sandboxed BuildStream builds,
  or resolving bootc target installation requirements.
metadata:
  context7-sources:
    - /bootc-dev/bootc
    - /ostreedev/ostree
    - /freedesktop/freedesktop-sdk
---

# bluefin-server Building and Maintenance — lab Skill

## When to Use

- Building or debugging the FSDK-based `bluefin-server-bootc` image on the cluster.
- Troubleshooting sandboxed, networkless Cargo compiles inside FSDK elements.
- Handling `bootc install to-disk` requirements and resolving `prepare-root.conf` or filesystem failures.

## When NOT to Use

- Deploying boot-test virtual machine instances → `kubevirt-vms.md`.

---

## Core Process

### 1. Maintain Sandboxed Offline Cargo Builds
BuildStream manual/script sandboxes have **no internet access**. All Rust/Cargo dependencies must be fully offline-vendored:

1. Package dependencies using `cargo vendor` and compress with `zstd` into a `-vendor.tar.zstd` archive.
2. In `bootc.bst`, declare both the gzip source tree and the raw `.zstd` vendor archive. BuildStream's tar plugin automatically extracts `.tar.gz` but leaves `.tar.zstd` raw.
3. Extract the `.zstd` archive in the build script using host tools:
   ```bash
   tar --zstd -xf bootc-vendor.tar.zstd
   ```
4. Generate a local `.cargo/config.toml` redirecting `crates-io` and any git dependencies (e.g., `composefs-ctl`) to local directories:
   ```toml
   [source.crates-io]
   replace-with = "vendored-sources"

   [source.vendored-sources]
   directory = "vendor"

   [patch."https://github.com/composefs/composefs-rs"]
   composefs-ctl = { path = "vendor/composefs-ctl" }
   ```
5. Remove test subdirectories (like `crates/tests-integration`) from the workspace to bypass missing network and system dependencies, and compile with `cargo build --release --offline -p bootc`.

### 2. Ensure Bootc Target Readiness
`bootc install to-disk` asserts on the presence of `/usr/lib/ostree/prepare-root.conf` (or `/etc/ostree/prepare-root.conf`) inside the OCI image. If it is missing, compilation will crash with `Failed to find ostree/prepare-root.conf`.

Always include `bluefin-server/prepare-root-config.bst` in `os-stack.bst` to write a basic config:
```ini
[sysroot]
readonly=false
```

---

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "I can build Cargo projects online by enabling network access in the element." | BuildStream policy prohibits network access during build phase to enforce complete reproducibility. All sources must be pre-fetched during fetch phase. |
| "prepare-root.conf is only needed at runtime, so we can omit it from the OCI build." | `bootc install to-disk` performs static image validation before writing blocks and refuses to install images lacking this file. |

## Red Flags

- `bootc install` failing with `Failed to find ostree/prepare-root.conf` → Missing the `prepare-root-config` dependency in `os-stack.bst`.
- `cargo` compiler failing with network errors inside BuildStream → Forgot to configure `.cargo/config.toml` or missing `--offline` flag.

## Verification

- [ ] `skopeo inspect --tls-verify=false docker://<lab-ip>:30500/bluefin-server-bootc:latest` returns a valid OCI manifest.
- [ ] `podman run --rm --tls-verify=false --entrypoint=ls <lab-ip>:30500/bluefin-server-bootc:latest /usr/lib/ostree/prepare-root.conf` completes successfully.
