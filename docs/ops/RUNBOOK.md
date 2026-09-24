# Lab Runbook

> Timeless architecture and failure-mode reference. For commands see [docs/reference/agent-cheatsheet.md](../reference/agent-cheatsheet.md). For long-form operator procedures see [docs/ops/lab-operations.md](lab-operations.md).

## Architecture summary

```
Git push / image digest change / manual submit
        │
        ▼
Argo Workflow (argo namespace)
        │
        ├─ dakota-qa-pipeline   ─► run-container-tests inside the published OCI image
        └─ explicit VM lanes    ─► provision, run, and tear down ephemeral KubeVirt VMs
```

Two steady-state execution paths exist:

| Path | Purpose | Persistent state |
|---|---|---|
| Container-only Dakota path | Image and PR QA | No persistent VM or host-disk state |
| Explicit VM-backed lanes | Dakota and bluefin-server boot tests | Ephemeral KubeVirt resources |

### Container-only QA contract

`dakota-qa-pipeline` fans out
`run-container-tests` inside the published OCI image. The runner clones
`projectbluefin/testsuite`, starts the nested systemd/Wayland session, and
attempts to publish `results.json` when `GITHUB_TOKEN` is available. A
publication warning does not change the suite exit status.

The Dakota digest poller runs at minute `:08` of each ten-minute interval for
freshness tracking, but its QA trigger is disabled (`run-qa: "false"`). Daily
suite coverage comes from `nightly-dakota` at 03:00 UTC.

## Cluster topology

| Host | Role | IP | Notes |
|---|---|---|---|
| ghost | k3s control-plane + KubeVirt compute | `<ghost-ip>` | Runs VM workloads and Argo control-plane services |
| exo-0 | k3s worker | `<exo-0-ip>` | Build and workflow pods |
| Argo UI | external entrypoint | `http://<ghost-ip>:32746` | Host-local service also exposed on port 2746 |
| ArgoCD | GitOps controller | `https://<ghost-ip>` | Reconciles this repo into the cluster |

Container QA runs on the control-plane graphical seat on ghost. Other workflow
pods may land on exo-0 according to template constraints.

## Local-path storage paths and stale PV cleanup

`manifests/local-path-config.yaml` is the source of truth for the bundled Rancher
Local Path Provisioner. The current node-local data mounts are:

| Node | Provisioner path |
|---|---|
| `ghost` | `/var/mnt/ghost-data/local-path` |
| `exo-0` | `/var/mnt/exo0-data/local-path` |

These names identify different physical mounts. Never apply `ghost`'s path to
`exo-0`, and do not add `DEFAULT_PATH_FOR_NON_LISTED_NODES`: an unconfigured
node must fail closed rather than provision PVCs on its root disk. Local-path
stores the selected path in each PV's `spec.local.path`; changing the ConfigMap
does not migrate or rewrite existing PVs.

In the August 2026 helper-pod incident, a privileged hostPath probe found
`/var/mnt/ghost-data` was a real directory on `ghost`, but a dangling symlink on
`exo-0` pointing to `/var/mnt/exo0-data/ghost-data`; the valid exo-0 storage
directory was `/var/mnt/exo0-data/local-path`. That mismatch is why kubelet
reported `mkdir ... file exists` before the teardown container could start.

### Helper-pod delete loop

When `helper-pod-delete-pvc-*` pods churn in `CreateContainerError` with
`failed to mkdir "/var/mnt/ghost-data/local-path/": mkdir /var/mnt/ghost-data:
file exists`, inspect the helper pod's node, its hostPath, and the PV's stored
path:

```bash
kubectl -n kube-system describe pod <helper-pod>
kubectl -n kube-system get cm local-path-config -o yaml
kubectl get pv <pv-name> -o yaml
kubectl get pvc -A | grep -i terminating
```

Do not SSH to a node. To inspect the host path, run a short-lived privileged
diagnostic pod with `/var/mnt` mounted read-only and pinned to the affected
node. A dangling symlink or other non-directory at the parent mount produces
the same kubelet `mkdir ... file exists` error.

After the node map is corrected in Git and reconciled, clean up only PVs that
are all of the following:

1. `Released`, with `reclaimPolicy: Delete`;
2. storing the incorrect node path and pinned to the affected node;
3. no longer referenced by any PVC or active workload; and
4. verified to contain no data that must be retained.

Delete the explicitly identified stale PVs, then remove any remaining helper
pods for those PVs. Never mass-delete `Bound` PVs or PVCs merely to stop the
churn. Confirm that new helper pods are absent and that newly provisioned
PV paths match the node map.

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
- Do not SSH from a workstation into `ghost` or `exo-0` for inspection, recovery, or file transfer.
- In-workflow access to explicit test VMs remains valid because it originates
  inside the cluster and is part of the test harness, not node administration.
- Host storage migration, node bootstrap, and host-service recovery are private
  maintainer procedures; they are not normal public workstation guidance.

## Image, disk, and VM model

| Object | Backing location | Used by | Notes |
|---|---|---|---|
| Published OCI image | Registry image and digest | Dakota container-only QA | `dakota-qa-pipeline` runs `run-container-tests`; no VM or disk is created |
| Boot-test VM | Ephemeral KubeVirt VM | `bluefin-server-boot-test` | Torn down after the run |

## Test execution stack

| Component | Responsibility |
|---|---|
| `run-container-tests` | Run Dakota GUI and contract suites inside the target OCI image |
| `git-sync` initContainer | Clone the requested repo ref into the runner pod |
| `qecore-headless` | Start the Wayland GNOME session inside the VM |
| `dogtail` | Traverse and interact with the AT-SPI tree |
| `gnome-ponytail-daemon` | Translate AT-SPI coordinates into Wayland input |
| `Shell.Eval` | Handle GNOME Shell 50 top-bar interactions that AT-SPI cannot drive reliably |

## GNOME Shell 50 constraints

- Clock, quick-settings, and calendar interactions are not reliably actionable through AT-SPI alone.
- `global.context.unsafe_mode = true` must be enabled before top-bar interaction.
- `findChild(..., requireResult=...)` is not a supported dogtail pattern in this repo's stack.
- `findChildren(...)` and `findChild(..., retry=False)` are the canonical presence-check APIs.

## Common failure modes

| Symptom | Root cause | Durable fix |
|---|---|---|
| `TypeError` involving `requireResult` | Stale dogtail step pattern | Replace with `findChildren(...)` or `findChild(..., retry=False)` |
| Clock / quick-settings scenarios miss their targets | GNOME Shell AT-SPI geometry gap | Drive the interaction via `Shell.Eval` |
| `outputs.result` contains debug text | Script template wrote extra stdout | Send debug output to stderr and reserve stdout for the actual result |
| VM stuck `Terminating` | KubeVirt controller race with launcher cleanup | Delete the `virt-launcher-*` pod and let reconciliation finish |
| Workflow pod fails at startup | Workflow template structure error, often misplaced `volumes:` | Fix the template in git and let ArgoCD reconcile it |
| WorkflowTemplate change appears ignored | Workflow was submitted before the new template was reconciled | Verify ArgoCD revision, wait or sync, then submit a new workflow |
| Container QA exits 1 before `useradd` | NSS-only `video`, `render`, or `input` groups can make `groupadd` succeed without adding `/etc/group` entries | Re-check `/etc/group` after `groupadd`; materialize the NSS entry or a fallback GID before calling `useradd -G`, then reconcile `run-container-tests` |
| Container QA exits 1 at `Failed to create required local group render` (video passes, render fails) | The nested provisioning script ran under `podman exec ... bash -c '...'`. The first apostrophe inside the block (from `printf '%s\n'`) closed the outer quote, so bash actually received `printf %sn` — no trailing newline. `video` was appended without a newline and `render` was spliced onto the same line, so `grep "^render:"` failed | Feed nested scripts over stdin (`podman exec -i ... bash -s <<'MARKER'`) instead of `bash -c '...'`, and guard appends to `/etc/group` with a trailing-newline check. Never reintroduce `bash -c '...'` for multi-line nested scripts |
| Container QA fails right after "Installing test-only Python dependencies" with a bare `exit status 1` | `pip install --quiet` swallowed the real error (PEP 668 externally-managed interpreter, missing pip, or a transient index failure) | The install now retries 3×, adds `--break-system-packages` when supported, and dumps the full pip log plus `python3`/`pip` versions on failure. Read the surfaced log rather than re-guessing |
| Container QA fails in `wait_for_shell.py` with `Shell.Eval not ready: ... Could not connect: No such file or directory` (error class `bus-unavailable`) until the readiness budget expires | **RESOLVED (closes the #611 open item).** ghost has one physical GPU and mutter's native backend takes *exclusive* DRM master on `/dev/dri/card*`. `ghost-container-qa` allows 6 concurrent lanes, so only the first nested GNOME session works; every other lane logs `Failed to open gpu '/dev/dri/card1': GDBus.Error:System.Error.EBUSY: Device or resource busy`, stalls ~50s in `Failed to make thread 'KMS thread' high priority scheduled`, and GDM loops `GdmDisplay: Session never registered, failing` forever. When `qecore-headless` then stops GDM and SIGTERMs the session, logind destroys `/run/user/1000` with the session bus inside it, and no replacement session ever registers to recreate it — hence a permanently absent socket, not a client-side race. GDM itself is healthy and `/etc/gdm/custom.conf` keeps `AutomaticLogin=bluefin-test` across the restart; it churns greeter sessions because every *display* dies, ending at `GdmLocalDisplayFactory: maximum number of display failures reached. Giving up.` An isolated A/B on ghost proved the headless drop-in is necessary and sufficient, and that `enable-linger` alone is not: a linger-only lane keeps the bus and the uid-1000 manager alive (`Linger=yes`) yet still fails, with the error class merely shifting from `bus-unavailable=137` to `service-unknown=151` | `run-container-tests` now (a) drops `/etc/systemd/user/org.gnome.Shell@.service.d/10-headless.conf` forcing `gnome-shell --headless --virtual-monitor 1920x1080` for the greeter and the autologin session, so no lane claims DRM master, and (b) as hardening runs `loginctl enable-linger bluefin-test` before starting GDM so `user@1000.service` and `/run/user/1000/bus` survive the `qecore-headless` GDM restart. **Operating rule: nothing in a nested QA target may take DRM master — the host GPU is exclusive and is not a per-lane resource.** Never respond to this symptom by raising the `wait_for_shell.py` timeout; the socket is absent indefinitely, not late |
| Container QA gets past the GNOME readiness gate, runs real checks, then reports `0 scenarios passed, 0 failed, N skipped` plus `HOOK-ERROR in after_all: AssertionError: No scenario matched tags`, with `qecore-headless startup failed: unrecoverable headless errors` and `headless: Issue was detected and might need attention` carrying `("stat: cannot statx '/run/user/1000/dconf/user': No such file or directory\n", 1, CalledProcessError(...))` | `/run/user/<uid>/dconf/user` is dconf's one-byte shm invalidation flag, not a persistent file. `dconf_shm_flag()` unlinks it on **every** write to the user database and `dconf_shm_open()` recreates it on the next read, so under a live session the path blinks in and out of existence. `qecore-headless`'s `verify_file_ownership()` tests `os.path.isfile()` and then runs a *separate* `sudo stat -c '%U %G' <path>`; spawning sudo is a tens-of-milliseconds window, so an ordinary dconf write between the two lookups makes stat exit 1 and qecore raise a non-recoverable failure, after which `before_scenario` skips every scenario. Reproduced on a live lane at ~10% (103 ENOENT hits per 1019 isfile-true iterations) with concurrent dconf read/write traffic. This is unrelated to `enable-linger`: qecore already returns early when the file is simply absent, so only the *disagreement* between the two lookups is fatal | Provisioning appends `|| true` to that one stat command in the installed `qecore-headless`, asserts the edit applied and that the script still parses, and fails the step otherwise. A vanished file then yields rc 0 with no `root` in the output, so qecore correctly does nothing, while a genuinely root-owned file still stats cleanly and is still removed. **Operating rule: never treat `/run/user/<uid>/dconf/user` as stable, and never respond to this symptom by sleeping, polling, or waiting for dconf to settle — there is no quiescent state while a session is running.** |
| Container QA scenarios fail with `RuntimeError: User 'bluefin-test' does not have write permissions for '/dev/uinput'` even though the user is in `input` | Podman gives the nested target its own tmpfs `/dev` — a different device and inode from both the pod's and the node's — and materializes `uinput` there as mode `0600 root:root` with **no group**. Group membership can never grant access to a node that has no group bit set | The nested provisioning now runs `chgrp input /dev/uinput && chmod 0660 /dev/uinput` after creating the test user. The node is lane-local, so this cannot affect concurrent lanes or ghost itself. Verify with `podman exec bluefin-qa-target ls -l /dev/uinput` |
| Every container QA scenario logs `ModuleNotFoundError: No module named 'pkg_resources'` from `qecore/sandbox.py` | qecore's `_attach_version_status_to_report()` imports `pkg_resources`, which ships only with setuptools, was dropped in setuptools 81, and is not seeded into fresh Python 3.12+ environments. `@non_critical_execution` catches it, so scenarios still run — this is lost version reporting and log noise, not a failure cause | The nested provisioning installs `setuptools<81` alongside `qecore dogtail behave`. Do not read a drop in this count as a drop in scenario failures; the two are independent |
| Container QA scenarios fail with `Cannot reach VM at 127.0.0.1 over SSH after 5 attempts: rc=255` | The scenario drives a device under test over SSH, but a container lane runs behave *inside* the target and has no sshd. This is a suite-selection defect, not a lane defect | Fix in `projectbluefin/testsuite`: tag the scenario `@vm_only`, which the suite hooks skip when `/run/.containerenv` is present. Never add an sshd to the nested target to make these pass |
| `lab-infra` sync wedged "waiting for healthy state of DaemonSet/..." | A DaemonSet pod is unhealthy on some node (e.g. hostPath missing on that host), blocking every subsequent manifests/ change | Fix or scope the DaemonSet (capability-label nodeSelector), then terminate the stuck operation so ArgoCD retries: `kubectl patch application lab-infra -n argocd --type=merge -p '{"status":{"operationState":{"phase":"Terminating"}}}'` |
| KubeStellar app sync stuck at kubeflex-controller-manager | Postgres hook deadlock under ArgoCD | Keep `installPostgreSQL: false` + separate `kubestellar-postgres` app; see `docs/skills/kubestellar/SKILL.md` |
| `kubeflex-controller-manager` consumes sustained host RX and repeats ControlPlane reconciliation about once per second | KubeFlex v0.9.1's status writes race across infrastructure, PostCreateHook, and final-readiness phases. The same controller unconditionally creates a `wds1` Ingress with `ingressClassName: nginx` even though the lab has no external WEC endpoint; the Ingress is outside PostCreateHook templates, has no address, and [upstream Kubernetes freezes the Ingress API in favor of Gateway](https://kubernetes.io/docs/concepts/services-networking/ingress-controllers/). The [Ingress NGINX retirement statement](https://kubernetes.io/blog/2026/01/29/ingress-nginx-statement/) says the controller receives no further fixes or security patches | Upgrade `argocd/kubestellar-app.yaml` to core-chart 0.30.0 (KubeFlex v0.9.3), reconcile through ArgoCD, and verify the controller logs. Do not install ingress-nginx, add an IngressClass, or introduce a Gateway controller solely to claim this unused artifact. If external reachability is needed later, use the [Gateway API getting-started path](https://gateway-api.sigs.k8s.io/guides/getting-started/). If the status-conflict loop survives v0.9.3, escalate it as an upstream KubeFlex controller defect; deleting the generated Ingress alone will not persist |
| `test-lane` nodes sit `Pending` for tens of minutes with no pods created, and `kubectl get wf <name> -o json \| jq .status.synchronization` shows `waiting` on `ghost-container-qa` | A single QA pipeline held most of the 6 semaphore slots. `spec.parallelism` is not inherited through `templateRef`, so poller-dispatched runs fanned out every `withItems` lane at once | The `pipeline` templates now carry template-level `parallelism: 2`, which survives `templateRef`. Never fix this by raising `ghost-container-qa`; that only moves the threshold. Verify with `pytest tests/unit/test_semaphore_topology.py` and see [patterns: semaphore topology](../skills/argo-workflows/patterns.md#semaphore-topology-hold-the-key-at-one-level-cap-every-fan-out) |
| Workflow pod rejected `failed quota: argo-quota` | Template missing resources requests/limits | Add explicit cpu+memory requests and limits to every container/script |

### KubeStellar / Console failure modes

See `docs/skills/kubestellar/SKILL.md` (downsync, WEC join, RBAC) and
`docs/skills/console-dashboard/SKILL.md` (Console recovery, exposure policy).
Upgrade order: KubeFlex/postgres -> core-chart -> Console; rerun
`kubestellar-smoke-test` after every core upgrade.

## Historical notes

Date-stamped iteration lessons were removed in the ponytail audit (commit
81f0cc6f); recover them from git history if needed.
Keep this file timeless: architecture, topology, and durable failure modes only.
