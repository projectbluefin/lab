# WorkflowTemplates — Agent Contract

This is the canonical interface for driving the lab. Every supported
operation is a single `argo submit --from workflowtemplate/<name> [-p k=v]`
invocation. No bash, no `kubectl apply`, no SSH.

Conventions:

- All templates live in `argo/workflow-templates/*.yaml` and are reconciled
  to namespace `argo` by the ArgoCD `lab` Application.
- Workflow-level parameters listed below are passed via `-p name=value`.
- Wall-clock targets are warm-cache numbers; cold-cache figures add ~5–10 min.
- The agent contract: prefer the **top-level** templates (`bluefin-qa-pipeline`,
  `knuckle-qa-pipeline`, `dakota-qa-pipeline`). The supporting templates (provision, run, teardown)
  are called as `templateRef` and rarely submitted directly.

---

## Top-level entry points

### `bluefin-qa-pipeline`

Container-only pipeline: validate the selected suites directly inside the
published bootc OCI image via `run-container-tests`. No KubeVirt VM or
containerDisk stage exists on this path.

| Parameter | Default | Notes |
|---|---|---|
| `image` | `ghcr.io/projectbluefin/bluefin` | Source image. Tag is appended from `image-tag` for some callers; pass with tag if invoking directly. |
| `image-tag` | `testing` | `testing`, `lts-testing`, `stable`, `lts-stable`. |
| `suites` | `smoke,common,developer,software,system` | Comma list; valid: `smoke`, `common`, `developer`, `software`, `system`. |
| `variant` | `bluefin` | Selects test fixtures and result slugging. |
| `branch` | `main` | Branch context recorded with published results. |
| `testsuite-branch` | `main` | Testsuite branch cloned by `run-container-tests`. |
| `testsuite-repo` | `https://github.com/projectbluefin/testsuite` | Override only for testsuite forks. |

Wall-clock: a few minutes per selected suite; no VM provisioning stage.

```
argo submit --from workflowtemplate/bluefin-qa-pipeline \
  -p image-tag=testing -p suites=smoke --wait
```

### `knuckle-qa-pipeline`

Builds the Knuckle installer ISO from source, boots a blank KubeVirt VM in
`knuckle-test`, runs a headless install with an explicit `/install-complete`
signal, reboots from the installed disk, rediscovers the new VMI IP, then runs
smoke tests against the installed system.

| Parameter | Default | Notes |
|---|---|---|
| `branch` | `main` | Knuckle source branch to clone and build. |
| `namespace` | `knuckle-test` | KubeVirt namespace for the ephemeral installer VM. |
| `suite` | `smoke` | Single GNOME test suite to run after install. |
| `ssh-key-secret` | `bluefin-test-ssh-key` | Secret in `argo` ns used for installer access and installed-system SSH. |
| `tests-branch` | `main` | `testsuite` branch cloned by `run-gnome-tests`. |

Wall-clock: ~12–20 min depending on ISO build cache and Flatcar download time.

```
argo submit --from workflowtemplate/knuckle-qa-pipeline \
  -p branch=main -p suite=smoke --wait
```

### `cosmic-qa-pipeline`

Container-only COSMIC QA pipeline: run the selected suite directly inside the
published bootc OCI image via `run-container-tests`. No containerDisk, KubeVirt
VM, or SSH stage exists on this path.

| Parameter | Default | Notes |
|---|---|---|
| `image` | `ghcr.io/razorfinos-org/cosmic-build-meta` | Source image repo. |
| `image-tag` | `cosmic-pr-33` | Published COSMIC bootc image tag under test. |
| `suites` | `smoke` | Test suite to execute. |
| `variant` | `cosmic` | Set to `cosmic` for the COSMIC desktop environment. |
| `branch` | `main` | Branch context recorded with published results. |
| `testsuite-branch` | `main` | Testsuite branch cloned by `run-container-tests`. |
| `testsuite-repo` | `https://github.com/projectbluefin/testsuite` | Override only for testsuite forks. |

Wall-clock: a few minutes per selected suite; no VM provisioning stage.

```
argo submit --from workflowtemplate/cosmic-qa-pipeline \
  -p image-tag=cosmic-pr-33 -p suites=smoke --wait
```

---

## Supporting templates (called via `templateRef`)

These are exposed because they are referenced by the entry points;
submit them directly only for diagnosis.

### `k8sgpt-on-demand`

Runs an on-demand K8sGPT cluster scan and stores the full analyzer output as a
workflow artifact while printing a concise findings summary to logs/stdout.

| Parameter | Default | Notes |
|---|---|---|
| `namespace` | `""` | Empty means cluster-wide scan |
| `filters` | `Pod,Deployment,Service,Ingress,Node` | Default core filters; override for focused triage |
| `ignored-services` | `argocd/argocd-applicationset-controller,argocd/argocd-dex-server,argocd/argocd-notifications-controller-metrics,kubevirt/virt-exportproxy` | Comma-delimited Service names (`namespace/name`) to suppress known no-endpoint noise |

Output parameter: analyze node emits `k8sgpt-results-json` from `/tmp/results/k8sgpt-results.json`.

### `image-poller` (template: `check-and-trigger`)

Fetches the current GHCR digest, compares it with `image-polling-digests`, runs
`bluefin-qa-pipeline` when the digest changes, and persists the new digest only
after the downstream workflow succeeds.

### `run-container-tests` (template: `run-container-tests`)

Runs `smoke`, `common`, `developer`, `software`, or `system` directly inside the
target bootc OCI image with `dbus-run-session` + `qecore-headless`, then
publishes per-suite results back to this repo when `github-token` is available.

### `provision-flatcar-vm` (template: `provision-vm`)

Same shape for Flatcar — accepts an `ssh-pubkey` parameter directly instead
of relying on the bluefin-test secret for cloud-init injection.

### `run-gnome-tests` (template: `run-gnome-tests`)

SSHes into the VM, installs test deps (qecore, behave, dogtail,
gnome-ponytail-daemon via `ostree admin unlock` + dnf), runs qecore-headless +
behave, captures results to pod stdout (Loki + `argo logs`).

`hostNetwork: true` is required — KubeVirt masquerade only routes from the host
network namespace.

### `run-flatcar-tests` (template: `run-flatcar-tests`)

Same shape for Flatcar; uses `core` as the SSH user and runs pytest+dogtail
fixtures from `tests/flatcar/`.

### `teardown-vm` (template: `teardown-vm`)

Delete the VM and wait for the VMI object to drain. Invoked as `onExit` from
the pipeline templates — always runs regardless of pipeline outcome.

---

## Dakota BST builds

### `dakota-build-pipeline`

Builds Dakota BuildStream OCI artifacts in-cluster using the shared Buildbarn
remote-execution fabric and a checked-in BuildStream config. The build pod still
owns the high-level BuildStream process, but it now routes artifact writes,
source-fetch activity, and remote execution through the in-cluster Buildbarn
frontend so the shared cache and worker pool can accelerate the full build path.
`build-bluefin` and `build-bluefin-nvidia` run in parallel and push to local Zot.

| Parameter | Default | Notes |
|---|---|---|
| `ref` | `testing` | Dakota git branch/ref to clone |
| `repo` | `https://github.com/projectbluefin/dakota.git` | Dakota git repo |
| `registry` | `<lab-ip>:30500` | Registry endpoint for builder image + pushed artifacts |

```
just run-bst-build                    # testing branch, default repo
just run-bst-build main               # build from main
```

## COSMIC BST builds

### `cosmic-build-pipeline`

Builds COSMIC BuildStream OCI artifacts in-cluster. `build-cosmic` and `build-cosmic-nvidia` run in parallel and push to local Zot.

| Parameter | Default | Notes |
|---|---|---|
| `ref` | `main` | COSMIC git branch/ref to clone |
| `repo` | `https://github.com/RazorfinOS-org/cosmic-build-meta.git` | COSMIC build meta git repo |
| `registry` | `<lab-ip>:30500` | Registry endpoint for builder image + pushed artifacts |

---

## CronWorkflows

Lives in `manifests/`, applied via the `lab-infra` ArgoCD app:

| Name | Schedule | Template called | Purpose |
|---|---|---|---|
| `nightly-smoke` | 02:00 UTC | `bluefin-qa-pipeline` (testing) | Catch upstream regressions |
| `nightly-smoke-lts` | 02:30 UTC | `bluefin-qa-pipeline` (lts-testing) | Same, for LTS branch |
| `nightly-dakota` | 03:00 UTC | `dakota-qa-pipeline` | Dakota nightly |
| `nightly-knuckle` | 03:30 UTC | `knuckle-qa-pipeline` | Knuckle installer nightly |
| `image-poll-bluefin-testing` | hourly | `image-poller` | Compare digest, fan out container-only QA, publish results, then persist digest for `bluefin:testing` |
| `image-poll-lts-testing` | hourly | `image-poller` | Same flow for `bluefin-lts:testing` |
| `image-poll-bluefin-stable` | weekly (Sun 01:00) | `image-poller` | Same flow for `bluefin:stable` |
| `image-poll-lts-stable` | weekly (Sun 01:30) | `image-poller` | Same flow for `bluefin-lts:stable` |
| `orphan-vm-cleanup` | every 2h | inline | GC VMs whose parent workflow was force-deleted |
| `orphan-pod-gc` | every 30min | inline | Clean ContainerStatusUnknown + failed pods |
| `golden-disk-gc` | 04:00 UTC | inline | GC stale disk.raw files on ghost |

---

## Editing this contract

When you add or rename a template, update this file in the same PR. Drift
between templates and this doc is what breaks autonomous agents.
