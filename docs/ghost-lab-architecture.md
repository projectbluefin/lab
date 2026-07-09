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

> **STATUS (2026-07-09): LINK UP AND ROUTED.** After the 2026-07-08 EC/PD failure
> (see RUNBOOK.md), a full power drain of both nodes restored the physical link.
> `thunderbolt0` is up on both hosts, static IPs and table-40 policy routing are
> applied and persisted in NetworkManager, and cross-node pod traffic is confirmed
> flowing over USB4 (~0.16 ms pod-to-pod RTT).
>
> **Important Hardware Slot Mapping**: On AMD Framework laptops (both 13 and 16), only **Slot 1 (back left)** and **Slot 4 (back right)** support USB4 / Thunderbolt 4. If the cable is plugged into Slot 2 or Slot 3, the link state will remain `none` indefinitely. Ensure the USB4 cable is connected strictly to Slot 1 or Slot 4 on both machines to bring the link up and initialize `thunderbolt0`. On Framework Desktops all rear Type-C ports are USB4.
> 
> The complete network design for East-West CNI route separation is fully specified below.

- `ghost`'s `thunderbolt0` (static IP `10.99.0.1/30`) and `exo-0`'s `thunderbolt0`
  (static IP `10.99.0.2/30`) form a direct, switchless point-to-point USB4 link (40 Gbps).
  The IPs are persisted in NetworkManager profiles (`Wired connection 1` on ghost,
  `Wired connection 2` on exo-0) with `ipv4.method manual`, so they survive reboots.
- Intended for pod-to-pod (East-West) traffic between these two nodes specifically:
  build artifact transfer, cache I/O.
- **Traffic Steering Design (Linux Policy Routing)**: Since the default k3s flannel CNI is
  configured with `flannel-backend: host-gw` (Host Gateway, no VxLAN encapsulation), routing
  can be optimized natively at the host kernel layer using policy routing rules (`ip rule`).
  This completely isolates pod-to-pod build traffic over USB4 while guaranteeing that k3s
  control-plane, API server, and etcd traffic remains strictly on the Ethernet LAN.
  
  **Deployed configuration (persisted in NetworkManager, survives reboots)**:

  1. Static IPs live in the NM profiles (`ipv4.method manual`):
     - ghost (`Wired connection 1`): `10.99.0.1/30`
     - exo-0 (`Wired connection 2`): `10.99.0.2/30`

  2. Policy routing on `ghost` routes the `exo-0` pod subnet (`10.42.1.0/24`) over USB4:
     ```bash
     sudo nmcli con mod "Wired connection 1" \
       +ipv4.routes "10.42.1.0/24 10.99.0.2 table=40" \
       +ipv4.routing-rules "priority 5209 to 10.42.1.0/24 table 40"
     ```

  3. Policy routing on `exo-0` routes the `ghost` pod subnet (`10.42.0.0/24`) over USB4:
     ```bash
     sudo nmcli con mod "Wired connection 2" \
       +ipv4.routes "10.42.0.0/24 10.99.0.1 table=40" \
       +ipv4.routing-rules "priority 5209 to 10.42.0.0/24 table 40"
     ```

  Verify with `ip route get 10.42.1.10` on ghost — it must show
  `via 10.99.0.2 dev thunderbolt0 table 40`.

  Since these rules are evaluated in custom routing table `40` before the `main` table, they override
  flannel's default Ethernet routes for pod-to-pod transit between the two nodes, while remaining
  completely invisible and immune to flannel/k3s route reconciliation. If any other node joins
  the cluster over the LAN, its traffic continues over Ethernet, preventing any route isolation.


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
test pods across whichever nodes are schedulable.

A Buildbarn Remote Execution (REAPI) grid **is** deployed, in the `buildbarn` namespace,
and is the canonical build cache/remote-exec mechanism for BuildStream (`bst`) jobs:

- `frontend` — Deployment, 2 replicas, **preferred** podAntiAffinity (degrades gracefully
  to co-location if a node is unavailable). Single entrypoint at
  `frontend.buildbarn.svc.cluster.local:8980` for artifact cache, source-cache, and
  remote-execution traffic from `bst` clients.
- `scheduler` — Deployment, 1 replica (single point of failure by design; see
  `production-hardening-*` follow-ups).
- `storage` — StatefulSet, 2 replicas, **required** podAntiAffinity + per-pod local-path
  PVC pinned via node affinity, one replica per node (`ghost` + `exo-0`).
- `worker` — DaemonSet, one pod per schedulable node, each with a `runner` sidecar
  (`CAP_SYS_CHROOT`) that executes BuildStream REAPI actions dispatched by the scheduler.
- `bb-remote-asset` — Deployment, source/asset resolution.

BuildStream jobs (`dakota-build-pipeline`, `cosmic-build-pipeline`,
`bluefin-server-build-pipeline`, `bst-qa-pipeline`, `dakota-buildstream-warm-cache`) point
their `artifacts`, `source-caches`, and `remote-execution` config at the shared frontend,
so build actions are genuinely distributed across the `worker` DaemonSet pods on both
`ghost` and `exo-0` rather than executing locally on a single node. The `bst-build` client
pod itself also carries a required podAntiAffinity so concurrent builds spread across
nodes instead of stacking on one.
