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

- **Admission:** Dakota and Bluefin Server pipelines accept only remote
  execution. Before admission, every lane requires a fresh USB4 `up` label and
  annotation (`lab.projectbluefin.io/usb4-link=up`, timestamp within 60s), a
  Ready BuildGrid worker on both `ghost` and `exo-0`, and a Ready BuildGrid
  controller. Additional nodes only add workers. Pipelines fail closed with an
  explicit rejection when the link is unavailable or stale rather than queueing
  in the scheduler. Check admission before debugging compilation: a rejection,
  or a graph-validation error from a step without resource requests/limits, is
  an admission failure, not a Dakota build result.
- **Checking USB4 state:** the `usb4-link-monitor` DaemonSet evaluates the
  link every 15s and publishes the node label and annotation
  `lab.projectbluefin.io/usb4-link=up|down` plus
  `lab.projectbluefin.io/usb4-link-observed-at`:
  `kubectl get nodes -L lab.projectbluefin.io/usb4-link`.
- **Evidence:** before calling a run distributed, verify its generated
  `projects.<name>.remote-execution` configuration, BuildStream RE startup, and
  BuildGrid `jobs` rows for the run. A local BuildStream success, a Podman/Zot
  push, or a container E2E pass is diagnostic only and never substitutes for a
  distributed build.
- **Storage:** artifact and source caches use the Buildbarn CAS/action-cache
  storage service. Each coordinator pod's BuildStream cache is a per-pod
  generic-ephemeral `local-path` PVC (200Gi); with `WaitForFirstConsumer` the
  provisioner binds it below the scheduled node's configured data mount. Never
  add node-local `hostPath` caches, root-filesystem paths, or node selectors to
  influence placement.
- **Capacity is admitted, not assumed:** execution capacity is the sum of
  BuildGrid worker slots (one worker per node, `CONCURRENT_JOBS` each). Never
  reserve capacity by pinning a build to a node.
- **Zot pull-through** on every node (`registry-mirror-config` DaemonSet) keeps
  image pulls off the WAN and off cross-node paths.

## Lanes and variants

- The `bst-build` semaphore in `manifests/workflow-semaphores.yaml` is `"2"`:
  two BuildStream pipelines share the grid at once. Its comment holds the
  sizing (controller Execute RPCs, `argo-quota` CPU limits, worker slots).
- `dakota-build-pipeline` with `variants=all` (the default) builds four
  variants: `oci/bluefin.bst` (`dakota`), `oci/bluefin-nvidia.bst`
  (`dakota-nvidia`), and both with `-o gaming true` (`dakota-gaming`,
  `dakota-nvidia-gaming`). Only `dakota` is blocking; the others run with
  `continueOn` because the lab has no GPU test hardware for them.
  `variants=default` builds only `oci/bluefin.bst`.
- Variant coordinators carry a **required** per-workflow pod anti-affinity: at
  most one coordinator per node per pipeline, so `variants=all` runs in two
  waves on the two-node grid by design. Lane sizing assumes it: `argo-quota`
  limits, and lanes × coordinators × `builders` < 64 worker slots.
- Every coordinator is the shared `bst-build-re` WorkflowTemplate
  (`argo/workflow-templates/bst-build-re.yaml`, template `build`), called
  through `templateRef` by both pipelines; per-pipeline differences
  (deadline, clone token, dev keys, toplevel upstream cache) are its inputs.
  It is a `containerSet`: `main` (`bst2`) builds and runs
  `bst artifact checkout` into `oci-export/` on the pod's cache volume, then
  `publish` (digest-pinned distroless `ghcr.io/projectbluefin/skopeo`) runs
  `skopeo copy --dest-tls-verify=false oci:<dir> docker://<registry>/<tag>:<image-tag>`
  to the lab Zot. `bst2` has no skopeo; publishing from it through
  containers-storage costs about a minute more per variant.
- The Dakota commit poller pins the checkout to the exact GitHub SHA it
  observed.
- Build Dakota test images with the lab pipeline (`just run-bst-build`),
  never by triggering `projectbluefin/dakota` GitHub Actions or relying on GHCR.

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
- **BuildStream config:** `manifests/buildstream-remote-cache-config.yaml`
  (ConfigMap `argo/buildstream-remote-cache`) is the client config; its
  comments explain each setting. The build template writes
  `dakota-buildstream.conf` and appends `remote-execution.conf` under
  `projects.<name>`: `execution-service` is the BuildGrid controller and
  `action-cache-service` the Buildbarn frontend. The CAS is the global
  `cache.storage-service` (Buildbarn frontend); `remote-execution` sets no
  `storage-service` of its own, because one there makes BuildStream download
  every remote build's outputs to the coordinator. Rules the config encodes:
  - Artifact servers are global so the `gnome-build-meta` and
    `freedesktop-sdk` junctions (most of a Dakota build) use the lab cache. A
    `projects.<name>.artifacts` block replaces the global list for that project
    only, and junction projects never see it.
  - Never list the frontend as a combined (`type: all`) artifact server: it has
    no Remote Asset service, so BuildStream drops it (`Failed to initialize
    remote`) and the junctions lose their lab index and push remote.
  - Source caches list gbm and fsdk read-only, then the lab index and storage
    (`push: true`), with `override-project-caches: true`; the `upstream-cache`
    block in `bst-build-re` repeats them because a
    `projects.<name>.source-caches` block replaces the global list. A build
    pushes the sources of every element it fetched: the element's staged
    sources (`FetchBlob`/`PushBlob` on
    `urn:fdc:buildstream.build:2020:source:<element sources key>`) and each
    source on its own (`FetchDirectory`/`PushDirectory` on
    `...:source:<kind>/<source key>`). The per-source entry still hits when
    only another source of the element changed (a kernel config script edit
    keeps the kernel git checkout). Keep the lab servers last: BuildStream's
    per-source pull (`SourceCache.pull`) does not stop at the first index hit,
    so any index remote after the lab overwrites a lab hit with its miss.
    bb-remote-asset must answer an index miss with `NOT_FOUND` (its `error`
    fetcher): its old `http` fetcher answered `FetchDirectory` misses with
    `PERMISSION_DENIED` ("HTTP Fetching of directories is not supported!"),
    which BuildStream treats as a failed fetch rather than a miss. Cache keys
    come from each plugin's unique key, not the URL (`git_repo` keys on `ref`
    only), so aliases do not affect source caching.

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
build that fails only in the lab sandbox is a runner bug: reproduce it with the
sandbox smoke test below and fix it in the grid (worker, runner, or controller
configuration), not in the project. Upstream GNOME `recc` defaults apply when
the pinned gnome-build-meta declares them. Do not bind the host's `/proc` or
other host files into the input root; it breaks hermeticity.

## Scale-out

- Adding a node adds a worker: the DaemonSet schedules a pod, the pod opens a
  bot session with the controller, and BuildGrid's queue hands it actions. That
  is the only automatic part; the host steps a new node needs are in
  `docs/ops/k3s-tuning.md` "Framework Desktop Nodes".
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

Before retrying a failed build, compare the live `buildstream-remote-cache`
ConfigMap, BuildGrid controller config, and worker pods with the repository;
reconcile through GitOps and wait for the worker rollout. A running workflow
and its Argo retries keep the template snapshot they were submitted with.

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
| Pod start, clone, config | ≤25 s | one `detect-build-mode` admission pod, then a depth-1 clone |
| Load, resolve, remote init | 13-15 s | junction and plugin sources (gnome-build-meta, freedesktop-sdk, PyPI plugins) are re-fetched every run in ~5 s; not worth caching |
| Pull, lab cache warm | ~16 s | ~880 artifacts, only protos and Directory blobs |
| Pull, first run after a junction bump | ~4 min | ~470 artifacts come from `gbm.gnome.org` over the WAN; pulls and source fetches share the 8 `--fetchers` slots, all busy the whole time |
| Kernel source fetch | 1.5-3 min miss, ~1 s hit | a miss is the Launchpad git fetch plus ~30 s adding 6385 objects, on the critical path; the build then pushes it to the lab source cache, and later runs that build the same kernel `ref` pull it from Buildbarn (102,585 files, 1.8 GB) |
| Kernel build | ~8.5 min | one action |
| initramfs, layers, `oci/bluefin.bst` | ~1 + 1-2.5 + ~3 min | sequential tail after the kernel |
| Export (`bst artifact checkout`) | 2.5-3 min (4+ min with four exports at once) | the OCI layout is one ~10 GB uncompressed layer blob, read cold from Buildbarn at ~68 MB/s (see below) |
| Publish (skopeo, 16 CPU) | 15-20 s | compresses to a ~4.3 GB gzip layer |

A fully cached run is pull plus export plus publish. When dakota CI already
built the commit, the final `oci/*` artifacts come from
`cache.projectbluefin.io` over the WAN (50 s-3 min) and are pushed to the lab.

Export is bound by Buildbarn's cold-read path, not by the coordinator or the
network. bb-storage serves every CAS read by copying from one shared `mmap` of
the 420 GiB `blocks` file. The kernel's per-open-file `mmap_miss` heuristic
turns off mmap read-around once faults on that file mostly miss the page
cache, and ordinary build traffic (random small-blob reads against an 8 Gi
page cache) keeps it off, so a large cold blob is read one 4 KiB page per
fault: storage-0 showed 16,500 major faults/s and 68 MB/s of disk reads while
serving the layer, from an NVMe that reads the same file at 2.8 GB/s with
`O_DIRECT`. Reproduced outside bb-storage: a fresh mapping of `blocks` read
cold regions sequentially at 2.0 GB/s, the same mapping after 400 random cold
faults at 259 MB/s, and a new `open()` at 1.7 GB/s again. Warm (cached) parts
of a blob stream at 0.7-1.2 GB/s. Neither the template nor the BuildStream
config can change this. It needs a bb-storage change (read large blobs with
`pread`, or `MADV_SEQUENTIAL` around them); a larger storage page cache
(the pods' 8 Gi memory limit) raises the hit rate but is unmeasured.

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
Low grid utilization during a failed action is expected; do not add workers or
jobs to work around a correctness failure.

### Failed builds and retries

When an element's own build fails (a command exits non-zero, or the plugin
raises an element error), BuildStream caches a *failed* artifact, pushes it to
the lab index, and fails the element. Errors around the build (BuildGrid/CAS
errors such as a worker CAS `Deadline Exceeded`, network, BuildStream bugs)
cache nothing.

- `build.retry-failed: true` (the config form of `bst build --retry-failed`)
  discards a cached failed artifact (`INFO Discarded failed build`) and runs
  the element again. Without it every later build of that cache key replays
  the failure, and with `cache.storage-service` the replay dies with
  `BUG Build ... FileNotFoundError: .../cas/objects/...`, hiding the real
  error. Running again does not mean executing again: the failed action result
  stays in the Buildbarn action cache until its 7-8 day expiry or an input
  change, so the rebuild fails at once.
- `bst-build-re` runs `bst build` through the ConfigMap's `bst-build.sh`. If
  an element's build task logged `SUCCESS Caching artifact` and then `FAILURE`
  under one cache key, the script exits 3; otherwise it keeps bst's exit code.
  The template retries once (`retryPolicy: Always`) except on exit 3
  (`expression: lastRetry.exitCode != "3"`; Argo shows `retryStrategy.expression
  evaluated to false`), because that retry would re-pull ~900 artifacts (about
  20 minutes) and fail the same way. Grid, network, and pod failures still get
  the retry.
- To build again after a deterministic failure, change the inputs (a new
  commit). A command failure the grid caused (for example a worker that could
  not fork) is cached and classified the same way, and keeps being served after
  the grid is fixed until it expires or the inputs change.

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
only when `kubectl -n argo get wf -l workflows.argoproj.io/phase=Running`
lists no BuildStream build (build pipelines and ad-hoc workflows that run
`bst`), and batch them into one window. Build-pipeline workflows carry no
`bluefin.io/bst-workload` label (only the pollers do), and `kubectl` rejects
`--field-selector status.phase` on workflows. WorkflowTemplates,
`workflow-semaphores`, the
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

## BST build scheduling: avoid preemption

BST build pods use the `bst-build` PriorityClass
(`manifests/bst-build-priorityclass.yaml`, 1,500,000), above `lab-test-vm`
(1,000,000) and below kubevirt/system critical classes, so long distributed
builds are not preempted by short-lived test VMs. A preempted build shows
`pod deleted` in `argo get` and a `Preempted` event:

```bash
kubectl get priorityclass bst-build lab-test-vm
kubectl get pod -n argo <pod-name> -o custom-columns='NODE:.spec.nodeName'
kubectl describe node <node> | grep -A8 "Allocated resources"
kubectl get events -n argo --field-selector reason=Preempted --sort-by='.lastTimestamp'
```

Node memory *requests* must leave room for the BuildGrid worker pods and the
BuildStream coordinators. The usual thieves are orphaned test VMs and completed
Jobs: Kubernetes counts a pod's request against node capacity until the Job
(and its pods) is deleted; a finished Job holding a 14Gi request once blocked
Buildbarn storage scheduling on `exo-0`. `manifests/orphan-vm-cleanup.yaml`
runs every 30 minutes and deletes VMs whose parent Argo workflow is gone or
terminal, looking up the `argo-workflow` label in the VM's own namespace.

## Queueing, cleanup, and Buildbarn storage recovery

When the cluster is already hot, the fastest recovery is usually to stop the noise
instead of submitting more work:

1. Delete stale terminal workflows first; leave the newest healthy run in place.
2. Delete orphaned VMs/PVCs whose parent workflow is already terminal so they do
   not keep memory or storage reservations pinned.
3. Confirm terminal workflows do not still hold `bst-build` semaphore locks
   before suspecting the grid. Gate expensive lanes at the template level with
   the semaphores in `manifests/workflow-semaphores.yaml`; workflow-level
   mutexes do not bind `workflowTemplateRef` / `templateRef` callers.
4. If Buildbarn storage pods stay `Pending` after a StatefulSet or PVC change, verify
   the PVC bindings and storage pods before resubmitting a build:
   `kubectl -n buildbarn get pvc` plus `kubectl -n buildbarn get pods`.

The two Buildbarn storage replicas are shards, not copies: `storage-0` and
`storage-1` each own half of the CAS and AC keyspace, so losing one shard's
PVCs loses that half. Shard backup and restore is a private-runbook procedure.
The in-repo recovery from cache loss is the `bst-cache-warm` WorkflowTemplate,
which re-seeds the Dakota and Bluefin Server caches through the normal
admission gates on its own `bst-cache-warm` semaphore lane.

## Buildbarn message-size floor

BuildStream can issue large CAS upload batches while importing bootstrap seed
artifacts. Keep `maximumMessageSizeBytes: 64 * 1024 * 1024` in
`manifests/buildbarn-config.yaml`; lower values fail lanes with
`Unable to upload <N> blobs to remote CAS`. When `buildbarn-config` changes,
bump the `buildbarn-config-revision` pod-template annotation in
`manifests/buildbarn-frontend.yaml`, `manifests/buildbarn-storage.yaml`, and
`manifests/buildbarn-remote-asset.yaml`.
