# Agent Cheatsheet — read this first, then stop

> Deterministic, recipe-only reference for running the lab cluster.
> Designed to be the **single file a weak-capability agent needs to load** for routine cluster operations.
>
> If your task is not in this file, escalate to:
> - [`/docs/ops/lab-operations.md`](/docs/ops/lab-operations.md) — long-form procedures
> - [`/docs/reference/workflow-reference.md`](/docs/reference/workflow-reference.md) — WorkflowTemplate parameter contracts, CronWorkflow schedules
> - [`/docs/ops/RUNBOOK.md`](/docs/ops/RUNBOOK.md) — architecture + failure-mode index
> - [`/docs/skills/cluster-tooling/buildstream.md`](/docs/skills/cluster-tooling/buildstream.md) — BuildStream, BuildGrid, Buildbarn
> - [`/docs/skills/dakota-pr-review/SKILL.md`](/docs/skills/dakota-pr-review/SKILL.md) — Dakota PR review loop
> - [`/docs/skills/cluster-tooling/SKILL.md`](/docs/skills/cluster-tooling/SKILL.md) — AMD GPU checks; [`hive-contribute`](/docs/skills/hive-contribute/SKILL.md) — llm-d models
> - [`/docs/ops/k3s-tuning.md`](/docs/ops/k3s-tuning.md) — node onboarding, reboot, offboarding
> - [`projectbluefin/testsuite`](https://github.com/projectbluefin/testsuite) — writing GUI tests
> - [`/AGENTS.md`](/AGENTS.md) — hard policy and tenets

> [!NOTE]
> **CLI/API-first.** Tool hierarchy: `just` (lifecycle recipes) → `argo`/`kubectl`
> (cluster ops). Routine/public-agent SSH is limited to workflow/probe pods
> connecting to explicit test VMs. Retained host-maintenance SSH is
> operator-only through an approved private channel; never use workstation SSH
> to administer `ghost` or `exo-0`.
> MCP tools are optional — never block on them. One bash call beats a tool search + MCP roundtrip every time.

## 1. Command selector — what should I run?

| Situation | Run |
|---|---|
| Submit Dakota BST build pipeline | `just run-bst-build [ref=testing] [repo=…] [variants=all\|default] [commit_sha=…]` |
| Submit Bluefin Server BST build pipeline | `just run-bluefin-server-build [ref=main]` |
| Rebuild the current Dakota `testing` SHA through the poller | `just force-dakota-poll` |
| Run Dakota QA against the published image | `just run-dakota-qa [branch=main] [variant=dakota]` |
| Run Dakota containerized smoke QA against a lab Zot image | `just run-dakota-container-qa [image-tag=testing] [variant=dakota]` |
| Promote a Zot candidate | `just run-zot-promotion <lane> <repository> candidate-<sha> sha256:<digest>` |
| Trigger the Dakota PR batch workflow | `argo submit -n argo --from workflowtemplate/dakota-pr-batch-pipeline -p pr-numbers=<number> --wait` |
| Tail the most recent workflow's logs | `just logs` |
| List workflows / VMs / CronWorkflows | `just list-workflows` · `just list-vms` · `argo cron list -n argo` |
| ArgoCD status / force sync | `just argocd-status` · `just argocd-sync` |
| Lint Argo YAML | `just lint` |
| Fresh cluster access | `just setup-argocd && just argocd-sync` (see [bootstrap](/docs/ops/bootstrap.md)) |

Rule: **if a `just` recipe exists, use it.** Otherwise use `argo`/`kubectl` directly; do not wait for MCP.

Every BST submission requires remote execution and fresh USB4 `up` observations
on both `ghost` and `exo-0`; the workflow rejects any other state. Local,
cache-backed, Ethernet-backed, automatic-fallback, and remote-cache-only paths
are prohibited. Confirm the generated BuildStream configuration, Ready
BuildGrid workers on both nodes, and the run's actions in the BuildGrid `jobs`
table before calling a run distributed.

## 2. Failure triage — symptom → exact next command

Run `just logs` first. Then match a row. **Dakota QA is container-only** — rows mentioning VM or VMI apply only to explicit KubeVirt workflows such as `bluefin-server-boot-test`.

| Symptom in logs | Run next |
|---|---|
| `No GITHUB_TOKEN or missing results.json - skipping publication` | `kubectl get secret -n argo github-token` — secret must exist; then inspect `just logs` for the failing suite before rerunning. |
| `results.json not found` or summary reports `Execution failed` | `just logs | grep -n "results.json not found\|Execution failed"` → identify the failing `run-container-tests` lane, then rerun after fixing the image or suite issue. |
| Expected image-poll rerun never starts after a new publish | `kubectl get configmap image-polling-digests -n argo -o yaml` — compare the stored digest with the workflow log; stale state means the previous run already claimed that digest. |
| `TypeError: ... requireResult` | Fix the step in the upstream `projectbluefin/testsuite` patterns (`findChildren(...)` / `retry=False`) |
| `Application "gnome-shell" is running` step fails | Replace it with `* GNOME Shell is accessible via AT-SPI` |
| All top-bar scenarios fail | Confirm `wait_for_shell.py` is present in the copied suite and that the runner re-asserts `unsafe_mode` |
| `outputs.result` is `Waiting...` or other debug text | Send debug output to `>&2`; keep stdout for the result only |
| BST build fails, or its retry is skipped with `retryStrategy.expression evaluated to false` | Follow [Diagnosing a failed remote element build](/docs/skills/cluster-tooling/buildstream.md#diagnosing-a-failed-remote-element-build) and [Failed builds and retries](/docs/skills/cluster-tooling/buildstream.md#failed-builds-and-retries); a cached element failure needs new inputs, not a resubmit. |
| VM stuck `Terminating` | `kubectl delete pod -n argo $(kubectl get pod -n argo -l kubevirt.io/vm=<name> -o name)` |
| Workflow stuck `Pending` | Run §3 |
| Workflow stuck on a `NotReady` node / pod never progresses | `kubectl get nodes`; if the worker is `NotReady`, `argo stop -n argo <workflow>` and submit a fresh run so the scheduler can place it on a healthy node (often `ghost`) |
| Template change did not take effect | Run §4 |

If no row matches:

```text
1. just logs
2. argo logs -n argo <workflow-name> --follow
3. argo get -n argo <workflow-name>
```

## 3. Capacity triage — cluster feels slow

```text
1. just list-workflows
2. kubectl top nodes
3. kubectl get vmi -A
4. kubectl get pods -A --field-selector=status.phase=Pending
5. kubectl top pods -A
```

| Symptom | Action |
|---|---|
| Workflows `Pending` | `kubectl top nodes` to identify the current CPU hog before submitting more work |
| Node has `DiskPressure` | Do not submit builds. Inspect PV node affinity and `kube-system/local-path-config`; every eligible node needs an explicit non-root data path and there must be no default root-disk fallback. |
| Many `virt-launcher-*` pods with no corresponding live workflow | `argo submit -n argo --from cronworkflow/orphan-vm-cleanup` |
| BuildGrid queue deep, all bots busy | The grid is saturated, not broken; see [Scale-out](/docs/skills/cluster-tooling/buildstream.md#scale-out). |

Per-template requests and limits live in
[workflow-reference: Resource Profiles](/docs/reference/workflow-reference.md#resource-profiles);
the namespace ceiling is the `argo-quota` ResourceQuota.

## 4. ArgoCD — my template change did not take effect

`testing-lab` syncs `argo/workflow-templates/`; `testing-lab-infra` syncs `manifests/`.

```text
1. git log -1 origin/main -- argo/workflow-templates/<file>
   -> expected: your commit is visible on origin/main.
   -> if not: push first.

2. just argocd-status
   -> expected: `testing-lab` is synced to a revision that matches or post-dates your commit.
   -> if older: just argocd-sync

3. just argocd-status
   -> expected: `testing-lab` is Healthy.
   -> if not Healthy: inspect the reported condition, fix the rejected field in git, push again, then repeat step 2.

4. argo template get -n argo <name>
   -> expected: the new field value is live.
   -> if still old: rerun `just argocd-sync`, wait for health, then re-check.

5. Was the workflow submitted before the reconcile finished?
   -> workflows snapshot the template at submit time.
   -> submit a NEW workflow.
```

Without the `argocd` CLI, request a refresh with
`kubectl -n argocd annotate application testing-lab argocd.argoproj.io/refresh=normal --overwrite`
and read `kubectl get application testing-lab -n argocd -o jsonpath='{.status.sync.status} {.status.health.status}'`.
A PreSync hook looping forever is cancelled with
`kubectl patch application testing-lab -n argocd --type=json -p='[{"op":"remove","path":"/operation"}]'`.

Do **not** `kubectl apply` a rejected WorkflowTemplate.

## 5. Safe cleanup — what you may delete

| Resource | Safe? |
|---|---|
| Test VM with no live workflow | Yes — `kubectl delete vm -n <namespace> <name>` or run `orphan-vm-cleanup` |
| `just delete-vms` | Only for full teardown when you intentionally accept that all test VMs in those namespaces will be deleted |
| Workflows in `argo` | Yes — `just delete-workflows` |

## 6. Self-check before claiming cluster healthy

```bash
1. just argocd-status
2. argo cron list -n argo
3. just list-vms
4. just list-workflows
```

Expected steady state:
- both ArgoCD applications are Synced + Healthy
- all expected CronWorkflows are present
- no idle test VMs remain after workflows finish
- the most recent container-only smoke run is green
