# Workflow Reference

This doc covers Argo Workflows and WorkflowTemplates.

## Table of Contents
- [Pipelines](#pipelines)
  - [dakota-qa-pipeline](#dakota-qa-pipeline)
  - [dakota-build-pipeline](#dakota-build-pipeline)
  - [zot-candidate-lifecycle](#zot-candidate-lifecycle)
  - [bluefin-server-build-pipeline](#bluefin-server-build-pipeline)
  - [bst-qa-pipeline](#bst-qa-pipeline)
- [Supporting Templates](#supporting-templates)
- [Distributed Build/RE Grid](#distributed-buildre-grid)
- [Cache Warming (Pollers)](#cache-warming-pollers)
- [Nightly Schedule](#nightly-schedule)
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
- **Parameters:** `image`, `image-tag`, `suites`, `variant`, `branch`, `pr-number`, `sha`,
  `repo`, `testsuite-branch`, `testsuite-repo`.
- **DAG:** `validate-suites` → parallel `test-lane` items (`smoke`, `common`, `developer`,
  `software`, `system`) through `run-container-tests`.
- **Just recipe:** `run-dakota-qa`.
- **PR review:** use the [Dakota PR review skill](../skills/dakota-pr-review/SKILL.md). Build the exact PR SHA first, then run smoke and required E2E suites against the resulting image. If lab validation identifies a scoped PR defect, repair the PR branch, rebuild from its new SHA, rerun E2E, and merge directly only after a fresh pass.

### dakota-build-pipeline
- **Purpose:** The actual BuildStream compile step for Dakota — builds
  `oci/bluefin.bst` and pushes the image to the local Zot registry under the tag
  given by `image-tag` (default `testing`). NVIDIA variants are built
  non-blocking and pushed with the same tag.
- **Parameters:** `repo`, `ref`, `commit-sha`, `image-tag` (default `testing`),
  `registry`, `build-mode`, `lock-key`.
- **Distribution:** `build-mode=re` is mandatory. A fresh USB4 `up` observation
  and a Ready BuildBarn worker are required on both `ghost` and `exo-0` before
  admission. Cache-only, Ethernet-backed, automatic fallback, runner-local
  execution, and remote-cache-only execution are failures, not alternatives.
  Scheduler-driven placement selects the coordinator; no task pins it to a node.
- **Capacity:** The coordinator uses four fetchers, two BuildStream builders and
  pushers, and eight jobs per action. Each of the two BuildBarn workers exposes
  one action slot. This capacity must not be increased until remote execution and
  full SDK CAS materialization are healthy. The workflow verifies its generated
  remote-execution configuration before it invokes BuildStream.
- **Priority:** `priorityClassName: bst-build` keeps the coordinator ahead of
  short-lived lab test workloads.
- **Who triggers it automatically:** the `daily-dakota-build` CronWorkflow
  (scheduled at 00:30 UTC, initially staged suspended) and the `dakota-commit-poller`
  CronWorkflow through the shared `bst-commit-poller` template (see
  [Cache Warming](#cache-warming-pollers)). The poller resolves the current
  GitHub SHA for `dakota:testing` and passes that exact commit into the local
  BuildStream run, so the lab build checks out the same source revision that
  GitHub is building instead of drifting to a later branch tip.

**Distributed-gate rule:** A Dakota PR build is valid only when a fresh
`build-mode=re` run completes for the exact PR head SHA and its pushed registry
image is verified. Local, cache-only, or fallback builds are diagnostic evidence
and do not satisfy the distributed gate. Inspect failed child nodes even when the
Argo parent phase is successful; classify BuildBarn storage/DNS/worker failures as
infrastructure blockers and repair the lab before retrying.

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
- **Just recipe:** `run-zot-promotion`.

### bluefin-server-build-pipeline
- **Purpose:** BuildStream compile pipeline for Bluefin Server elements
  (`oci/bluefin-server-ddi.bst`, `oci/bluefin-server-installer.bst`) and push to local Zot.
- **Safety guards (aligned with dakota):**
  `activeDeadlineSeconds: 14400` (workflow), `activeDeadlineSeconds: 5400` (step),
  `retryStrategy: limit=2, retryPolicy=Always`, `GRPC_POLL_STRATEGY=poll`,
  `GRPC_ENABLE_FORK_SUPPORT=1`, `request-timeout: 900`,
  `scheduler.network-retries: 4`, `scheduler.fetchers: 1`.
- **Cache policy:** uses the shared Buildbarn frontend (`frontend.buildbarn.svc.cluster.local:8980`) for artifact cache writes and remote execution; the current BuildStream image in this cluster does not accept the legacy `remoteasset:` config block, so the config omits it. The checked-in `buildstream-remote-cache` config leaves project cache overrides disabled and lists the project's own upstream artifact/source cache URLs as read-only fallbacks.

### bst-qa-pipeline
- **Purpose:** Smoke-tests the Buildbarn distributed remote-execution grid itself
  by running a trivial BuildStream element through it.
- **Cache + RE wiring:** artifact cache writes, remote execution, and remote asset
  fetches all flow through the shared Buildbarn frontend and remote-asset service
  (`frontend.buildbarn.svc.cluster.local:8980` and
  `bb-remote-asset.buildbarn.svc.cluster.local:8984`). The project cache remotes
  are Buildbarn-only. See [Distributed Build/RE Grid](#distributed-buildre-grid).
- **Known limitation:** the current test element (`hello.bst`, an `import` kind)
  proves config wiring (BuildStream connects to the frontend with no errors) but
  never actually dispatches an action through the scheduler to a worker — verified
  by checking the CAS blocks file (zero bytes written). A real build-dispatch
  test element would be needed to prove end-to-end RE execution conclusively.

## Supporting Templates

| Template | Role |
| --- | --- |
| `image-poller` | Digest-comparison helpers (`poll-digest`, `update-local`). Flow: fetch upstream digest → compare with `image-polling-digests` → run downstream → persist digest only after downstream success. `image-poll-dakota` routes to `dakota-qa-pipeline`. |
| `run-container-tests` | Shared container-only runner for Dakota bootc images. Clones `projectbluefin/testsuite`, starts a Wayland session in the target OCI image, runs `behave`, attempts best-effort result publication when `github-token` is available, and writes a summary file for workflow outputs. Publication warnings do not change the suite exit status. |

## Distributed Build/RE Grid

Two independent distributed-build mechanisms exist on the cluster — they solve
different problems and do not overlap:

| Mechanism | What it distributes | Used by |
| --- | --- | --- |
| k8s scheduler (no pin) | Full privileged bootc OCI builds (needs real FUSE/mount-namespace access) | `dakota-build-pipeline`, `bluefin-server-build-pipeline` |
| Buildbarn (`buildbarn` namespace) | BuildStream cache writes and remote-execution actions (chroot-only sandbox, `CAP_SYS_CHROOT`) | `dakota-build-pipeline`, `bluefin-server-build-pipeline`, `bst-qa-pipeline` |

Buildbarn topology (2 storage shards, 1 scheduler, 2 frontend replicas, 1
worker+runner DaemonSet pair per node — storage replicas spread with
`podAntiAffinity`) is defined in `manifests/buildbarn-*.yaml`. Every BST lane
requires the real BuildBarn execution grid over a fresh USB4 link between
`ghost` and `exo-0`. If a link, worker, or action is unavailable, the workflow
must fail for repair; it must not use an Ethernet, local, or cache-only fallback.

## Cache Warming (Pollers)

| CronWorkflow | Interval | Triggers | Keeps warm |
| --- | --- | --- | --- |
| `dakota-commit-poller` | **suspended** (was every 5 min at minute +2) | shared `bst-commit-poller` → `dakota-build-pipeline` when `dakota:testing` changes | Dakota BuildStream cache/execution path; on-demand via `just force-dakota-poll` |
| `image-poll-dakota` | every 10 min at :08 | custom digest DAG with `run-qa=false` | Dakota testing digest freshness; daily QA runs at 03:00 UTC |

Dakota/Bluefin Server/BST lanes now use the shared Buildbarn frontend for
cache writes and remote execution while leaving upstream mirrors read-only. Cold
runs may fetch from upstream source origins, but cache writes stay in-cluster via
Buildbarn.

**`bluefin-server-build-pipeline` has no poller at all** — it is manual-trigger
only and uses the same USB4-gated BuildBarn remote-execution contract as every
other BST lane.

- **`daily-dakota-build`** is the dedicated daily compile schedule (00:30 UTC,
  staged `suspend: true`). It routes through `bst-commit-poller` (`entrypoint: poll-dakota`,
  `force: "true"`), enforcing the two-workflow BST admission limit while capturing the
  current Git SHA and triggering `dakota-build-pipeline` to export to local Zot (`:30500`).
- **`nightly-dakota` does not warm anything** — it's wired to `dakota-qa-pipeline`
  (test runner against pre-built images), not `dakota-build-pipeline` (the actual
  compile step). The real Dakota cache-warming trigger is `dakota-commit-poller`. It must not be
  interpreted as proof of a green distributed build: the poller succeeds only when
  the remote BuildStream workflow, image export, registry push, and configured
  validation path succeed.

The commit CronWorkflows share one implementation. It compares
the source SHA, defers when two BST workflows are already admitted, invokes the
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

**Bandwidth contract (PR #632):** `image-poller` resolves digests by inspecting
the **upstream registry directly** — never through the zot cache. Zot on-demand
sync copies manifest + all blobs on a tag read, so polling through zot pulled
every new multi-GB image even when QA was skipped.

## Nightly Schedule

| CronWorkflow | Time (UTC) | Pipeline | Parameters |
| --- | --- | --- | --- |
| `nightly-dakota` | 03:00 | `dakota-qa-pipeline` | `image=ghcr.io/projectbluefin/dakota`, `image-tag=testing`, `suites=smoke,developer,system`, `variant=dakota` |

## Priority Classes

| PriorityClass | Value | Applied to |
| --- | --- | --- |
| `lab-test-vm` | 1,000,000, `PreemptLowerPriority` | All explicit VM-backed KubeVirt test VMs (`bluefin-server-boot-test`) |
| `bst-build` | (see `manifests/bst-build-priorityclass.yaml`) | Heavy/long BuildStream compiles: `dakota-build-pipeline`, `bluefin-server-build-pipeline` |

Test VMs are meant to win resource contention over background build workloads —
`lab-test-vm`'s higher priority value plus `PreemptLowerPriority` enforces this
against any pod using `bst-build`.

## Resource Profiles

Pod resource requests/limits used by workflow steps:

| Template | CPU req/limit | Memory req/limit |
| --- | --- | --- |
| `run-container-tests` | 1 / 2 | 2Gi / 4Gi |
| `dakota-build-pipeline/bst-build` | 8 / 16 | 16Gi / 32Gi |
| `bluefin-server-build-pipeline/bst-build` | 6 / 10 | 16Gi / 30Gi |
