# Workflow Reference

This doc covers Argo Workflows and WorkflowTemplates. For the GitHub Actions
bridge that submits Argo Workflows from ephemeral ARC runners, see
`.github/workflows/example-container-mode-build.yml` and
`docs/maintainer-onboarding.md`.

## Table of Contents
- [Pipelines](#pipelines)
  - [bluefin-qa-pipeline](#bluefin-qa-pipeline)
  - [dakota-qa-pipeline](#dakota-qa-pipeline)
  - [dakota-build-pipeline](#dakota-build-pipeline)
  - [cosmic-build-pipeline](#cosmic-build-pipeline)
  - [bluefin-server-build-pipeline](#bluefin-server-build-pipeline)
  - [bst-qa-pipeline](#bst-qa-pipeline)
  - [knuckle-qa-pipeline](#knuckle-qa-pipeline)
- [Supporting Templates](#supporting-templates)
- [Distributed Build/RE Grid](#distributed-buildre-grid)
- [Cache Warming (Pollers)](#cache-warming-pollers)
- [Nightly Schedule](#nightly-schedule)
- [Priority Classes](#priority-classes)
- [Resource Profiles](#resource-profiles)

## Pipelines

### bluefin-qa-pipeline
- **Purpose:** Test an already-built Bluefin/Bluefin-LTS containerDisk. Does **not**
  build anything itself — fails fast if the containerDisk is missing from Zot.
- **Parameters:** `image`, `image-tag`, `namespace`, `suites`, `variant`,
  `containerdisk-tag`, `ssh-key-secret`, `branch`, `pr-number`, `sha`, `repo`, `vm-memory`.
- **DAG:** `assert-cd` (skopeo inspect against Zot) → parallel `test-lane` items
  (`smoke`, `common`, `developer`, `software`, `system`, filtered by `suites`) →
  `onExit: cleanup-and-report` (deletes any orphaned lane VMs, posts a GitHub commit
  status on PR runs).
- **Each lane:** `provision-containerdisk-vm` (containerDisk VM boot) → `run-gnome-tests`
  (SSH in, run `behave`) → `teardown-vm`.
- **Who builds the containerDisk:** the `digest-watch` CronWorkflow (see
  [Cache Warming](#cache-warming-pollers)) — completely decoupled from this pipeline.
- **Just recipe:** `run-tests`, `run-tests-tag`, `run-tests-matrix`.

### dakota-qa-pipeline
- **Purpose:** Validate an existing Dakota containerDisk (same assert-cd/fail-fast
  model as bluefin-qa-pipeline).
- **Parameters:** `image`, `image-tag`, `containerdisk-tag`, `namespace`, `suites`,
  `variant`, `ssh-key-secret`, `branch`, `vm-memory`.
- **DAG:** `assert-cd` → parallel `test-lane` items (`smoke`, `developer`, `system`);
  `onExit: cleanup-orphan-vms`.
- **Just recipe:** `run-dakota-qa`.

### dakota-build-pipeline
- **Purpose:** The actual BuildStream compile step for Dakota — builds
  `oci/bluefin.bst` and `oci/bluefin-nvidia.bst` in parallel and pushes both to
  the local Zot registry. This is what populates/warms the Dakota build cache.
- **Distribution:** No `nodeSelector` pin — the k8s scheduler naturally spreads
  the two variant builds across ghost and exo-0 in parallel (confirmed live:
  one variant per node, both `Running` simultaneously).
- **Cache:** Uses the shared Buildbarn cache/execution path for artifact writes,
  remote execution, and cache reuse only, with a pod-local BuildStream cache kept
  for fast per-pod retry state. The workflow uses a checked-in BuildStream config
  and a warm-cache pre-step so the shared object, action, and remote-asset caches
  are primed before the main build. Dakota now defaults to `build-mode=cache-only`
  while the current BuildBarn remote-execution sandbox remains unstable in the
  webkitgtk path; explicit `build-mode=re` or `build-mode=auto` overrides remain
  available for operators who need to test the RE lane. The semaphore for the
  heavy BuildStream step lives on the actual `bst-build-local`/`bst-build-re`
  templates, so retries can re-enter the lane instead of getting stuck behind a
  parent-template lock.
- **Priority:** `priorityClassName: bst-build` — preemptable by `lab-test-vm`
  pods on resource contention.
- **Who triggers it automatically:** `dakota-commit-poller` (see
  [Cache Warming](#cache-warming-pollers)). The poller resolves the current
  GitHub SHA for `dakota:testing` and passes that exact commit into the local
  BuildStream run, so the lab build checks out the same source revision that
  GitHub is building instead of drifting to a later branch tip.

### cosmic-build-pipeline
- **Purpose:** BuildStream compile pipeline for COSMIC variants
  (`oci/cosmic/image.bst`, `oci/cosmic-nvidia/image.bst`) and push to local Zot.
- **Safety guards (aligned with dakota):**
  `activeDeadlineSeconds: 14400` (workflow), `activeDeadlineSeconds: 5400` (step),
  `retryStrategy: limit=2, retryPolicy=Always`, `GRPC_POLL_STRATEGY=poll`,
  `GRPC_ENABLE_FORK_SUPPORT=1`, `request-timeout: 900`,
  `scheduler.network-retries: 4`, `scheduler.fetchers: 1`.
- **Cache policy:** uses the same shared Buildbarn remote cache/execution path as
  Dakota, via the checked-in `buildstream-remote-cache` config, with
  `override-project-caches: false` and explicit upstream artifact/source cache URLs
  listed as read-only fallbacks while Buildbarn handles artifact writes.

### bluefin-server-build-pipeline
- **Purpose:** BuildStream compile pipeline for Bluefin Server elements
  (`oci/bluefin-server-ddi.bst`, `oci/bluefin-server-installer.bst`) and push to local Zot.
- **Safety guards (aligned with dakota/cosmic):**
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

### knuckle-qa-pipeline
- **Purpose:** Build the Knuckle installer ISO/binary, provision a blank VM, run
  the headless installer in-cluster, boot the installed system, rediscover SSH
  reachability, and run smoke tests.
- **Parameters:** `branch`, `namespace`, `suite`, `ssh-key-secret`, `tests-branch`.
- **DAG:** `clone-source` → `build-installer` → `provision-target-vm` →
  `boot-installer` → `wait-install-complete` → `transition-to-installed` →
  `discover-installed-ip` → `run-smoke-tests`; `onExit: teardown`
  (`teardown-vm` → `cleanup-installer-artifacts`).
- **Disk:** PVC-backed (`local-path`), not hostDisk/hostPath — KubeVirt
  co-schedules the VM automatically on the PVC's node, no explicit
  `nodeSelector` needed.
- **Just recipe:** None currently; submit the WorkflowTemplate directly or use
  the nightly CronWorkflow.

## Supporting Templates

| Template | Role |
| --- | --- |
| `build-containerdisk` | Shared containerDisk builder used by `digest-watch`/`image-poller`. Flow: `check` (Zot existence) → `install-to-disk` → `convert-and-push`. Builds a KubeVirt containerDisk from a bootc image and pushes it to the local registry. |
| `provision-containerdisk-vm` | Shared Bluefin/Dakota/COSMIC VM bring-up directly from a containerDisk (no reflink, no golden disk, no hostDisk). `create-vm` defines a 4 vCPU KubeVirt VM (`vm-memory` param, default 8Gi), `wait-for-vm-ready` starts `sshd.socket` via QEMU guest agent and returns the pod IP once SSH is reachable. `priorityClassName: lab-test-vm`. |
| `provision-flatcar-vm` / `provision-gnomeos-vm` | Same containerDisk-VM pattern as above for their respective variants. Both set `priorityClassName: lab-test-vm`. |
| `teardown-vm` | Deletes any KubeVirt test VM (and hostDisk/PVC where applicable). |
| `run-gnome-tests` | Shared test runner. Clones `projectbluefin/testsuite` (the single source of truth), waits for SSH, installs test dependencies, copies `tests/<suite>`, and runs `behave`. GUI suites (smoke) run via `qecore-headless` inside the VM. The `common` suite runs from the runner container with `VM_IP`/`SSH_KEY` exported so its SSH steps reach the VM directly — do NOT run common via qecore-headless. The `system` suite runs inside the VM without a display. |
| `run-incluster-tests` | Shared in-cluster pytest runner. Git-syncs `lab`, runs a pytest module against a live k8s workload, emits JUnit XML. |

## Distributed Build/RE Grid

Two independent distributed-build mechanisms exist on the cluster — they solve
different problems and do not overlap:

| Mechanism | What it distributes | Used by |
| --- | --- | --- |
| k8s scheduler (no pin) | Full privileged bootc OCI builds (needs real FUSE/mount-namespace access) | `dakota-build-pipeline`, `bluefin-server-build-pipeline` |
| Buildbarn (`buildbarn` namespace) | BuildStream cache writes and remote-execution actions (chroot-only sandbox, `CAP_SYS_CHROOT`) | `dakota-build-pipeline`, `cosmic-build-pipeline`, `bluefin-server-build-pipeline`, `bst-qa-pipeline` |

Buildbarn topology (2 storage shards, 1 scheduler, 2 frontend replicas, 1
worker+runner DaemonSet pair per node — one shard/worker pair pinned per node
via `podAntiAffinity`) is defined in `manifests/buildbarn-*.yaml`. It **cannot**
run the real dakota/bluefin-server OCI builds — those require privileges
Buildbarn's runner deliberately does not grant. For Dakota, the BuildStream lane
uses the shared Buildbarn cache path for artifact writes and only opts into remote
execution when the USB4 data-plane is confirmed up; if the link is down or a
retry happens, it falls back to the low-concurrency, cache-only path so the
cluster stays usable over 2.5GbE.

## Cache Warming (Pollers)

| CronWorkflow | Interval | Triggers | Keeps warm |
| --- | --- | --- | --- |
| `digest-watch` | 5 min | `build-containerdisk` (force rebuild) when `bluefin`/`bluefin-lts` GHCR digest changes | The containerDisk that `bluefin-qa-pipeline`'s `assert-cd` depends on |
| `dakota-commit-poller` | 5 min | `dakota-build-pipeline` when `dakota:testing` gets a new commit digest | the shared Buildbarn cache/execution path for the Dakota BuildStream layer |
| `image-poll-bluefin-{testing,stable}` / `image-poll-lts-{testing,stable}` | 10 min | `bluefin-qa-pipeline` when the respective GHCR tag digest changes | Test coverage freshness (not a build cache) |
| `image-poll-snosi-latest` | 30 min past every 3 hours | `bluefin-qa-pipeline` when `ghcr.io/frostyard/snow:latest` changes | Snosi GNOME desktop image coverage |
| `flatcar-kernel-poller` | 10 min | `flatcar-kernel-build` when kernel.org's latest stable version changes | Flatcar kernel build cache |
| `flatcar-kernel-gate` | 30 min | (gate/promotion check, see `docs/skills/flatcar-node-onboarding.md`) | N/A |

Dakota/Cosmic/Bluefin Server/BST lanes now use the shared Buildbarn frontend for
cache writes and remote execution while leaving upstream mirrors read-only. Cold
runs may fetch from upstream source origins, but cache writes stay in-cluster via
Buildbarn.

**`bluefin-server-build-pipeline` has no poller at all** — manual-trigger only,
and deliberately local-cache-only (commit `22c7d2ad`, "use local-cache path... for
extreme stability"). It is cold by design on every run; this is an intentional
trade-off, not a gap.

**`nightly-dakota` does not warm anything** — it's wired to `dakota-qa-pipeline`
(test runner against pre-built images), not `dakota-build-pipeline` (the actual
compile step). The real dakota cache-warming trigger is `dakota-commit-poller`.

All 6 bluefin/lts pollers plus `digest-watch`/`dakota-commit-poller`/
`flatcar-kernel-poller`/`flatcar-kernel-gate` were briefly suspended (2026-06-28
through 2026-07-02) while a real bug was fixed — `containerdisk-tag`'s default
value self-referenced `{{workflow.parameters.image-tag}}` inside the same
`arguments.parameters` block, which Argo does not resolve. Fixed in commit
`be045b12` with an explicit literal default plus a regression test
(`tests/unit/test_workflow_defaults.py`). Re-enabled 2026-07-03 after
confirming the fix holds and re-verifying package/network availability.

**Known gap:** `digest-watch` only rebuilds a containerDisk when the upstream
GHCR digest *changes* vs the `containerdisk-source-digests` ConfigMap — it has
no way to notice the containerDisk itself disappeared out-of-band (e.g. a Zot
disk wipe) while the upstream digest stayed the same. This actually happened
2026-07-03 after the ghost XFS migration wiped Zot: `bluefin-containerdisk`
was completely absent, `digest-watch` kept reporting "no change — skipping",
and `bluefin-qa-pipeline`'s `assert-cd` would have failed indefinitely.
Recovered by manually submitting `build-containerdisk` with `force=true`
directly. If this needs to self-heal automatically next time, `digest-watch`
would need an additional Zot-existence check (like `assert-cd`'s skopeo probe)
alongside the digest comparison — not implemented yet.

## Nightly Schedule

| CronWorkflow | Time (UTC) | Pipeline | Parameters |
| --- | --- | --- | --- |
| `nightly-smoke` | 02:00 | `bluefin-qa-pipeline` | `image=ghcr.io/projectbluefin/bluefin`, `image-tag=testing`, `containerdisk-tag=testing`, `namespace=bluefin-test`, `suites=smoke,developer,system` |
| `nightly-smoke-lts` | 02:30 | `bluefin-qa-pipeline` | `image=ghcr.io/projectbluefin/bluefin-lts`, `image-tag=testing`, `containerdisk-tag=lts-testing`, `namespace=bluefin-lts-test`, `suites=smoke,developer,system`, `vm-memory=8Gi` |
| `nightly-dakota` | — | `dakota-qa-pipeline` | Tests pre-built images only — does not build/warm cache. Currently `suspend: true`. |
| `nightly-knuckle` | 03:30 | `knuckle-qa-pipeline` | `branch=main`, `namespace=knuckle-test`, `suite=smoke`, `tests-branch=main` |

## Priority Classes

| PriorityClass | Value | Applied to |
| --- | --- | --- |
| `lab-test-vm` | 1,000,000, `PreemptLowerPriority` | All KubeVirt test VMs (`provision-containerdisk-vm`, `provision-flatcar-vm`, `provision-gnomeos-vm`, `knuckle-qa-pipeline`'s two VM specs) |
| `bst-build` | (see `manifests/bst-build-priorityclass.yaml`) | Heavy/long BuildStream compiles: `dakota-build-pipeline`, `bluefin-server-build-pipeline`, `flatcar-kernel-build`'s VM spec |

Test VMs are meant to win resource contention over background build workloads —
`lab-test-vm`'s higher priority value plus `PreemptLowerPriority` enforces this
against any pod using `bst-build`.

## Resource Profiles

Pod resource requests/limits used by workflow steps:

| Template | CPU req/limit | Memory req/limit |
| --- | --- | --- |
| `build-containerdisk/check` | 100m / 500m | 128Mi / 512Mi |
| `build-containerdisk/install-to-disk` | 4 / 8 | 4Gi / 12Gi |
| `build-containerdisk/convert-and-push` | 2 / 4 | 4Gi / 8Gi |
| `wait-for-vm-ready` | 100m / 500m | 128Mi / 256Mi |
| `run-gnome-tests` | 1 / 2 | 1Gi / 2Gi |
| `dakota-build-pipeline/bst-build` | 2 / 4 | 14Gi / 28Gi |
| `cosmic-build-pipeline/bst-build` | 4 / 8 | 14Gi / 28Gi |
| `bluefin-server-build-pipeline/bst-build` | 6 / 10 | 16Gi / 30Gi |
| `knuckle build-installer` | 4 / 4 | 8Gi / 8Gi |
| `knuckle write-ignition` | 100m / 500m | 128Mi / 256Mi |
| `knuckle boot-installer` | 1 / 2 | 1Gi / 2Gi |
| `knuckle wait-install-complete` / `transition-to-installed` / `discover-installed-ip` | 250m / 1 | 256Mi / 512Mi |
| `knuckle cleanup-installer-artifacts` | 50m / 200m | 64Mi / 128Mi |
