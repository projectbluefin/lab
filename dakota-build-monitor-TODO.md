# Dakota Distributed BST Build — Monitor & Iterate TODO

## Current State (as of session start)

- Cluster: ghost + exo-0 both Ready, k3s v1.36.2+k3s1.
- BuildBarn grid:
  - workers: worker-wfjw4 on exo-0, worker-zm67q on ghost (DaemonSet, 8 CPU / 16 Gi requests, concurrency=1 each)
  - storage: storage-0 on exo-0, storage-1 on ghost (StatefulSet with podAntiAffinity)
  - frontend/scheduler on ghost
- In-cluster registry: `192.168.1.102:30500` (writable Zot), currently has a `dakota:testing` manifest created 2026-06-29.
- Active dakota build: `dakota-build-pipeline-z7m77`, Running 2h+, progress 3/4, currently building `oci/bluefin.bst` (tag `dakota`). `oci/bluefin-nvidia.bst` is queued behind it.

## Observations / Capture

1. **Serial inefficiency in `dakota-build-pipeline`**: `build-bluefin-nvidia` depends on `build-bluefin.Succeeded`. This forces the two elements to run sequentially even though the `bst-build` semaphore allows 2 concurrent lanes and there are 2 BuildBarn workers. Fix: remove the DAG dependency so both `run-bst-step`s are admitted together (BST/BuildBarn will still respect internal element deps).
2. **Agents are running local BST builds via `dakota-pr-batch-pipeline`**: template uses `just validate && just build default` inside a privileged pod with 12 CPU / 24 Gi requests. This bypasses BuildBarn remote execution and violates the lab core tenet. Need to investigate whether any of the active `dak-117*` workflows are actually from this template.
3. **Cache warming is happening**: current run pulling many artifacts from `https://gbm.gnome.org:11003` and some sources from `grpc://frontend.buildbarn.svc.cluster.local:8980` (BuildBarn). The 57-minute `fetch:gnomeos-deps/bootc.bst` indicates a cold source fetch.
4. **Worker requests are heavy but fixed**: 8 CPU / 16 Gi per worker means one action slot per node. This is the design; driver pods request only 1 CPU / 2 Gi which may be tight for large checkouts/exports.

## Iteration Plan

- Iteration 1: use / wait for `dakota-build-pipeline-z7m77` to complete. Capture wall time, per-node worker utilization, and whether it pushes a new `dakota:testing` manifest.
- Iteration 2: submit a second dakota build (preferably with the serial dependency removed) and compare wall-clock time. It should be materially faster due to warmed BuildBarn artifact/source cache.
- Final proof: `skopeo inspect --tls-verify=false docker://192.168.1.102:30500/dakota:testing` returns a manifest with a digest >= the iteration-2 build time.

## Fixes Applied (pushed to main)

1. **Parallelize `dakota-build-pipeline`** — removed the `build-bluefin.Succeeded` dependency for `build-bluefin-nvidia`. Both BST lanes now start together once PVCs and USB4 gate pass. BuildStream/BuildBarn still respects internal element dependencies; this just removes an artificial workflow-level serialization so the two-worker grid can be used end-to-end.
2. **Fix `dakota-qa-pipeline` default tag** — changed `image-tag` default from `latest` to `testing`. Agent-submitted dak-117* QA workflows were defaulting to `:latest`, which is not the factory production testing tag and would test a stale ghcr.io image instead of the in-cluster `:testing` build.

Both templates linted with `argo template lint` and synced by ArgoCD (`testing-lab` app).

## Monitoring Actions

- Background agent `fb6ce553` watching `dakota-build-pipeline-z7m77` for completion and writing `dakota-build-pipeline-z7m77-report.md`.
- Background agent `913611a0` drove the full iteration loop but hit harness timeout; it left `dakota-iteration1-report.md`, `dakota-iteration2-manifest.yaml`, and `poll-iteration1.sh`.
- Unattended shell loop `drive-dakota-loop.sh` (PID in `drive-dakota-loop.pid`, log in `drive-dakota-loop.log`) now polls iteration 1, submits iteration 2, and writes the final summary + `skopeo inspect` proof.

## Unattended Loop Driver

`/var/home/jorge/src/lab/drive-dakota-loop.sh` is running in the background and will:
1. Poll `dakota-build-pipeline-z7m77` until terminal.
2. If Succeeded, record `skopeo inspect` and submit iteration 2 using `dakota-iteration2-manifest.yaml` (`lock-key: bst-build-manual`, parallel template).
3. Poll iteration 2 until terminal and record final `skopeo inspect`.
4. Write `/var/home/jorge/src/lab/dakota-build-loop-summary.md`.

If either iteration fails, it writes `dakota-iteration{1,2}-FAILED.md` and exits for human review.
