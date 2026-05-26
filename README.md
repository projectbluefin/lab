# bluefin-test-suite

A cloud-native QA pipeline for [Project Bluefin](https://projectbluefin.io) desktops.

Runs inside Kubernetes on [ghost](https://github.com/castrojo/utah), driven by **Argo Workflows**, booting Bluefin as a **KubeVirt hostDisk VM** (golden disk + btrfs reflink), and executing GUI tests via **qecore-headless + behave/pytest + Dogtail (AT-SPI)** — no ISO installer, no pixel matching.

## Guiding mantra

This repo exists to prove **Bluefin's image-based operating model**, not to recreate traditional package-manager testing on an atomic desktop.

When choosing what to build, prioritize work that verifies:

1. the **booted image contract** (`bootc`, staged deployments, rollback behavior, read-only `/usr`)
2. the **integrity contract** (composefs, fs-verity, signature policy, fallback behavior)
3. the **update orchestration contract** (`uupd`, stream behavior, staged update visibility)
4. the **user-space isolation contract** (Homebrew, Flatpak, rootless Podman, Docker/Colima living outside the host image)

UI and app coverage still matter, but they should reinforce this model instead of pulling the suite back toward mutable-package assumptions.

---

## Architecture

```
GitHub webhook / just run-tests
        │
        ▼
  Argo Workflow (argo ns)
        │
        ├─ bib-build-and-push ────► BIB golden disk on ghost hostPath
        │                           /var/tmp/bluefin-golden/<tag>/disk.raw
        │
        ├─ provision-bluefin-vm ──► btrfs reflink (~24ms) + KubeVirt VM
        │                           namespace: bluefin-test
        │
        ├─ run-gnome-tests ────────► runner pod (Fedora + qecore-headless)
        │                           git-sync → SSH → VM
        │                           behave/pytest + Dogtail (AT-SPI)
        │
        └─ teardown (onExit) ──────► delete VM + hostDisk clone
```

## Test phases

| Phase | Suite | Runs on |
|---|---|---|
| 1 — Golden Path smoke | `smoke` | Every PR |
| 2 — Developer tooling | `developer` | Every merge |
| 3 — Software management | `software` | Nightly |

## Quick start

All routine commands live in the repo `Justfile`. For the canonical command matrix and operator recipes, use [`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md).

- Test-only iteration: titan lane (`just run-titan-smoke`)
- Image or golden-disk validation: fresh-VM lane (`just run-tests` / `just run-tests-matrix`)
- Branch-under-test: `BLUEFIN_TEST_BRANCH=<ref>` on the relevant `just` target
- Workflow evidence: `just logs`

> **Agents:** load [`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md) first —
> single-file, deterministic recipes for 80% of routine cluster ops (test runs,
> failure triage, ArgoCD, titan recovery, CronWorkflow ops, SSH rotation, safe
> cleanup). Escalate to [`docs/lab-operations.md`](docs/lab-operations.md) for the
> long-form ops guide, [`RUNBOOK.md`](RUNBOOK.md) for architecture, or
> [`WORKFLOWS.md`](WORKFLOWS.md) for WorkflowTemplate parameter contracts.

## Repository layout

```
bluefin-test-suite/
├── Justfile                          # all commands live here
├── plans/
│   ├── main.fmf                      # fmf root metadata
│   └── flatcar.fmf                   # tmt plans for Flatcar tests only
├── tests/
│   ├── smoke/features/               # Phase 1: GNOME Shell, Activities, extensions
│   ├── developer/                    # Phase 2: terminal, brew, podman, micro
│   ├── software/                     # Phase 3: GNOME Software, Flatpak
│   └── flatcar/                      # Flatcar OS tests (tmt)
└── argo/
    ├── bluefin-smoke-test.yaml       # single-image workflow
    ├── bluefin-test-matrix.yaml      # multi-channel matrix workflow
    └── workflow-templates/
        ├── bib-build-and-push.yaml   # golden disk build (BIB)
        ├── provision-bluefin-vm.yaml # reflink clone + KubeVirt VM
        ├── run-gnome-tests.yaml      # qecore-headless SSH runner for behave/pytest
        └── teardown-vm.yaml          # VM + hostDisk cleanup (onExit)
```

## Prerequisites

- k3s + KubeVirt running on ghost (`192.168.1.102`)
- Argo Workflows installed in `argo` namespace
- `bluefin-test` and `bluefin-lts-test` namespaces exist
- Secret `bluefin-test-ssh-key` in `argo` namespace with an `id_ed25519` key

### Create the SSH secret

```bash
ssh-keygen -t ed25519 -f /tmp/bluefin-test-key -N ""
kubectl create secret generic bluefin-test-ssh-key \
    --from-file=id_ed25519=/tmp/bluefin-test-key \
    --from-file=id_ed25519.pub=/tmp/bluefin-test-key.pub \
    -n argo
```

## Writing new tests

1. Add a `.feature` file under `tests/<suite>/features/`
2. Add step definitions to `tests/<suite>/features/steps/steps.py`
3. Use `context.sandbox.shell` (qecore) for gnome-shell AT-SPI access
4. Use `Shell.Eval` for panel interactions not exposed in AT-SPI (clock, system menu)
5. Each scenario must be independent — no shared state between scenarios
