# bluefin-test-suite

A cloud-native QA pipeline for [Project Bluefin](https://projectbluefin.io) desktops.

Runs inside Kubernetes on [ghost](https://github.com/castrojo/utah), driven by **Argo Workflows**, booting Bluefin as a **KubeVirt hostDisk VM** (golden disk + btrfs reflink), and executing GUI tests via **behave + qecore + Dogtail (AT-SPI)** — no ISO installer, no pixel matching.

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
        │                           behave + Dogtail (AT-SPI)
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

```bash
# Run smoke tests against latest Bluefin
just run-tests

# Run the full matrix (latest + lts)
just run-tests-matrix

# Apply WorkflowTemplates to the cluster
just apply-templates

# Watch logs
just logs
```

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
        ├── run-gnome-tests.yaml      # behave + qecore-headless SSH runner
        └── teardown-bluefin-vm.yaml  # VM + hostDisk cleanup (onExit)
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
