---
name: cluster-buildstream
description: >
  USB4 admission rules, BuildStream distributed builds, and Buildbarn recovery.
---

# BuildStream and Distributed Builds

## USB4 is a hard BuildStream admission requirement

When the ghost<->exo-0 USB4 link is down (see RUNBOOK), all cross-node traffic
falls back to 2.5GbE, but **no BuildStream build may run**. Repair the link and
wait for fresh `lab.projectbluefin.io/usb4-link=up` observations on both nodes
before submitting or retrying. An Ethernet-backed, cache-only, runner-local, or
remote-cache-only run is not an acceptable substitute.

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
- **BST lane policy:** Dakota, COSMIC, and Bluefin Server accept only
  `build-mode=re`. Before admission, every lane requires a fresh USB4 `up`
  annotation and a Ready BuildBarn worker on both `ghost` and `exo-0`.
  Runner-local, cache-only, Ethernet-backed, automatic fallback, and
  remote-cache-only execution are prohibited. Before treating a run as
  distributed, verify its generated `projects.<name>.remote-execution`
  configuration, BuildStream RE startup, and current worker action activity.
  Dakota uses a two-slot `bst-build` semaphore and workflow-owned 200Gi
  `local-path` cache PVCs. `oci/bluefin-nvidia.bst` waits for its Bluefin parent
  artifact. The Dakota commit poller pins the checkout to the exact GitHub SHA
  it observed.
- **Buildbarn RE sandbox device nodes**: `bb_runner` with
  `chrootIntoInputRoot: true` can fail when `/dev/null`, `/dev/zero`,
  `/dev/random`, and `/dev/urandom` are missing inside the chroot. The cluster-side
  fix is now in the manifests: `manifests/buildbarn-worker.yaml` creates a
  minimal `/worker/dev` tree with those nodes, and `manifests/buildbarn-config.yaml`
  sets `bb_runner.devDirectoryPath` to `/worker/dev`. That removes the old
  device-node failure mode without requiring a cache-only fallback for this
  specific issue. A failing RE action must stop the build for repair; never
  route the work to a local or Ethernet-backed fallback.

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

BST build pods use the `bst-build` PriorityClass. Long distributed builds lose all
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

2. **Use verified parallel BST capacity.** Keep independent work concurrent when
BuildBarn workers and node requests have safe headroom. Respect actual
BuildStream graph dependencies: Dakota's NVIDIA image requires the Bluefin OCI
artifact, so it must wait for that artifact rather than racing an empty remote
cache. Reassess live worker and node capacity before raising or lowering the
two-slot `bst-build` limit.

3. **Clear stale semaphore holders before reducing capacity.** The semaphore in
`manifests/workflow-semaphores.yaml` gates all BST build lanes. Confirm terminal
workflows do not retain locks, then set `bst-build` to the safe live worker
capacity instead of serializing independent work by default.

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

## BuildStream 2.x Distributed Builds and Caching

BuildStream 2.x uses the cluster's shared Buildbarn deployment for artifact
cache writeback and remote execution. Workflow-local state belongs on a
PVC-backed workspace, while the shared Buildbarn frontend provides cluster-wide
artifact reuse. Neither cache layer may use a root-backed `hostPath`.

### 0. Mandatory remote execution

Dakota builds must use BuildBarn remote execution regardless of transport path.
If remote execution is unhealthy, fail the workflow, diagnose it, and repair the
grid; do not fall back to a runner-local or cache-only build. The
`remote-execution.conf` ConfigMap key is appended under the Dakota project in
the generated BuildStream configuration. A healthy run has two Ready workers,
two action slots, and observable current worker actions.

### 1. Shared Buildbarn frontend
- **Endpoint**: `grpc://frontend.buildbarn.svc.cluster.local:8980`
- **Role**: CAS/AC artifact writes and reads; execute-forwarding for BuildStream actions that use the in-cluster execution grid
- **Deployment**: Frontend, scheduler, storage shards, and workers are defined under `manifests/buildbarn-*.yaml` and run in the `buildbarn` namespace

### 2. BuildStream client config
  The build pods should generate a deterministic `buildstream.conf` that keeps upstream project caches as read-only fallbacks and pushes artifacts to the shared Buildbarn frontend first. Dakota's concurrency matches its two one-slot BuildBarn workers, while fetches remain bounded by coordinator capacity:
 
```yaml
scheduler:
  network-retries: 8
  fetchers: 4
  builders: 2
  pushers: 2
build:
  max-jobs: 8
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
  - url: grpc://bb-remote-asset.buildbarn.svc.cluster.local:8984
    type: index
    push: true
  - url: grpc://frontend.buildbarn.svc.cluster.local:8980
    type: storage
    push: true
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

   ssh core@<worker-ip> "sudo mkdir -p /var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/cas /var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/ac"
   ssh core@<lab-ip> "sudo mkdir -p /var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/cas /var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/ac"
   ```
4. **Back up `storage-1` (ghost) onto exo-0's 4TB drive.**
   ```bash
   ssh core@<lab-ip> "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<cas-storage-1-pv-dir>/ core@<worker-ip>:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/cas/"
   ssh core@<lab-ip> "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<ac-storage-1-pv-dir>/ core@<worker-ip>:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/ac/"
   ```
5. **Back up `storage-0` (exo-0) onto ghost.**
   ```bash
   ssh core@<worker-ip> "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<cas-storage-0-pv-dir>/ core@<lab-ip>:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/cas/"
   ssh core@<worker-ip> "sudo rsync -aHAXSx --numeric-ids --info=progress2 -e 'ssh -c aes128-gcm@openssh.com' /var/mnt/ghost-data/local-path/<ac-storage-0-pv-dir>/ core@<lab-ip>:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/ac/"
   ```
6. **Verify the copy before resuming traffic.**
   ```bash
   ssh core@<lab-ip> "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<cas-storage-1-pv-dir>/ core@<worker-ip>:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/cas/"
   ssh core@<lab-ip> "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<ac-storage-1-pv-dir>/ core@<worker-ip>:/var/mnt/exo0-data/buildbarn-backups/storage-1/${STAMP}/ac/"
   ssh core@<worker-ip> "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<cas-storage-0-pv-dir>/ core@<lab-ip>:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/cas/"
   ssh core@<worker-ip> "sudo rsync -aHAXSxn --delete /var/mnt/ghost-data/local-path/<ac-storage-0-pv-dir>/ core@<lab-ip>:/var/mnt/ghost-data/buildbarn-backups/storage-0/${STAMP}/ac/"
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

