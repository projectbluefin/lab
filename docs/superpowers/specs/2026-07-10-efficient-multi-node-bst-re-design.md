# Efficient Multi-Node BuildStream RE and Semaphore Scaling

Date: 2026-07-10
Status: superseded

> Superseded by the USB4-only BuildStream policy. Current worker capacity and
> admission rules live in `/agents.md` and `/docs/skills/cluster-tooling/SKILL.md`.

## Problem

With the expansion of the homelab cluster to a 3-node USB4 private mesh network (10.99.0.x), the previous sequential-queuing model (where the `bst-build` semaphore is capped at 1 and client control-loop pods request 12-16 GiB of RAM) is highly inefficient:
1. **Unnecessary Scheduling Bottlenecks:** Main workflow executor pods request heavy resource footprints (12-16 GiB RAM) even when they are running in Remote Execution (RE) mode and offloading all compilation to Buildbarn worker pods. This causes Kubernetes to reject pods with `Insufficient memory` even when actual node CPUs and RAM are mostly idle.
2. **Lack of Concurrency:** Serializing all builds under `bst-build=1` prevents the cluster from utilizing its multi-node Buildbarn worker grid simultaneously.
3. **Inconsistent Fallbacks:** `cosmic` and `bluefin-server` pipelines do not use the `bst-build` semaphore or the USB4 detection fallback, leading to potential node exhaustion and network saturation on 1 GbE during transient link failures.

## Proposed Solution: The "Ultra-Lean Coordinator" Grid

We will transition the cluster to a model of **"macro-concurrency, micro-distribution"**:
1. **Split Templates:** Split the `bst-build` template in each pipeline into two parallel, conditional templates:
   - `bst-build-re` (Remote Execution Coordinator): Requests `1 CPU` and `2Gi RAM`. It acts as a lightweight gRPC coordinator. It handles checkout, appends the remote-execution config, runs `bst build` (offloading work to Buildbarn), and exports/pushes the resulting image.
   - `bst-build-local` (Ethernet Fallback / Cache-Only): Requests `8 CPU` and `12-16Gi RAM`. It compiles everything locally when the USB4 link is down or during retries (`{{retries}} > 0`).
2. **Semaphore Scaling:** Scale the `bst-build` semaphore limit to **3** (matching the 3-node USB4 cluster scale).
3. **Pipeline Harmonization:** Harmonize `cosmic-build-pipeline` and `bluefin-server-build-pipeline` to use:
   - The shared `bst-build` semaphore.
   - The `detect-build-mode` task.
   - The split-template branching logic.

## Detailed Design

### 1. ConfigMap Semaphore Scaling (`manifests/workflow-semaphores.yaml`)

Increase `bst-build` capacity to `3` to allow parallel coordinator orchestration while preventing unmanaged thrashing:

```yaml
data:
  qa-vm-fleet: "1"
  containerdisk-build: "2"
  bst-build: "3"
```

### 2. DAG Conditional Branching (`workflow-templates`)

Each of the three templates (`dakota-build-pipeline`, `cosmic-build-pipeline`, `bluefin-server-build-pipeline`) will implement the same DAG structure:

```yaml
dag:
  tasks:
    - name: detect-build-mode
      template: detect-build-mode
    - name: build-re
      template: bst-build-re
      depends: detect-build-mode
      when: "{{tasks.detect-build-mode.outputs.parameters.mode}} == re"
      # ... arguments ...
    - name: build-local
      template: bst-build-local
      depends: detect-build-mode
      when: "{{tasks.detect-build-mode.outputs.parameters.mode}} == cache-only"
      # ... arguments ...
```

### 3. Resource Allocation Details

#### `bst-build-re` (Remote Execution Coordinator)
```yaml
resources:
  requests:
    cpu: "1"
    memory: 2Gi
  limits:
    cpu: "2"
    memory: 4Gi
```

#### `bst-build-local` (Local Fallback Build)
```yaml
resources:
  requests:
    cpu: "8"
    memory: 12Gi  # (or 14Gi/16Gi depending on pipeline-specific profiles)
  limits:
    cpu: "12"
    memory: 16Gi  # (or 28Gi/30Gi limits)
```

## Rollout Plan

1. **Deploy config changes:** Update `manifests/workflow-semaphores.yaml` to scale the semaphore.
2. **Refactor pipelines:** Update `dakota-build-pipeline.yaml`, `cosmic-build-pipeline.yaml`, and `bluefin-server-build-pipeline.yaml`.
3. **Lint & Validate:** Run `just lint` to ensure syntactical validity of all Argo Workflows and manifests.
