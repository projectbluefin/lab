---
name: cluster-tooling
description: "Cluster management tools for the lab: kubectl, k3s, zot, external-secrets, and K8sGPT. Use when managing cluster state, installing cluster add-ons, configuring the OCI registry, or running cluster analysis through MCP."
metadata:
  type: reference
  context7-sources:
    - /helm/helm
    - /k3s-io/k3s
    - /project-zot/zot
    - /external-secrets/external-secrets
    - /k8sgpt-ai/k8sgpt
---

# Cluster Tooling — lab

## Mandatory first step

Before any kubectl, k3s, or K8sGPT operation, look up the current API via Context7:

```
resolve-library-id "/k3s-io/k3s" → get-library-docs
resolve-library-id "/k8sgpt-ai/k8sgpt" → get-library-docs
```

Do not guess flags, chart schema, or MCP method names. The K8sGPT MCP server exposes `analyze`, `cluster-info`, `list-resources`, `get-resource`, `list-namespaces`, `get-logs`, `list-events`, `list-filters`, `add-filters`, `remove-filters`, `list-integrations`, and `config`; verify the current docs before wiring it into a client.

## Tool roles

| Tool | Role |
|------|------|
| `k3s` | Lightweight Kubernetes — cluster runtime |
| `kubectl` | Direct cluster inspection and apply |
| `zot` | OCI registry for test artifacts |
| `external-secrets` | Pulls secrets from vault into k8s Secrets |
| `k8sgpt` | Cluster analysis / MCP troubleshooting bridge |

## Key references

- Cluster topology: `AGENTS.md`
- Bootstrap procedure: `docs/bootstrap.md`
- Recovery: `docs/skills/k3s-cluster-ops` (user skill, load before any cluster recovery)
- K8sGPT MCP config: `~/.copilot/mcp-config.json` on this machine, with `k8sgpt serve --mcp` or `--mcp --mcp-http` as the client target

## K8sGPT usage notes

- Use `k8sgpt analyze --explain` for broad triage.
- Narrow with `--filter=Pod`, `--filter=Deployment`, or `--namespace=<ns>`.
- For assistant integration, prefer the MCP server mode (`k8sgpt serve --mcp`) and register it in Copilot/Claude-style MCP configs.
- Verified source: `/k8sgpt-ai/k8sgpt`

## NVMe and PCIe Power Management Quirks on Strix Halo (e.g., exo-0)

On Strix Halo platforms, DRAM-less NVMe controllers (like the Innogrit IG5220 / RainierQX) can experience active I/O timeouts (`QID 32 timeout`) and uninterruptible sleep state (`D` state) due to aggressive PCIe ASPM and APST power state transitions.

### 1. Diagnosis
Check for timeouts or blocked processes in kernel logs or list active file-locks on raw block devices:
```bash
# Check dmesg for NVMe errors
dmesg | grep -i nvme

# Find lock holders on raw disk
fuser -v /dev/nvme1n1
```

### 2. Remediation Workflow
Follow this precise order of operations to disable power management and reset the device at runtime without a host reboot:

1. **Set PCIe ASPM Policy to Performance**:
   ```bash
   echo performance > /sys/module/pcie_aspm/parameters/policy
   ```
2. **Reset the NVMe Controller**:
   ```bash
   echo 1 > /sys/class/nvme/nvme1/device/reset
   ```
3. **Disable Autonomous Power State Transition (APST)** on the drive:
   ```bash
   nvme set-feature -f 0x0c -V 0 /dev/nvme1
   ```
4. **Settle Udev Triggers**:
   ```bash
   udevadm settle
   ```

### 3. Optimal Formatting and Mounting (Btrfs to XFS Migration)

The 4TB local data NVMe drives (mounted at `/var/mnt/ghost-data` on `ghost` and `exo-0`) have been transitioned from Btrfs to XFS to optimize for container builds and BuildStream cache workloads.

XFS with `reflink=1` provides copy-on-write capability (e.g. `cp --reflink=auto`) identical to Btrfs, but without the high write metadata fragmentation and degradation under OverlayFS / loop-device pressure.

#### Optimal XFS Formatting:
Format the NVMe devices skipping hardware blocks pre-discards (`-K`) and explicitly enabling reflink/copy-on-write and CRC verification (`-m reflink=1,crc=1`):
```bash
mkfs.xfs -f -K -m reflink=1,crc=1 /dev/nvme1n1
```

#### Optimal XFS Mount Options:
For high-concurrency container building, use these options:
- `noatime,nodiratime`: Drastically reduces metadata write wear on the NVMe SSD.
- `logbufs=8,logbsize=256k`: Allocates 8 in-memory log buffers of 256KB to accelerate tiny file overlay metadata updates.
- `allocsize=64m`: Sets sequential disk preallocation block sizes to 64MB to optimize sequential virtual machine `.raw` disk block updates.

```ini
Options=defaults,noatime,nodiratime,logbufs=8,logbsize=256k,allocsize=64m
```

---

## 4TB SSD Migration Procedures (Btrfs to XFS)

### 1. Migrating `exo-0` (Ephemeral Cache Storage)
`exo-0` contains transient REAPI cache items under `/var/mnt/ghost-data`.

1. **Scale down cache pod**:
   ```bash
   kubectl scale deployment bst-artifact-server -n argo --replicas=0
   ```
2. **Stop and unmount unit on `exo-0`**:
   ```bash
   ssh core@192.168.1.170 "sudo systemctl stop 'var-mnt-ghost\x2ddata.mount'"
   ```
3. **Format to XFS**:
   ```bash
   ssh core@192.168.1.170 "sudo mkfs.xfs -f -K -m reflink=1,crc=1 /dev/nvme1n1"
   ```
4. **Update systemd mount file**:
   Edit `/etc/systemd/system/var-mnt-ghost\x2ddata.mount` on `exo-0` to specify XFS:
   ```ini
   [Mount]
   What=/dev/nvme1n1
   Where=/var/mnt/ghost-data
   Type=xfs
   Options=defaults,noatime,nodiratime,logbufs=8,logbsize=256k,allocsize=64m
   ```
5. **Reload systemd, mount, and enable**:
   ```bash
   ssh core@192.168.1.170 "sudo systemctl daemon-reload && sudo systemctl start 'var-mnt-ghost\x2ddata.mount' && sudo systemctl enable 'var-mnt-ghost\x2ddata.mount'"
   ```
6. **Recreate empty cache structure**:
   ```bash
   ssh core@192.168.1.170 "sudo mkdir -p /var/mnt/ghost-data/ac.v2 /var/mnt/ghost-data/cas.v2 /var/mnt/ghost-data/raw.v2 && sudo chmod -R 777 /var/mnt/ghost-data"
   ```
7. **Scale up deployment**:
   ```bash
   kubectl scale deployment bst-artifact-server -n argo --replicas=1
   ```

### 2. Migrating `ghost` (Stateful Control Plane Storage)
`ghost` holds persistent states like OCI cache layers in `zot-local` and persistent volume data in `local-path`. This data must be preserved.

1. **Verify destination space on `exo-0` XFS storage**:
   Make sure `exo-0` has sufficient disk space before starting the copy:
   ```bash
   ssh core@192.168.1.170 "df -h /var/mnt/ghost-data"
   ```
2. **Stop dependent workloads**:
   Scale down all services writing to `/var/mnt/ghost-data` (Zot registries, persistent volume consumers):
   ```bash
   kubectl scale deployment registry -n local-registry --replicas=0
   kubectl scale deployment zot-cache -n local-registry --replicas=0
   ```
3. **Stop K3s service on `ghost`**:
   Stop the container orchestrator to release all active container storage references, open file descriptors, and mount locks on `/var/mnt/ghost-data`:
   ```bash
   ssh jorge@192.168.1.102 "sudo systemctl stop k3s"
   ```
4. **Back up `/var/mnt/ghost-data` to `exo-0`**:
   Perform a root-elevated rsync using `sudo` and `--rsync-path="sudo rsync"` to preserve numeric UIDs, ACLs, and SELinux contexts over the 40G USB4 link:
   ```bash
   ssh jorge@192.168.1.102 "sudo rsync -aHAXxv --numeric-ids --rsync-path=\"sudo rsync\" /var/mnt/ghost-data/ core@192.168.1.170:/var/mnt/ghost-data/ghost-backup-temp/"
   ```
5. **Unmount on `ghost`**:
   Since K3s is stopped, the unmount will now succeed cleanly without "device is busy" failures:
   ```bash
   ssh jorge@192.168.1.102 "sudo umount /var/mnt/ghost-data"
   ```
6. **Format `ghost` NVMe to XFS**:
   ```bash
   ssh jorge@192.168.1.102 "sudo mkfs.xfs -f -K -m reflink=1,crc=1 /dev/nvme0n1"
   ```
7. **Update `/etc/fstab` on `ghost`**:
   Get the new UUID: `blkid /dev/nvme0n1`.
   Replace the old Btrfs entry in `/etc/fstab` with the new UUID, optimized options, and type `xfs`:
   ```text
   UUID=<new-uuid> /var/mnt/ghost-data xfs defaults,noatime,nodiratime,logbufs=8,logbsize=256k,allocsize=64m 0 0
   ```
8. **Mount `/var/mnt/ghost-data` on `ghost`**:
   ```bash
   ssh jorge@192.168.1.102 "sudo mount -a"
   ```
9. **Restore from backup**:
   Pull the backed-up directories back with precise attributes using root-elevated rsync:
   ```bash
   ssh jorge@192.168.1.102 "sudo rsync -aHAXxv --numeric-ids --rsync-path=\"sudo rsync\" core@192.168.1.170:/var/mnt/ghost-data/ghost-backup-temp/ /var/mnt/ghost-data/"
   ```
10. **Resume services**:
    Restart the K3s engine and scale up your workloads:
    ```bash
    ssh jorge@192.168.1.102 "sudo systemctl start k3s"
    kubectl scale deployment registry -n local-registry --replicas=1
    kubectl scale deployment zot-cache -n local-registry --replicas=1
    ```
11. **Clean up backup**:
    Once all pods are healthy and verified, clean up the backup folders on both hosts.

## BuildStream 2.x Distributed Builds and Caching

BuildStream 2.x has migrated to standard Remote Execution API (REAPI) Content Addressable Storage (CAS) protocols, replacing legacy python-based daemons with lightweight, high-performance, single-binary REAPI caching servers (e.g. `bazel-remote`).

### 1. REAPI CAS Backend (`bazel-remote`)
- **Image**: `quay.io/bazel-remote/bazel-remote`
- **Deployment**: Pinned to the high-speed Btrfs storage node (`exo-0` via `/var/mnt/ghost-data`).
- **Configuration**: Exposes gRPC on port `9092` and HTTP/1.1 on port `8080`.

### 2. Client-Side `buildbox-casd` Mandate
- BuildStream 2.x **cannot** initialize any remote cache (gRPC or HTTP) unless `buildbox-casd` is installed and available in the client's `PATH`. Standard python/pip container environments will fail with connection or type errors.
- Always use the mirrored `bst2` image (`192.168.1.102:30500/bst2:<tag>`) as the build-container runner, which includes both `buildbox-casd` and BuildStream 2.7.0.

### 3. Client-Side Configuration Layout
In `buildstream.conf`, project-specific remote caches must be nested within the project dictionary under the `servers:` list. Using an incorrect structure causes configuration type errors:

```yaml
projects:
  bst-prototype:
    artifacts:
      servers:
      - url: grpc://bst-artifact-server:9092
        push: true
```

### 4. YAML Scripting and Indentation Safety
When generating `buildstream.conf` dynamically inside an Argo YAML script block:
- **Avoid multiline heredocs (`cat << EOF`)**: Indented heredocs preserve spaces unless processed carefully, while non-indented lines violate YAML script structure.
- **Prefer explicit sequential writes**: Use `echo "..." > file` and `echo "..." >> file` for structured text generation. This completely eliminates YAML indentation parse bugs and is 100% robust.

### 5. `cache.projectbluefin.io` is reachable but slow — not "down"

Diagnosed 2026-07-03. `cache.projectbluefin.io:11001` (artifacts/source-caches) and
`:11002` (remote-execution, currently disabled) complete TLS handshakes fine from
both the homelab LAN and from inside the k3s cluster (ghost node). Port 443 refused
is a non-issue — nothing is meant to listen there.

The real symptom is **latency, not downtime**: in `projectbluefin/dakota`'s GitHub
Actions "Build Bluefin dakota" run logs, individual cache-miss lookups against
`cache.projectbluefin.io:11001`/`:11002` stall for 6–17+ minutes before BuildStream
logs `Remote (...) does not have artifact <ref> cached`. Confirmed via
`gh run view <id> --log-failed`, timestamp-diffing consecutive
`Pulling artifact`/`does not have ... cached` lines.

This is why **dakota builds on GitHub Actions are unreliable but the in-cluster
`dakota-commit-poller` build (this repo) works**: the GH Actions workflow's
`project.conf` points artifacts/source-caches at the remote Hetzner box
(`cache.projectbluefin.io`), which is degraded. The in-cluster BST build
(`dakota-build-pipeline`/`bst-qa-pipeline`) is configured local-first and
credential-free: it writes to `bst-artifact-server:9092` and explicitly overrides
project cache remotes (`override-project-caches: true`) so no push path depends
on external cache keys.

The dakota CI workflow (`.github/workflows/build.yml`) already documents related
CAS incidents inline: enabling `remote-execution` with a top-level
`cache.storage-service` previously caused a 3.5-hour gRPC flooding failure — hence
`enable-remote-execution: 'false'` and a global `concurrency: dakota-bst-build-global`
(max 1 build cluster-wide) because "the remote CAS at cache.projectbluefin.io is
rate-limited and supports a single build at a time."

**Do not conclude cache.projectbluefin.io is "broken" from connection-refused
tests on port 443 — test the actual BST ports (11001/11002) and check for
multi-minute stalls in build logs, not connectivity.** Filed as a human-priority
gap: see `projectbluefin/lab` issue tracker (CAS latency root cause — box
resourcing vs. network path — needs Hetzner-side investigation, not something
fixable from the k3s cluster).
