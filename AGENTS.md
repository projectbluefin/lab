# Testing Lab — Agent Instructions

> **Operating routine? Load [`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md)
> first — it covers 80% of cluster ops with deterministic recipes.** This file is
> the canonical *policy* document; load it for tenets, scope rules, and the data
> tables. Long-form procedures live in [`docs/lab-operations.md`](docs/lab-operations.md);
> WorkflowTemplate parameter contracts in [`WORKFLOWS.md`](WORKFLOWS.md);
> architecture/failure-mode index in [`RUNBOOK.md`](RUNBOOK.md).

## What This Repo Is

Bluefin QA pipeline: Argo Workflows + KubeVirt + ArgoCD + behave/dogtail.
Tests boot Bluefin Linux VMs and run GNOME Shell accessibility smoke tests.
Canonical issue tracker: **castrojo/testing-lab** (this repo). Do NOT file issues in castrojo/copilot-config.

## Test Suite Mantra

This repo's north star is to verify **Bluefin as an image-based, atomic operating system**.
Agents should treat that as the primary culture of the project, not as a side concern.

When deciding what to test or prioritize:

1. **Prefer platform-contract coverage over package-era habits.**
   Validate `bootc`, staged deployments, rollback behavior, read-only `/usr`, signature policy,
   composefs/fs-verity, and `uupd` orchestration before inventing DNF/RPM-style checks.
2. **Treat Homebrew, Flatpak, Podman, and Docker/Colima as decoupled user-space layers.**
   The job is to prove those layers integrate cleanly without mutating the host image.
3. **Use UI coverage to reinforce system guarantees.**
   GNOME, Ptyxis, Podman Desktop, Bazaar, and related flows are valuable when they prove the
   Bluefin contract holds in real user workflows, not when they drift into generic desktop QA.
4. **Bias new issues and tests toward immutable-state evidence.**
   If a choice exists between another cosmetic UI check and a missing image/update/integrity
   assertion, prefer the image/update/integrity work.
5. **Keep everything VM-backed, GitOps-managed, and operator-friendly.**
   The expected output is durable workflow evidence that another agent or operator can rerun.

## Core Tenet: All Agent Operations Are API-Driven

**Agents must use the Kubernetes API and MCP servers. Never SSH to nodes. Never kubectl from outside the cluster.**

| Operation | Correct tool |
|---|---|
| Submit a workflow | Argo MCP `submit_workflow` or `just <target>` from a host with cluster access |
| Check workflow status | Argo MCP `get_workflow` / `list_workflows` |
| Get workflow logs | Argo MCP `logs_workflow` or `just logs` |
| Update a WorkflowTemplate | Edit YAML → `git push main` → ArgoCD auto-syncs (~3 min) |
| Update cluster infra | Edit `manifests/` → `git push main` → ArgoCD auto-syncs |
| Read cluster state | kubectl MCP or Argo MCP |

If an MCP tool doesn't exist for an operation, the right fix is to build or deploy that capability — not to fall back to SSH.

## Cluster Topology

| Host | Role | IP | Specs |
|---|---|---|---|
| ghost | k3s control-plane + KubeVirt compute | 192.168.1.102 | Ryzen AI MAX+ 395, 16c/32t, 64GB RAM |
| exo-1 | k3s worker (workflow pods only) | 192.168.1.239 | — |
| Argo UI | — | http://192.168.1.102:32746 | NodePort; also http://192.168.1.102:2746 on host |
| Loki | log aggregation | http://192.168.1.102:30100 | Scrapes pods labeled `app.kubernetes.io/part-of=bluefin-test-suite` |
| ArgoCD | GitOps controller | https://192.168.1.102 (argocd NS) | Two Applications: `testing-lab` + `testing-lab-infra` |

All KubeVirt VMs are pinned to ghost via `nodeSelector: kubernetes.io/hostname: ghost`.

## GitOps Rules

Two ArgoCD Applications manage this repo:

| Application | Syncs | Namespace |
|---|---|---|
| `testing-lab` | `argo/workflow-templates/` | argo |
| `testing-lab-infra` | `manifests/` | argo (+ others via namespace in manifest) |

Rules:
1. **WorkflowTemplate changes**: edit `argo/workflow-templates/*.yaml` → push to `main` → ArgoCD syncs
2. **Cluster infra changes**: edit `manifests/*.yaml` → push to `main` → ArgoCD syncs
3. **Never `kubectl apply`** WorkflowTemplates — ArgoCD overwrites manual applies
4. **Never `argo-mcp-create_workflow_template`** — ArgoCD owns that reconciliation loop
5. **Never amend published commits** — create new commits
6. Force sync when needed: `just argocd-sync`

`manifests/` uses `ServerSideApply: true` — manifests patch rather than replace. Safe to define partial resources (e.g. patching a Helm-managed ConfigMap by adding a key).

### KubeVirt feature gates

- `HostDisk` is required for the Bluefin/Flatcar hostDisk VM flows in this repo.
- `ExperimentalIgnitionSupport` is required for knuckle-style installer VMs that use the `kubevirt.io/ignitiondata` annotation.
- If VM creation fails with `feature gate is not enabled in kubevirt-config`, treat that as **cluster infra drift** and persist the fix via GitOps under `manifests/` rather than relying on an in-cluster manual patch.

## Repo Layout

```
argo/
  workflow-templates/          ← ArgoCD (testing-lab App) syncs these
    bib-build-and-push.yaml       build golden disk via BIB
    provision-vm.yaml             reflink golden disk + boot KubeVirt VM
    run-gnome-tests.yaml          SSH into VM, run qecore-backed behave/pytest suites
    teardown-vm.yaml              delete VM + hostDisk
    bluefin-titan-smoke.yaml      smoke against persistent titan VMs (fast path)
    bluefin-qa-pipeline.yaml      full pipeline: ensure-disk + provision + tests
    patch-golden-disk.yaml        retroactively fix SSH auth on existing disk
  bluefin-smoke-test.yaml         submit: full BIB+provision+test run (latest)
  bluefin-test-matrix.yaml        submit: parallel latest+lts matrix
manifests/                     ← ArgoCD (testing-lab-infra App) syncs these
  argo-server-nodeport.yaml       NodePort 32746 for external Argo API access
  titan-bluefin.yaml              persistent titan VM (latest)
  titan-lts.yaml                  persistent titan VM (lts)
  orphan-vm-cleanup.yaml          CronWorkflow: clean orphaned VMs every 2h
  nightly-smoke.yaml              CronWorkflow: nightly smoke latest @ 02:00 UTC
  nightly-smoke-lts.yaml          CronWorkflow: nightly smoke lts @ 02:30 UTC
  golden-disk-gc.yaml             CronWorkflow: GC stale golden disks @ 04:00 UTC (DRY_RUN=true default)
  workflow-controller-configmap.yaml  global TTL patch (7d success, 30d failure)
  flatcar-test-namespace.yaml     Flatcar test namespace
argocd/
  application.yaml               ArgoCD Application: testing-lab
  infra-application.yaml         ArgoCD Application: testing-lab-infra
tests/
  smoke/features/                behave/qecore GNOME Shell smoke tests ← ACTIVE
  developer/features/            behave GNOME desktop tests (podman, ptyxis, etc.)
  software/features/             behave flatpak/Bazaar tests
  flatcar/                       Flatcar systemd/container tests
AGENTS.md                        This file
RUNBOOK.md                       Operations reference — read before debugging
SECURITY.md                      Accepted homelab trade-offs and risks
Justfile                         Local shortcuts (require kubectl/argo access)
```

## Image Variants

| Tag | Image | Golden disk | Nightly |
|---|---|---|---|
| `latest` | `ghcr.io/ublue-os/bluefin:latest` | `/var/tmp/bluefin-golden/latest/disk.raw` on ghost | 02:00 UTC |
| `lts` | `ghcr.io/ublue-os/bluefin:lts` | `/var/tmp/bluefin-golden/lts/disk.raw` — built on first nightly fire | 02:30 UTC |

`gts` and `lts-hwe` do NOT exist. Never use these tags.

## Persistent (Titan) VMs

Two always-on VMs for fast test iteration — no BIB build needed, no VM provisioning wait:

| VM | Namespace | IP | Disk |
|---|---|---|---|
| `titan-bluefin` | bluefin-test | *(retrieve below)* | `/var/home/jorge/VMs/titans/titan-bluefin/image/disk.raw` |
| `titan-lts` | bluefin-lts-test | *(retrieve below)* | `/var/home/jorge/VMs/titans/image/disk.raw` |

> IPs are KubeVirt-allocated and drift. Always retrieve live:
> ```bash
> kubectl get vmi titan-bluefin -n bluefin-test     -o jsonpath='{.status.interfaces[0].ipAddress}{"\n"}'
> kubectl get vmi titan-lts     -n bluefin-lts-test -o jsonpath='{.status.interfaces[0].ipAddress}{"\n"}'
> ```

Managed by ArgoCD via `manifests/titan-bluefin.yaml` and `manifests/titan-lts.yaml`.
SSH key: `bluefin-test-ssh-key` secret in `argo` namespace.

**Titan run time**: ~5 min (SSH wait + dep check + copy + behave).
Deps skip if already installed — check is: `python3 -c 'import qecore, behave, dogtail'` + `rpm -q` + `qecore-headless` binary.

To run smoke against them: `just run-titan-smoke` or submit `bluefin-titan-smoke` WorkflowTemplate via Argo MCP with current VM IPs.

## Test Stack

| Component | Role |
|---|---|
| **behave** | BDD test runner |
| **qecore** | Red Hat test framework; `qecore-headless` starts Wayland session |
| **dogtail** | AT-SPI accessibility tree traversal |
| **gnome-ponytail-daemon** | Bridges AT-SPI coordinates to Wayland surface coordinates |
| **Shell.Eval** | `gdbus call --session --dest org.gnome.Shell --method org.gnome.Shell.Eval` — required for GNOME Shell 50 top-bar interactions (AT-SPI gaps) |

`qecore-headless` must be invoked with `--session-type wayland --session-desktop gnome` (explicit flags required).

**unsafe_mode** (`global.context.unsafe_mode = true`) must be enabled before top-bar AT-SPI interactions. Set via `gdbus call` in `environment.py` `before_all`.

## Known GNOME Shell 50 Limitations

On Bluefin 44 / GNOME Shell 50.1, the clock and system-status (quick-settings, dateMenu) toggle nodes **are present** in AT-SPI but report `INT_MIN` geometry — clicking them via dogtail silently misses. All clock/quick-settings/calendar interactions must use `Shell.Eval` JS via `gdbus`. The actionable top-bar nodes exposed normally are `Activities` and `Show Apps`. See [`docs/dogtail-testing.md`](docs/dogtail-testing.md) §6.4–§6.5 for the canonical `Shell.Eval` patterns.

## dogtail 4.16 API

`findChild(pred, requireResult=True/False)` — `requireResult` kwarg raises TypeError at the logging decorator. Use instead:
- `findChildren(pred)` → returns list, never raises
- `findChild(pred, retry=False)` → fast fail without 20s wait
- `searchCutoffCount` and `searchBackoffDuration` are deprecated no-ops

For the full guide on writing, submitting, and debugging dogtail/qecore/behave tests
in this repo, read [`docs/dogtail-testing.md`](docs/dogtail-testing.md).

For day-to-day cluster operations (running tests, triaging failures, rotating SSH keys,
recovering titans, pausing CronWorkflows, Loki queries, ArgoCD diagnosis, safe VM
cleanup), read [`docs/lab-operations.md`](docs/lab-operations.md) — it is the
paint-by-numbers operator manual and the canonical source for those procedures.

## Vanguard PR Report — required for PR queue mode

When this repo is supporting PR review for `knuckle`, `dakota`, or this repo itself,
**approval requires a canonical Vanguard Lab Strike Report posted as a PR comment** with
real lab evidence. The template lives at `~/src/skills/ghost-testlab/report-template.md`
on the operator's host. See [`docs/lab-operations.md`](docs/lab-operations.md) §11 for
the exit checklist. Metadata-only or narrative-only reviews do not satisfy this gate.

## Resource Limits (all workflow pods)

| Template | CPU req/limit | Memory req/limit |
|---|---|---|
| bib-img-build | 4 / 8 | 8Gi / 16Gi |
| bib-img-pull | 2 / 4 | 2Gi / 4Gi |
| bib-disk-configure | 2 / 4 | 4Gi / 8Gi |
| bib-disk-check | 100m / 500m | 128Mi / 512Mi |
| run-gnome-tests | 1 / 2 | 1Gi / 2Gi |
| reflink-disk | 100m / 500m | 128Mi / 512Mi |
| preflight (titan) | 100m / 200m | 64Mi / 128Mi |

Global TTL default (via workflow-controller-configmap): 7 days success, 30 days failure.

## Workflow History

Workflows are retained: 7 days on success, 30 days on failure (global workflowDefaults in workflow-controller-configmap) unless a workflow overrides TTL explicitly. No external archive database. Loki captures all pod logs. Use Argo MCP `logs_workflow` or `just logs` to retrieve results from completed runs.

## SSH Key

`bluefin-test-ssh-key` secret in `argo` namespace. Contains `id_ed25519` and `id_ed25519.pub`.

```bash
# Always retrieve the live fingerprint — do not trust hardcoded values:
kubectl get secret bluefin-test-ssh-key -n argo \
  -o jsonpath='{.data.id_ed25519\.pub}' | base64 -d | ssh-keygen -lf -
```

## Namespaces

| Namespace | Purpose |
|---|---|
| argo | Argo Workflows + ArgoCD control plane |
| argocd | ArgoCD |
| bluefin-test | latest variant test VMs |
| bluefin-lts-test | lts variant test VMs |
| flatcar-test | Flatcar test VMs |

**Never delete VMs or resources in namespaces outside this list.**

## YAML Authoring Rules

- **No inline Python inside bash inside YAML** — colons and quotes in Python cause YAML parse errors. Use `kubectl` + `jsonpath` instead.
- **No `generateName` in `manifests/`** — ArgoCD needs stable names to track resources. Use fixed `name:` fields.
- **Use `workflowTemplateRef`** in CronWorkflows instead of inlining DAG templates — avoids duplication.
- **Server-side apply is enabled** for `manifests/` — you can patch a subset of a resource's fields without owning the whole object.

## Issue Filing

- All issues go in **castrojo/testing-lab** (this repo)
- Label: `bug` for test failures and infrastructure breaks; `enhancement` for new capabilities
- Include: current behavior, expected behavior, exact file:line if code issue, acceptance criteria
- For infra failures: include workflow name, pod name, and relevant log excerpt

## Common Operations

```bash
# Check cluster state
just list-vms
just list-workflows

# Run smoke against titan VMs (fast — no BIB needed, ~5min)
just run-titan-smoke

# Run broader GNOME desktop coverage
just run-developer-tests
just run-software-tests
just run-titan-developer
just run-titan-software

# Run full smoke (BIB + provision + test + teardown, ~10min warm)
just run-tests

# Build/rebuild golden disk
just ensure-disk         # latest
just ensure-disk lts     # lts

# Fix SSH auth on existing disk after secret rotation
just patch-disk          # latest
just patch-disk lts

# Force ArgoCD sync
just argocd-sync

# Check ArgoCD status
just argocd-status

# Clean up orphaned VMs
just delete-vms

# Lint Argo YAML
just lint
```
