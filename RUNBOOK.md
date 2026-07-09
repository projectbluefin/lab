# Bluefin QA Pipeline — Runbook

> Timeless architecture and failure-mode reference. For commands see [docs/agent-cheatsheet.md](docs/agent-cheatsheet.md). For long-form operator procedures see [docs/lab-operations.md](docs/lab-operations.md).

## Architecture summary

```
Git push / manual submit
        │
        ▼
Argo Workflow (argo namespace)
        │
        ├─ build-containerdisk    ─► containerDisk in local Zot registry (digest-checked)
        ├─ provision-bluefin-vm   ─► boot KubeVirt VM from containerDisk
        ├─ run-gnome-tests        ─► runner pod SSHes VM, executes qecore + behave
        └─ teardown (onExit)      ─► delete VM (always runs, success or failure)
```

All pipelines use ephemeral VMs — every run provisions a fresh VM and tears it down on exit. There are no persistent test VMs.

## Cluster topology

| Host | Role | IP | Notes |
|---|---|---|---|
| ghost | k3s control-plane + KubeVirt compute | 192.168.1.102 | Runs VM workloads and Argo control-plane services |
| exo-1 | k3s worker | 192.168.1.239 | Workflow pods only |
| Argo UI | external entrypoint | http://192.168.1.102:32746 | Host-local service also exposed on port 2746 |
| Loki | log aggregation | http://192.168.1.102:30100 | Captures workflow pod logs |
| ArgoCD | GitOps controller | https://192.168.1.102 | Reconciles this repo into the cluster |

ArgoCD intentionally scales `argocd-applicationset-controller`, `argocd-dex-server`, and
`argocd-notifications-controller` to zero in this homelab. K8sGPT may flag those Services as
no-endpoint findings; that is expected, not drift.

HostDisk VMs (Flatcar, Knuckle, GnomeOS) must pin to ghost — their disk files live on ghost's local storage.
ContainerDisk VMs (Bluefin test VMs) float freely and can schedule on any KubeVirt-capable node.

## GitOps ownership

| Area | Source of truth | Reconciler |
|---|---|---|
| WorkflowTemplates | `argo/workflow-templates/*.yaml` | ArgoCD application `lab` |
| Cluster infra and CronWorkflows | `manifests/*.yaml` | ArgoCD application `lab-infra` |
| Operator entrypoints | `Justfile` | Local operator / MCP tooling |

The repo is intentionally GitOps-first: cluster state should converge from git, not from manual template applies or node SSH.

## Operator access model

- Use Kubernetes MCP and Argo MCP for workstation-side cluster reads and mutations.
- Prefer the `just` entrypoints when they exist; they are the human-facing wrappers around the same API-driven workflow.
- Do not SSH from a workstation into `ghost` or `exo-1` for inspection, recovery, or file transfer.
- In-workflow SSH into test VMs remain valid because they originate inside the cluster and are part of the test harness, not node administration.

## Image, disk, and VM model

| Object | Backing location | Used by | Notes |
|---|---|---|---|
| ContainerDisk (`testing`) | `192.168.1.102:30500/bluefin-containerdisk:testing` | Bluefin QA pipeline | Built by `build-containerdisk` |
| ContainerDisk (`lts-testing`) | `192.168.1.102:30500/bluefin-containerdisk:lts-testing` | Bluefin QA pipeline | Built by `build-containerdisk` |
| Flatcar hostDisk | `/var/mnt/ghost-data/flatcar-test/<vm-name>/disk.raw` | Flatcar pipeline | Reflinked from golden, removed by teardown |
| Knuckle hostDisk | `/var/mnt/ghost-data/knuckle-test/<vm-name>/disk.raw` | Knuckle pipeline | Reflinked from golden, removed by teardown |

The SSH secret lives in the `bluefin-test-ssh-key` Kubernetes secret in namespace `argo`.
Golden disks can be rotated via the `build-containerdisk` template.

## Test execution stack

| Component | Responsibility |
|---|---|
| `git-sync` initContainer | Clone the requested repo ref into the runner pod |
| `run-gnome-tests` | Copy suites to the VM and orchestrate execution |
| `qecore-headless` | Start the Wayland GNOME session inside the VM |
| `dogtail` | Traverse and interact with the AT-SPI tree |
| `gnome-ponytail-daemon` | Translate AT-SPI coordinates into Wayland input |
| `Shell.Eval` | Handle GNOME Shell 50 top-bar interactions that AT-SPI cannot drive reliably |
| Loki | Preserve logs and emitted test artifacts after pod cleanup |

## GNOME Shell 50 constraints

- Clock, quick-settings, and calendar interactions are not reliably actionable through AT-SPI alone.
- `global.context.unsafe_mode = true` must be enabled before top-bar interaction.
- `findChild(..., requireResult=...)` is not a supported dogtail pattern in this repo's stack.
- `findChildren(...)` and `findChild(..., retry=False)` are the canonical presence-check APIs.

## Common failure modes

| Symptom | Root cause | Durable fix |
|---|---|---|
| `Permission denied (publickey)` during SSH wait | ContainerDisk or hostDisk contains an old public key | Rebuild the containerDisk via `build-containerdisk` |
| `wait-for-vm` exits 1 with `Error from server (Forbidden)` | argo SA has no kubevirt-manager Role in the VM namespace | Add Role + RoleBinding to `manifests/kubevirt-rbac.yaml` for the new namespace |
| `AccessCredentialsSynchronized` never becomes True; `wait-for-vm` times out | `qemu-guest-agent.service` not enabled in VM image | `build-containerdisk` symlinks it; check post-install step was preserved |
| `force=true` rebuild workflow stalls with only 2 nodes (DAG + Skipped check) | Downstream `when` references a Skipped task's outputs (resolves to empty string); Argo v4 does not schedule the task | Let `check` always run; handle `force=true` bypass inside the script body (see `argo-workflows.md` §18) |
| dakota builds accumulate, hold `ghost-heavy-compute` mutex, starve other rebuilds | `image-poll-dakota` CronWorkflow not suspended; dakota pipeline permanently blocked (composefs, no UKI) | `image-poll-dakota` has `spec.suspend: true` in git; if builds appear, stop them immediately |
| Cross-node SSH from workflow pods to VM fails (VM and workflow pod on different nodes) | firewalld on node blocks flannel/pod-to-pod traffic | `k3s-firewalld-config` DaemonSet disables firewalld on all nodes; if re-enabled, rollout restart the DaemonSet |
| Workflow hangs before GUI steps start | VM boot or SSH readiness never completed | Inspect VMI readiness and runner logs, then re-run the appropriate recovery path |
| K8sGPT reports no-endpoint Services for `argocd-applicationset-controller`, `argocd-dex-server`, `argocd-notifications-controller-metrics`, or `virt-exportproxy` | These are documented control-plane exceptions in this cluster shape | Ignore those specific findings; they are intentional |
| `TypeError` involving `requireResult` | Stale dogtail step pattern | Replace with `findChildren(...)` or `findChild(..., retry=False)` |
| Clock / quick-settings scenarios miss their targets | GNOME Shell AT-SPI geometry gap | Drive the interaction via `Shell.Eval` |
| `outputs.result` contains debug text | Script template wrote extra stdout | Send debug output to stderr and reserve stdout for the actual result |
| VM stuck `Terminating` | KubeVirt controller race with launcher cleanup | Delete the `virt-launcher-*` pod and let reconciliation finish |
| `run-gnome-tests` pod fails at startup | Workflow template structure error, often misplaced `volumes:` | Fix the template in git and let ArgoCD reconcile it |
| WorkflowTemplate change appears ignored | Workflow was submitted before the new template was reconciled | Verify ArgoCD revision, wait or sync, then submit a new workflow |
| Overview image status shows `—`/stale values despite recent stable/testing publishes | Collector used release-only timestamps while some lanes publish via GHCR tags (`stable`/`testing`) without matching release metadata | In `scripts/refresh_factory_stats.py`, source lane freshness from GHCR package tag timestamps first (`orgs/projectbluefin/packages/container/<image>/versions`), then fallback to releases; regenerate page datasets afterward |
| `flatcar-kernel-build` fails after hours with `Pod was active on the node longer than the specified deadline` | Workflow/template `activeDeadlineSeconds` too short for a full Flatcar SDK kernel+image compile | Use a 6h workflow deadline for the pipeline and avoid tighter per-step deadline caps; if still blocked, use bare-metal fallback in `docs/skills/flatcar-node-onboarding.md` |
| `flatcar-kernel-build` sits at `Preflight SDK pull` with an active `docker pull --quiet` in the VM | Docker daemon is still coming up or the mirror pull is just slow; the SDK layers are large | Keep the SDK data-root on the PVC, watch `/var/tmp/build/docker` grow, and use the cache-first timeout + upstream fallback pattern rather than killing the run immediately |
| Flatcar runner: `pip3: command not found` | Fedora minimal lacks standalone `pip3` | Use `python3 -m pip install` in runner pods |
| Flatcar runner: exit code 64 | Template has `outputs.artifacts` but Argo artifact storage is not configured | Remove artifact `outputs:` from the template |
| Flatcar test: `ctr version` fails as `core` | containerd socket requires root | Use `sudo ctr version` (core has passwordless sudo) |
| Pods on worker nodes get `no route to host` or `connection refused` to ClusterIP / control-plane (`10.43.0.1`) | Flannel `--flannel-iface=thunderbolt0` is configured but physical USB4 link is `none` (unestablished), causing routing isolation | Ensure physical USB4 cable is in Slot 1/4 on both AMD Framework nodes, and reboots or physical power cycles restore physical link. |
| `ucsi_acpi GET_CABLE_PROPERTY failed (-5)` or `spurious native interrupt!` kernel spam on PCIe bridge `0000:00:08.3` | PCIe runtime power management (ASPM) or a volatile EC state suspends the USB4 controllers under the bridge | Force power control of bridge `0000:00:08.3` and controllers `c5:00.5`/`c5:00.6` to `on` via sysfs on both nodes. |
| `exo-0` has no `thunderbolt0` interface at all (`ip link show thunderbolt0` → `Device does not exist`), `/sys/bus/thunderbolt/devices/` shows only local host routers `0-0`/`1-0` (both `authorized=1`) with no downstream peer entry (e.g. no `0-1`) | **EC/PD-level failure, below the OS — confirmed 2026-07-08.** Cable is good and seated in the rear USB4 ports on both machines (all rear Type-C on Framework Desktops are USB4). Both nodes: `thunderbolt`+`thunderbolt_net` loaded, controllers `c5:00.5`/`c5:00.6` runtime PM `on`/active, two `usb4_portN` entries registered, firmware current (BIOS 3.05) — yet `/sys/class/typec/port*` shows **zero partner/attach events ever**, including during a live cable reseat, and warm reboots of both nodes do not help. The embedded PD controller never sees the cable; warm reboots do not reset EC/PD state. | Full power drain required: shut down both nodes, unplug AC for ~30s, replug, boot (known Framework Desktop EC recovery). **Confirmed working 2026-07-09**: after power drain, link came up (`usb4_portN/link` = `usb4`, peer `1-2` enumerated, `thunderbolt0` present). Deployed IPs are `10.99.0.1/30` (ghost) / `10.99.0.2/30` (exo-0), persisted in NetworkManager profiles along with table-40 policy routing rules — see `docs/ghost-lab-architecture.md` for the exact `nmcli` commands. If the link is down the cluster still runs in ethernet mode (see buildbarn worker readCaching CAS). |
| Host daemons (`sshd`, `systemd-journald`, `systemd-logind`, `systemd-oomd`) die simultaneously on a node; journal shows `systemd-journald: Received SIGTERM from PID <n> (argoexec)` | Workflow steps that set `podSpecPatch: '{"hostPID":true,...}'` (e.g. `build-containerdisk` `install-to-disk`, required by bootc) share the host PID namespace; when Argo terminates the pod (deadline, retry, delete), argoexec signals processes it can see — which is every host process — killing sshd and friends on whichever node ran the step (confirmed 2026-07-08 on both ghost and exo-0) | Immediate recovery: privileged rescue pod (`hostPID`, `runAsUser: 0`, nsenter to PID 1) → `systemctl restart sshd`. Mitigation applied live on both nodes: `/etc/systemd/system/sshd.service.d/50-restart.conf` with `Restart=always`. Root fix tracked in projectbluefin/lab issues: constrain argoexec signal scope or drop hostPID from the bootc install path. |
| `ghost` SSH (port 22) and k3s API (6443) intermittently refuse connections; NodePort services (zot cache) time out; `kubectl describe node ghost` shows ~99% memory allocated | Two overlapping `image-poll-lts-stable` runs both failed but their `onExit` VM teardown couldn't execute (cluster was already too resource-starved to schedule the teardown step), leaving 10 orphaned 8Gi `virt-launcher` VMs (~80Gi requested) running indefinitely; combined with 774 un-garbage-collected `Failed`/`Succeeded` Argo pods bloating etcd/API server object count and starving control-plane CPU (crashing/starving `sshd` on the same host) — confirmed 2026-07-08 | Delete orphaned `vm`/`vmi` objects whose parent workflow already shows `Failed`/`Error` (`argo list -n argo --status Failed,Error`, then `kubectl delete vm --all -n <test-namespace>`), and bulk-delete stale terminal pods (`kubectl delete pods -n argo --field-selector=status.phase=Failed` / `=Succeeded`). Root fix still open: `ghost` has no `system-reserved`/`kube-reserved` kubelet memory carve-out, so pod requests can legitimately reach ~100% of allocatable with zero headroom for host daemons like `sshd`; and VM teardown-on-exit has no resilience against running during a resource-starved cluster. See `production-hardening` backlog. |

## Durable recovery patterns

The following recovery patterns have been validated in live cluster work and are worth
keeping as durable operator guidance:

- Queueing and deduplication: use template-level semaphores for VM-heavy and
  build-heavy templates; workflow-level mutexes do not protect
  `workflowTemplateRef` / `templateRef` callers.
- Cluster overload: delete stale terminal workflows and orphaned VMs before
  resubmitting a build; leaving the old noise in place causes the next run to
  compete for the same memory budget.
- Buildbarn backend recovery: if storage pods stay `Pending` after a StatefulSet or
  PVC change, verify the PVC bindings and the storage pods before re-running the
  build. Treat the storage pods as the live signal, not the old workflow status.
- ArgoCD access: if the local port-forward drops, restart it and verify the health
  endpoint before forcing a sync or submitting a workflow against the updated
  template.
- USB4/PD failure mode: if the USB4 link never establishes below the OS and a
  cable reseat / warm reboot do not help, treat it as an EC/PD issue and continue
  in Ethernet mode with local read caches rather than chasing Linux networking
  settings.

## Historical notes

Keep this file timeless: architecture, topology, and durable failure modes only.
