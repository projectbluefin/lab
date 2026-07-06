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
    - /apache/buildstream
---

# Cluster Tooling — lab

## When to Use

- Managing cluster state, infra add-ons, registry/cache services, or k8s ops runbooks.
- Debugging BuildStream cache behavior for Dakota/Cosmic/BST workflow lanes.

## When NOT to Use

- Argo WorkflowTemplate authoring details (`docs/skills/argo-workflows.md`).
- KubeVirt VM provisioning/test authoring workflows (`docs/skills/kubevirt-vms.md`, `docs/skills/test-authoring.md`).

## Core Process

1. Resolve tool/library docs in Context7 first (kubectl/k3s/K8sGPT/BuildStream as needed).
2. Prefer `just` recipes, then `kubectl`/`argo`, then host SSH only when k8s API cannot do it.
3. For BST lanes, configure local and upstream cache fallback in workflow configs:
   - never configure external cache credentials/keys in cluster workflows
   - set `override-project-caches: false` to allow falling back to upstream caches (like Freedesktop SDK and GNOME OS), preventing extremely slow, full OS recompilations of basic bootstrap toolchains.
   - point artifact writes at the shared in-cluster Buildbarn frontend (`grpc://frontend.buildbarn.svc.cluster.local:8980`) so local additions are cached across the cluster.
   - set `source-caches.servers: []` to keep source-cache configuration minimal.
4. Validate workflow YAML with `just lint` before push.
5. Confirm live behavior from workflow logs/config output, not assumptions.

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
- For this repo's `k8sgpt-on-demand` Argo template, keep intentionally-idle services in `ignored-services` (for example `llm-d/llm-d-modelserver` while `replicas: 0`) to avoid known false-positive "Service has no endpoints" noise during stabilization.
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

1. **Scale down any legacy artifact-server deployment** if one still exists; current BuildStream lanes use the shared Buildbarn frontend and workers rather than a single `bst-artifact-server` pod.
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
7. **Re-enable shared Buildbarn workloads** after the filesystem migration: confirm the Buildbarn frontend/scheduler/storage/worker pods are healthy before resuming heavy BST traffic.

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

BuildStream 2.x uses the cluster's shared Buildbarn deployment for artifact cache writeback and remote execution. The current design is a two-layer cache: a pod-local hostPath cache under `/root/.cache/buildstream` for fast per-pod state, and a shared Buildbarn frontend for cluster-wide artifact sharing.

### 1. Shared Buildbarn frontend
- **Endpoint**: `grpc://frontend.buildbarn.svc.cluster.local:8980`
- **Role**: CAS/AC artifact writes and reads; execute-forwarding for BuildStream actions that use the in-cluster execution grid
- **Deployment**: Frontend, scheduler, storage shards, and workers are defined under `manifests/buildbarn-*.yaml` and run in the `buildbarn` namespace

### 2. BuildStream client config
  The build pods should generate a deterministic `buildstream.conf` that keeps upstream project caches as read-only fallbacks and pushes artifacts to the shared Buildbarn frontend first. For memory-constrained homelab lanes, keep scheduler concurrency intentionally low (`fetchers: 1`, `builders: 1`, `pushers: 1`) and set `build.max-jobs: 1` so one pod does not blow through the node memory budget while uploading or compiling large bootstrap trees:
 
```yaml
scheduler:
  network-retries: 8
  fetchers: 1
  builders: 1
  pushers: 1
build:
  max-jobs: 1
artifacts:  override-project-caches: false
  servers:
  - url: grpc://frontend.buildbarn.svc.cluster.local:8980
    push: true
  - url: https://cache.projectbluefin.io:11001
    push: false
  - url: https://cache.freedesktop-sdk.io:11001
    push: false
  - url: https://gbm.gnome.org:11003
    push: false
source-caches:
  override-project-caches: false
  servers:
  - url: https://cache.projectbluefin.io:11001
    push: false
  - url: https://cache.freedesktop-sdk.io:11001
    push: false
  - url: https://gbm.gnome.org:11003
    push: false
```

Repeat the same override and server ordering at the project level so the primary project uses the same cache policy as the top-level config.

### 3. BuildStream parser constraints
- **No top-level `source:` key**: `buildstream.conf` does not support a top-level `source:` block.
- **Nested under `scheduler`**: fetch / retry / network settings belong under `scheduler:`.
- **Sequence writes in Argo scripts**: prefer `echo "..." >> file` over multiline heredocs when generating config in YAML script blocks.

### 4. Persistent pod-local cache
Each BuildStream pod should keep a persistent hostPath cache at `/root/.cache/buildstream` (for example `/var/tmp/bst-cache/<tag>`). This keeps the pod-local cache warm across retries and avoids losing the local work state between attempts while still allowing the shared Buildbarn frontend to serve cluster-wide artifact reuse.

### 5. Buildbarn message-size floor
BuildStream can issue large CAS upload batches while importing bootstrap seed artifacts. Keep the Buildbarn config's gRPC message size high enough for those uploads:

```jsonnet
maximumMessageSizeBytes: 64 * 1024 * 1024
```

If the value is too low, BuildStream lanes can fail with `Unable to upload <N> blobs to remote CAS`.
When `buildbarn-config` changes, also bump the `buildbarn-config-revision` pod-template annotations in:
- `manifests/buildbarn-frontend.yaml`
- `manifests/buildbarn-scheduler.yaml`
- `manifests/buildbarn-storage.yaml`
- `manifests/buildbarn-worker.yaml`

## Fedora CoreOS 44 (FCOS) Container Memory Limits and systemd-cgroup v2 Overhead

On Fedora CoreOS, containers scheduled using unified cgroups v2 and the `systemd` cgroup driver undergo scope registration via dbus. This triggers kernel memory allocations, systemd-user accounting, and auditing, which requires a baseline overhead of 12-20 MiB of memory before any user workload even executes.

### 1. Diagnosis
If containers or shims crash instantly during initialization with exit code `128`, check `kubectl describe pod` for:
`failed to create containerd task: ... OCI runtime create failed: container init was OOM-killed (memory limit too low?)`

### 2. Remediation
Always configure container memory limits well above this threshold for nodes running CoreOS.
- **Standard pause/sleep containers**: minimum `32Mi` memory limits (such as in `k3s-firewalld-config`, `mask-sleep-targets`, `registry-mirror-config`, and `inotify-tuning`).
- **Shell-based or kubectl utility containers**: minimum `64Mi` memory limits (such as in `virtio-console-module`).

### 3. SELinux Key Injection Warning
When running privileged pods that mount the host root `/` (`hostPath: /`) and write or create files (like writing public keys to `/home/core/.ssh/authorized_keys`), containerd applies container-specific SELinux labels (`container_file_t` or `home_root_t`). This prevents `sshd` from reading the keys on the host, resulting in `Permission denied (publickey)`.
- **Fix**: Run `nsenter -t 1 -m -u -i -n restorecon -R -v /home/core/.ssh` on the host OS from a privileged container to restore the correct `ssh_home_t` contexts.

## Common Rationalizations

- "It only touches cache config, no lint needed." → Wrong; run `just lint` for every workflow YAML change.
- "Project defaults are fine." → Wrong for this lab; project-defined remotes can re-enable external cache push paths.
- "Port 443 refused means cache host down." → Wrong; validate actual BST ports (`11001`/`11002`) and latency behavior.

## Red Flags

- BuildStream configs setting `override-project-caches: true` for pipelines that depend on upstream bootstrap artifacts (like Freedesktop SDK and GNOME OS meta), causing extremely slow and completely cold builds of the entire OS.
- Any BST lane includes external cache host URLs in generated config.
- Docs describe local-first but YAML still allows project cache remotes.

## Verification

- [ ] Workflow templates align `override-project-caches` to `false` for base fallback coverage.
- [ ] No external cache host appears in relevant workflow YAML/scripts.
- [ ] `just lint` passes after edits.
- [ ] Skill content reflects the current shared Buildbarn cache policy.
