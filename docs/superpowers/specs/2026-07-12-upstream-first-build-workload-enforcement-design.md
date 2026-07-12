# Upstream-First Build Workload Enforcement Design

## Goal

Restore Kubernetes as the sole admission and placement authority for build and
CI workloads. Dakota and Buildbarn must not depend on root-backed storage,
hard node placement, host namespaces, or undeclared local-disk consumption.

## Scope

This design applies to the `argo` and `buildbarn` build/CI paths:

- Dakota and other BuildStream workflow templates.
- Buildbarn worker, runner, and storage configuration.
- Build-related admission, resource, and repository validation.

It does not change node-management DaemonSets in `kube-system`. Those
components may require host access for their stated node-management purpose and
need a separate evidence-based review.

## Verified Starting Point

- `local-path` uses `WaitForFirstConsumer`; Kubernetes provisions the selected
  volume only after scheduler placement. Its GitOps configuration maps only
  `ghost` and `exo-0` to non-root data mounts and has no default mapping.
- Buildbarn's worker currently stores a 20 GiB local CAS under
  `/var/tmp/bb-local-cas` through `hostPath`, which bypasses Kubernetes volume
  lifecycle and root-disk safeguards.
- Each Buildbarn runner currently requests 16 CPU and 32 GiB while registering
  two remote-execution slots. That capacity claim is not compatible with the
  available two-node build budget and leaves core Buildbarn services
  unschedulable or evicted.
- Build and CI templates still contain cache `hostPath`, node placement, and
  unbounded or insufficiently accounted local scratch patterns.

## Target Architecture

### Scheduler and storage

1. Every persistent build workspace uses a `local-path` PVC.
2. Every eligible node is explicitly mapped to a non-root data mount in
   `manifests/local-path-config.yaml`; unlisted nodes fail provisioning.
3. PVC consumers use `WaitForFirstConsumer`. No build template selects a node
   by hostname or adds node affinity to steer local storage.
4. Buildbarn's root-backed local CAS is removed. Its durable shared storage
   remains PVC-backed and is the source of truth.
5. Per-pod temporary directories use `emptyDir.sizeLimit`, and their containers
   declare matching `ephemeral-storage` requests and limits.

### Capacity and concurrency

1. Buildbarn runner concurrency is reduced to the number of slots supported by
   the admitted per-node resource budget.
2. Runner and worker CPU, memory, and ephemeral-storage requests describe the
   actual maximum work they can execute.
3. Dakota and other BST lanes use template-level semaphores, PriorityClass, and
   declared requests/limits. They do not use placement workarounds to escape
   scheduler decisions.
4. Cache-only Dakota remains the default until a separately verified
   remote-execution capacity profile is available.

### Prevention

1. Kubernetes `ValidatingAdmissionPolicy` resources bind only to the build/CI
   namespaces. They roll out with `Warn`, then use `Deny` after the live
   workload inventory is clean.
2. The policies reject `hostPID`, `hostIPC`, hostname `nodeSelector`, and
   root-backed build-cache `hostPath` volumes. A narrowly defined read-only
   `/dev/fuse` exception is permitted only if template and live-workload
   verification prove it is required.
3. Repository lint/tests enforce the same prohibitions before GitOps
   reconciliation. They also require bounded `emptyDir` volumes and
   `ephemeral-storage` requests and limits for build containers.

## Migration Sequence

1. Record live resource allocation, eviction events, PVC topology, and
   Buildbarn execution-slot state.
2. Quiesce only proven harmful build pollers or workflows; preserve PVC and CAS
   data.
3. Remove the Buildbarn local root `hostPath`, set a scheduler-admitted worker
   resource profile, and adjust the Buildbarn slot count to match it.
4. Convert remaining BuildStream cache paths to workflow PVCs and bound all
   temporary storage.
5. Add repository checks and the admission policy in `Warn` mode.
6. Reconcile through ArgoCD, inspect warnings and live workload specs, then
   change the policy action to `Deny`.
7. Submit one cache-only Dakota build. It must complete without node pressure,
   eviction, unmanaged storage, or admission-policy violations.

## Failure Handling

- A PVC that cannot provision on an unconfigured node is a deliberate
  fail-closed error. Add the node's verified non-root data mount through GitOps;
  never add a default path or a node selector.
- A workload rejected by the policy is fixed in its tracked template. It is not
  bypassed by a manual apply, exception label, or host-mounted cache.
- Existing PVC/PV/CAS data is not deleted or moved without explicit approval.
- A resource profile that cannot be admitted is reduced in concurrency or
  resource use; it is not pinned to a preferred node.

## Validation

1. `just lint` and targeted repository policy tests pass.
2. ArgoCD reconciles the manifests and templates from Git.
3. The live admission policy emits no build/CI warnings before `Deny` is
   enabled.
4. `kubectl get pods -A` shows Buildbarn services and workers Ready with no
   eviction or pending reason caused by resource pressure.
5. Nodes are Ready with `DiskPressure=False`.
6. A fresh cache-only Dakota workflow succeeds and publishes its expected
   output.

## Sources

- Kubernetes StorageClass volume binding mode:
  `/kubernetes/website`, “Volume binding mode”. `WaitForFirstConsumer` delays
  provisioning until the consuming Pod is scheduled, considering scheduling
  constraints.
- Kubernetes resource requests and limits:
  `/kubernetes/website`, “Resource requests and limits”. Requests inform
  scheduler placement and limits constrain kubelet-enforced consumption.
- Kubernetes local ephemeral-storage accounting:
  `/kubernetes/website`, “Configure Pod with Ephemeral Storage Requests and
  Limits”.
- Kubernetes ValidatingAdmissionPolicy:
  `/kubernetes/website`, “Validating Admission Policy”. Namespace-selected
  bindings can begin with `Warn` and later enforce `Deny`.
