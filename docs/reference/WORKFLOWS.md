# WorkflowTemplates — Agent Contract

This is the canonical interface for driving the lab. Every supported
operation is a single `argo submit --from workflowtemplate/<name> [-p k=v]`
invocation. No bash, no `kubectl apply`, no SSH.

Conventions:

- All templates live in `argo/workflow-templates/*.yaml` and are reconciled
  to namespace `argo` by the ArgoCD `testing-lab` Application.
- Workflow-level parameters listed below are passed via `-p name=value`.
- Prefer the **top-level** pipelines. Supporting templates are called as
  `templateRef` and rarely submitted directly.

---

## Dakota BST builds

`dakota-build-pipeline` is documented in [workflow-reference.md](workflow-reference.md#dakota-build-pipeline).

### `zot-candidate-lifecycle`

Reusable single-lane foundation for immutable local Zot candidates. Call the
`candidate-preflight` template before pushing `candidate-<commit-sha>`; it
rejects reused tags and fails under Zot storage pressure. After that lane's QA
passes, call `promote-candidate` with the candidate digest. It pulls the
manifest back by digest, promotes the same digest to `:testing`, verifies the
target, and attaches the versioned ORAS promotion evidence contract.

The optional `zot-writer-auth` docker config keeps the template compatible with
today's anonymous-write registry and becomes required only at the documented
authentication activation gate. See
[Zot candidate promotion](../ops/zot-candidate-promotion.md).

## CronWorkflows

Lives in `manifests/`, applied via the `testing-lab-infra` ArgoCD app:

| Schedule | Cron | Template called | Purpose |
|---|---|---|---|
| `orphan-vm-cleanup` | every 30 min | inline | Delete KubeVirt test VMs whose parent workflow is gone or terminal |

---

## KubeStellar workflows

Reusable WorkflowTemplates in `argo/workflow-templates/`, reconciled by the
`testing-lab` ArgoCD Application. KubeStellar installation and upgrades are
owned by the `kubestellar-applications` ArgoCD parent Application.

### `register-wec`

| Parameter | Default | Notes |
|---|---|---|
| `wec-name` | `ghost` | Cluster name to register with the its1 OCM hub. Labels the ManagedCluster `name=<wec>` after accept. |

SA: `kubestellar-bootstrap` (cluster-admin; klusterlet install writes CRDs).

### `kubestellar-smoke-test`

| Parameter | Default | Notes |
|---|---|---|
| `wec-name` | `ghost` | Target WEC. Verifies BindingPolicy downsync and singleton status upsync via wds1 (`kubeconfig-incluster` key), then cleans up. |

SA: `kubestellar-bootstrap`. Acceptance gate after any core upgrade.

### `kubestellar-platform-verify`

Ordered, read-only platform gate:

```text
verify-datasource → verify-query-surfaces → verify-controller-wiring
  → kubestellar-smoke-test
```

The first three tasks verify Console kubeconfig wiring, real KubeStellar/OCM
API surfaces, and controller wiring. The referenced smoke template is the only task that creates
resources, and it uses only its existing ephemeral BindingPolicy, Namespace,
and Deployment.

| Parameter | Default | Notes |
|---|---|---|
| `wec-name` | `ghost` | Passed explicitly to the final smoke gate. |

Run with `just run-kubestellar-verify`. Read-only checks use the scoped
`kubestellar-observability` ServiceAccount. Argo `templateRef` does not inherit
the referenced WorkflowTemplate's workflow-level identity, so the smoke
template declares `kubestellar-bootstrap` at template level.

---

## Editing this contract

When you add or rename a template, update this file in the same commit. Drift
between templates and this doc is what breaks autonomous agents.