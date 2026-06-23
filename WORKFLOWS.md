# WorkflowTemplates — Agent Contract

This is the canonical interface for driving the lab. Every supported
operation is a single `argo submit --from workflowtemplate/<name> [-p k=v]`
invocation. No bash, no `kubectl apply`, no SSH.

Conventions:

- All templates live in `argo/workflow-templates/*.yaml` and are reconciled
  to namespace `argo` by the ArgoCD `testing-lab` Application.
- Workflow-level parameters listed below are passed via `-p name=value`.
- Wall-clock targets are warm-cache numbers; cold-cache figures add ~5–10 min.
- The agent contract: prefer the **top-level** templates (`bluefin-qa-pipeline`,
  `knuckle-qa-pipeline`, `dakota-qa-pipeline`). The supporting templates (provision, run, teardown)
  are called as `templateRef` and rarely submitted directly.

---

## Top-level entry points

### `bluefin-qa-pipeline`

Full pipeline: build containerDisk (if digest changed) → boot a fresh KubeVirt VM →
run test suites → teardown VM on exit.

| Parameter | Default | Notes |
|---|---|---|
| `image` | `ghcr.io/projectbluefin/bluefin` | Source image. Tag is appended from `image-tag` for some callers; pass with tag if invoking directly. |
| `image-tag` | `latest` | `latest`, `lts`, etc. Also used as the golden-disk dir name. |
| `namespace` | `bluefin-test` | KubeVirt VM namespace. Use `bluefin-lts-test` for LTS. |
| `suites` | `smoke,developer` | Comma list; valid: `smoke`, `developer`, `software`. |
| `variant` | `bluefin` | Selects test fixtures (e.g. `dakota` for Ghostty). |
| `ssh-key-secret` | `bluefin-test-ssh-key` | Secret in `argo` ns with `id_ed25519`. |

Wall-clock: ~5 min (warm, containerDisk cached), ~10–14 min (cold containerDisk build).

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
| `tests-branch` | `main` | `testing-lab` branch cloned by `run-gnome-tests`. |

Wall-clock: ~12–20 min depending on ISO build cache and Flatcar download time.

```
argo submit --from workflowtemplate/knuckle-qa-pipeline \
  -p branch=main -p suite=smoke --wait
```

---

## Supporting templates (called via `templateRef`)

These are exposed because they are referenced by the entry points;
submit them directly only for diagnosis.

### `build-containerdisk` (template: `build-containerdisk`)

Builds a KubeVirt containerDisk from a bootc image and pushes it to the local
Zot registry at `192.168.1.102:30500`. Checks if an up-to-date image already
exists (digest comparison) and skips the build if so.

### `provision-bluefin-vm` (template: `provision-vm`)

Creates a KubeVirt VM using the containerDisk from the local Zot registry,
waits for the VMI to be Ready and SSH to become reachable, emits `vm-ip`.

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

### `teardown-bluefin-vm` / `teardown-flatcar-vm`

Delete the VM and wait for the VMI object to drain. Invoked as `onExit` from
the pipeline templates — always runs regardless of pipeline outcome.

---

## Dakota BST builds

### `dakota-bst`

Drives dakota BuildStream builds on ghost via the existing `just` recipes.
Mounts jorge's BST cache for warm builds (~2–5 min warm, ~60–90 min cold).
No changes to the dakota repo — this is purely an orchestration wrapper.

| Parameter | Default | Notes |
|---|---|---|
| `variant` | `default` | `default`, `nvidia`, or `all` |
| `branch` | `main` | dakota branch to clone |

Pipeline: `bst-validate` (fast graph check) → `bst-build` (build + lint).

```
just run-dakota-validate              # bst show only, ~5 min
just run-dakota-build                 # default variant
just run-dakota-build nvidia          # nvidia variant
just run-dakota-build all             # both variants sequentially
```

---

## CronWorkflows

Lives in `manifests/`, applied via the `testing-lab-infra` ArgoCD app:

| Name | Schedule | Template called | Purpose |
|---|---|---|---|
| `nightly-smoke` | 02:00 UTC | `bluefin-qa-pipeline` (testing) | Catch upstream regressions |
| `nightly-smoke-lts` | 02:30 UTC | `bluefin-qa-pipeline` (lts-testing) | Same, for LTS branch |
| `nightly-dakota` | 03:00 UTC | `dakota-qa-pipeline` | Dakota nightly |
| `nightly-knuckle` | 03:30 UTC | `knuckle-qa-pipeline` | Knuckle installer nightly |
| `image-poll-bluefin-testing` | hourly | `image-poller` | Trigger on new bluefin:testing digest |
| `image-poll-lts-testing` | hourly | `image-poller` | Trigger on new bluefin-lts:testing digest |
| `image-poll-bluefin-stable` | weekly (Sun 01:00) | `image-poller` | Trigger on new bluefin:stable digest |
| `image-poll-lts-stable` | weekly (Sun 01:30) | `image-poller` | Trigger on new bluefin-lts:stable digest |
| `orphan-vm-cleanup` | every 2h | inline | GC VMs whose parent workflow was force-deleted |
| `orphan-pod-gc` | every 30min | inline | Clean ContainerStatusUnknown + failed pods |
| `golden-disk-gc` | 04:00 UTC | inline | GC stale disk.raw files on ghost |

---

## Editing this contract

When you add or rename a template, update this file in the same PR. Drift
between templates and this doc is what breaks autonomous agents.
