# Ghost Lab Architecture

> This document describes the next-generation Ghost Lab hardware and network topology,
> designed around a distributed Remote Execution (RE) build grid that eliminates both
> the network and disk I/O bottlenecks of traditional monolithic ("fat pod") CI builds.

## Hardware

| Host | Role | Form Factor |
|---|---|---|
| `ghost` | k3s control-plane + KubeVirt compute | Existing workstation |
| `exo-0` | k3s worker | Framework Desktop |
| `exo-1` | k3s worker | Framework Desktop |
| `exo-2` | k3s worker | Framework Desktop |

All three `exo` nodes are **Framework Desktops** — compact, upgradeable machines that
each contribute significant CPU core counts to the cluster's parallel build capacity.

---

## Networking: Dual-Homed k3s

Each node has two active network interfaces, each serving a distinct traffic class.

### Control Plane + LAN — 2.5 GbE

- k3s control-plane traffic (API server, etcd, kubelet heartbeats)
- External LAN access and ingress to cluster services
- Configured via `--node-ip` to bind k3s to the 2.5 GbE address

### Data Plane — 40 Gbps USB4 Daisy-Chain Mesh

- Pod-to-pod (East-West) traffic: build job scheduling, artifact transfer, cache I/O
- All four nodes (`ghost` + `exo-0/1/2`) are connected in a **daisy-chain USB4 mesh**,
  delivering 40 Gbps of bandwidth between every pair of nodes
- Configured via `--flannel-iface` to bind Flannel's VXLAN overlay to the USB4 interface

This separation means a heavy East-West build storm never degrades the control plane
or external access, and the 40 Gbps wire is exclusively available to pod workloads.

---

## Storage

### Physical Media

Each Framework Desktop node carries a **4 TB NVMe drive** formatted with **ZFS**,
with native `zstd` compression enabled. ZFS provides:

- Copy-on-write (CoW) integrity — no silent corruption
- ARC (Adaptive Replacement Cache) — hot data served from RAM automatically
- `zstd` compression — compresses compiler headers and object files transparently,
  effectively multiplying usable throughput and capacity

### Storage Architecture: Two Tiers

#### Tier 1 — Distributed Cache (Longhorn over ZFS)

**Longhorn** runs on top of the ZFS pools to provide highly-available, replicated PVCs
across the cluster. This tier hosts:

- **Zot image registry** — replicated OCI artifact store; survives node failure
- **Buildbarn `bb-storage`** — the CAS (Content Addressable Storage) and AC (Action Cache)
  nodes that form the heart of the RE grid; must be durable and HA

Replication factor is configurable per PVC. ZFS beneath Longhorn adds an extra layer of
on-disk integrity checking and compresses Longhorn replica traffic before it hits the NVMe.

#### Tier 2 — Local Scratch (OpenEBS ZFS LocalPV / `local-path`)

**OpenEBS ZFS LocalPV** (or `local-path` for simpler workloads) provides un-replicated,
bare-metal-speed scratch PVCs pinned to a single node. This tier hosts:

- **Buildbarn `bb-worker`** pods — compilation workers that need maximum sequential write
  throughput for object file staging; replication adds only latency here
- ZFS ARC automatically caches frequently-read compiler headers in RAM, turning repeated
  `#include` chains into sub-microsecond memory reads instead of NVMe seeks

---

## Compute Model: Remote Execution Grid (Buildbarn / BuildGrid)

The cluster runs a full **Remote Execution (RE)** grid compliant with the
[Bazel Remote Execution API](https://github.com/bazelbuild/remote-apis).

### Components

| Component | Role | Storage Tier |
|---|---|---|
| `bb-scheduler` | Evaluates the build DAG; dispatches actions to workers | Stateless |
| `bb-storage` (CAS + AC) | Content-addressable object store; action result cache | Longhorn (HA) |
| `bb-worker` × N | Executes individual compilation actions (cc, link, etc.) | LocalPV (scratch) |

### How a Build Runs

1. The build client (Bazel/Buck2) uploads source digests to `bb-storage` CAS.
2. `bb-scheduler` walks the dependency graph and emits thousands of independent
   compile actions simultaneously.
3. Actions are dispatched over the **40 Gbps USB4 mesh** to `bb-worker` pods
   spread across all nodes.
4. Each worker fetches its inputs from CAS, compiles, and uploads outputs back — all
   over the 40 Gbps wire.
5. Linked outputs and action results are cached in `bb-storage` AC, so unchanged
   actions are cache-hits on subsequent builds.

With 86+ cores available across the cluster, the scheduler can keep every core
saturated on a large C++/Rust codebase simultaneously.

---

## Why This Architecture Eliminates the "Fat Pod" Bottlenecks

A **fat pod** build runs the entire compiler toolchain inside a single Kubernetes pod
on a single node. It suffers from two hard limits:

### Bottleneck 1 — Network I/O

A fat pod can only use one node's network interface. Pulling a large sysroot or image
layer means all traffic flows through a single NIC — typically 1–10 Gbps.

**How the Ghost Lab eliminates it:**  
The RE grid distributes compilation units across all nodes simultaneously. Each worker
fetches only its own slice of the build graph from `bb-storage` CAS, and all inter-node
transfers run over the dedicated 40 Gbps USB4 mesh. Aggregate East-West bandwidth scales
with the number of active workers, not with any single NIC's limit.

### Bottleneck 2 — Disk I/O

A fat pod compiles sequentially (or with limited parallelism) on one node's disk.
Even with a fast NVMe, a large build saturates write bandwidth staging object files,
and the OS page cache on a single node can only absorb so much.

**How the Ghost Lab eliminates it:**  
Each `bb-worker` pod has its own local ZFS NVMe scratch PVC. Object file staging is
spread across all nodes in parallel — no single disk is a bottleneck. ZFS ARC on each
node caches compiler headers in RAM, converting the most-read inputs (libc headers,
C++ STL, Rust libstd) from NVMe reads to memory reads on every worker simultaneously.

### Summary

| Constraint | Fat Pod | Ghost Lab RE Grid |
|---|---|---|
| Max parallel cores | 1 node | 86+ across all nodes |
| Network bandwidth | 1 NIC (≤10 Gbps typical) | 40 Gbps mesh, fully parallel |
| Disk write bandwidth | 1 NVMe | N NVMe drives in parallel |
| Cache misses (headers) | Cold after pod eviction | ZFS ARC warm per node |
| Fault tolerance | Pod crash = full rebuild | AC cache survives; restart from last hit |
| Incremental builds | Full re-exec of changed subtree | AC cache-hit for unchanged actions |

The result is a build cluster where adding a node linearly increases both compute
capacity and I/O bandwidth — without any single-node chokepoint.
