---
name: cluster-buildstream
description: >
  Use when operating USB4 admission, BuildStream distributed builds, BuildGrid
  remote execution, or Buildbarn cache recovery.
metadata:
  context7-sources:
    - /apache/buildstream
---

# BuildStream and Distributed Builds

## USB4 is a hard BuildStream admission requirement

When the ghost<->exo-0 USB4 link is down (see RUNBOOK), all cross-node traffic
falls back to 2.5GbE, but **no BuildStream build may run**. Repair the link and
wait for fresh `lab.projectbluefin.io/usb4-link=up` observations on both nodes
before submitting or retrying. An Ethernet-backed, cache-only, local-sandbox, or
remote-cache-only run is not an acceptable substitute.

- **Shared Buildbarn storage**: artifact and source caches use the Buildbarn
  CAS/action-cache storage service. Do not add node-local `hostPath` caches; they bypass
  Kubernetes storage accounting and can fill a node root filesystem.
- **Build capacity is admitted, not assumed:** execution capacity is the sum of
  BuildGrid worker slots (one worker per node, `CONCURRENT_JOBS` each). Never
  reserve capacity by pinning a build to a node.
- **Zot pull-through** on every node (`registry-mirror-config` DaemonSet) keeps
  image pulls off the WAN and off cross-node paths.
- **BuildStream workspaces** use workflow PVCs. With `WaitForFirstConsumer`,
  Kubernetes selects a schedulable node and the local-path provisioner binds
  the PVC below that node's configured data mount. Do not select a node or
  bind-mount a cache path to influence placement.
- **BST lane policy:** Dakota, Bluefin Server, and QA pipelines accept only
  remote execution. Before admission, every lane requires a fresh USB4 `up`
  label and annotation (`lab.projectbluefin.io/usb4-link=up`, timestamp within 60s),
  a Ready BuildGrid worker on both `ghost` and `exo-0`, and a Ready BuildGrid
  controller. Additional nodes only add workers.
  Local-sandbox, cache-only, Ethernet-backed, automatic fallback, and
  remote-cache-only execution are prohibited. Before treating a run as
  distributed, verify its generated `projects.<name>.remote-execution`
  configuration, BuildStream RE startup, and BuildGrid `jobs` rows for the run.
  Pipelines fail closed immediately with an explicit rejection when the USB4
  link is unavailable or stale rather than queueing indefinitely in the scheduler.
- **Verifying USB4 admission state:** The `usb4-link-monitor` DaemonSet evaluates
  link health every 15s and publishes both the node label `lab.projectbluefin.io/usb4-link=up|down`
  and annotations (`lab.projectbluefin.io/usb4-link` and timestamp
  `lab.projectbluefin.io/usb4-link-observed-at`). Check state before submitting:
  ```bash
  kubectl get nodes -L lab.projectbluefin.io/usb4-link
  ```
  or verify the label directly:
  ```bash
  kubectl get nodes --show-labels | grep -o 'usb4-link=[a-z]*'
  ```
  The `bst-build` semaphore in `manifests/workflow-semaphores.yaml` is `"2"`:
  two BuildStream pipelines share the grid at once, and each pipeline's variants
  run in parallel inside it. Workflows use workflow-owned 200Gi
  `local-path` cache PVCs. The distributed workflow builds both `oci/bluefin.bst`
  (`dakota:testing`) and `oci/bluefin-nvidia.bst` (`dakota-nvidia:testing`). NVIDIA
  builds run in parallel with continueOn (non-blocking) as the lab does not have GPU
  hardware to test NVIDIA runtime execution. The Dakota
  commit poller pins the checkout to the exact GitHub SHA
  it observed.

## Execution model

BuildStream remote execution runs on **BuildGrid** with **buildbox** workers;
Buildbarn is storage only.

- **Scheduler front ends:** `manifests/buildgrid-controller.yaml`, namespace
  `buildgrid`. Deployment `controller`
  (`controller.buildgrid.svc.cluster.local:50051`) serves Execution and
  Operations to BuildStream and to nested recc actions; each queued or
  running action holds a streaming Execute RPC, so `thread-pool-size` and
  `maximum-concurrent-rpcs` are 6000. Deployment `bots`
  (`bots.buildgrid.svc.cluster.local:50051`) serves only the Remote Workers API.
  Keep them separate: with Bots co-located, a recc fan-out exhausts the RPC
  limit, `UpdateBotSession` fails with `Concurrent RPC limit exceeded`, and
  workers exit mid-action. Both share one scheduler (identical config, enforced
  by `tests/unit/test_workflow_defaults.py`) whose state lives in Postgres
  (`manifests/buildgrid-database.yaml`, StatefulSet `database`, local-path PVC,
  trust auth reachable only from pods labelled `buildgrid-scheduler-client`).
  Platform matching compares `OSFamily` and `ISA`; `unixUID`, `unixGID`,
  `network`, `remoteApisSocketPath`, `chrootRootDigest`, and `capability` are
  accepted as wildcard keys.
- **Workers:** DaemonSet `worker` in `buildgrid`
  (`manifests/buildgrid-worker.yaml`), one pod per node. Each pod runs
  `buildbox-casd` and `buildbox-worker` in one privileged container. The runner
  is `buildbox-run-bubblewrap`: each action gets a private PID namespace, a
  fresh `/proc`, a `/dev`, and its input root as `/`, staged through FUSE.
  `--concurrent-jobs` comes from the `CONCURRENT_JOBS` env (32). casd keeps its
  cache on a per-pod generic-ephemeral local-path PVC (200Gi, quota 150G) and
  logs at `warning` (at `info` it logs every RPC and pushes worker failures out
  of `kubectl logs` within minutes). The container's task limit is host-level:
  see `docs/ops/k3s-tuning.md` "Container task limit".
- **Storage:** Buildbarn (`grpc://frontend.buildbarn.svc.cluster.local:8980`,
  two sharded storage replicas, plus `bb-remote-asset`) is the only CAS and
  action cache. Worker casd uses Buildbarn AC directly but reaches CAS through
  the BuildGrid CAS front (`manifests/buildgrid-cas.yaml`, Deployment `cas`,
  Service `cas.buildgrid.svc.cluster.local:50051`, stateless, 2 replicas).
  `buildbox-run-bubblewrap` resolves recc actions' `chrootRootDigest` with
  REAPI `GetTree`, which Buildbarn's frontend does not implement
  (`UNIMPLEMENTED: This service does not support downloading directory trees`);
  the BuildGrid CAS front serves `GetTree` itself and stores every blob in
  Buildbarn. casd forwards nested Execute requests (recc via BuildStream's
  `remote-apis-socket`) to the BuildGrid controller.
- **Action cache:** BuildStream element actions request
  `OutputDirectoryFormat` `DIRECTORY_ONLY`, so their results carry
  `root_directory_digest` and no `tree_digest`. bb-storage's
  `completenessChecking` rejects that as `malformed digest: No digest provided`,
  which made every element action re-execute; only recc compiles (output files)
  hit. The frontend AC therefore uses `actionResultExpiring` (7 days plus up to
  1 day jitter after `worker_completed_timestamp`) instead, a window that must
  stay shorter than the CAS retention. Prove an element hit with
  `GetActionResult` on an element action digest from the BuildGrid `jobs` table.
- **BuildStream config** (`manifests/buildstream-remote-cache-config.yaml`):
  `execution-service` points at the BuildGrid controller and
  `action-cache-service` at the Buildbarn frontend. The CAS is the global
  `cache.storage-service` (Buildbarn frontend); `remote-execution` sets no
  `storage-service` of its own, because one there makes BuildStream download
  every remote build's outputs to the coordinator.

This matches upstream BuildStream's own CI: `.github/compose/ci.buildgrid.yml`
runs BuildGrid with `buildbox-worker --buildbox-run=buildbox-run-bubblewrap`,
and `ci.buildbarn.yml` uses Buildbarn only as a cache. `buildbox-run-bubblewrap`
(`run-bubblewrap/buildboxrun_bubblewrap.cpp`) passes `--unshare-pid` and
`--proc /proc`. Buildbarn's chroot runners still lack procfs upstream
([bb-remote-execution#115](https://github.com/buildbarn/bb-remote-execution/issues/115),
PR #116), which is why Buildbarn is not used for execution.

**Invariant: build the exact upstream commit.** Lab pipelines build the pinned
dakota (and gnome-build-meta) sources unmodified. Never patch elements,
junctions, or upstream patch queues to work around the execution sandbox. A
build that fails only in the lab sandbox is a runner bug: fix it in the grid
(worker, runner, or controller configuration), not in the project. Upstream
GNOME `recc` defaults apply when the pinned gnome-build-meta declares them.
Do not bind the host's `/proc` or other host files into the input root; it
breaks hermeticity.

## Scale-out

- Adding a node adds a worker: the DaemonSet schedules a pod, the pod opens a
  bot session with the controller, and BuildGrid's queue hands it actions. That
  is the only automatic part. The USB4 data path is per-peer and manual: each
  host has a static `ip rule ... to <peer pod CIDR> lookup 40` route over
  `thunderbolt0`, and `usb4-link-monitor` maps only `ghost` and `exo-0` to
  peers (any other node exits 1). A new node's cross-node traffic stays on
  2.5GbE until both are extended.
- Nodes run at full sustained CPU power: `manifests/node-performance-tuning.yaml`
  holds ACPI `platform_profile=performance` and amd-pstate EPP `performance`
  on every node, re-applying every 60s because reboots, tuned, and
  power-profiles-daemon reset them. Under the firmware `balanced` profile the
  Strix Halo nodes run at a lower package power limit. Check with
  `kubectl -n kube-system logs ds/node-performance-tuning`: it logs only the
  settings it had to change.
- Work spreads by fan-out: with gnome-build-meta's `recc: remote-execution`,
  each element action submits its compiles as separate actions, so one element
  can queue hundreds of actions that any worker drains. `scheduler.builders` (12)
  only bounds element actions in flight; `build.max-jobs` (32, one node's
  threads) is the make/cargo parallelism inside each action. A non-recc element
  such as the kernel is one action, so `max-jobs` caps it to that many threads.
  The value stays out of cache keys (environment-nocache), so changing it
  keeps every upstream artifact a cache hit.
- An element action holds a worker slot while it waits for its own compiles.
  Each `bst-build` lane runs at most one coordinator per node, so keep lanes ×
  `builders` below one node's `CONCURRENT_JOBS` (32) and element actions can
  never occupy every slot; `tests/unit/test_workflow_defaults.py` enforces it.
- Every queued or running action holds one streaming Execute RPC on the
  controller, so `maximum-concurrent-rpcs` (6000) bounds queue depth, not
  worker slots. Past it the controller answers `RESOURCE_EXHAUSTED`
  (`Concurrent RPC limit exceeded`), which recc does not retry, so the compile
  and its element fail. recc releases its jobserver token while it waits on
  the grid, so a recc element queues every ready compile regardless of
  `max-jobs` (gtk submitted 1531). Replaying that load at `max-jobs` 32 gives
  up to ~1860 in-flight RPCs per lane; `tests/unit/test_workflow_defaults.py`
  requires `maximum-concurrent-rpcs >= lanes × 1860`. Waiting streams hold no
  database connection: the scheduler SQL pools (controller 40+30, bots 10+10)
  must stay under Postgres `max_connections` (100) regardless of the RPC limit.
- A worker container's task limit (`pids.max`, derived from systemd
  `DefaultTasksMax`) also bounds fan-out: a wide recc fan-out exhausted it on
  ghost, `buildbox-worker` crashed with `EAGAIN`, and actions failed with
  `bwrap: Can't fork`.
- Queued actions wait in BuildGrid, not in BuildStream or Argo. A deep queue
  with all bots busy means the grid is saturated, not broken.
- Assignment spreads by remaining capacity. Both scheduler blocks configure
  `!priority-age-assigner` with `!assign-by-capacity` sampling. Without
  sampling, BuildGrid leases to the first healthy bot with a free slot, so one
  node fills all its slots with `-j12` element builds while the other idles.
  Check the per-run split with
  `select worker_name, count(*) from jobs where queued_timestamp > '<run start>' group by 1;`.
  An all-time count hides per-run skew.
- Workers have no CPU limit: a CFS quota throttles compile bursts even when
  cores are idle. Their CPU request keeps a fair share against other pods.

## Operating BuildGrid

Health checks:

```bash
kubectl -n buildgrid get pods -o wide            # controller, database-0, one worker per node
kubectl -n buildgrid rollout status deployment/controller
kubectl -n buildgrid rollout status statefulset/database
kubectl -n buildgrid rollout status daemonset/worker
# Bot sessions (one per worker pod; bot_id is the node name)
kubectl -n buildgrid exec database-0 -- psql -U bgd -d bgd -At -c "select bot_id, bot_status from bots;"
# Recent actions and which worker ran them
kubectl -n buildgrid exec database-0 -- psql -U bgd -d bgd -At -c "select worker_name, stage, status_code from jobs order by queued_timestamp desc limit 20;"
```

A run is distributed when its actions appear in `jobs` with a `worker_name`.
A missing bot row for a node means its worker pod is not Ready or cannot reach
the controller; check that node's `worker` pod logs.

Sandbox smoke test: submit a throwaway Workflow that builds a `manual` element
whose `build-commands` read `/proc/cpuinfo`, `readlink /proc/self/exe`,
`ls /dev/fd`, and compile a file with `gcc`, then confirm the action ran
remotely in the `jobs` table.

The controller logs `Unable to get action input size` with `RESOURCE_EXHAUSTED
... Received message larger than max (N vs. 4194304)` for large element input
roots. It is a metrics walk after the job is queued; the exception is caught
and the Execute succeeds. `channel-options` on `!remote-storage` and
`!remote-action-cache` parse but do not fix it: BuildGrid (0.8.13 and current
master) stores them and never passes them to `setup_channel`, so the client
keeps gRPC's 4 MiB receive limit.

Cache hits are not execution. An unmodified dakota commit that dakota CI has
already built resolves `oci/bluefin.bst` from `cache.projectbluefin.io`
(`Build Queue: processed 0`), so a pipeline success alone proves nothing about
the grid. To prove execution for real elements, run a throwaway Workflow that
(1) `bst artifact pull --deps build <targets>` with every artifact cache
configured, then (2) `bst build <targets>` with
`artifacts: {override-project-caches: true, servers: []}` and the BuildGrid
`remote-execution` block, and count the run's `jobs` rows per `worker_name`.
With gnome-build-meta's default `recc: remote-execution`, one element fans out
into hundreds of compile actions spread across every worker.

An element action that is answered from the action cache creates no new
`jobs` row. Element results hit only because the frontend AC uses
`actionResultExpiring` (see Action cache above); a run that re-executes an
unchanged element (same `action_digest` in `jobs` twice) means the AC lookup
missed.

### Where a build's wall time goes

Phase costs read from `bst-build-re` coordinator logs (`main` and `publish`
containers) of Dakota `oci/bluefin.bst` runs:

| Phase | Cost | Notes |
| --- | --- | --- |
| Pod start, clone, config | ~25 s | two `detect-build-mode` pods, then a depth-1 clone |
| Load, resolve, remote init | 13-15 s | junction and plugin sources (gnome-build-meta, freedesktop-sdk, PyPI plugins) are re-fetched every run in ~5 s; not worth caching |
| Pull, lab cache warm | ~16 s | ~880 artifacts, only protos and Directory blobs |
| Pull, first run after a junction bump | ~4 min | ~470 artifacts come from `gbm.gnome.org` over the WAN; pulls and source fetches share the 8 `--fetchers` slots, all busy the whole time |
| Kernel source fetch | 1.5-3 min | Launchpad git plus ~30 s adding 6385 objects; no lab source cache, so every run that builds the kernel pays it on the critical path |
| Kernel build | ~8.5 min | one action |
| initramfs, layers, `oci/bluefin.bst` | ~1 + 1-2.5 + ~3 min | sequential tail after the kernel |
| Export (`bst artifact checkout`) | casd fetch ~15 s, then a local copy | the OCI layout is one ~10 GB uncompressed layer blob; 2.5 min (4+ min with four exports at once) under `GRPC_POLL_STRATEGY=poll` |
| Publish (skopeo, 16 CPU) | 15-20 s | compresses to a ~4.3 GB gzip layer |

A fully cached run is pull plus export plus publish. When dakota CI already
built the commit, the final `oci/*` artifacts come from
`cache.projectbluefin.io` over the WAN (50 s-3 min) and are pushed to the lab.

The coordinator container must not set `GRPC_POLL_STRATEGY=poll`.
buildbox-casd inherits the coordinator's environment, and under the `poll`
poller each of its gRPC streams tops out near 68 MB/s: casd `FetchTree` of the
10 GB layer took 144 s with `poll` and 15 s without, and a plain Python
ByteStream read of the same blob from the frontend measured 68 MB/s against
710 MB/s. Every large blob casd moves (export, big pulls, input-root uploads)
pays that cap. BuildStream 2 runs jobs in threads and forks plugin helpers
from a forkserver, so it does not need `poll`.

Read these timings from the pod logs; build pipelines keep their pods 2h after
the workflow ends (`podGC` in the template).

### Diagnosing a failed remote element build

The coordinator pod log is the only place BuildStream prints a remote build's
output, and only the last `--error-lines` lines of it. The build step passes
`--error-lines 2000`; at the default 20 a `-j32` failure shows trailing
warnings and no error. When even 2000 lines are not enough, read the full
stdout/stderr from Buildbarn. The `jobs.result` column is the digest of the
`ExecuteResponse`; its `ActionResult` names the stdout and stderr blobs:

```bash
kubectl -n buildgrid exec database-0 -- psql -U bgd -d bgd -At -c \
  "select name, worker_name, status_code, result from jobs where stage=4 and queued_timestamp > '<run start>' order by queued_timestamp desc limit 20;"
kubectl -n buildgrid exec -i deploy/controller -- python3 - <result-digest> <<'EOF'
import sys, grpc
from buildgrid._protos.build.bazel.remote.execution.v2 import remote_execution_pb2 as re
from buildgrid._protos.google.bytestream import bytestream_pb2 as bs, bytestream_pb2_grpc as bsg
st = bsg.ByteStreamStub(grpc.insecure_channel("frontend.buildbarn.svc.cluster.local:8980"))
def blob(d):
    return b"".join(r.data for r in st.Read(bs.ReadRequest(resource_name=f"blobs/{d}")))
r = re.ExecuteResponse(); r.ParseFromString(blob(sys.argv[1]))
print("exit", r.result.exit_code, "status", r.status.code, r.status.message)
for d in (r.result.stdout_digest, r.result.stderr_digest):
    if d.size_bytes: sys.stdout.write(blob(f"{d.hash}/{d.size_bytes}").decode(errors="replace"))
EOF
```

A non-zero `status.code` with exit 0 is a grid failure (4 = deadline, 14 =
unavailable): read that node's `worker` pod log, not the element. casd logs at
`info` rotate in roughly 20 minutes, so collect worker evidence immediately.

### recc outside gnome-build-meta

Only gnome-build-meta elements declare `recc: remote-execution`. Do not add
recc to other elements (for example the kernel) to spread their compiles: it
measured no wall-time gain over one `-j32` action on a 32-thread node, and its
fan-out is what exhausts controller RPCs and worker `pids.max`. A non-recc
element is one action; give it the node's threads through `max-jobs`.

### Deploy windows for grid-restarting changes

Changes that restart BuildGrid (controller, bots, cas, database, worker),
Buildbarn, or Zot pods kill every in-flight action and fail running builds;
a worker restart also wipes its casd cache (generic-ephemeral PVC). Land them
only when `kubectl -n argo get wf -l bluefin.io/bst-workload=true
--field-selector status.phase=Running` is empty, and batch them into one
window. WorkflowTemplates, `workflow-semaphores`, the
`buildstream-remote-cache` ConfigMap, docs, and tests restart no grid pod and
can land at any time; a running workflow keeps the template snapshot it was
submitted with.

**Images.** Upstream publishes only `:nightly` for `buildgrid`, `buildbox`, and
`buildgrid-postgres` at `registry.gitlab.com/buildgrid/buildgrid.hub.docker.com`.
Mirror a specific digest into the lab Zot in OCI format and pin manifests by
digest:

```bash
skopeo copy --format oci --dest-tls-verify=false \
  docker://registry.gitlab.com/buildgrid/buildgrid.hub.docker.com/<name>@<digest> \
  docker://192.168.1.102:30500/buildgrid/<name>:nightly-YYYYMMDD
```

Zot rejects docker v2s2 manifests with `--preserve-digests`, so convert to OCI
and pin the resulting Zot digest. Moving these images to fsdk-containers per
the image policy remains the long-term goal.

**Worker concurrency.** To change slots per node, edit `CONCURRENT_JOBS` in
`manifests/buildgrid-worker.yaml` and let GitOps roll the DaemonSet. Keep
`bst-build` lanes × `scheduler.builders` below the new `CONCURRENT_JOBS`, and
check node CPU/memory headroom against `max-jobs` per action.

Capacity guard: node memory *requests* must leave room for the BuildGrid
worker pods and the BuildStream coordinator. Orphaned 8Gi test VMs from
failed image-poll runs are the usual thief — check
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

## Lessons for future agents

1. **Verify admission before debugging compilation.** Fresh USB4 timestamps, node
   readiness, Ready BuildGrid workers on `ghost` and `exo-0`, a Ready controller,
   and workflow resource requests/limits are prerequisites. A graph-validation
   rejection caused by a step lacking resources is an admission failure, not a
   Dakota compiler result.
2. **Separate evidence classes.** Local BuildStream success, Podman/Zot push
   success, and container E2E are useful diagnostics, but none substitutes for
   a successful distributed remote-execution build. Never report overall green
   from those results alone.
3. **Check live-vs-git configuration.** Before retrying, compare the live
   `buildstream-remote-cache` ConfigMap, BuildGrid controller config, and worker
   pods with the repository. Reconcile through GitOps and wait for the worker
   DaemonSet rollout before testing. Argo retries retain the original template
   snapshot, so stop deterministic retries until the corrected template reconciles.
4. **Fix sandbox failures in the grid.** If an element builds upstream but fails
   only under lab remote execution, reproduce it with the sandbox smoke test and
   fix the runner or worker configuration. Do not patch the element.
5. **Do not tune capacity around a correctness failure.** Low utilization during
   a failed action is expected and is not a reason to add workers or jobs.
6. **Use the lab pipeline for Dakota testing images, never GitHub Actions.**
   Work in the lab repository and cluster is meant to validate Dakota builds locally
   via `just run-bst-build` (`argo/workflow-templates/dakota-build-pipeline.yaml`)
   targeting the local Buildbarn grid and the lab-local Zot registry configured by the WorkflowTemplate.
   Do not trigger external GitHub Actions workflows on `projectbluefin/dakota`
   (such as opening pull requests to run `build.yml` or relying on GHCR) to obtain test images.
   The lab exists specifically to build, test, and validate changes hermetically
   without consuming upstream GitHub Actions quota or cluttering upstream PR queues.

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
BuildGrid workers and node requests have safe headroom. Respect actual BuildStream
graph dependencies. The `bst-build` semaphore admits two pipelines; raise it only
after rechecking `argo-quota` limits, controller Execute RPC headroom, and the
lanes × `builders` slot bound. NVIDIA variants are built in parallel with
continueOn (non-blocking).

3. **Clear stale semaphore holders.** The semaphore in
`manifests/workflow-semaphores.yaml` gates all BST build lanes. Confirm terminal
workflows do not retain locks before suspecting the grid.

4. **Verify the fix live.** After submission, confirm the pod is Running and on a
node with enough free requested memory:

```bash
kubectl get pod -n argo <pod-name> -o custom-columns='NODE:.spec.nodeName'
kubectl describe node <node> | grep -A8 "Allocated resources"
kubectl get events -n argo --field-selector reason=Preempted --sort-by='.lastTimestamp'
```

## Queueing, cleanup, and Buildbarn storage recovery

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

BuildStream 2.x uses the BuildGrid controller for remote execution and the
shared Buildbarn deployment for artifact cache writeback. Workflow-local state belongs on a
PVC-backed workspace, while the shared Buildbarn frontend provides cluster-wide
artifact reuse. Neither cache layer may use a root-backed `hostPath`.

### 0. Mandatory remote execution

Dakota builds must use BuildGrid remote execution regardless of transport path.
If remote execution is unhealthy, fail the workflow, diagnose it, and repair the
grid; do not fall back to a local-sandbox or cache-only build. The
`remote-execution.conf` ConfigMap key is appended under each project in
the generated BuildStream configuration. A healthy run has a bot session per
node and its actions in the BuildGrid `jobs` table with a `worker_name`.

### 1. Shared Buildbarn frontend
- **Endpoint**: `grpc://frontend.buildbarn.svc.cluster.local:8980`
- **Role**: CAS/AC artifact writes and reads for BuildStream and buildbox-casd; no execution
- **Deployment**: Frontend (2 replicas), sharded storage (2 replicas), and `bb-remote-asset` are defined under `manifests/buildbarn-*.yaml` and run in the `buildbarn` namespace

### 2. BuildStream client config
  The build pods generate a deterministic `buildstream.conf` from `manifests/buildstream-remote-cache-config.yaml`. Its artifact servers are global, so the `gnome-build-meta` and `freedesktop-sdk` junction projects (most of a Dakota build) pull from and push to the lab first:

```yaml
scheduler:
  network-retries: 8
  fetchers: 8
  builders: 12
  pushers: 4
build:
  max-jobs: 32
  retry-failed: true
artifacts:
  override-project-caches: false
  servers:
  - url: grpc://bb-remote-asset.buildbarn.svc.cluster.local:8984
    type: index
    push: true
  - url: grpc://frontend.buildbarn.svc.cluster.local:8980
    type: storage
    push: true
  - url: https://gbm.gnome.org:11003
    push: false
  - url: https://cache.freedesktop-sdk.io:11001
    push: false
source-caches:
  override-project-caches: true
  servers:
  - url: https://gbm.gnome.org:11003
    push: false
  - url: https://cache.freedesktop-sdk.io:11001
    push: false
cache:
  storage-service:
    url: grpc://frontend.buildbarn.svc.cluster.local:8980
```

- **Never list the frontend as a combined (`type: all`) artifact server.** It has no Remote Asset service, so BuildStream drops it entirely (`WARNING Failed to initialize remote grpc://frontend...: Configured remote does not implement the Remote Asset Fetch service`). Any project left with only that entry has no lab index and no push remote: before this was fixed, every Dakota run re-pulled ~900 junction artifacts from gbm.gnome.org at ~1/s and logged `Push Queue: skipped 923`.
- **`cache.storage-service`** makes buildbox-casd keep content in Buildbarn rather than on the coordinator's disk: an artifact pull fetches the artifact and Directory protos plus only the file blobs Buildbarn lacks, remote build outputs are not downloaded back, and `bst artifact checkout` fetches file blobs on demand. Measured on 149 freedesktop-sdk artifacts (2.0 GB): 134 s from gbm with the old config, 17 s from the lab without `storage-service`, 13 s with it (68 MB written locally). Checkout through it measured the same as a local-CAS pull plus checkout (9.4 GB bluefin OCI layout: 139 s vs 153 s; 3,629-file python3: 3 s vs 1 s plus its pull).
- A project-level `projects.<name>.artifacts` block replaces the global list for that project only, so do not use one to reach the lab cache: junction projects never see it.
- **Failed builds and Argo retries.** When an element's own build fails (a command exits non-zero, or the plugin raises an element error), BuildStream caches a *failed* artifact, pushes it to the lab index, and fails the element. Errors around the build (BuildGrid/CAS errors such as a worker CAS `Deadline Exceeded`, network, BuildStream bugs) cache nothing. The two need different handling:
  - `build.retry-failed: true` (the `--retry-failed` flag of `bst build` as config) makes BuildStream discard a cached failed artifact (`INFO Discarded failed build`) and run the element again. Without it, every later build of that cache key, in a retry or a new workflow, replays the failure, and with `cache.storage-service` the replay cannot read the failed build's log from local CAS and dies with `BUG Build ... FileNotFoundError: .../cas/objects/...`, which hides the real error. Running again does not mean executing again: BuildGrid stores failed action results in the Buildbarn action cache, so the remote action returns the same failed result at once until its 7-8 day expiry or a change to its inputs.
  - `bst-build-re` runs `bst build` through the ConfigMap's `bst-build.sh`. If bst failed and an element's build task logged `SUCCESS Caching artifact` and then `FAILURE` under one cache key, the script exits 3; otherwise it keeps bst's exit code (255). The template's `retryStrategy` (`limit: 1`, `retryPolicy: Always`, `expression: lastRetry.exitCode != "3"`) does not retry exit 3: the retry would re-pull ~900 artifacts (about 20 minutes) and fail the same way. Argo shows `retryStrategy.expression evaluated to false` on the retry node. Grid, network, and pod failures still get one retry.
  - To build again after a deterministic failure, change the inputs (a new commit). Resubmitting the same commit fails the same way. A command failure that the grid caused (for example a worker that could not fork) is cached and classified the same way, and its failed action result keeps being served after the grid is fixed, until it expires or the inputs change.

**Lab-wide source-cache rule:** the deployed `bb-remote-asset` image (`ghcr.io/buildbarn/bb-remote-asset:20241031T230517Z-4926e8e`) cannot handle BuildStream source-key URNs. Pushing sources to `grpc://bb-remote-asset.buildbarn.svc.cluster.local:8984` (`type: index`) produces `FetchDirectory ... PERMISSION_DENIED` / `HTTP Fetching of directories is not supported!` and, because the remote-asset asset cache reads through the unsharded storage headless Service, `could not get action from action cache: Object not found`. Until the endpoint is upgraded/configured for URNs, **every** lab BuildStream pipeline (`dakota-build-pipeline`, `bluefin-server-build-pipeline`, `bst-qa-pipeline`) must set `source-caches.override-project-caches: true` and list only the upstream read-only source caches. Leaving it `false` inherits junction source-cache entries and causes the build to spend minutes retrying a source push before failing. Artifact and action-cache writes still use the Buildbarn frontend.

**Distributed NVIDIA Policy:** The distributed Dakota workflow builds both default (`oci/bluefin.bst`) and NVIDIA (`oci/bluefin-nvidia.bst`) variants in parallel. Because the lab cluster lacks NVIDIA GPU hardware to execute GPU test suites, the NVIDIA build runs as non-blocking (`continueOn`). When the NVIDIA build succeeds, its image (`dakota-nvidia:testing`) is published directly to Zot.

**Registry publication:** the local Zot registry accepts anonymous pushes over its configured insecure HTTP endpoint. `bst-build-re` is a `containerSet`: the `main` container (`bst2`) builds and runs `bst artifact checkout` into `oci-export/` on the pod's CAS volume, then the `publish` container (digest-pinned distroless `ghcr.io/projectbluefin/skopeo`) runs `skopeo copy --dest-tls-verify=false oci:<dir> docker://<registry>/<tag>:<image-tag>` from the same volume. `bst2` ships podman but no skopeo; publishing from it round-trips the export through containers-storage and costs about a minute more per variant. Verify the registry digest with `skopeo inspect --tls-verify=false` and pull the registry tag back before smoke testing.

**GPU validation boundary:** the NVIDIA image contains `/usr/sbin/nvidia-smi`, but the lab has no NVIDIA hardware — no node advertises `nvidia.com/gpu` and no NVIDIA device plugin is running. Docker's `--gpus all` fails because the host has no communicating NVIDIA driver; Podman CDI fails because `nvidia.com/gpu=all` is unresolvable. NVIDIA GPU runtime validation is therefore not actionable on this cluster. Both nodes *do* advertise `amd.com/gpu: 1` (Radeon 8060S / gfx1151, ROCm) via `manifests/amdgpu-device-plugin.yaml`, so AMD-side GPU validation is possible — see `docs/skills/cluster-tooling/SKILL.md` § "AMD GPU topology".

### 3. BuildStream parser constraints
- **No top-level `source:` key**: `buildstream.conf` does not support a top-level `source:` block.
- **Nested under `scheduler`**: fetch / retry / network settings belong under `scheduler:`.
- **Sequence writes in Argo scripts**: prefer `echo "..." >> file` over multiline heredocs when generating config in YAML script blocks.

### 4. PVC-backed workflow workspace
Each BuildStream pod mounts a workflow PVC at `/root/.cache/buildstream` when
state must persist between workflow steps. It must use the `local-path` StorageClass
with the explicit GitOps node-to-data-mount mapping. Never use `/var/tmp`, a
root filesystem, or a node-local `hostPath` cache.

### 5. Private maintainer procedure — Buildbarn durable shard backup / restore

> [!CAUTION]
> This section is retained for maintainers who have explicitly approved a
> storage maintenance window and backup plan. It is not a normal public recipe:
> host-path copies, retained PVC/PV deletion, and restore commands can destroy
> data. Never run it from a workstation or use workstation SSH to `ghost` or
> `exo-0`; use the approved private operator channel. Routine Buildbarn
> operations remain API-driven through `just`, `argo`, `kubectl`, and GitOps.

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
   kubectl scale deployment/frontend deployment/bb-remote-asset -n buildbarn --replicas=0
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
   kubectl scale deployment/frontend -n buildbarn --replicas=2
   kubectl scale deployment/bb-remote-asset -n buildbarn --replicas=1
   kubectl rollout status deployment/frontend -n buildbarn --timeout=180s
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
   kubectl scale deployment/frontend -n buildbarn --replicas=2
   kubectl scale deployment/bb-remote-asset -n buildbarn --replicas=1
   kubectl rollout status deployment/frontend -n buildbarn --timeout=180s
   kubectl rollout status deployment/bb-remote-asset -n buildbarn --timeout=180s
   kubectl get pods -n buildbarn -o wide
   kubectl get endpointslice -n buildbarn -l kubernetes.io/service-name=storage
   ```

#### Post-restore verification
- **Filesystem check**: rerun `find ... -printf '%P %s\n' | sort`, `du -sh`, and `du -sh --apparent-size` against the restored host paths and compare them with the backup copy.
- **Pod readiness**: `storage-0` and `storage-1` must both be `Running`, and `kubectl rollout status statefulset/storage -n buildbarn` must succeed.
- **Client reachability**: `frontend` and `bb-remote-asset` must be `Available`, and the `storage` headless Service must show endpoints for both storage pods.
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
- `manifests/buildbarn-storage.yaml`
- `manifests/buildbarn-remote-asset.yaml`
