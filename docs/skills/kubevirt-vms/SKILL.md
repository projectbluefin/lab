---
name: kubevirt-vms
description: >
  KubeVirt ephemeral VM lifecycle in the lab: VM provisioning, boot checks,
  teardown. Use when writing boot-test templates, debugging VM boot failures,
  or working with KubeVirt manifests.
metadata:
  context7-sources:
    - /kubevirt/kubevirt
    - /kubevirt/user-guide
    - /kubevirt/containerized-data-importer
---

# KubeVirt VMs — lab Skill

## When to Use

- Editing `bluefin-server-boot-test.yaml` or other boot-test VM templates
- Debugging VM boot timeouts
- Enabling a new KubeVirt feature gate
- Understanding why a VM is stuck `Terminating`

## When NOT to Use

- Argo Workflows YAML syntax issues → `argo-workflows.md`
- ArgoCD sync problems → `gitops-argocd.md`

## Core Process

- [VM lifecycle](vm-lifecycle.md) — disks, scheduling, teardown.

## Console-driven VM control (imperative path)

The KubeStellar Console ServiceAccount can start/stop/restart VMs
imperatively via `patch virtualmachines` (ClusterRole
`kubestellar-console-lab-surfaces`, see `console-dashboard/SKILL.md`).
Verified live (2026-07-25): a probe VM was started and stopped via
`kubectl patch vm <name> --as=system:serviceaccount:kubestellar-console:kubestellar-console
--type=merge -p '{"spec":{"running":true}}'`; the VMI scheduled on exo-0
with no pinning.

- `spec.running` is deprecated (warning emitted); prefer
  `spec.runStrategy: Always|Halted` in new manifests. Both work today.
- Serial/graphical access: `virtctl console <vm>` / `virtctl vnc <vm>`
  from a workstation with cluster access — the Console GUI does not
  proxy VNC; virtctl is the documented fallback.
- VM *definitions* remain GitOps (manifests in git); only runtime
  start/stop/restart is imperative. Ephemeral test VMs created by
  workflows are exempt (owned by the workflow, cleaned by onExit and
  `orphan-vm-cleanup`).

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "I'll keep the VM up between runs to save time." | No persistent test VMs. The `orphan-vm-cleanup` CronWorkflow will delete it. |
| "The teardown step can be optional." | A missing `onExit` handler leaks VMs and disk clones on failure. Always required. |
| "VMs must pin to ghost." | No VM type requires a ghost pin. VMs use containerDisk or PVC. Adding a node requires no YAML changes. |
| "Workflow pods need hostNetwork + nodeSelector: ghost to reach VMs." | False. Pod IPs route across nodes via flannel. kubectl exec works from any node. hostNetwork was a workaround for broken exo-1 flannel — not a KubeVirt requirement. |
| "The zot image from yesterday is still there." | Zot-writable loses its index.json on pod restart. Always check before running the pipeline. |
| "HostDisk feature gate is probably already on." | Verify with `kubectl get kubevirt kubevirt -n kubevirt -o jsonpath='{.spec.configuration}'`. Don't assume. |
| "inotify limits are a kernel concern, not a k8s concern." | KubeVirt virt-handler + containerd exhaust defaults at scale. The `inotify-tuning` DaemonSet is required. |

## Red Flags

- A VM provision template with `nodeSelector: kubernetes.io/hostname: ghost` — no VM type requires this anymore (no hostDisk VMs remain)
- An `onExit` handler that doesn't delete the VM object
- Hardcoded IPs in VM templates (use pod IP from `kubectl get pod -l kubevirt.io/vm=...`)
- **Any hostPath for VM disks** — use a containerDisk or PVC instead
- `registry.k8s.io/kubectl` used as image for a step that needs bash — it is distroless, use `cgr.dev/chainguard/kubectl:latest-dev`
- VM boot timeout with no disk or network explanation — check `cat /proc/sys/fs/inotify/max_user_watches` (should be >= 1048576)
- VM goes `Stopped` with `FailedCreate` and `metadata.labels: must be no more than 63 characters` — VM name exceeds Kubernetes label-value limit. Use short, unique names such as `{{workflow.name}}-{{item}}`.
- Orphaned VMs from a prior workflow consuming ghost resources — run `just list-vms` before submitting a new run; delete orphans with `kubernetes-mcp-resources_delete` if present.
- **VM immediately fails with `No disk capacity`** — virt-launcher was evicted because its `ephemeral-storage` limit was exceeded during disk extraction. This usually means `disk.Capacity == nil` in KubeVirt because the containerDisk image is missing the `/containerDisk.json` metadata file. See [VM lifecycle](vm-lifecycle.md) section 1.

## Verification

Before merging any VM provisioning change:

- [ ] No VM provision template has `nodeSelector: kubernetes.io/hostname: ghost` — no hostDisk VMs remain; all types float freely
- [ ] No `hostNetwork: true` or `nodeSelector: ghost` on kubectl workflow step pods — flannel handles routing
- [ ] `onExit` teardown deletes VM object; containerDisk teardown is VM-delete only; PVC-backed teardown also deletes the PVC
- [ ] Feature gates checked if adding a new VM capability
- [ ] `just list-vms` shows empty after workflow completion
- [ ] VM disks use containerDisk or PVC volumes; no VM workflow relies on hostPath storage
- [ ] No hardcoded IPs — pod IP derived at runtime via `kubectl get pod`
- [ ] **containerDisk builds**: Image contains `/containerDisk.json` with the correct `capacity` explicitly declared (prevents `No disk capacity` limits)
