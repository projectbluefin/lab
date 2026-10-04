# Workflow Reference

Canonical interface for driving the lab: every supported operation is one
`argo submit --from workflowtemplate/<name> [-p k=v]` (usually via a `just`
recipe). Templates live in `argo/workflow-templates/` and ArgoCD `testing-lab`
reconciles them to namespace `argo`; CronWorkflows live in `manifests/`
(`testing-lab-infra`). Prefer the top-level pipelines; supporting templates are
called through `templateRef`. Update this file in the same commit as any
template add or rename.

## Table of Contents
- [Pipelines](#pipelines)
  - [dakota-qa-pipeline](#dakota-qa-pipeline)
  - [dakota-build-pipeline](#dakota-build-pipeline)
  - [zot-candidate-lifecycle](#zot-candidate-lifecycle)
  - [bluefin-server-build-pipeline](#bluefin-server-build-pipeline)
- [KubeStellar Workflows](#kubestellar-workflows)
- [Supporting Templates](#supporting-templates)
- [Distributed Build/RE Grid](#distributed-buildre-grid)
- [Cache Warming (Pollers)](#cache-warming-pollers)
- [Nightly and Maintenance Schedules](#nightly-and-maintenance-schedules)
- [Priority Classes](#priority-classes)
- [Resource Profiles](#resource-profiles)

## Pipelines

### dakota-qa-pipeline
- **Purpose:** Run Dakota suites directly inside the published bootc OCI image using
  a container-only fan-out model.
- **VM boundary:** This active QA pipeline has no containerDisk build, VM boot,
  reboot, or SSH stage.
- **Current coverage:** `run-container-tests` provides the container-only suite
  fan-out, including the nested systemd/Wayland session used by qecore-headless.
  This lane provides image and GUI-session evidence, but not VM boot or reboot
  evidence.
- **Parameters:** `image`, `image-tag`, `image-digest`, `suites`, `variant`, `branch`,
  `testsuite-branch`, `testsuite-repo`.
- **DAG:** parallel `test-lane` items (`smoke`, `common`, `developer`, `software`,
  `system`) through `run-container-tests`, filtered by `suites`.
- **Just recipe:** `run-dakota-qa`.
- **PR review:** use the [Dakota PR review skill](../skills/dakota-pr-review/SKILL.md). Build the exact PR SHA first, then run smoke and required E2E suites against the resulting image. If lab validation identifies a scoped PR defect, repair the PR branch, rebuild from its new SHA, rerun E2E, and merge directly only after a fresh pass.

### dakota-build-pipeline
- **Purpose:** The actual BuildStream compile step for Dakota — builds
  `oci/bluefin.bst` and pushes the image to the local Zot registry under the tag
  given by `image-tag` (default `testing`). NVIDIA variants are built
  non-blocking and pushed with the same tag.
- **Parameters:** `repo`, `ref`, `commit-sha`, `image-tag` (default `testing`),
  `registry`, `variants`.
- **Distribution:** remote execution on BuildGrid is mandatory. A fresh USB4
  `up` observation and a Ready BuildGrid worker are required on both `ghost` and
  `exo-0`, plus a Ready BuildGrid controller, before admission. Cache-only,
  Ethernet-backed, automatic fallback, local-sandbox execution, and
  remote-cache-only execution are failures, not alternatives.
  Scheduler-driven placement selects the coordinator; no task pins it to a node.
- **Source fidelity:** builds the exact dakota commit unmodified. The workflow
  applies no element, junction, or patch-queue changes; upstream GNOME `recc`
  defaults apply when the pinned gnome-build-meta declares them.
- **Capacity:** the coordinator keeps `scheduler.builders: 12` element actions
  in flight with `max-jobs: 32` each; GNOME recc fans each element out into
  compile actions. All actions queue in BuildGrid and run on the `worker`
  DaemonSet (one per node, `CONCURRENT_JOBS: 32` slots each). The `bst-build`
  semaphore admits two pipelines at once; each runs at most one coordinator per
  node, so lanes × `builders` stays below one node's slots and element actions
  cannot starve their own compiles.
  A new node adds capacity with no config change. The workflow verifies its generated
  remote-execution configuration before it invokes BuildStream.
- **Coordinator:** each variant runs the shared `bst-build-re` WorkflowTemplate
  (`argo/workflow-templates/bst-build-re.yaml`, `templateRef` template `build`).
  It is a `containerSet`: `main` (`bst2`) builds and checks the OCI layout out
  onto the pod's CAS volume; `publish` (digest-pinned
  `ghcr.io/projectbluefin/skopeo`) then runs one `skopeo copy` to Zot. A failed
  build never starts `publish`. Required per-workflow pod anti-affinity keeps
  one coordinator per node, so `variants=all` runs in two waves.
- **Retry:** `bst-build-re` retries once (`retryPolicy: Always`) unless `main`
  exits 3 (`expression: lastRetry.exitCode != "3"`). `main` runs `bst build`
  through `bst-build.sh` from the `buildstream-remote-cache` ConfigMap, which
  exits 3 when an element's own build failed and BuildStream cached the failed
  artifact: the retry would fail the same way after ~20 minutes of pulls.
  Grid, CAS, network, and pod failures keep bst's exit code (255) and are
  retried. `build.retry-failed: true` rebuilds instead of replaying a cached
  failure. See
  [BuildStream: Failed builds and retries](../skills/cluster-tooling/buildstream.md#failed-builds-and-retries).
- **Priority:** `priorityClassName: bst-build` keeps the coordinator ahead of
  short-lived lab test workloads.
- **Who triggers it automatically:** the `dakota-commit-poller`
  CronWorkflow through the shared `bst-commit-poller` template (see
  [Cache Warming](#cache-warming-pollers)). The poller resolves the current
  GitHub SHA for `dakota:testing` and passes that exact commit into the local
  BuildStream run, so the lab build checks out the same source revision that
  GitHub is building instead of drifting to a later branch tip.

**Distributed-gate rule:** A Dakota PR build is valid only when a fresh
remote-execution run completes for the exact PR head SHA and its pushed registry
image is verified. Local, cache-only, or fallback builds are diagnostic evidence
and do not satisfy the distributed gate. Inspect failed child nodes even when the
Argo parent phase is successful; classify BuildGrid controller/worker and
Buildbarn storage/DNS failures as infrastructure blockers and repair the lab
before retrying.

### zot-candidate-lifecycle
- **Purpose:** Reusable, independent single-lane lifecycle for immutable local
  Zot candidates and digest-preserving promotion to `:testing`.
- **Preflight:** rejects reused `candidate-<commit-sha>` tags and fails when the
  Zot data filesystem has less than 20 GiB or 10% free.
- **Integrity:** resolves the expected digest, fetches and hashes the raw
  manifest, copies that digest to `:testing`, and verifies the target digest.
- **Evidence:** attaches and rediscovers
  `application/vnd.projectbluefin.lab.promotion-evidence.v1+json` with ORAS.
- **Authentication:** optionally mounts `zot-writer-auth` while anonymous writes
  remain active; the secret becomes required at the Zot auth activation gate.
- **Just recipe:** `run-zot-promotion`. See
  [Zot candidate promotion](../ops/zot-candidate-promotion.md).

### bluefin-server-build-pipeline
- **Purpose:** BuildStream compile pipeline for `oci/bluefin-server-image.bst`
  (the whole release set, built with the fixed dev keys) and push to local Zot
  as `bluefin-server-image:latest`.
- **Coordinator:** the same `bst-build-re` template as dakota, with
  `deadline: 10800` (3h per pod; workflow `activeDeadlineSeconds: 28800`),
  a `pod-patch` that adds the `github-token` env and the
  `bluefin-server-dev-boot-keys` Secret mount (dakota passes none, so its pods
  carry neither Secret), and
  `upstream-cache: https://cache.projectbluefin.io:11001` (replaces
  `cache.freedesktop-sdk.io` for the toplevel project only). Retry, priority,
  and execution are identical to dakota.
- **Cache policy:** CAS (`cache.storage-service`) and action cache use the shared Buildbarn frontend (`frontend.buildbarn.svc.cluster.local:8980`); artifacts are indexed in `bb-remote-asset` (`:8984`) with the projects' upstream artifact caches as read-only fallbacks. Source caches override the project entries and list only the upstream caches, read-only. See `manifests/buildstream-remote-cache-config.yaml`.

## KubeStellar Workflows

KubeStellar installation and upgrades are owned by the `kubestellar-applications`
ArgoCD parent Application. Each template takes `wec-name` (default `ghost`).

- **`register-wec`:** registers the WEC with the its1 OCM hub and labels the
  ManagedCluster `name=<wec>`. SA `kubestellar-bootstrap` (cluster-admin;
  klusterlet install writes CRDs).
- **`kubestellar-smoke-test`:** verifies BindingPolicy downsync and singleton
  status upsync via wds1 (`kubeconfig-incluster` key), then cleans up. SA
  `kubestellar-bootstrap`. Acceptance gate after any core upgrade.
- **`kubestellar-platform-verify`:** read-only gate
  `verify-datasource → verify-query-surfaces → verify-controller-wiring →
  kubestellar-smoke-test`, run with `just run-kubestellar-verify`. Read-only
  checks use `kubestellar-observability`; the smoke task is the only one that
  creates resources. `templateRef` does not inherit workflow-level identity, so
  the smoke template declares `kubestellar-bootstrap` at template level.

## Supporting Templates

| Template | Role |
| --- | --- |
| `image-poller` | Digest-comparison helpers (`poll-digest`, `update-local`). Flow: fetch upstream digest → compare with `image-polling-digests` → run downstream → persist digest only after downstream success. `image-poll-dakota` routes to `dakota-qa-pipeline`. |
| `run-container-tests` | Shared container-only runner for Dakota bootc images. Clones `projectbluefin/testsuite`, starts a Wayland session in the target OCI image, runs `behave`, attempts best-effort result publication when `github-token` is available, and writes a summary file for workflow outputs. Publication warnings do not change the suite exit status. |
| `bst-build-re` | Shared BuildStream coordinator (template `build`): builds one element on BuildGrid and publishes it to Zot. Callers hold the `bst-build` semaphore. |
| `bst-admission-gate` | `detect-build-mode`: the USB4/BuildGrid admission check each BST pipeline runs before its build tasks. |
| `bst-cache-warm` | Manual cache re-seed after cache loss: runs the Dakota and Bluefin Server `build-warmup` templates on the `bst-cache-warm` semaphore lane. |

## Distributed Build/RE Grid

Three cluster mechanisms cooperate on a distributed BuildStream build, each with
one job:

| Mechanism | What it distributes | Used by |
| --- | --- | --- |
| k8s scheduler (no pin; coordinators prefer `exo-0`) | BuildStream coordinator pods and OCI export/push | `dakota-build-pipeline`, `bluefin-server-build-pipeline` |
| BuildGrid (`buildgrid` namespace) | Remote-execution actions on `buildbox-run-bubblewrap` workers (private PID namespace, fresh `/proc`) | `dakota-build-pipeline`, `bluefin-server-build-pipeline` |
| Buildbarn (`buildbarn` namespace) | CAS, action cache, and remote asset only; no execution | same lanes, plus BuildGrid workers' casd |

BuildGrid topology (`manifests/buildgrid-*.yaml`): `controller`
(`controller.buildgrid.svc.cluster.local:50051`, Execution/Operations), `bots`
(Remote Workers API), the stateless `cas` front, a Postgres `database`
StatefulSet, and a `worker` DaemonSet with one buildbox-casd + buildbox-worker
pod per node.
Buildbarn topology: 2 storage shards (spread with `podAntiAffinity`), 2
frontend replicas, and `bb-remote-asset` (`manifests/buildbarn-*.yaml`). Every
BST lane requires BuildGrid execution over a fresh USB4 link between `ghost`
and `exo-0`. If a link, worker, or action is unavailable, the workflow must fail
for repair; it must not use an Ethernet, local, or cache-only fallback.

## Cache Warming (Pollers)

| CronWorkflow | Interval | Triggers | Keeps warm |
| --- | --- | --- | --- |
| `dakota-commit-poller` | **suspended** | shared `bst-commit-poller` → `dakota-build-pipeline` when `dakota:testing` changes | Dakota BuildStream cache/execution path; on-demand via `just force-dakota-poll` |
| `server-commit-poller` | every 15 min at :02 | shared `bst-commit-poller` → `bluefin-server-build-pipeline` when `projectbluefin/server` `main` changes | Bluefin Server BuildStream cache/execution path |
| `image-poll-dakota` | every 10 min at :08 | custom digest DAG with `run-qa=false` | Dakota testing digest freshness; daily QA runs at 03:00 UTC |

Dakota/Bluefin Server/BST lanes execute on BuildGrid and write caches to the
shared Buildbarn frontend while leaving upstream mirrors read-only. Cold
runs may fetch from upstream source origins, but cache writes stay in-cluster via
Buildbarn.

Both commit pollers use the same USB4-gated BuildGrid remote-execution
contract as manual BST runs. `bst-cache-warm` (manual) re-seeds the Dakota and
Bluefin Server caches after cache loss on its own semaphore lane.

- **`nightly-dakota` does not warm anything** — it's wired to `dakota-qa-pipeline`
  (test runner against pre-built images), not `dakota-build-pipeline` (the actual
  compile step). The real Dakota cache-warming trigger is `dakota-commit-poller`. It must not be
  interpreted as proof of a green distributed build: the poller succeeds only when
  the remote BuildStream workflow, image export, registry push, and configured
  validation path succeed.

The commit CronWorkflows share one implementation. It compares
the source SHA, defers when every `bst-build` lane is busy and one more BST
workflow is already waiting (the ceiling is read from `workflow-semaphores`), invokes the
repository-specific build template, and writes the SHA only after that build
succeeds. Failed builds therefore remain eligible on the next poll.

The CronWorkflows pass `force=false`. For recovery after an out-of-band artifact
loss, `just force-dakota-poll` submits the Dakota CronWorkflow with `force=true`.
Force bypasses only the stored-SHA equality check: queue admission and the
`bst-build` semaphore still apply. A retried or resumed older workflow builds its
captured SHA, but skips the final state write if another successful workflow
advanced the stored SHA while it was running.

**Current contract:** `image-poller` must not update `image-polling-digests`
until `run-pipeline.Succeeded`. If the digest is written before QA passes, the
poller will treat the image as already seen and silently skip the failed lane on
the next cycle.

**Bandwidth contract:** `image-poller` resolves digests by inspecting
the **upstream registry directly** — never through the zot cache. Zot on-demand
sync copies manifest + all blobs on a tag read, so polling through zot pulled
every new multi-GB image even when QA was skipped.

## Nightly and Maintenance Schedules

| CronWorkflow | Time (UTC) | Pipeline | Parameters |
| --- | --- | --- | --- |
| `nightly-dakota` | 03:00 | `dakota-qa-pipeline` | `image=ghcr.io/projectbluefin/dakota`, `image-tag=testing`, `suites=smoke,developer,system`, `variant=dakota` |
| `orphan-vm-cleanup` | every 30 min | inline | Delete KubeVirt test VMs whose parent workflow is gone or terminal |
| `orphan-pod-gc` | every 30 min | inline | Delete ContainerStatusUnknown and stale Failed pods Argo podGC missed |

## Priority Classes

| PriorityClass | Value | Applied to |
| --- | --- | --- |
| `bst-build` | 1,500,000 (`manifests/bst-build-priorityclass.yaml`) | Long BuildStream coordinators: `dakota-build-pipeline`, `bluefin-server-build-pipeline` |
| `lab-test-vm` | 1,000,000, `PreemptLowerPriority` | Explicit VM-backed KubeVirt test VMs (`bluefin-server-boot-test`) |

`bst-build` sits above `lab-test-vm` so a short-lived test VM never preempts a
multi-hour build and loses its progress.

## Resource Profiles

Pod resource requests/limits used by workflow steps:

| Template / container | CPU req/limit | Memory req/limit |
| --- | --- | --- |
| `run-container-tests` | 2 / 4 | 4Gi / 8Gi |
| `bst-build-re` `main` | 2 / 4 | 4Gi / 8Gi |
| `bst-build-re` `publish` | 1 / 16 | 256Mi / 2Gi |
