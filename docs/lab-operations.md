# Lab Operations Guide

> **Routine work? Load [`agent-cheatsheet.md`](agent-cheatsheet.md) first.** That
> file is the deterministic recipe-only reference for 80% of cluster ops. This
> guide is the expanded long-form version: extra detail, edge cases, and the
> sections the cheatsheet links into (titan recovery deep dive, full ArgoCD
> decision tree, load triage, etc.).
>
> A **paint-by-numbers operations manual** for running the testing-lab cluster end-to-end.
> Designed so that a less-capable agent (or a new operator) can complete every routine task
> using only this guide plus the `Justfile`, without SSHing to nodes or guessing.

Pair with:
- [`agent-cheatsheet.md`](agent-cheatsheet.md) — single-file deterministic 80% recipe set
- [`../AGENTS.md`](../AGENTS.md) — policy + tenets
- [`../RUNBOOK.md`](../RUNBOOK.md) — architecture + failure-mode index
- [`../WORKFLOWS.md`](../WORKFLOWS.md) — WorkflowTemplate parameter contracts
- [`dogtail-testing.md`](dogtail-testing.md) — GUI test authoring
- [`vanguard-report-template.md`](vanguard-report-template.md) — PR verification report (vendored)

> **Hard rules** (do not bend, ever):
> 1. No SSH to ghost (192.168.1.102) or exo-1 (192.168.1.239).
> 2. No `kubectl apply` for anything under `argo/workflow-templates/` or `manifests/` —
>    edit YAML → `git push main` → ArgoCD syncs.
> 3. Never delete VMs labelled `app=titan-bluefin`, `app=titan-lts`, or anything in the
>    `knuckle-test` namespace.
> 4. PR validation must produce **real lab evidence** in a **canonical Vanguard report**.
>    Never approve from metadata alone.

---

## 1. The 60-second mental model

```
You ──submit──► Argo Workflow (argo NS)
                  │
                  ├─ git-sync initContainer clones testing-lab @ <branch>
                  ├─ ensure-disk (BIB) ── if cold, builds golden disk on ghost hostPath
                  ├─ provision-vm     ── btrfs reflink + KubeVirt VM (skipped for titan path)
                  ├─ run-gnome-tests  ── runner pod SSHes VM → qecore-headless + behave/pytest
                  └─ teardown (onExit) ── delete VM + per-run hostDisk
```

Two execution paths:

| Path                 | Speed     | Use when                                             |
|----------------------|-----------|------------------------------------------------------|
| **Titan (persistent)** | ~5 min   | Iterating on tests; default for any test-only change |
| **Fresh VM (BIB)**   | ~10–14 min| Validating image changes; pre-merge gate; matrix     |

---

## 2. Decision flowcharts

### "What command do I run?"

```
Need to validate a test/feature/step change?
  └─► just run-titan-smoke           (or run-titan-developer / run-titan-software)
        └─ green?  re-run on fresh VM: just run-tests (or run-tests-matrix)
        └─ red?   read logs: just logs   → fix → repeat on titan

Need to validate an image / golden disk change?
  └─► just ensure-disk [tag]   (rebuilds golden disk if missing/stale)
        └─► just run-tests-tag <tag>
              └─► matrix gate: just run-tests-matrix

Need to validate a PR branch (mine or someone else's)?
  └─► export BLUEFIN_TEST_BRANCH=<branch-or-sha>
        └─► just run-titan-smoke      (fast lane)
        └─► just run-tests-matrix     (gate before approval)

Validation failed?
  └─► Section 5 — Failure triage

Cluster feels slow / over-capacity?
  └─► Section 7 — Load triage
```

### "Which template should I submit directly?"

You should rarely submit a template directly. Use a `just` recipe. If you must:

| Goal                                    | Submit                                                  |
|-----------------------------------------|---------------------------------------------------------|
| Full pipeline, fresh VM                 | `workflowtemplate/bluefin-qa-pipeline`                  |
| Test only against titans                | `workflowtemplate/bluefin-titan-smoke`                  |
| Refresh SSH on existing golden disk     | `workflowtemplate/patch-golden-disk`                    |
| In-cluster homelab lane                 | `workflowtemplate/homelab-substrate` (and siblings)     |
| Service-catalog lane (media/nonmedia)   | `workflowtemplate/bluefin-service-catalog-pipeline`     |

Never submit `bib-build-and-push`, `provision-vm`, `run-gnome-tests`, or `teardown-*`
directly — they are called via `templateRef` from the pipelines above. Submitting them
in isolation breaks the cleanup contract.

---

## 3. Every operator command, with safety notes

All commands assume cwd = repo root and that `kubectl` / `argo` / `argocd` CLIs can reach
the cluster (or you're using the corresponding MCP tool).

### 3.1 Test execution

| Command                                | Effect                                                                                  |
|----------------------------------------|-----------------------------------------------------------------------------------------|
| `just run-tests`                       | Smoke against `latest`, fresh VM, full pipeline. ~10 min warm, ~14 min cold.            |
| `just run-tests-tag <tag>`             | Smoke against `<tag>` (`latest` or `lts`).                                              |
| `just run-tests-matrix`                | `latest` + `lts` in parallel. Pre-merge gate.                                           |
| `just run-developer-tests [tag]`       | Smoke + developer suites, fresh VM.                                                     |
| `just run-software-tests [tag]`        | Smoke + developer + software, fresh VM.                                                 |
| `just run-titan-smoke`                 | Smoke against persistent titans (latest + lts). **Fastest loop, ~5 min.**               |
| `just run-titan-developer`             | Developer suite on titans.                                                              |
| `just run-titan-software`              | Software suite on titans.                                                               |
| `just run-flatcar-smoke`               | Flatcar systemd/container suite.                                                        |
| `just run-homelab-substrate`           | In-cluster homelab smoke (no VM, ephemeral namespace).                                  |
| `just run-service-media`               | In-cluster service-catalog: media lane.                                                 |
| `just run-service-nonmedia`            | In-cluster service-catalog: non-media lane.                                             |
| `just run-homelab-access`              | HTTPS probe against an in-cluster TLS fixture.                                          |
| `just run-homelab-auth`                | Authenticated/unauthenticated probe variant.                                            |
| `just run-homelab-restore`             | Local-path restore drill.                                                               |
| `just run-homelab-storage`             | Local-path PVC persistence & observability.                                             |
| `just run-gnomeos-spike`               | GNOME OS upstream spike workflow (requires `GNOMEOS_*` env vars; see §10).              |
| `just run-upstream-terminal-tests`     | Pulls and runs upstream `GNOMETerminalAutomation` against titans.                       |

Pass a non-`main` ref by setting `BLUEFIN_TEST_BRANCH`:

```bash
BLUEFIN_TEST_BRANCH=fix/my-branch just run-titan-smoke
```

### 3.2 Observation

| Command                  | Effect                                                                  |
|--------------------------|-------------------------------------------------------------------------|
| `just list-workflows`    | All workflows in `argo` ns, newest first.                               |
| `just list-vms`          | VMs across `bluefin-test`, `bluefin-lts-test`, `flatcar-test`, `gnomeos-test`. |
| `just logs`              | Tail of the most recent workflow.                                       |
| `just argocd-status`     | Sync/health of `testing-lab` + `testing-lab-infra` Applications.        |

### 3.3 Maintenance

| Command                  | Effect                                                                  |
|--------------------------|-------------------------------------------------------------------------|
| `just argocd-sync`       | Force ArgoCD reconcile (don't run blindly; see §6).                     |
| `just ensure-disk [tag]` | Build golden disk if missing/stale.                                     |
| `just patch-disk [tag]`  | Re-apply SSH auth on existing golden disk (after secret rotation).      |
| `just delete-vms`        | Delete non-titan VMs in test namespaces. **Titan-safe** — see §8.       |
| `just delete-workflows`  | Delete all workflows in `argo` ns.                                      |
| `just teardown`          | Convenience: delete-vms + delete-workflows.                             |
| `just lint`              | `argo lint --offline` on `argo/workflow-templates/*.yaml` + `argo/*.yaml`. |

### 3.4 Bootstrap (one-time)

| Command                  | Effect                                                                  |
|--------------------------|-------------------------------------------------------------------------|
| `just setup-ssh-secret`  | Create `bluefin-test-ssh-key` secret in `argo`. Idempotent. **Does NOT rotate** — see §9. |
| `just setup-argocd`      | Apply `argocd/application.yaml`. Run once per fresh cluster.            |

---

## 4. Retrieving evidence (logs, artifacts, AT-SPI tree)

> **Never `kubectl exec` into a workflow pod to read results.** `podGC: OnWorkflowSuccess`
> deletes succeeded pods immediately; failed pods linger only until TTL. Always read via
> Argo / Loki.

### 4.1 Argo logs (canonical)

```bash
# Most recent workflow's full logs:
just logs

# Specific workflow by name:
argo logs <workflow-name> -n argo --no-color

# Live tail:
argo logs <workflow-name> -n argo --follow
```

### 4.2 Loki queries (use these strings to skip log walls)

Loki is at <http://192.168.1.102:30100>. Pods carry the label
`app.kubernetes.io/part-of=bluefin-test-suite`. Useful filters:

| What you want                         | Loki query                                                                  |
|---------------------------------------|-----------------------------------------------------------------------------|
| Full behave JSON for a run            | `{app_kubernetes_io_part_of="bluefin-test-suite"} \|= "=== BEHAVE RESULTS JSON ==="` |
| Step-level traceback                  | `{app_kubernetes_io_part_of="bluefin-test-suite"} \|= "STEP_ERROR"`         |
| AT-SPI tree dump                      | `{app_kubernetes_io_part_of="bluefin-test-suite"} \|= "AT-SPI tree written"` |
| Dependency-install path taken         | `{app_kubernetes_io_part_of="bluefin-test-suite"} \|~ "Test dependencies already installed\|Installing test dependencies"` |
| Variant filter                        | `{bluefin_io_variant="lts"}`                                                |
| Suite filter                          | `{bluefin_io_suite="developer"}`                                            |

### 4.3 Artifact files inside the runner pod (only while it lives)

The runner SCPs `/tmp/results/` back from the VM into its own pod filesystem:

- `results.json` — behave structured results
- `pytest-results.xml` — pytest JUnit
- `atspi_tree.txt` — full GNOME Shell AT-SPI dump (depth=4) — written by the first smoke
  scenario and always copied back

These are echoed to pod stderr at run end, so they survive in Loki even after `podGC`.
Search Loki for `=== BEHAVE RESULTS JSON ===` (start) and `=== END BEHAVE RESULTS ===`
(end) to extract.

### 4.4 Titan VM results (persist across runs)

`/tmp/results/` on the titan VM is not wiped between runs. If you need a previous run's
artifacts and Loki retention has rolled them off, submit a one-shot diagnostic workflow
that SCPs from the titan rather than introducing manual SSH — but in practice, Loki
should always be sufficient.

---

## 5. Failure triage

Match the symptom; perform the action. Do not skip to "rerun" without identifying the
class of failure first.

| Symptom                                         | Likely cause                          | Action                                                            |
|-------------------------------------------------|---------------------------------------|-------------------------------------------------------------------|
| `Permission denied (publickey)` during SSH wait | Key mismatch on golden disk           | `just patch-disk <tag>` then re-run.                              |
| Workflow times out at SSH wait                  | VM boot slow or sshd config broken    | `just list-vms`; if VM Ready but no IP, `just patch-disk`.        |
| `STEP_ERROR` with `TypeError: ... requireResult`| Stale dogtail call                    | Fix step per [`docs/dogtail-testing.md`](dogtail-testing.md) §6.2. |
| All top-bar scenarios fail                      | `unsafe_mode` not enabled             | Check `wait_for_shell.py` is in the SCP'd suite; runner re-asserts it.|
| `Application "gnome-shell" is running` fails    | Wrong step                            | Use `* GNOME Shell is accessible via AT-SPI` (dogtail guide §6.6).|
| Workflow stuck `Pending`                        | Cluster capacity                      | §7 load triage.                                                   |
| `outputs.result` is "Waiting..."                | Script template stdout pollution      | Send debug to `>&2`; only output value on stdout (RUNBOOK §191).  |
| VM stuck `Terminating`                          | KubeVirt controller race              | `kubectl delete pod virt-launcher-<vm> -n <ns> --force`.          |
| `qemu-img: command not found` in Flatcar prep   | Wrong base image                      | Use `quay.io/fedora/fedora:latest`.                               |
| `run-gnome-tests` pod errors immediately        | `volumes:` inside `container:` block  | Move `volumes:` to template level.                                |
| Titan VM has no IP                              | KubeVirt masquerade race              | Wait 30s; if still gone, delete VMI and let ArgoCD recreate (§8.2).|

**Decision tree if you don't know:**

```
1. just logs                          # most recent run's tail
2. Loki: search '=== BEHAVE RESULTS JSON ===' for the workflow's pod
3. Loki: search 'STEP_ERROR' for the failing step's traceback
4. Loki: search 'AT-SPI tree' if a node lookup failed — match against actual tree
5. Still unclear? read argo events:  argo get <wf-name> -n argo
```

---

## 6. ArgoCD operations

### 6.1 What ArgoCD owns

| Application          | Syncs                                | Auto-sync? |
|----------------------|--------------------------------------|------------|
| `testing-lab`        | `argo/workflow-templates/*.yaml`     | Yes        |
| `testing-lab-infra`  | `manifests/*.yaml`                   | Yes (server-side apply) |

Reconcile interval ~3 min. Force sync only when you need it immediately and can justify it.

### 6.2 Decision tree — "my template change didn't take effect"

```
1. Did you push to main?
     - git log -1 origin/main -- argo/workflow-templates/<file>
     - Expected: the top commit is the one you just pushed.
     - If your commit is missing, you did not push the change. Push first and stop here.
2. Has ArgoCD seen the commit?
     - argocd app get testing-lab     # check 'Revision' field
     - Expected: `Revision:` is at or newer than your commit SHA.
     - If `Revision:` is older, run `just argocd-sync`, then re-run `argocd app get testing-lab`.
3. Is the Application healthy/synced?
     - just argocd-status
     - Expected: both applications show `Synced` and `Healthy`.
     - If `OutOfSync` and `Healthy`, run `just argocd-sync`, then re-run `just argocd-status`.
     - If `Degraded`, run `argocd app get testing-lab`, copy the first rejection/error message from `.status.conditions`, fix the YAML it names, push again, and restart at step 1.
4. Is the live WorkflowTemplate updated?
     - kubectl get workflowtemplate <name> -n argo -o yaml | grep <field>
     - Expected: the field you changed matches your new value.
     - If the live template still shows the old value, run `just argocd-sync`, then `argocd app wait testing-lab --health`, then re-run the `kubectl get workflowtemplate ...` command.
5. Did you submit a workflow created BEFORE the template change?
     - Workflows snapshot the template at submission time.
     - Expected: the workflow `CREATED` time is after the template was synced.
     - If the workflow was submitted earlier, submit a new workflow. The old run will keep the old behavior.
```

### 6.3 When NOT to force-sync

- Within 3 min of pushing — wait for the next poll instead.
- When `argocd app get` shows `Sync Status: Synced` — there is nothing to do.
- When you're about to push another change in ~1 min — let one sync pick up both.

### 6.4 Recovering a sync failure

```bash
argocd app get testing-lab          # read .status.conditions
argocd app sync testing-lab --prune # only if you intend to delete-and-recreate
just argocd-sync                    # full sync of both apps + wait healthy
```

If a manifest is rejected as "schema invalid", fix the YAML and push again — never
`kubectl apply` the rejected file by hand.

---

## 7. Load triage — "cluster feels slow"

### 7.1 Quick capacity snapshot

```bash
just list-workflows                 # how many workflows are running?
kubectl top nodes                   # ghost / exo-1 CPU + memory pressure
kubectl get pods -A --field-selector=status.phase=Running | wc -l
kubectl get pods -A --field-selector=status.phase!=Running --field-selector=status.phase!=Succeeded
kubectl get vmi -A                  # how many VMs are live?
```

### 7.2 Common load patterns

| Symptom                            | Probable cause                                  | Action                                                              |
|------------------------------------|-------------------------------------------------|---------------------------------------------------------------------|
| Many `bib-img-*` pods Running      | Concurrent BIB builds (8 CPU each)              | Wait or suspend `nightly-smoke` (§8.4) before submitting more.      |
| Workflows `Pending`                | No node has free CPU                            | `kubectl top pods -A --sort-by=cpu` to find hogs.                   |
| Many `virt-launcher-*` pods        | Orphan VMs accumulated                          | `kubectl create job --from=cronworkflow/orphan-vm-cleanup orphan-now -n argo` |
| Disk full on ghost                 | Old golden disks or per-run hostDisks           | Run `golden-disk-gc` CronWorkflow (§8.3) manually.                  |
| Loki ingester slow                 | High-volume runs writing huge results.json      | Don't pipe entire AT-SPI trees in tight loops; trim depth in steps. |

### 7.3 Per-template resource ceiling (from AGENTS.md)

| Template           | CPU req/limit | Memory req/limit |
|--------------------|---------------|------------------|
| `bib-img-build`    | 4 / 8         | 8Gi / 16Gi       |
| `bib-img-pull`     | 2 / 4         | 2Gi / 4Gi        |
| `bib-disk-configure`| 2 / 4        | 4Gi / 8Gi        |
| `run-gnome-tests`  | 1 / 2         | 1Gi / 2Gi        |
| `reflink-disk`     | 100m / 500m   | 128Mi / 512Mi    |
| `preflight` (titan)| 100m / 200m   | 64Mi / 128Mi     |

Two concurrent BIB builds will saturate ghost's 16-core CPU. Prefer one variant at a time
unless you specifically need matrix coverage.

---

## 8. VM lifecycle and safe cleanup

### 8.1 Inventory rules (memorise)

| Namespace            | Owner                                  | Safe to delete? |
|----------------------|----------------------------------------|-----------------|
| `bluefin-test`       | Test workflows + `titan-bluefin`       | Only non-titan, only if workflow gone |
| `bluefin-lts-test`   | Test workflows + `titan-lts`           | Only non-titan, only if workflow gone |
| `flatcar-test`       | Flatcar test workflows                 | Only if parent workflow is gone |
| `gnomeos-test`       | GNOME OS spike workflows               | Only if parent workflow is gone |
| `knuckle-test`       | knuckle-qa skill                       | **Never. Different repo owns this.** |

### 8.2 Safe deletion

Use `just delete-vms` — it skips any VM labelled `app=titan-*`. The cluster also runs
`orphan-vm-cleanup` every 2h with three safety rules (titan exclusion, live-workflow
exclusion, age ≥ 3h).

If you need to delete a single VM, **always check the label first**:

```bash
kubectl get vm <name> -n <ns> -o jsonpath='{.metadata.labels.app}{"\n"}'
# If output starts with "titan-", STOP. Don't delete.
kubectl delete vm <name> -n <ns> --wait=false
```

### 8.3 Restoring missing titans

If a titan VM is gone (manifest deleted, namespace recreated, ArgoCD pruned):

```bash
# 1. Trigger a sync — ArgoCD recreates the VM from manifests/titan-*.yaml.
just argocd-sync

# 2. Wait for the VMI to publish an IP (KubeVirt ~30–60s):
kubectl get vmi titan-bluefin -n bluefin-test -w
kubectl get vmi titan-lts     -n bluefin-lts-test -w

# 3. Once both have .status.interfaces[0].ipAddress, run:
just run-titan-smoke
```

Do **not** rebuild the golden disk to restore a titan — titans run their own persistent
disk image at `/var/home/jorge/VMs/titans/...`, not the golden disk.

### 8.4 Manually triggering housekeeping CronWorkflows

```bash
# Run orphan-vm-cleanup now (instead of waiting for the 2h schedule):
kubectl create job --from=cronworkflow/orphan-vm-cleanup orphan-$(date +%s) -n argo

# Run golden-disk-gc now (dry-run default; flip DRY_RUN to "false" by editing the
# manifest and pushing if you want it to actually delete):
kubectl create job --from=cronworkflow/golden-disk-gc gdgc-$(date +%s) -n argo
```

### 8.5 CronWorkflow inventory

| Name                  | Schedule (UTC) | Calls                                  | Purpose                                                    |
|-----------------------|----------------|----------------------------------------|------------------------------------------------------------|
| `nightly-smoke`       | 02:00          | `bluefin-qa-pipeline` (latest)         | Catch upstream regressions on `latest`.                    |
| `nightly-smoke-lts`   | 02:30          | `bluefin-qa-pipeline` (lts)            | Same for `lts`. First run also builds the LTS golden disk. |
| `orphan-vm-cleanup`   | every 2h       | inline                                 | GC orphan VMs + per-run hostDisks. Titan-safe.             |
| `golden-disk-gc`      | 04:00          | inline                                 | GC stale golden disks. Defaults to `DRY_RUN=true`.         |

### 8.6 Pause / resume / backfill nightlies

```bash
# Suspend a nightly during debugging (does NOT git-commit; expires on next ArgoCD sync if
# you don't also edit the manifest):
kubectl patch cronworkflow nightly-smoke     -n argo --type=merge -p '{"spec":{"suspend":true}}'
kubectl patch cronworkflow nightly-smoke-lts -n argo --type=merge -p '{"spec":{"suspend":true}}'

# Resume:
kubectl patch cronworkflow nightly-smoke     -n argo --type=merge -p '{"spec":{"suspend":false}}'

# Backfill (run nightly NOW instead of waiting for 02:00):
kubectl create job --from=cronworkflow/nightly-smoke backfill-$(date +%s) -n argo
```

> If you patch `suspend: true` for more than a single debug session, **also edit the
> manifest under `manifests/nightly-smoke*.yaml` and push** — otherwise the next ArgoCD
> reconcile flips it back.

---

## 9. SSH key rotation

`setup-ssh-secret` exits if the secret already exists; it is *not* a rotation procedure.
Rotation is a deliberate operation because every existing golden disk has the old pubkey.

```bash
# 1. Generate a new key locally (do not commit it):
ssh_key=$(mktemp)
ssh-keygen -t ed25519 -f "${ssh_key}" -N "" -C "bluefin-test-suite@ghost"

# 2. Replace the secret in-place:
kubectl create secret generic bluefin-test-ssh-key \
  --from-file=id_ed25519="${ssh_key}" \
  --from-file=id_ed25519.pub="${ssh_key}.pub" \
  -n argo --dry-run=client -o yaml | kubectl apply -f -

# 3. Shred the local copies:
shred -u "${ssh_key}" "${ssh_key}.pub"

# 4. Re-patch every golden disk with the new pubkey:
just patch-disk latest
just patch-disk lts

# 5. Titans use a separate persistent disk path. There is no automated titan
#    authorized_keys refresh workflow today, so treat titan SSH failure as a
#    human-gated escalation. Check by running:
just run-titan-smoke
#    If titan SSH fails with `Permission denied (publickey)`, open an issue and
#    stop. Do not SSH to the host and do not patch titan disks by hand.

# 6. Verify the new fingerprint, update RUNBOOK.md ("Current fingerprint" line),
#    commit, push:
kubectl get secret bluefin-test-ssh-key -n argo \
  -o jsonpath='{.data.id_ed25519\.pub}' | base64 -d | ssh-keygen -lf -
```

If `patch-disk` fails because the old key can't SSH anymore, you must rebuild the disk:

```bash
rm /var/tmp/bluefin-golden/<tag>/disk.raw   # from a privileged workflow, NOT SSH —
                                            # submit a one-shot maintenance workflow or
                                            # use the cluster's existing golden-disk-gc.
just ensure-disk <tag>
```

---

## 10. Undocumented-elsewhere recipes

These exist in the `Justfile` but were missing from older docs. Operators should know they
exist before assuming a workflow has to be authored from scratch.

| Recipe                              | What it does                                                                                 | Env vars                                                                 |
|-------------------------------------|----------------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| `just run-gnomeos-spike`            | Submits `argo/gnomeos-access-spike.yaml` — provisions a GNOME OS VM from upstream installer. | `GNOMEOS_IMAGE_URL`, `GNOMEOS_IMAGE_SHA256`, `GNOMEOS_IMAGE_FORMAT`, `GNOMEOS_NAMESPACE`, `GNOMEOS_CONSOLE_HOOK_CONFIG_MAP` |
| `just run-upstream-terminal-tests`  | Clones `modehnal/GNOMETerminalAutomation` and runs against the titan path.                   | `UPSTREAM_TERMINAL_REPO`, `UPSTREAM_TERMINAL_REF`                        |

These are spike/experimental lanes; do **not** wire them into nightlies without filing an
issue first.

---

## 11. PR queue mode — Vanguard Lab Strike Report

When this repo is supporting PR review for `knuckle`, `dakota`, or this repo itself, the
job is **lab-first**:

1. Run lab validation (titan smoke at minimum, full matrix for high-risk PRs).
2. Collect real evidence (workflow IDs, behave summaries, screenshots if produced).
3. Post a **canonical Vanguard report** as a PR comment.
4. Only then approve / queue / label `agent-tested`.

**The canonical report template is vendored at
[`docs/vanguard-report-template.md`](vanguard-report-template.md)** so any agent
without access to the operator's local `~/src/skills/ghost-testlab/` can comply.
It MUST be followed exactly — header order, verdict line, evidence sections, real
command/output blocks. No narrative-only reports.

Hard exit checklist before approval:

- [ ] Real VM-backed lab evidence exists (titan or fresh).
- [ ] The **entire loop** was tested, not isolated commands.
- [ ] A canonical Vanguard report with real data is posted on the PR.
- [ ] Any blocker is captured as an issue in the **owning repo** (not always this one).
- [ ] PR carries the `agent-tested` label only after the report is posted.

Dakota-specific gates (from AGENTS.md memories):

- Only act on Dakota issues/PRs labelled `needs-human/agent-ready`.
- Prefer the documented Dakota ghost/QEMU fast path; do not rediscover.

---

## 12. Authoring a new in-cluster homelab lane

Use this checklist so a new lane is consistent with existing ones and an agent reading the
docs later can run it.

1. **Tests:** add `tests/<lane_name>/test_*.py` (pytest, no GUI). Keep one assertion family
   per file (deployment, service, persistence, cleanup).
2. **Fixture manifest:** if your lane needs an in-cluster Deployment/Service/PVC, add a
   single YAML under `manifests/<lane>-fixture.yaml` (server-side-apply friendly).
3. **Lane WorkflowTemplate:** add `argo/workflow-templates/<lane>.yaml`. Pattern: create
   ephemeral namespace → apply fixture → wait rollout → `templateRef: run-incluster-tests`
   → `onExit` delete namespace.
4. **Submit wrapper:** add `argo/<lane>.yaml` (top-level Workflow) that takes `branch`.
5. **`Justfile` recipe:** `just run-<lane>` that `argo submit`s the wrapper with `--watch`.
6. **Docs:** add a row to `WORKFLOWS.md` "Top-level entry points" AND a row to §3.1 of
   this file.
7. **ArgoCD:** nothing to do — `testing-lab` and `testing-lab-infra` Apps will pick up
   `argo/workflow-templates/<lane>.yaml` and `manifests/<lane>-fixture.yaml` on next sync.
8. **Validate:** push, wait for sync (`just argocd-status`), submit (`just run-<lane>`),
   inspect with Loki. Then add a CronWorkflow under `manifests/` only if the lane needs
   scheduled regression coverage.

Cleanup contract: every lane MUST delete its ephemeral namespace on `onExit`. Test this
by listing namespaces before and after a failed run.

---

## 13. Self-check before declaring the cluster "healthy"

Run this short sequence after any non-trivial change to infra or templates:

```bash
just argocd-status                            # both apps Synced + Healthy
just list-vms                                 # titans Running, no orphans
just list-workflows | head -20                # newest workflows Succeeded or Running
kubectl get cronworkflow -n argo              # 4 entries, none suspended
just run-titan-smoke                          # end-to-end fast path works
```

All five must pass. If any fails, do not claim healthy; triage with the relevant section
above.

---

## 14. What this guide deliberately does NOT cover

- Hardware-level operations on ghost / exo-1 (replace disks, change networking,
  reinstall k3s). Those are out of scope for the lab; raise an issue and escalate.
- Editing the underlying k3s / KubeVirt / ArgoCD install. Bootstrap lives in
  `castrojo/utah` and the homelab skill set.
- Pixel-based UI testing. We use AT-SPI / qecore exclusively — see
  [`docs/dogtail-testing.md`](dogtail-testing.md).
- Changing the security posture. See [`SECURITY.md`](../SECURITY.md) for accepted
  trade-offs; do not silently tighten or loosen them.
