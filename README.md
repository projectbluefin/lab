# Bluefin Testing Lab

> CI infrastructure for [Project Bluefin](https://projectbluefin.io). When a new
> Bluefin image is published, this lab boots it in a real KubeVirt VM and runs
> acceptance tests — GNOME shell, extensions, bootc contract, uupd, filesystem
> integrity. Screenshots from passing runs appear in Bluefin GitHub Releases.

---

## What This Is

The automated QA pipeline for Project Bluefin and Project Dakota. Every image
publish triggers: boot a fresh VM from the OCI image, run GUI and system acceptance
tests, collect screenshots, tear down. Everything is declared in git, reconciled by
ArgoCD, and orchestrated by Argo Workflows.

**No persistent VMs. No manual `kubectl`. No SSH to the cluster host.**

See [docs/bluefin-integration.md](docs/bluefin-integration.md) for the full
image-poll → test → screenshot → release pipeline.

---

## Bluefin Integration

Three images are under continuous test:

| Image | Tag | Schedule |
|---|---|---|
| `ghcr.io/projectbluefin/bluefin` | `testing` | Nightly 02:00 UTC + on every new digest |
| `ghcr.io/projectbluefin/bluefin-lts` | `testing` | Nightly 02:30 UTC + on every new digest |
| `ghcr.io/projectbluefin/dakota` | latest | Nightly 03:00 UTC + on every BST build |

**Image-poll trigger:** hourly CronWorkflows check the ghcr.io digest against a stored
ConfigMap state. When the digest changes, a full `bluefin-qa-pipeline` run fires
automatically — no human needed.

**Screenshot pipeline:** `run-gnome-tests` captures desktop PNGs, SCPs them to the
workflow pod, and pushes to `ghcr.io/projectbluefin/testsuite/desktop-screenshot:<slug>-<suite>-latest`
via oras. `publish-to-pages.yml` in projectbluefin/testsuite pulls every 2h to GitHub
Pages. `reusable-release.yml` in projectbluefin/actions reads
`https://projectbluefin.github.io/testsuite/screenshots/<slug>-smoke-latest.png`
and embeds it in the GitHub Release automatically.

See [docs/bluefin-integration.md](docs/bluefin-integration.md) for full details.

---



| Layer | Project | CNCF Status | Role |
|---|---|---|---|
| Kubernetes | [k3s](https://k3s.io) | [Sandbox](https://www.cncf.io/projects/k3s/) | Lightweight single-node cluster |
| VM workloads | [KubeVirt](https://kubevirt.io) | [Incubating](https://www.cncf.io/projects/kubevirt/) | Ephemeral test VMs on bare metal |
| CI/CD | [Argo Workflows](https://argoproj.github.io/argo-workflows/) | [Graduated](https://www.cncf.io/projects/argo/) | DAG pipeline orchestration |
| GitOps | [Argo CD](https://argo-cd.readthedocs.io) | [Graduated](https://www.cncf.io/projects/argo/) | Declarative cluster state from git |
| Observability | [Grafana Loki](https://grafana.com/oss/loki/) | CNCF landscape | Log aggregation for workflow pods |
| Images | [OCI](https://opencontainers.org) + [bootc](https://containers.github.io/bootc/) | Standard | Atomic OS image format |

> All pipelines run on commodity x86_64 hardware (single Ryzen AI node, 64GB RAM).
> The architecture scales horizontally — add worker nodes and the workflows follow.

---

## Architecture

```
Git push / manual submit
        │
        ▼
  Argo Workflow (argo namespace)
        │
        ├─ build-containerdisk ─────► containerDisk in local Zot registry
        │   (bootc install-to-disk)     192.168.1.102:30500/bluefin-containerdisk:<tag>
        │                               digest-checked; skips if already current
        │
        ├─ provision-bluefin-vm ────► KubeVirt VM booting from containerDisk
        │   (VMI + wait for SSH)        ~2 min from submit to SSH-ready
        │
        ├─ run-gnome-tests ─────────► runner pod (Fedora + qecore-headless)
        │   (behave + AT-SPI)           SSH → VM → behave + Dogtail
        │
        └─ teardown (onExit) ───────► delete VM
            (always runs)               guaranteed cleanup on success or failure
```

**GitOps loop:**

```
git push main
    │
    ▼
ArgoCD polls (or webhook)
    │
    ├─ argo/workflow-templates/ ──► WorkflowTemplates reconciled in cluster
    └─ manifests/               ──► CronWorkflows, RBAC, infra reconciled in cluster
```

---

## Repository Layout

```
testing-lab/
├── README.md                         # This file
├── RUNBOOK.md                        # Timeless architecture + failure modes
├── AGENTS.md                         # Agent policy, scope rules, cluster topology
├── Justfile                          # Operator convenience wrappers
│
├── argo/
│   ├── workflow-templates/           # ← ArgoCD (testing-lab App) auto-syncs these
│   │   ├── build-containerdisk.yaml      build containerDisk from bootc image → Zot registry
│   │   ├── bluefin-qa-pipeline.yaml      full pipeline: containerDisk + VM + tests
│   │   ├── bluefin-migration-test.yaml   bootc switch migration validation
│   │   ├── bluefin-service-catalog-pipeline.yaml  service catalog smoke lanes
│   │   ├── provision-bluefin-vm.yaml     boot containerDisk KubeVirt VM
│   │   ├── run-gnome-tests.yaml          behave + qecore + Dogtail GNOME tests
│   │   ├── run-incluster-tests.yaml      in-cluster (kubectl-based) tests
│   │   ├── run-flatcar-tests.yaml        Flatcar OS test runner
│   │   ├── provision-flatcar-vm.yaml     provision Flatcar test VM (hostDisk)
│   │   ├── provision-gnomeos-vm.yaml     provision GNOME OS test VM
│   │   ├── teardown-bluefin-vm.yaml      delete Bluefin containerDisk VM
│   │   ├── teardown-flatcar-vm.yaml      delete Flatcar VM + hostDisk
│   │   ├── teardown-gnomeos-vm.yaml      delete GNOME OS VM
│   │   ├── collect-vm-logs.yaml          gather VM journal logs post-test
│   │   ├── bst-build.yaml               BuildStream (BST) build + zot push
│   │   ├── bst-cache-warm.yaml          warm BST cache on ghost
│   │   ├── dakota-bst.yaml              Dakota BST validate / build pipeline
│   │   ├── dakota-iso-pr-test.yaml      Dakota ISO PR end-to-end pipeline
│   │   ├── dakota-qa-pipeline.yaml      Full Dakota QA: BST → VM → tests
│   │   ├── knuckle-qa-pipeline.yaml     Knuckle installer QA pipeline
│   │   ├── image-poller.yaml            Digest-polling trigger for image-poll CronWorkflows
│   │   ├── pr-poller.yaml               PR label poller for CI gate
│   │   ├── ghost-cleanup.yaml           Clear stale podman lock files on ghost
│   │   ├── ghost-kernel-args.yaml       Set Strix Halo performance kernel args
│   │   ├── ghost-otel-patch.yaml        Patch otelcol-agent.service config on ghost
│   │   ├── homelab-access-probe.yaml    Homelab SSH/auth test probe
│   │   ├── homelab-restore-drill.yaml   Homelab backup restore drill
│   │   ├── homelab-storage.yaml         Homelab storage tests
│   │   └── homelab-substrate.yaml       Homelab substrate (networking, DNS) tests
│   │
│   ├── bootstrap/                    # ← NOT ArgoCD managed — run once to set up cluster
│   │   ├── README.md                     bootstrap guide
│   │   ├── install-kubevirt.yaml         install KubeVirt (CNCF Incubating)
│   │   ├── install-cdi.yaml             install Containerized Data Importer
│   │   ├── install-kubevirt-manager.yaml install KubeVirt Manager web UI
│   │   ├── install-kubestellar.yaml     install KubeStellar (optional, multi-cluster)
│   │   ├── install-test-vms.yaml        apply initial test VM manifests
│   │   └── setup-otel.yaml              deploy OTel observability stack
│   │
│   ├── bluefin-smoke-test.yaml       submit: single-image smoke run
│   ├── bluefin-test-matrix.yaml      submit: parallel testing + lts-testing matrix
│   ├── bluefin-service-catalog-smoke.yaml  submit: service catalog smoke
│   ├── flatcar-smoke-test.yaml       submit: Flatcar smoke run
│   ├── gnomeos-access-spike.yaml     submit: GNOME OS accessibility spike
│   ├── homelab-access-probe.yaml     submit: homelab access probe (no auth)
│   ├── homelab-auth-probe.yaml       submit: homelab access probe (with auth)
│   ├── homelab-restore-drill.yaml    submit: homelab restore drill
│   ├── homelab-storage.yaml          submit: homelab storage tests
│   ├── homelab-substrate.yaml        submit: homelab substrate tests
│   └── one-shot-delete-golden-disks.yaml  emergency: delete all golden disks to reclaim space
│
├── manifests/                        # ← ArgoCD (testing-lab-infra App) auto-syncs these
│   ├── nightly-smoke.yaml                CronWorkflow: nightly latest @ 02:00 UTC
│   ├── nightly-smoke-lts.yaml            CronWorkflow: nightly lts @ 02:30 UTC
│   ├── nightly-dakota.yaml               CronWorkflow: nightly dakota @ 03:00 UTC
│   ├── nightly-knuckle.yaml              CronWorkflow: nightly knuckle @ 03:30 UTC
│   ├── orphan-vm-cleanup.yaml            CronWorkflow: clean orphaned VMs every 2h
│   ├── golden-disk-gc.yaml               CronWorkflow: GC stale golden disks
│   ├── workflow-controller-configmap.yaml TTL patch (7d success, 30d failure)
│   ├── argo-default-sa-rbac.yaml         Argo executor RBAC
│   ├── homelab-runner-rbac.yaml          homelab-runner SA + ClusterRole
│   ├── argo-server-nodeport.yaml         NodePort for external Argo API access
│   ├── flatcar-test-namespace.yaml       Flatcar test namespace
│   ├── promtail-config.yaml              Loki log scraping config
│   ├── rocm-device-plugin.yaml           AMD ROCm GPU device plugin
│   ├── llm-d-gateway-crds.yaml           Gateway API Inference Extension CRDs
│   └── llm-d.yaml                        Qwen3.6-35B-A3B model server on ROCm
│
├── argocd/
│   ├── application.yaml              ArgoCD App: argo/workflow-templates → cluster
│   └── infra-application.yaml        ArgoCD App: manifests/ → cluster
│
├── tests/
│   ├── smoke/features/               Phase 1: GNOME Shell, Activities, top-bar
│   ├── developer/features/           Phase 2: terminal, Homebrew, Podman, micro
│   ├── software/features/            Phase 3: Flatpak, Bazaar, GNOME Software
│   ├── system/features/              Phase 4: bootc contract, atomic OS assertions
│   └── flatcar/features/             Phase 5: Flatcar systemd + container tests
│
└── docs/
    ├── bootstrap.md                  ← how to replicate this lab from scratch
    ├── agent-cheatsheet.md           canonical command reference
    ├── lab-operations.md             long-form operator procedures
    ├── dogtail-testing.md            GUI test authoring + debugging
    ├── homelab-contracts.md          expected cluster behaviour contracts
    └── WORKFLOWS.md                  WorkflowTemplate parameter contracts
```

---

## Test Phases

| Phase | Suite | Trigger |
|---|---|---|
| 1 — Smoke | `smoke` | Every PR, nightly |
| 2 — Developer tooling | `developer` | Nightly, targeted |
| 3 — Software management | `software` | Targeted |
| 4 — Atomic OS contract | `system` | Nightly, every image build |
| 5 — Flatcar substrate | `flatcar` | Dedicated workflow |
| — Migration validation | `migration` | On rechunk → chunkah switches |
| — Dakota BST | `dakota` | Every Dakota PR |

---

## GitOps Model

This repo follows [Argo CD best practices](https://argo-cd.readthedocs.io/en/stable/user-guide/best_practices/)
with two ArgoCD Applications that own distinct resource classes:

| Application | Syncs path | Namespace | prune | selfHeal |
|---|---|---|---|---|
| `testing-lab` | `argo/workflow-templates/` | argo | ✅ | ✅ |
| `testing-lab-infra` | `manifests/` | argo (+ others) | ✅ | ✅ |

**Rules:**
1. Edit files in `argo/workflow-templates/` or `manifests/` → push to `main` → ArgoCD reconciles within ~3 minutes.
2. **Never** `kubectl apply` WorkflowTemplates directly — ArgoCD will overwrite it.
3. **Never** `argo create workflow-template` for production templates — same reason.
4. Bootstrap templates in `argo/bootstrap/` are **not** in any ArgoCD sync path — run them once by hand during cluster setup.

---

## Getting Started

See **[docs/bootstrap.md](docs/bootstrap.md)** for the complete lab setup guide.

**TL;DR for an existing k3s + KubeVirt cluster:**

```bash
git clone https://github.com/castrojo/testing-lab
cd testing-lab

# 1. Bootstrap ArgoCD Applications (once)
just setup-argocd

# 2. Create SSH key secret for VM access (once)
just setup-ssh-secret

# 3. Push — ArgoCD reconciles all WorkflowTemplates automatically
git push origin main

# 4. Run smoke tests
just run-tests
```

---

## Cluster Topology

| Host | Role | Specs |
|---|---|---|
| ghost | k3s control-plane + KubeVirt compute | Ryzen AI MAX+ 395, 16c/32t, 64GB RAM |
| exo-1 | k3s worker (workflow pods only) | — |

**Namespaces:**

| Namespace | Purpose |
|---|---|
| `argo` | Argo Workflows + ArgoCD (control plane) |
| `argocd` | ArgoCD controller |
| `bluefin-test` | `latest` test VMs |
| `bluefin-lts-test` | `lts` test VMs |
| `flatcar-test` | Flatcar test VMs |
| `llm-d` | Qwen3.6-35B-A3B on ROCm (hive swarm node) |
| `mcp` | Kubernetes MCP server |

---

## Key Design Decisions

**btrfs reflink over CDI/PVC** — Golden disk is a single `.raw` file on `hostPath`.
Each test run reflinking it takes ~24ms (copy-on-write, near-zero extra disk). No
CDI DataVolume overhead, no registry round-trips. Teardown is `rm -f disk.raw`.

**No persistent test VMs** — All VMs are ephemeral. Every pipeline provisions a
fresh VM on start and destroys it via `onExit` handler. `just list-vms` should
show zero VMs when no workflows are running.

**API-only operator model** — All cluster reads and mutations go through the
Kubernetes API (MCP tools or `just` wrappers). No SSH to the cluster host for
operations. The only SSH in this system is **in-cluster**: workflow pods SSHing
into freshly-booted test VMs to run behave steps.

**WorkflowTemplate over inline DAG** — All reusable pipeline logic lives in
`WorkflowTemplate` objects in `argo/workflow-templates/`. Submit-time `Workflow`
files in `argo/` reference templates via `workflowTemplateRef` or `templateRef`.
This lets ArgoCD own the template lifecycle while keeping submission flexible.

---

## Writing New Tests

1. Add a `.feature` file under `tests/<suite>/features/`.
2. Add step definitions in `tests/<suite>/features/steps/`.
3. Tag new scenarios `@wip` until stable.
4. Submit a run: `just run-tests` (smoke) or `just run-tests-tag lts`.

See [docs/dogtail-testing.md](docs/dogtail-testing.md) for AT-SPI test authoring.

---

## Documentation Map

| Doc | Purpose |
|---|---|
| [README.md](README.md) | Architecture overview (this file) |
| [docs/bluefin-integration.md](docs/bluefin-integration.md) | Image-poll → test → screenshot → release pipeline |
| [docs/bootstrap.md](docs/bootstrap.md) | How to replicate this lab from scratch |
| [RUNBOOK.md](RUNBOOK.md) | Timeless architecture + failure-mode reference |
| [AGENTS.md](AGENTS.md) | Agent policy, cluster topology, issue filing rules |
| [docs/agent-cheatsheet.md](docs/agent-cheatsheet.md) | Canonical command reference |
| [docs/lab-operations.md](docs/lab-operations.md) | Long-form operator procedures |
| [docs/dogtail-testing.md](docs/dogtail-testing.md) | GUI test authoring + debugging |
| [docs/WORKFLOWS.md](docs/WORKFLOWS.md) | WorkflowTemplate parameter contracts |

---

## Related Projects

- [Project Bluefin](https://projectbluefin.io) — primary subject under test; this lab validates every image publish
- [ublue-os/bluefin](https://github.com/ublue-os/bluefin) — upstream Bluefin image builds
- [Project Dakota](https://github.com/projectbluefin/dakota) — BST-built Bluefin variant; Dakota PRs trigger `dakota-qa-pipeline`
- [projectbluefin/testsuite](https://github.com/projectbluefin/testsuite) — screenshot hosting + GitHub Pages publishing
- [projectbluefin/actions](https://github.com/projectbluefin/actions) — `reusable-release.yml` embeds lab screenshots in GitHub Releases
- [bootc](https://containers.github.io/bootc/) — image-based Linux standard
- [KubeVirt](https://kubevirt.io) — CNCF Incubating, VM workloads on Kubernetes
- [Argo Workflows](https://argoproj.github.io/argo-workflows/) — CNCF Graduated
- [Argo CD](https://argo-cd.readthedocs.io) — CNCF Graduated
- [k3s](https://k3s.io) — CNCF Sandbox
