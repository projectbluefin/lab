# Ghost Lab Architecture

> This document describes the **current, physically-verified** Ghost Lab hardware and
> network topology. Only `ghost` and `exo-0` have a live point-to-point USB4 link today.
> Any multi-node daisy-chain mesh, ZFS storage tier, or Remote Execution grid described
> in earlier drafts of this document was aspirational and did not match deployed hardware
> — it has been removed. Extend this doc only after physically verifying new links/nodes
> (see RUNBOOK.md:108-116 for the incident this caused previously).

## Hardware

| Host | Role | Form Factor |
|---|---|---|
| `ghost` | k3s control-plane + KubeVirt compute | Existing workstation |
| `exo-0` | k3s worker | Framework Desktop |

Other opt-in workers (`exo-1`, `exo-2`, `hamilton`) join the cluster over
standard Ethernet only — none currently have a physical USB4/Thunderbolt link to `ghost`.

---

## Networking: Dual-Homed k3s (ghost + exo-0 only)

`ghost` and `exo-0` each have two active network interfaces:

### Control Plane + LAN — Gigabit Ethernet

- k3s control-plane traffic (API server, etcd, kubelet heartbeats)
- External LAN access and ingress to cluster services
- All other nodes reach the cluster exclusively over this Ethernet LAN

### Data Plane — Point-to-Point USB4 Link (ghost <-> exo-0 only)

- `ghost`'s `thunderbolt0` (169.254.79.234/16) and `exo-0`'s `thunderbolt0`
  (169.254.238.103/16) form a direct point-to-point USB4 link — confirmed via live ping,
  not a switched/broadcast segment, and not shared with any other node
- Intended for pod-to-pod (East-West) traffic between these two nodes specifically:
  build artifact transfer, cache I/O
- Must be scoped per-node-pair (e.g. via node annotations), **not** via a cluster-wide
  `--flannel-iface=thunderbolt0` change — that setting previously broke connectivity for
  every node without a live physical USB4 link (see RUNBOOK.md:108-116)

This separation means heavy East-West traffic between `ghost` and `exo-0` doesn't need to
compete with control-plane/API traffic on the shared Ethernet LAN.

---


---

## Storage (as deployed today)

### ghost

- `nvme1n1` — system disk, btrfs (ostree/bootc root + `/var` + `/var/home`)
- `nvme0n1` ("ghost-data", 3.7T) — workload/scratch storage, formatted **XFS** (migrated
  from btrfs 2026-07-03; XFS chosen for lower per-file metadata overhead on the CAS
  workload below, and to avoid btrfs COW/checksum overhead on data that doesn't need it)
  - `zot-local/` — Zot registry local image store (durable; backed up before any reformat)
  - `local-path/` — k3s `local-path-provisioner` PVC scratch storage, including the
    BuildStream `buildbox-casd` CAS cache (~1.3M small objects, sharded `00`-`ff`) — the
    only `local-path` content treated as durable; other PVCs here are disposable
    build-job scratch data and are not backed up

### exo-0 / other workers

Local disk only, no shared/replicated storage layer. PVCs are node-local
(`local-path-provisioner`); there is no Longhorn, ZFS, or cross-node replication in this
cluster today.

---

## Compute Model

Standard k3s workload scheduling — Argo Workflows dispatch KubeVirt VM provisioning and
test pods across whichever nodes are schedulable. There is no Buildbarn/BuildGrid Remote
Execution grid deployed. BuildStream (`bst`) jobs use the local `buildbox-casd` cache on
`ghost` directly; they are not distributed across nodes as independent remote-exec
actions.
