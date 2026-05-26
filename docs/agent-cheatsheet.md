# Agent Cheatsheet — read this first, then stop

> Deterministic, recipe-only reference for running the testing-lab cluster.
> Designed to be the **single file a weak-capability agent needs to load** for 80%
> of routine cluster operations. No reasoning required: look up the row, run the
> command.
>
> If your task is not in this file, escalate to:
> - [`docs/lab-operations.md`](lab-operations.md) — long-form procedures
> - [`WORKFLOWS.md`](../WORKFLOWS.md) — WorkflowTemplate parameter contracts
> - [`RUNBOOK.md`](../RUNBOOK.md) — architecture + failure-mode index
> - [`docs/dogtail-testing.md`](dogtail-testing.md) — writing GUI tests
> - [`AGENTS.md`](../AGENTS.md) — hard policy and tenets
>
> Hard policy is short:
> 1. **No SSH** to ghost (192.168.1.102) or exo-1 (192.168.1.239). Ever.
> 2. **No `kubectl apply`** on anything under `argo/workflow-templates/` or `manifests/` — those are GitOps-owned by ArgoCD.
> 3. **Never delete** VMs labelled `app=titan-*` or anything in the `knuckle-test` namespace.
> 4. **Never act on Dakota issues/PRs** without the `needs-human/agent-ready` label.
> 5. **PR approval requires a Vanguard report** with real lab evidence (see §8). Template: [`docs/vanguard-report-template.md`](vanguard-report-template.md).

---

## 1. Command selector — "what should I run?"

| Situation                                                           | Run                                                                            |
|---------------------------------------------------------------------|--------------------------------------------------------------------------------|
| Validate a test/feature/step change                                 | `just run-titan-smoke`                                                         |
| Validate a test change against a PR branch                          | `BLUEFIN_TEST_BRANCH=<branch-or-sha> just run-titan-smoke`                     |
| Pre-merge gate / promote a passing titan run                        | `just run-tests-matrix`                                                        |
| Validate a single Bluefin tag end-to-end                            | `just run-tests-tag <latest\|lts>`                                             |
| Validate a golden-disk / image change                               | `just ensure-disk <tag>` then `just run-tests-tag <tag>`                       |
| Broader UI coverage (developer/software suites)                     | `just run-titan-developer` · `just run-titan-software`                         |
| Flatcar systemd/container suite                                     | `just run-flatcar-smoke`                                                       |
| In-cluster homelab lane (no VM)                                     | `just run-homelab-substrate`                                                   |
| In-cluster service catalog (media / nonmedia)                       | `just run-service-media` · `just run-service-nonmedia`                         |
| HTTPS / auth probe lane                                             | `just run-homelab-access` · `just run-homelab-auth`                            |
| Local-path restore / storage lanes                                  | `just run-homelab-restore` · `just run-homelab-storage`                        |
| Tail the most recent workflow's logs                                | `just logs`                                                                    |
| List workflows / VMs                                                | `just list-workflows` · `just list-vms`                                        |
| ArgoCD status / force sync                                          | `just argocd-status` · `just argocd-sync`                                      |
| Lint Argo YAML                                                      | `just lint`                                                                    |
| Clean orphan test VMs (titan-safe)                                  | `just delete-vms`                                                              |

Rule: **if a `just` recipe exists, use it.** Submit a `workflowtemplate/...` directly
only when the situation isn't in this table (then see WORKFLOWS.md for parameters).

---

## 2. Failure triage — symptom → exact next command

Run `just logs` first. Then match a row:

| Symptom in logs                                                      | Run next                                                                  |
|---------------------------------------------------------------------|---------------------------------------------------------------------------|
| `Permission denied (publickey)` at SSH wait                          | `just patch-disk <tag>` → rerun                                           |
| Workflow times out at SSH wait                                       | `just list-vms`; VM Ready but no IP → `just patch-disk <tag>` → rerun     |
| `TypeError: ... requireResult`                                       | Fix step per `docs/dogtail-testing.md` §6.2 (use `findChildren` / `retry=False`) |
| `Application "gnome-shell" is running` step fails                    | Replace with `* GNOME Shell is accessible via AT-SPI`                     |
| All top-bar scenarios fail                                           | Confirm `wait_for_shell.py` is in the SCP'd suite (runner re-asserts `unsafe_mode`) |
| `outputs.result` is `Waiting...`                                     | Script template stdout pollution — send debug to `>&2`, last line = value |
| VM stuck `Terminating`                                               | `kubectl delete pod virt-launcher-<vm> -n <ns> --force`                   |
| `qemu-img: command not found` (Flatcar prep)                         | Use `quay.io/fedora/fedora:latest` base image                             |
| `run-gnome-tests` pod errors immediately                             | `volumes:` was nested under `container:` — move to template level         |
| Titan has no IP                                                      | Wait 30s; still gone → delete VMI and let ArgoCD recreate (§5)            |
| Workflow stuck `Pending`                                             | Run §3 (capacity triage)                                                  |
| Template change "didn't take effect"                                 | Run §4 (ArgoCD decision tree)                                             |

If no row matches:

```
1. just logs                                # tail
2. Loki: |= "=== BEHAVE RESULTS JSON ==="    # full behave output
3. Loki: |= "STEP_ERROR"                     # traceback
4. Loki: |= "AT-SPI tree written"            # AT-SPI snapshot
5. argo get <workflow-name> -n argo          # events
```

Loki: <http://192.168.1.102:30100>. Pod label: `app.kubernetes.io/part-of=bluefin-test-suite`.

---

## 3. Capacity triage — "cluster feels slow"

```bash
just list-workflows                                     # how many running?
kubectl top nodes                                       # ghost / exo-1 pressure
kubectl get vmi -A                                      # how many live VMs?
kubectl get pods -A --field-selector=status.phase=Pending
kubectl top pods -A --sort-by=cpu | head -10            # find hogs
```

| Symptom                       | Action                                                                                      |
|-------------------------------|---------------------------------------------------------------------------------------------|
| Many `bib-img-*` running      | Two BIB builds saturate ghost. Suspend `nightly-smoke` (§6) before submitting more.         |
| Workflows `Pending`           | `kubectl top pods -A --sort-by=cpu` → find the hog; cancel non-essential.                   |
| Many `virt-launcher-*` pods   | `kubectl create job --from=cronworkflow/orphan-vm-cleanup orphan-$(date +%s) -n argo`       |
| Disk full on ghost            | `kubectl create job --from=cronworkflow/golden-disk-gc gdgc-$(date +%s) -n argo` (dry-run by default — read logs first) |

Per-template ceilings: see [`AGENTS.md`](../AGENTS.md) §"Resource Limits".

---

## 4. ArgoCD — "my template change didn't take effect"

```
1. git log -1 origin/main -- argo/workflow-templates/<file>
     → if your commit isn't on origin/main, you didn't push. Fix and stop.
2. argocd app get testing-lab        # check Revision
     → if older than your commit, just argocd-sync
3. just argocd-status                # both apps Synced + Healthy
     → if Degraded, read .status.conditions; fix YAML; push again
4. kubectl get workflowtemplate <name> -n argo -o yaml | grep <field>
     → if still old, just argocd-sync && argocd app wait testing-lab --health
5. Did you submit a workflow BEFORE the change?
     → workflows snapshot the template at submit time. Submit a NEW workflow.
```

Do **not** `kubectl apply` a rejected file. Fix the YAML and push.

---

## 5. Titan recovery (fast path is down)

```bash
just argocd-sync                                      # 1. ArgoCD recreates VMs from manifests/titan-*.yaml
kubectl get vmi titan-bluefin -n bluefin-test -w      # 2. wait for IP (~30–60s)
kubectl get vmi titan-lts     -n bluefin-lts-test -w
just run-titan-smoke                                  # 3. verify
```

**Do not rebuild a golden disk to fix a titan.** Titans use a separate persistent
disk under `/var/home/jorge/VMs/titans/...`.

If titan SSH fails after secret rotation, file an issue — do not SSH to the host
to refresh `authorized_keys`. The titan key refresh path is human-gated.

---

## 6. CronWorkflow ops (pause / resume / backfill)

```bash
# Suspend (debug session only — also edit manifests/nightly-smoke*.yaml if longer):
kubectl patch cronworkflow nightly-smoke     -n argo --type=merge -p '{"spec":{"suspend":true}}'
kubectl patch cronworkflow nightly-smoke-lts -n argo --type=merge -p '{"spec":{"suspend":true}}'

# Resume:
kubectl patch cronworkflow nightly-smoke     -n argo --type=merge -p '{"spec":{"suspend":false}}'

# Backfill / run now:
kubectl create job --from=cronworkflow/nightly-smoke       backfill-$(date +%s) -n argo
kubectl create job --from=cronworkflow/orphan-vm-cleanup   orphan-$(date +%s)   -n argo
kubectl create job --from=cronworkflow/golden-disk-gc      gdgc-$(date +%s)     -n argo
```

| Name                  | Schedule (UTC) | Purpose                                              |
|-----------------------|----------------|------------------------------------------------------|
| `nightly-smoke`       | 02:00          | `bluefin-qa-pipeline` (latest)                       |
| `nightly-smoke-lts`   | 02:30          | `bluefin-qa-pipeline` (lts) — first run builds disk  |
| `orphan-vm-cleanup`   | every 2h       | GC orphan VMs + per-run hostDisks. Titan-safe.       |
| `golden-disk-gc`      | 04:00          | GC stale golden disks. `DRY_RUN=true` by default.    |

Any patch longer than a debug session **must also be persisted in `manifests/`** —
otherwise the next ArgoCD reconcile flips it back.

---

## 7. SSH key rotation (deliberate, high-risk)

```bash
# 1. Generate a new key locally (do not commit):
ssh_key=$(mktemp)
ssh-keygen -t ed25519 -f "${ssh_key}" -N "" -C "bluefin-test-suite@ghost"

# 2. Replace the secret in-place:
kubectl create secret generic bluefin-test-ssh-key \
  --from-file=id_ed25519="${ssh_key}" \
  --from-file=id_ed25519.pub="${ssh_key}.pub" \
  -n argo --dry-run=client -o yaml | kubectl apply -f -
shred -u "${ssh_key}" "${ssh_key}.pub"

# 3. Patch every existing golden disk:
just patch-disk latest
just patch-disk lts

# 4. Confirm via a real run:
just run-tests-matrix       # fresh-VM, both tags
just run-titan-smoke        # if this fails: file an issue, do NOT SSH

# 5. Verify and record the new fingerprint:
kubectl get secret bluefin-test-ssh-key -n argo \
  -o jsonpath='{.data.id_ed25519\.pub}' | base64 -d | ssh-keygen -lf -
```

If `patch-disk` fails because the old key can no longer SSH: the disk has to be
rebuilt via a privileged workflow on ghost (`golden-disk-gc` then `just ensure-disk <tag>`).
Do not SSH to ghost to delete it.

---

## 8. PR queue mode — Vanguard Lab Strike Report

Mandatory gate for `knuckle`, `dakota`, and this repo's PRs.

1. Run the lab loop end-to-end — `just run-titan-smoke` minimum, `just run-tests-matrix` for high-risk.
2. Collect **real evidence**: workflow names, behave summary, log excerpts.
3. Post the report on the PR using **[`docs/vanguard-report-template.md`](vanguard-report-template.md)** — header order, verdict line, evidence sections, real command blocks. No narrative-only reports.
4. Only then apply `agent-tested` and approve / queue.

Hard exit checklist (all four required):

- [ ] Real VM-backed lab evidence exists (titan or fresh).
- [ ] The **entire loop** was tested, not isolated commands.
- [ ] A canonical Vanguard report with real data is posted on the PR.
- [ ] Any blocker is filed as an issue in the **owning repo** (not always this one).

Dakota-specific: act only on `needs-human/agent-ready` labelled items.

---

## 9. Safe cleanup — what you may delete

| Resource                                              | Safe?                                                |
|-------------------------------------------------------|------------------------------------------------------|
| VM in `bluefin-test` labelled `app=titan-bluefin`     | **No.** Never.                                       |
| VM in `bluefin-lts-test` labelled `app=titan-lts`     | **No.** Never.                                       |
| Anything in `knuckle-test`                            | **No.** Different repo owns it.                      |
| Non-titan VM in `bluefin-test` / `bluefin-lts-test` / `flatcar-test` / `gnomeos-test`, no live workflow | Yes — `just delete-vms` (label-aware). |
| Workflows in `argo`                                   | Yes — `just delete-workflows`.                       |
| Per-run hostDisk clone under `/var/tmp/bluefin-golden/*-runs/` | Yes — handled by `orphan-vm-cleanup` CronWorkflow. |
| Golden disk under `/var/tmp/bluefin-golden/<tag>/`    | Only via `golden-disk-gc` CronWorkflow (with `DRY_RUN=false`). |

Single-VM deletion preflight (always):

```bash
kubectl get vm <name> -n <ns> -o jsonpath='{.metadata.labels.app}{"\n"}'
# Starts with "titan-"?  STOP.
kubectl delete vm <name> -n <ns> --wait=false
```

---

## 10. Bootstrap (one-time, fresh cluster)

```bash
just setup-ssh-secret      # idempotent — does NOT rotate (see §7)
just setup-argocd          # applies both ArgoCD Applications
just argocd-sync           # first reconcile
just ensure-disk latest    # first golden disk
just run-titan-smoke       # smoke once titans publish IPs
```

---

## 11. Self-check before claiming "cluster healthy"

```bash
just argocd-status                  # both apps Synced + Healthy
just list-vms                       # titans Running, no orphans
just list-workflows | head -20      # newest workflows OK
kubectl get cronworkflow -n argo    # 4 entries, none suspended unexpectedly
just run-titan-smoke                # end-to-end fast path green
```

All five must pass. If any fails, do not claim healthy — triage with §2–§5.

---

## 12. Discover cluster facts (don't trust docs for drift-prone values)

| Fact                       | Command                                                                                       |
|----------------------------|-----------------------------------------------------------------------------------------------|
| SSH key fingerprint        | `kubectl get secret bluefin-test-ssh-key -n argo -o jsonpath='{.data.id_ed25519\.pub}' \| base64 -d \| ssh-keygen -lf -` |
| Titan VM IPs               | `kubectl get vmi titan-bluefin -n bluefin-test -o jsonpath='{.status.interfaces[0].ipAddress}{"\n"}'` (and `titan-lts -n bluefin-lts-test`) |
| Live WorkflowTemplate body | `kubectl get workflowtemplate <name> -n argo -o yaml`                                         |
| CronWorkflow schedule      | `kubectl get cronworkflow <name> -n argo -o jsonpath='{.spec.schedules[*]}{"\n"}'`            |
| ArgoCD revision in cluster | `argocd app get testing-lab \| grep Revision`                                                 |

Treat these commands as the source of truth. Documented IPs / fingerprints are
hints; cluster state wins.
