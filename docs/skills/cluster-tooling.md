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
    - /kubernetes/website
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
   - set `override-project-caches: false` to allow the project's own upstream caches (for example Freedesktop SDK and GNOME OS) to be used as read-only fallbacks, preventing extremely slow, full OS recompilations of basic bootstrap toolchains.
   - point artifact writes at the shared in-cluster Buildbarn frontend (`grpc://frontend.buildbarn.svc.cluster.local:8980`) while also listing the upstream artifact/source cache URLs as read-only fallback servers so BuildStream can pull prebuilt objects instead of recompiling bootstrap toolchains.
   - keep `source-caches` and `artifacts` populated with the project cache URLs rather than wiping them out; an empty server list forces BuildStream to rebuild bootstrap toolchains locally.
   - when the checkout uses upstream `gnome-build-meta`/`freedesktop-sdk` junctions, mirror their patch queues into the checkout before the build so the cache keys match the upstream remote caches instead of diverging on local patch-set differences.
   - keep BuildStream concurrency intentionally conservative for homelab lanes (`fetchers/builders/pushers: 1`, `build.max-jobs: 1`) so cache-backed builds finish without oversubscribing the cluster.
4. Validate workflow YAML with `just lint` before push.
5. Confirm live behavior from workflow logs/config output, not assumptions.
6. Never use a root filesystem for persistent workload data or a `hostPath` build
   cache. `manifests/local-path-config.yaml` is the GitOps source for explicit
   node-to-data-mount mappings. It intentionally has no default mapping, so
   PVC provisioning fails on an unconfigured node instead of falling back to
   that node's root disk.

## Ethernet mode (USB4 data plane down)

When the ghost<->exo-0 USB4 link is down (see RUNBOOK), all cross-node traffic
rides 2.5GbE. The cluster stays good at ingesting BST builds via:

- **Shared Buildbarn storage**: artifact and source caches use the scheduler-managed
  Buildbarn storage service. Do not add node-local `hostPath` caches; they bypass
  Kubernetes storage accounting and can fill a node root filesystem.
- **Build capacity is admitted, not assumed:** derive Buildbarn runner slots
  from live allocatable CPU, memory, and storage. Never reserve capacity by
  pinning a build to a node.
- **Zot pull-through** on every node (`registry-mirror-config` DaemonSet) keeps
  image pulls off the WAN and off cross-node paths.
- **BuildStream workspaces** use workflow PVCs. With `WaitForFirstConsumer`,
  Kubernetes selects a schedulable node and the local-path provisioner binds
  the PVC below that node's configured data mount. Do not select a node or
  bind-mount a cache path to influence placement.
- **Dakota lane policy:** `dakota-build-pipeline` defaults to `build-mode=cache-only`
  for ordinary builds so the lane stays on the local BuildStream path and avoids
  the 1-CPU RE coordinator pod. The explicit RE mode remains `build-mode=re`; the
  normal `auto` path is forced to `cache-only` so ordinary Dakota runs stay on
  the local cache-only lane unless someone is explicitly debugging the distributed
  path. The dakota commit poller also pins the checkout to the exact GitHub SHA it
  observed, so the lab build follows the same revision that GitHub is building.
- **Buildbarn RE sandbox device nodes**: `bb_runner` with
  `chrootIntoInputRoot: true` can fail when `/dev/null`, `/dev/zero`,
  `/dev/random`, and `/dev/urandom` are missing inside the chroot. The cluster-side
  fix is now in the manifests: `manifests/buildbarn-worker.yaml` creates a
  minimal `/worker/dev` tree with those nodes, and `manifests/buildbarn-config.yaml`
  sets `bb_runner.devDirectoryPath` to `/worker/dev`. That removes the old
  device-node failure mode without requiring a cache-only fallback for this
  specific issue.

Capacity guard: node memory *requests* must leave room for the 32Gi runner.
Orphaned 8Gi test VMs from failed image-poll runs are the usual thief — check
`kubectl describe node | grep -A8 "Allocated resources"` and delete VMs whose
parent workflow is terminal.

`manifests/orphan-vm-cleanup.yaml` runs every 30 minutes and deletes VMs whose
parent Argo workflow is gone or in a terminal phase. The cleanup looks up the
`argo-workflow` label in the VM's own namespace; image-poller and QA workflows
run in their test namespace, not in `argo`.

Also check for completed Jobs whose pods still reserve large memory requests;
Kubernetes counts a pod's request against node capacity until the Job (and its
pods) is deleted. Example: a finished `atspi-cacheonly` Job held a 14Gi request
and blocked Buildbarn storage scheduling on `exo-0` until the Job was removed.

## BST build scheduling: avoid preemption

BST build pods use the `bst-build` PriorityClass. Long cache-only builds lose all
progress when preempted, so `bst-build` (1,500,000) is set higher than
`lab-test-vm` (1,000,000). Builds therefore take scheduling precedence over
short-lived image-poll VMs, while still remaining below kubevirt/system critical
classes.

Symptom of the old lower-priority setting: `argo get` shows `pod deleted` and
`kubectl get events --field-selector reason=Preempted` lists the build pod
displaced by a VM pod. If this returns, verify the PriorityClass values:

```bash
kubectl get priorityclass bst-build lab-test-vm
```

Mitigations:

1. **Keep `bst-build` above `lab-test-vm`.** `manifests/bst-build-priorityclass.yaml`
owns the value. Raising it lets builds preempt polling VMs instead of the
reverse.

2. **Serialize high-memory BST variants.** Do not run two 14 GiB BST build pods
in parallel; the second pod may be forced onto a node without enough headroom.
The Dakota pipeline already runs base and NVIDIA variants sequentially.

3. **Limit the `bst-build` semaphore to 1.** The semaphore in
`manifests/workflow-semaphores.yaml` gates all BST build lanes. Set
`bst-build: "1"` so expensive cache-heavy builds queue instead of colliding.

4. **Verify the fix live.** After submission, confirm the pod is Running and on a
node with enough free requested memory:

```bash
kubectl get pod -n argo <pod-name> -o custom-columns='NODE:.spec.nodeName'
kubectl describe node <node> | grep -A8 "Allocated resources"
kubectl get events -n argo --field-selector reason=Preempted --sort-by='.lastTimestamp'
```

## Queueing, cleanup, and Buildbarn recovery

When the cluster is already hot, the fastest recovery is usually to stop the noise
instead of submitting more work:

1. Delete stale terminal workflows first; leave the newest healthy run in place.
2. Delete orphaned VMs/PVCs whose parent workflow is already terminal so they do
   not keep memory or storage reservations pinned.
3. Gate the expensive lane at the template level with the semaphores in
   `manifests/workflow-semaphores.yaml`; workflow-level mutexes are not enough for
   `workflowTemplateRef` / `templateRef` callers.
4. If Buildbarn storage pods stay `Pending` after a StatefulSet or PVC change, verify
   the PVC bindings and storage pods before resubmitting a build. The cluster health
   signal is `kubectl -n buildbarn get pvc` plus `kubectl -n buildbarn get pods`.

This pattern keeps the cluster from falling into a feedback loop where duplicate
pollers, stale build runs, and orphaned VMs all compete for the same memory and
storage budget.

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

## Common Rationalizations

- "Ghost has 64 GiB, so the build pod will fit."  
  Fitting is not the same as surviving. VM pods use a higher PriorityClass and
  will preempt a `bst-build` pod for memory. The pod gets deleted, the workflow
  retries, and the build never finishes.

- "I will just retry the workflow again."  
  Retries do not change the resource envelope. Fix the requests, limits, and
  concurrency budget, then retry; do not pin the pod to a preferred node.

- "Two variants should build in parallel to save time."  
  Parallel high-memory pods force one onto ghost where it is preempted. The
  wall-clock savings are lost to retries and partial work. Serialize first;
  parallelize only after the cluster has enough dedicated memory capacity.

- "The semaphore already limits concurrency."  
  The `bst-build` semaphore was set to 3, allowing multiple BST lanes to run
  at once. On a two-node lab where each pod requests 14 GiB, that causes
  collisions and preemptions. Set it to 1 and let the scheduler choose among
  nodes that can satisfy the declared requests.

## Red Flags

- `argo get` shows `pod deleted` for a BST build step.
- `kubectl get events --field-selector reason=Preempted` shows BST pods
  displaced by VM pods on `ghost`.
- Two BST build pods are `Running` at the same time with 14 GiB requests each.
- Builds repeatedly fail fast (seconds to a few minutes) without a build error
  in the container logs.

## Verification

- [ ] `just lint` passes after any WorkflowTemplate change.
- [ ] ArgoCD reports `Synced` for `testing-lab` after the push.
- [ ] The submitted build pod is scheduler-admitted without a node selector:
      `kubectl get pod -n argo <pod> -o jsonpath='{.spec.nodeName}'` returns
      a Ready node with adequate allocatable resources.
- [ ] `kubectl get configmap -n argo workflow-semaphores` shows
      `bst-build: "1"`.
- [ ] No `Preempted` events appear for the build pod after 10 minutes.
- [ ] The build progresses past source fetches into artifact pulls/builds.
- [ ] Workflow reaches `Succeeded`, or if it fails, the failure is a real build
      error (not `pod deleted`).

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

### 3. Permanent Bootloader Fix
For permanent stability across host reboots (especially on atomic host operating systems like Bluefin/Silverblue running on `ghost`), APST should be disabled in the kernel command-line. This completely prevents the drive from entering high-latency sleep states that trigger link dropouts under high parallel I/O.

Append the `nvme_core.default_ps_max_latency_us=0` parameter:
```bash
# On atomic hosts (rpm-ostree)
sudo rpm-ostree kargs --append-if-missing="nvme_core.default_ps_max_latency_us=0"
```

A reboot is required to activate the bootloader change. After reboot, verify the setting:
```bash
cat /sys/module/nvme_core/parameters/default_ps_max_latency_us
# Expected output: 0 (APST disabled)
```

### 3. Optimal Formatting and Mounting (Btrfs to XFS Migration)

The 4TB local data NVMe drives are mounted at `/var/mnt/ghost-data` on `ghost` and
`/var/mnt/exo0-data` on `exo-0` (node-specific names — do not reuse `ghost-data` as the
mount point name on other hosts, it refers to a specific disk on a specific machine, not
a cluster-wide convention). Both have been transitioned from Btrfs to XFS to optimize for
container builds and BuildStream cache workloads.

**Device path is not consistent across hosts by name alone — verify with `lsblk`/`blkid`
before touching any device.** On `ghost`, the 4TB data drive is `/dev/nvme0n1` and the
system disk is `/dev/nvme1n1`. On `exo-0`, the 4TB data drive is **also** `/dev/nvme0n1`
and the system disk is `/dev/nvme1n1` — the naming convention happens to match today, but
this has been a real source of error (see the corrected `exo-0` procedure below, which
previously pointed `mkfs.xfs`/the mount unit at `/dev/nvme1n1` — `exo-0`'s live *system*
disk — instead of `/dev/nvme0n1`, the actual 4TB drive). Never assume the device name;
confirm the model/size with `lsblk -o NAME,SIZE,MODEL` first.

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
`exo-0`'s 4TB drive is `/dev/nvme0n1`, mounted at `/var/mnt/exo0-data`; it holds
non-root local-path PVC data. **`/dev/nvme1n1` on `exo-0` is the live system disk
— never target it.**

1. **Scale down any legacy artifact-server deployment** if one still exists; current BuildStream lanes use the shared Buildbarn frontend and workers rather than a single `bst-artifact-server` pod (removed from `manifests/` — see git history if reviving).
2. **Stop and unmount unit on `exo-0`**:
   ```bash
   ssh core@192.168.1.170 "sudo systemctl stop 'var-mnt-exo0\x2ddata.mount'"
   ```
3. **Format to XFS** (only if not already XFS — check with `blkid` first, reformatting destroys data):
   ```bash
   ssh core@192.168.1.170 "sudo mkfs.xfs -f -K -m reflink=1,crc=1 /dev/nvme0n1"
   ```
4. **Update systemd mount file**:
   Edit `/etc/systemd/system/var-mnt-exo0\x2ddata.mount` on `exo-0` to specify XFS:
   ```ini
   [Mount]
   What=/dev/nvme0n1
   Where=/var/mnt/exo0-data
   Type=xfs
   Options=defaults,noatime,nodiratime,logbufs=8,logbsize=256k,allocsize=64m
   ```
5. **Reload systemd, mount, and enable**:
   ```bash
   ssh core@192.168.1.170 "sudo systemctl daemon-reload && sudo systemctl start 'var-mnt-exo0\x2ddata.mount' && sudo systemctl enable 'var-mnt-exo0\x2ddata.mount'"
   ```
6. **Recreate the non-root local-path base**:
   ```bash
   ssh core@192.168.1.170 "sudo mkdir -p /var/mnt/exo0-data/ac.v2 /var/mnt/exo0-data/cas.v2 /var/mnt/exo0-data/raw.v2 /var/mnt/exo0-data/bst-cache /var/mnt/exo0-data/local-path && sudo chmod 777 /var/mnt/exo0-data/bst-cache && sudo chmod 700 /var/mnt/exo0-data/local-path"
   ```
7. **Configure the local-path provisioner through GitOps**:
   `manifests/local-path-config.yaml` must list `exo-0` with
   `/var/mnt/exo0-data/local-path`. It must not contain
   `DEFAULT_PATH_FOR_NON_LISTED_NODES`; omitting that entry makes provisioning
   fail closed on future nodes until their non-root data mount is explicitly
   configured.
8. **Re-enable shared Buildbarn workloads** after the filesystem migration: confirm the Buildbarn frontend/scheduler/storage/worker pods are healthy before resuming heavy BST traffic.

### 2. Migrating `ghost` (Stateful Control Plane Storage)
`ghost` holds persistent states like OCI cache layers in `zot-local` and persistent volume data in `local-path`. This data must be preserved.

1. **Verify destination space on `exo-0` XFS storage**:
   Make sure `exo-0` has sufficient disk space before starting the copy:
   ```bash
   ssh core@192.168.1.170 "df -h /var/mnt/exo0-data"
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
   ssh jorge@192.168.1.102 "sudo rsync -aHAXxv --numeric-ids --rsync-path=\"sudo rsync\" /var/mnt/ghost-data/ core@192.168.1.170:/var/mnt/exo0-data/ghost-backup-temp/"
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
   ssh jorge@192.168.1.102 "sudo rsync -aHAXxv --numeric-ids --rsync-path=\"sudo rsync\" core@192.168.1.170:/var/mnt/exo0-data/ghost-backup-temp/ /var/mnt/ghost-data/"
   ```
10. **Resume services**:
    Restart the K3s engine and scale up your workloads:
    ```bash
    ssh jorge@192.168.1.102 "sudo systemctl start k3s"
    kubectl scale deployment registry -n local-registry --replicas=1
    kubectl scale deployment zot-cache -n local-registry --replicas=1
    ```
11. **Clean up backup**:
    Once all pods are healthy and verified, clean up the backup folders on both hosts:
    ```bash
    ssh core@192.168.1.170 "rm -rf /var/mnt/exo0-data/ghost-backup-temp"
    ```
    **This step is easy to skip and has been skipped before** — a stale
    `ghost-backup-temp/` directory was found still consuming ~286G on `exo-0`'s 4TB drive
    well after the migration it supported had completed. Verify with
    `ssh core@192.168.1.170 "du -sh /var/mnt/exo0-data/ghost-backup-temp"` before deleting,
    but don't leave it indefinitely — it silently eats into the same 4TB drive that
    `bst-cache` and `local-path` PVCs need.

### 3. Offloading Host User Caches and AI Datasets (ramalama, local buildstream, containers, and Flatpaks)

To prevent the `ghost` system root disk from filling up and strictly enforce the "4TB NVMe SSD for all workloads, no exceptions" mandate, heavy host-level user directories under `/var/home/jorge/` are relocated to `/var/mnt/ghost-data/` and replaced with symbolic links. Additionally, all host-level Flatpaks must be completely uninstalled to preserve precious system root disk space.

#### Host-Level Flatpak Removal:
All host-level Flatpaks (both system and user-level) must be completely uninstalled to prevent the root partition (under `/var/lib/flatpak` and `~/.local/share/flatpak`) from saturating. Run these commands to purge all Flatpaks from the host:

```bash
# Uninstall all user-level Flatpaks
flatpak uninstall --user --all -y

# Uninstall all system-level Flatpaks
flatpak uninstall --system --all -y

# Reclaim any dangling references or unused runtimes
flatpak uninstall --unused -y
```

Never install or run Flatpaks directly on the host; keep the host system thin and let container/VM workloads manage their own runtimes.

#### Target Paths on 4TB NVMe SSD:
All folders are stored under `/var/mnt/ghost-data/` with exact ownership of `jorge:jorge` (`1000:1000`) and linked back transparently:
- `/var/home/jorge/.local/share/ramalama` -> `/var/mnt/ghost-data/ramalama` (~278 GB)
- `/var/home/jorge/.cache/buildstream` -> `/var/mnt/ghost-data/user-bst-cache` (~244 GB)
- `/var/home/jorge/.local/share/containers` -> `/var/mnt/ghost-data/user-containers` (~78 GB)
- `/var/home/jorge/.lmstudio` -> `/var/mnt/ghost-data/lmstudio` (~30 GB)
- `/var/home/jorge/.cache/Homebrew` -> `/var/mnt/ghost-data/user-homebrew` (~18 GB)
- `/var/home/jorge/.cache/uv` -> `/var/mnt/ghost-data/user-uv` (~11 GB)

#### Relocation Procedure:
1. Create directories on the 4TB NVMe SSD and grant ownership to the host user:
   ```bash
   sudo mkdir -p /var/mnt/ghost-data/{ramalama,user-bst-cache,user-containers,lmstudio,user-homebrew,user-uv}
   sudo chown -R 1000:1000 /var/mnt/ghost-data/{ramalama,user-bst-cache,user-containers,lmstudio,user-homebrew,user-uv}
   ```
2. High-performance, attribute-preserving sync using rsync:
   ```bash
   rsync -aHAXx --numeric-ids /var/home/jorge/.local/share/ramalama/ /var/mnt/ghost-data/ramalama/
   rsync -aHAXx --numeric-ids /var/home/jorge/.cache/buildstream/ /var/mnt/ghost-data/user-bst-cache/
   rsync -aHAXx --numeric-ids /var/home/jorge/.local/share/containers/ /var/mnt/ghost-data/user-containers/
   rsync -aHAXx --numeric-ids /var/home/jorge/.lmstudio/ /var/mnt/ghost-data/lmstudio/
   rsync -aHAXx --numeric-ids /var/home/jorge/.cache/Homebrew/ /var/mnt/ghost-data/user-homebrew/
   rsync -aHAXx --numeric-ids /var/home/jorge/.cache/uv/ /var/mnt/ghost-data/user-uv/
   ```
3. Swap original directories with symbolic links:
   ```bash
   mv /var/home/jorge/.local/share/ramalama /var/home/jorge/.local/share/ramalama.bak
   ln -s /var/mnt/ghost-data/ramalama /var/home/jorge/.local/share/ramalama
   
   # Repeat for all other directories (buildstream, containers, lmstudio, Homebrew, uv)...
   ```
4. Verify symlinks are correct:
   ```bash
   ls -ld /var/home/jorge/.local/share/ramalama /var/home/jorge/.cache/buildstream /var/home/jorge/.local/share/containers /var/home/jorge/.lmstudio /var/home/jorge/.cache/Homebrew /var/home/jorge/.cache/uv
   ```
5. Safe deletion of backups once validated:
   ```bash
   rm -rf /var/home/jorge/.local/share/ramalama.bak /var/home/jorge/.cache/buildstream.bak /var/home/jorge/.local/share/containers.bak /var/home/jorge/.lmstudio.bak /var/home/jorge/.cache/Homebrew.bak /var/home/jorge/.cache/uv.bak
   ```

## BuildStream 2.x Distributed Builds and Caching

BuildStream 2.x uses the cluster's shared Buildbarn deployment for artifact
cache writeback and remote execution. Workflow-local state belongs on a
PVC-backed workspace, while the shared Buildbarn frontend provides cluster-wide
artifact reuse. Neither cache layer may use a root-backed `hostPath`.

### 0. USB4-gated remote execution (link-state fallback)

Remote execution fans action input roots out across nodes. That is only safe on
the ghost<->exo-0 USB4 data plane (10.99.0.0/30, table-40 policy routing) —
over 1 GbE it saturates the LAN and starves the control plane. The lab
therefore gates RE on live link state:

- `manifests/usb4-link-monitor.yaml`: DaemonSet on ghost + exo-0 annotates each
  node with `lab.projectbluefin.io/usb4-link: up|down` (operstate + carrier +
  TCP probe of the peer's kubelet over the tb subnet, every 15 s).
- `dakota-build-pipeline` runs a `detect-build-mode` step first: `re` only when
  both annotations are `up` (or `-p build-mode=re` forces it); anything else —
  including missing annotations — fails safe to `cache-only`.
- In `cache-only` mode bst builds locally in the pod and uses Buildbarn purely
  for artifact pulling/pushing.

> ⚠️ **CRITICAL SRE Operational Guidance on Thunderbolt Link Down and DNS Routing Override**:
> If the physical USB4/Thunderbolt link is down, the static table-40 policy routing rules persisted in NetworkManager on `exo-0` and `ghost` would match pod traffic destined for the other node's pod CIDR (e.g., `10.42.0.0/24` for ghost) and try to route it over `thunderbolt0` (which has `NO-CARRIER`), silently blackholing all cross-node pod-to-pod and DNS traffic.
>
> **The SRE Solution (DNS routed strictly over Ethernet)**:
> To isolate the control plane and make cluster discovery completely immune to USB4 link drops, DNS queries and responses (UDP/TCP port 53) are routed strictly over the Ethernet LAN interface. This is enforced at priority `5208` (higher than the USB4 route priority `5209`), steering port 53 traffic via the `main` table.
>
> This override is dynamically maintained and auto-injected every 15 seconds by the `usb4-link-monitor` DaemonSet using host namespaces:
> - Outbound queries/replies: `nsenter -t 1 -m -n -- /usr/sbin/ip rule add ipproto [udp|tcp] [dport|sport] 53 lookup main pref 5208`
>
> If the physical link is down, data-plane pod-to-pod traffic can be fell back to Ethernet by deactivating the NM connection:
> 1. On `exo-0`, deactivate the NM Thunderbolt connection: `sudo nmcli con down "Wired connection 2"`
> 2. On `ghost`, delete the stale routing rule: `sudo ip rule del priority 5209`
>
> All DNS queries and responses remain completely unaffected and healthy over Ethernet throughout, preventing cluster outages!
  as artifact/source cache (bounded transfers, ethernet-safe).
- Retries always force cache-only (`{{retries}} > 0`): a mid-build link drop
  fails the attempt and the retry rides the cache instead of RE.
- The RE endpoint snippet lives in the `remote-execution.conf` key of the
  `buildstream-remote-cache` ConfigMap; the baseline `dakota-buildstream.conf`
  is cache-only. Pipelines append the snippet per-project only in RE mode.
- Spec: `docs/superpowers/specs/2026-07-09-distributed-bst-usb4-fallback-design.md`

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

### 4. PVC-backed workflow workspace
Each BuildStream pod mounts a workflow PVC at `/root/.cache/buildstream` when
state must persist between workflow steps. It must use the `local-path` StorageClass
with the explicit GitOps node-to-data-mount mapping. Never use `/var/tmp`, a
root filesystem, or a node-local `hostPath` cache.

### 5. Buildbarn durable shard backup / restore

#### What is durable vs. disposable
- **Durable**: the `storage` StatefulSet's per-ordinal `local-path` PVCs. `manifests/buildbarn-storage.yaml` defines two replicas (`storage-0`, `storage-1`) with **required** podAntiAffinity, plus two PVCs per ordinal: `cas` mounted at `/storage-cas` and `ac` mounted at `/storage-ac`.
- **Not replicas of the same bytes**: `manifests/buildbarn-config.yaml` shards both CAS and AC across two equally weighted shards (`"0"` and `"1"` with `weight: 1` each). That means `storage-0` and `storage-1` each own part of the keyspace. Losing one shard without a backup loses roughly half of the CAS blobs and AC entries permanently.
- **Disposable**: workflow-local BuildStream PVC contents are safe to wipe after
  the workflow is terminal. Do **not** treat them as a substitute for backing
  up Buildbarn storage PVCs, and never replace them with a root-backed
  `hostPath`.

#### First inspect the live shard mapping
Command patterns in this subsection were verified against current Kubernetes docs via Context7 (`/kubernetes/website`).

Never assume yesterday's path layout is still true. Before any backup or restore, record the live PVC → PV → node → host-path mapping:

```bash
kubectl get configmap local-path-config -n kube-system -o jsonpath='{.data.config\.json}{"\n"}'

for claim in cas-storage-0 ac-storage-0 cas-storage-1 ac-storage-1; do
  pv=$(kubectl get pvc -n buildbarn "$claim" -o jsonpath='{.spec.volumeName}')
  kubectl get pv "$pv" -o jsonpath="$claim"' node={.spec.nodeAffinity.required.nodeSelectorTerms[0].matchExpressions[0].values[0]} path={.spec.local.path}{"\n"}'
done
```

`manifests/local-path-config.yaml` defines explicit per-node paths:
- `ghost` is mapped to `/var/mnt/ghost-data/local-path`
- `exo-0` is mapped to `/var/mnt/exo0-data/local-path`

This ensures that both nodes write their local-path persistent volume data directly to their respective 4TB NVMe SSD drives instead of the root system partition. Always verify the live configuration via:
`kubectl get configmap local-path-config -n kube-system -o jsonpath='{.data.config\.json}{"\n"}'`

#### Why `rsync --sparse` is the right tool here
This storage is **not** shaped like the old multi-million-file BuildStream cache. The live shard layout is sparse block-device files:
- `/storage-cas`: `blocks`, `key_location_map`, `persistent_state/state`
- `/storage-ac`: `blocks`, `key_location_map`, `persistent_state/state`

Use `rsync` with `--sparse`; do **not** use a naive `tar | ssh | tar` pipe that inflates sparse files and gives poor restartability.

#### Backup procedure
1. **Quiesce writers first.** Do not back up while BST jobs are actively pushing new CAS/AC entries.
   ```bash
   kubectl get workflows -n argo
   kubectl scale deployment/frontend deployment/scheduler deployment/bb-remote-asset -n buildbarn --replicas=0
   kubectl scale statefulset/storage -n buildbarn --replicas=0
   kubectl wait --for=delete pod -l app=storage -n buildbarn --timeout=180s
   ```
2. **Record the live PV paths** with the mapping commands above.
3. **Create backup roots on the opposite host** so one node loss does not take the live shard and its backup together.
   ```bash
   STAMP=$(date -u +%Y%m%dT%H%M%SZ)

   ssh core@192.168.1.170 "sudo mkdir -p /var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/cas /var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/ac"
   ssh jorge@192.168.1.102 "sudo mkdir -p /var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/cas /var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/ac"
   ```
4. **Back up `storage-1` (ghost) onto exo-0's 4TB drive.**
   ```bash
   ssh jorge@192.168.1.102 "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<cas-storage-1-pv-dir>/ core@192.168.1.170:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/cas/"
   ssh jorge@192.168.1.102 "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<ac-storage-1-pv-dir>/ core@192.168.1.170:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/ac/"
   ```
5. **Back up `storage-0` (exo-0) onto ghost.**
   ```bash
   ssh core@192.168.1.170 "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<cas-storage-0-pv-dir>/ jorge@192.168.1.102:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/cas/"
   ssh core@192.168.1.170 "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<ac-storage-0-pv-dir>/ jorge@192.168.1.102:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/ac/"
   ```
6. **Verify the copy before resuming traffic.**
   ```bash
   ssh jorge@192.168.1.102 "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<cas-storage-1-pv-dir>/ core@192.168.1.170:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/cas/"
   ssh jorge@192.168.1.102 "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<ac-storage-1-pv-dir>/ core@192.168.1.170:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/ac/"
   ssh core@192.168.1.170 "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<cas-storage-0-pv-dir>/ jorge@192.168.1.102:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/cas/"
   ssh core@192.168.1.170 "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<ac-storage-0-pv-dir>/ jorge@192.168.1.102:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/ac/"
   ```
   Then compare file lists and logical sizes on source vs. destination:
   ```bash
   sudo find <dir> -type f -printf '%P %s\n' | sort
   sudo du -sh <dir>
   sudo du -sh --apparent-size <dir>
   ```
   Expect the same three-file layout per volume (`blocks`, `key_location_map`, `persistent_state/state`) and matching apparent sizes.
7. **Bring Buildbarn back.**
   ```bash
   kubectl scale statefulset/storage -n buildbarn --replicas=2
   kubectl rollout status statefulset/storage -n buildbarn --timeout=180s
   kubectl scale deployment/frontend deployment/scheduler deployment/bb-remote-asset -n buildbarn --replicas=1
   kubectl rollout status deployment/frontend -n buildbarn --timeout=180s
   kubectl rollout status deployment/scheduler -n buildbarn --timeout=180s
   kubectl rollout status deployment/bb-remote-asset -n buildbarn --timeout=180s
   ```

#### Restore procedure
1. **Quiesce Buildbarn** using the same scale-down sequence as the backup procedure.
2. **Identify the failed ordinal and its old PVs.**
   ```bash
   kubectl get pvc -n buildbarn
   kubectl get pv | grep 'buildbarn/.*storage-[01]'
   ```
3. **Decide where the replacement shard should live before recreating PVCs.**
   - If you are restoring `storage-1` on `ghost`, the live path should stay under ghost's `local-path` base.
   - If you are restoring `storage-0` onto `exo-0`'s 4TB drive, first fix `local-path-config` so `exo-0` maps to `/var/mnt/exo0-data/local-path`; otherwise a recreated PV will land back on `/var/mnt/ghost-data/local-path` on `exo-0`'s system disk.
4. **Delete only the failed ordinal's retained PVCs/PVs** after confirming you have a good backup.
   ```bash
   kubectl delete pvc -n buildbarn cas-storage-0 ac-storage-0
   kubectl delete pv <cas-storage-0-pv> <ac-storage-0-pv>
   ```
   Substitute ordinal `1` if the ghost shard failed.
5. **Recreate fresh empty PVCs/PVs by scaling storage back up, then record the new host paths.**
   ```bash
   kubectl scale statefulset/storage -n buildbarn --replicas=2
   kubectl get pvc -n buildbarn -w
   ```
   Once the new claims are bound, rerun the PVC → PV → node → path lookup and capture the new target directories.
6. **Scale storage back down again before copying data into the fresh PV paths.**
   ```bash
   kubectl scale statefulset/storage -n buildbarn --replicas=0
   kubectl wait --for=delete pod -l app=storage -n buildbarn --timeout=180s
   ```
7. **Restore the backed-up shard into the new host directories.**
   ```bash
   sudo rsync -aHAXSx --numeric-ids --delete --info=progress2 <backup-root>/cas/ <new-cas-pv-path>/
   sudo rsync -aHAXSx --numeric-ids --delete --info=progress2 <backup-root>/ac/  <new-ac-pv-path>/
   ```
8. **Bring the storage shard back, then the clients.**
   ```bash
   kubectl scale statefulset/storage -n buildbarn --replicas=2
   kubectl rollout status statefulset/storage -n buildbarn --timeout=180s
   kubectl scale deployment/frontend deployment/scheduler deployment/bb-remote-asset -n buildbarn --replicas=1
   kubectl rollout status deployment/frontend -n buildbarn --timeout=180s
   kubectl rollout status deployment/scheduler -n buildbarn --timeout=180s
   kubectl rollout status deployment/bb-remote-asset -n buildbarn --timeout=180s
   kubectl get pods -n buildbarn -o wide
   kubectl get endpointslice -n buildbarn -l kubernetes.io/service-name=storage
   ```

#### Post-restore verification
- **Filesystem check**: rerun `find ... -printf '%P %s\n' | sort`, `du -sh`, and `du -sh --apparent-size` against the restored host paths and compare them with the backup copy.
- **Pod readiness**: `storage-0` and `storage-1` must both be `Running`, and `kubectl rollout status statefulset/storage -n buildbarn` must succeed.
- **Client reachability**: `frontend`, `scheduler`, and `bb-remote-asset` must be `Available`, and the `storage` headless Service must show endpoints for both storage pods.
- **End-to-end smoke test**: run one lightweight BST workflow that exercises CAS/AC and remote execution:
  ```bash
  argo submit -n argo --from workflowtemplate/bst-qa-pipeline --watch
  ```
  Do not declare the restore complete until that workflow succeeds against the restored shard.

### 6. Buildbarn message-size floor
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

## Wedged Node Triage Without SSH (2026-07-09 incident)

When a node shows `Ready` but pods on it are stuck Terminating/Pending for hours, do not trust
the Ready condition alone — check three signals via the k8s API:

1. **Lease vs conditions skew**: `kubectl get lease -n kube-node-lease <node>` fresh while
   `.status.conditions[].lastHeartbeatTime` is hours stale means the kubelet's lease goroutine
   is alive but its pod-sync and status loops are wedged.
2. **Kubelet's own pod view**: `kubectl get --raw /api/v1/nodes/<node>/proxy/pods` — if it
   returns 0 pods while the API server lists pods on the node, those API objects are orphans
   and containers are verifiably gone. That satisfies the documented precondition for
   `kubectl delete pod --grace-period=0 --force` (kubernetes/website:
   force-delete-stateful-set-pod.md — safe only when processes are confirmed terminated).
3. **SSH exec probe**: if a shell opens and builtins (`echo`) work but any binary exec hangs,
   host root-filesystem I/O is wedged (D-state) — only a power cycle fixes it. Cordon the node.

Cleanup order: cordon → force-delete orphaned pods (frees scheduler requests) → reschedule
movable workloads → for Released local-path PVs on the dead node, patch
`persistentVolumeReclaimPolicy: Retain` to stop the provisioner's helper-pod retry churn;
flip back to Delete after the node returns. Never delete the PV object while the node is down
(orphans the backing directory).

Known trigger: hostPID build pods SIGTERMing host daemons (issue #268) — the same event can
kill journald/sshd on workers and crash k3s on ghost (systemd restarts it; expect a short
API outage and a wave of `connection refused` workflow errors that self-heal on next cron tick).

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
