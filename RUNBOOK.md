# Bluefin QA Pipeline — Runbook

> For future agents and human operators. Last updated: 2026-05-26.
>
> **For paint-by-numbers operations** (run a test, triage a failure, rotate SSH, recover
> a titan, pause a nightly, query Loki, diagnose ArgoCD, safely delete VMs), use the
> step-by-step operator manual: [`docs/lab-operations.md`](docs/lab-operations.md).
> This runbook is the architectural reference and failure-mode index it builds on.

## Test Suite Mantra

This runbook assumes the suite's primary mission is to verify **Bluefin's immutable, image-based
contract**.

Operationally, that means:

- prioritize checks around `bootc`, staged deployments, rollback behavior, composefs/fs-verity,
  signature policy, `uupd`, and read-only host-state expectations
- treat Homebrew, Flatpak, rootless Podman, and Docker/Colima as **user-space layers** that must
  integrate cleanly without turning the host back into a mutable package-managed system
- use GNOME/UI tests to confirm real workflows on top of that model, not as a substitute for the
  underlying platform assertions
- prefer new workflows, suites, and issues that strengthen this image-based contract when tradeoffs
  are required

## GitOps Policy — Read Before Doing Anything

**No SSH to nodes.** Never SSH to ghost (192.168.1.102) or exo-1 (192.168.1.239).
All node-level operations run as privileged Argo Workflow pods. If you think you need SSH,
you need a WorkflowTemplate instead.

**No `kubectl apply` for WorkflowTemplates.** Edit the YAML in `argo/workflow-templates/`,
push to `main`, and let ArgoCD reconcile it. Use [`docs/lab-operations.md`](docs/lab-operations.md)
for the exact sync decision tree and commands.

**MCP tool usage for agents:**
| Operation | Use |
|---|---|
| Submit a workflow | Argo MCP `submit_workflow` or `just <target>` |
| Watch workflow status | Argo MCP `get_workflow` / `list_workflows` |
| Get workflow logs | Argo MCP `logs_workflow` or `just logs` |
| Update a WorkflowTemplate | Edit YAML → git push → ArgoCD syncs |
| Read cluster state | kubectl MCP or `just list-vms` / `just list-workflows` |
| Patch a golden disk | `just patch-disk [tag]` (submits `patch-golden-disk` workflow) |

**Never use `argo-mcp-create_workflow_template` for template updates** — ArgoCD owns
that reconciliation loop. MCP template creation bypasses git history and will be
overwritten on the next ArgoCD sync.

## Cluster Topology

| Host | Role | IP | Specs |
|---|---|---|---|
| ghost | control-plane + primary compute | 192.168.1.102 | AMD Ryzen AI MAX+ 395, 16c/32t, 64GB RAM |
| exo-1 | worker node | 192.168.1.239 | — |
| Argo UI | — | http://192.168.1.102:32746 | NodePort for LAN/MCP access; host-local service also listens on `:2746` |
| Loki | log aggregation | http://192.168.1.102:30100 | Scrapes pods with label `app.kubernetes.io/part-of=bluefin-test-suite` |

KubeVirt VMs are pinned to ghost (16-core host; exo-1 is for workflow pods only).

## Valid Bluefin Image Variants

| Tag | Image | Notes |
|---|---|---|
| `latest` | `ghcr.io/ublue-os/bluefin:latest` | Bleeding edge, tested every PR |
| `lts` | `ghcr.io/ublue-os/bluefin:lts` | Long-term support |

**`gts` does NOT exist. `lts-hwe` does NOT exist.** Do not use these tags.

## Golden Disk Location

Golden disks live on ghost at `/var/tmp/bluefin-golden/<variant>/disk.raw`.

| Variant | Path | Provisioning behavior |
|---|---|---|
| `latest` | `/var/tmp/bluefin-golden/latest/disk.raw` | Created/refreshed by `bib-build-and-push` when missing or stale |
| `lts` | `/var/tmp/bluefin-golden/lts/disk.raw` | Same flow; first `just ensure-disk lts` or nightly LTS run builds it if absent |

### Building and Patching Golden Disks

`bib-disk-configure` reads the SSH pubkey from the `bluefin-test-ssh-key` secret automatically
via `secretKeyRef` — no pubkey parameter needed, no SSH to node required.

This submits the `patch-golden-disk` WorkflowTemplate which runs as a privileged pod on ghost.
**Do not SSH to ghost to run the patch manually.** The workflow does the same operations.

### After Secret Rotation

If `bluefin-test-ssh-key` is regenerated, all existing golden disks have the old pubkey.
Re-patch every golden disk variant through the workflow path documented in
[`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md) and
[`docs/lab-operations.md`](docs/lab-operations.md).

## SSH Key

Stored in k8s secret `bluefin-test-ssh-key` in `argo` namespace. Use
[`docs/lab-operations.md`](docs/lab-operations.md) for live fingerprint retrieval and
rotation steps; this runbook avoids embedding time-sensitive command output.

## bib-disk-configure — Fixed Bugs (2026-05-25)

All four SSH auth bugs were fixed. Golden disks built after 2026-05-25 do not need manual patching.

| Bug | Fix |
|---|---|
| Home dir 777 — sshd StrictModes rejects it | `chmod 750` added after chown |
| sshd_config.d drop-in 666 — sshd rejects it | `chmod 600` added after write |
| authorized_keys write not verified | `test -s` check; exits 1 on empty file |
| Disk not flushed before umount | `sync` added before umount |
| pubkey read via kubectl (not in image) | Replaced with `secretKeyRef` env var |

1. **Home directory permissions 777**: `mkdir -p` inside container creates home dir with 0777.
   sshd `StrictModes yes` refuses `authorized_keys` from world-writable home dirs.
   Fix: `chmod 750 "${VAR}/home/bluefin-test"` in configure script.

2. **`sshd_config.d` file permissions 666**: Written without explicit chmod.
   sshd on Fedora/RHEL rejects world-writable config drop-ins.
   Fix: `chmod 600 "${ROOT}/etc/ssh/sshd_config.d/99-bluefin-test.conf"` after writing.

3. **Authorized_keys may not be written**: Silent failure if the target path doesn't exist
   or there's a mount flush issue. Fix: add `test -f ... || exit 1` verification + `sync`.

4. **Key mismatch**: If the secret is regenerated, golden disks still have the old public key.
   No automated reconciliation exists. Always re-patch after secret rotation.

**All four bugs are tracked and fixed. New issues go to castrojo/testing-lab.**

## Disk Layout (BIB --type raw --rootfs ext4)

| Partition | Size | Content |
|---|---|---|
| p1 | 1MB | BIOS boot |
| p2 | 100MB | EFI |
| p3 | 1GB | /boot (ext4, BLS boot entries) |
| p4 | ~13GB | root (ext4, OSTree) |

At runtime:
- `/home` → symlink to `/var/home`
- bluefin-test user: UID 1001, GID 1001
- Authorized keys path: `/var/home/bluefin-test/.ssh/authorized_keys`

## Workflow Templates

| Template | Namespace | Purpose |
|---|---|---|
| `bib-build-and-push` | argo | Build golden disk with BIB |
| `patch-golden-disk` | argo | Patch SSH auth on existing disk (no SSH to node) |
| `provision-bluefin-vm` | argo | Reflink + hostDisk + KubeVirt VM |
| `provision-flatcar-vm` | argo | Prepare Flatcar disk + KubeVirt VM |
| `run-gnome-tests` | argo | SSH into VM, run behave/pytest suites via qecore-headless |
| `run-incluster-tests` | argo | Run plain pytest against in-cluster homelab workloads |
| `run-service-tests` | argo | Run the shared service-catalog lane tests against in-cluster workloads |
| `homelab-substrate` | argo | Ephemeral namespace + in-cluster workload + lifecycle assertions |
| `bluefin-service-catalog-pipeline` | argo | Ephemeral namespace + in-cluster service workload + lane assertions |
| `homelab-access-probe` | argo | Ephemeral namespace + TLS fixture + HTTPS/auth probe assertions |
| `homelab-restore-drill` | argo | Ephemeral namespace + local-path stateful workload + restore assertions |
| `homelab-storage` | argo | Ephemeral namespace + local-path PVC workload + storage persistence assertions |

### Updating a WorkflowTemplate

Edit `argo/workflow-templates/<name>.yaml`, commit, push to `main`, and let ArgoCD apply it.
Use [`docs/lab-operations.md`](docs/lab-operations.md) for the exact sync and verification flow.

**Do NOT** use `argo-mcp-create_workflow_template` or `kubectl apply -f` for template updates.
These bypass git and will be overwritten by ArgoCD.

### Argo `outputs.result` stdout pollution

Any text printed to stdout in a `script:` template becomes the step output parameter.
ALL debug output MUST go to `>&2` or `>/dev/null`. The last line of stdout only should
be the actual output value. Pattern:
```bash
echo "debug info" >&2
printf '%s' "${RESULT_VALUE}"   # last line = output parameter
```

## Test Structure

```
tests/
  smoke/          # Phase 1: GNOME shell boot, top bar, Activities (every PR)
  developer/      # Phase 2: Ptyxis, brew, podman, micro editor
  software/       # Phase 3: Flatpak UI via gnome-software
  flatcar/        # Flatcar: systemd health, containerd, networking
  homelab_substrate/  # In-cluster homelab workload lifecycle assertions
  service_catalog/    # In-cluster media and non-media workload lanes
  homelab_access/     # In-cluster HTTPS and auth probe lanes
  homelab_backup/     # In-cluster local-path backup and restore drills
  homelab_storage/    # In-cluster local-path storage persistence and observability
```

GNOME suites use **behave + qecore-headless + dogtail** for `.feature` coverage,
and also run suite-local **pytest + dogtail** files when `test_*.py` exists.
Flatcar remains separate. tmt is not used.

- `qecore-headless` starts the GNOME Wayland session and hands off to behave or pytest
- `dogtail` does AT-SPI tree traversal inside the VM
- `gnome-ponytail-daemon` bridges AT-SPI coordinates to Wayland surface coordinates
- Steps live in `tests/<suite>/features/steps/steps.py`
- **Authoring guide:** [`docs/dogtail-testing.md`](docs/dogtail-testing.md) — read before
  adding a scenario, step, or suite.

## In-cluster Homelab Lane

The homelab queue now starts **k8s-first**. The first implementation path does
not provision a VM; it deploys an ephemeral workload directly in the cluster on
the control node and validates:

- deployment readiness
- service endpoint population
- controlled rollout restart
- post-restart pod identity change
- namespace cleanup on workflow exit

Operator entrypoints live in [`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md).

Key evidence files from the initial lane:

- `deployment-status-before.json`
- `service.json`
- `endpoints.json`
- `pods-before-restart.json`
- `pods-after-restart.json`
- `restart.txt`
- `rollout-status.txt`

## In-cluster Service-catalog Lanes

The first service-catalog implementation is also **k8s-first**. It deploys the
lane fixture directly inside Kubernetes, validates service reachability and
state persistence, and tears the namespace down on exit.

Operator entrypoints live in [`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md).

Initial lane guarantees:

- raw-manifest deployment in the cluster
- local-path PVC-backed state
- service endpoint reachability via cluster DNS
- sentinel persistence across rollout restart
- namespace cleanup on workflow exit

## In-cluster Access and Restore Lanes

Additional k8s-first homelab lanes now cover:

- **HTTPS access probing** against a TLS-enabled in-cluster fixture
- **auth-gated probing** for authenticated vs unauthenticated behavior
- **local-path restore drills** for the first stateful workload recovery path

Operator entrypoints live in [`docs/agent-cheatsheet.md`](docs/agent-cheatsheet.md).

## Dogtail / Wayland Setup

`gnome-ponytail-daemon` is required for AT-SPI coordinate injection on Wayland.
The `run-gnome-tests` runner starts it via `qecore-headless` before handing off to behave or pytest.

**unsafe_mode is required** for GNOME Shell 50+ AT-SPI access to top-bar elements.
Add this to `environment.py` `before_all` as `subprocess.run()` AFTER qecore-headless starts:
```bash
gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
  --method org.gnome.Shell.Eval 'global.context.unsafe_mode = true'
```
**Confirmed on Bluefin 44 (GNOME Shell 50.1):** `toolkit-accessibility=true` IS set correctly;
AT-SPI gaps in the top-bar are a GNOME Shell issue, not a config issue. `unsafe_mode` is
required to expose clock/system-status nodes.

## Archived Iteration Notes

Time-bound lessons and migration notes now live in
[`docs/archive/runbook-iteration-notes.md`](docs/archive/runbook-iteration-notes.md)
so this runbook stays focused on architecture and durable failure modes.

---

## Common Failure Modes

| Symptom | Root cause | Fix |
|---|---|---|
| `Permission denied (publickey)` | Key mismatch or home dir 777 | Re-patch golden disk (see above) |
| `outputs.result` gets "Waiting…" string | stdout pollution in script template | All debug to `>&2` |
| Workflow times out at SSH wait | sshd_config.d has 666 permissions | chmod 600 in configure script |
| `qemu-img: command not found` | Wrong container image for Flatcar prep | Use `quay.io/fedora/fedora:latest` |
| VM stuck Terminating | KubeVirt controller race | `kubectl delete pod virt-launcher-... -n ns --force` |
| run-gnome-tests pod Error immediately | `volumes:` inside `container:` block | Move `volumes:` to template level |
