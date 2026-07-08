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

> **STATUS (2026-07-08, re-verified): link still down — NOT a cable problem.** Owner
> confirmed the physical USB4 wire is fine on both nodes. `exo-0` still has no
> `thunderbolt0` interface at all (`ip link show thunderbolt0` → "Device does not
> exist"; `ip -br addr` shows only `enp191s0` + `cni0`). `/sys/bus/thunderbolt/devices/`
> shows `exo-0`'s own host router present and `authorized=1` (`c5:00.6`, driver
> `thunderbolt`, normal `ACPI: USB4 _OSC` + `usb usb4` init lines in `journalctl -k`
> at boot), but no downstream peer entry ever appears — `exo-0` never sees `ghost`
> enumerate as a remote Thunderbolt/XDomain peer, cable or no cable. A direct ping
> from `exo-0` to `ghost`'s previously-observed USB4 link-local address
> (`169.254.79.234`) still fails 100%. This is still not the known ASPM/power-suspend
> failure mode (RUNBOOK.md) — forcing `power/control=on` on PCIe bridge `0000:00:08.3`
> and controllers `c5:00.5`/`c5:00.6` had already proven ineffective. Given the cable
> itself is confirmed good, the remaining suspects are all on the **`ghost` side**:
> wrong physical port (must be the certified USB4/TB4 port, not a plain USB-C port),
> port not authorized, or BIOS/EC state disabling the controller on that host. A
> read-only SSH probe to `ghost` still returns `Connection refused`, so this session
> could not inspect `ghost`'s own thunderbolt kernel/dmesg state to confirm which of
> these it is. **Until both hosts re-enumerate the USB4 peer and point-to-point ping
> succeeds again, treat this link as aspirational/blocked-on-`ghost`-side-config, not
> a currently usable production data path.** All Buildbarn cross-node gRPC traffic
> (frontend↔worker, worker↔storage) currently rides the shared Ethernet LAN like
> everything else; flannel is not pinned to `thunderbolt0` (see below), so no build
> traffic is currently USB4-accelerated even when the link is physically up.

- `ghost`'s `thunderbolt0` (169.254.79.234/16) and `exo-0`'s `thunderbolt0`
  (169.254.238.103/16) formed a direct point-to-point USB4 link — confirmed via live ping
  at an earlier date, not a switched/broadcast segment, and not shared with any other node
- Intended for pod-to-pod (East-West) traffic between these two nodes specifically:
  build artifact transfer, cache I/O
- Must be scoped per-node-pair (e.g. via node annotations), **not** via a cluster-wide
  `--flannel-iface=thunderbolt0` change — that setting previously broke connectivity for
  every node without a live physical USB4 link (see RUNBOOK.md:108-116)
- **No such per-node-pair scoping has been implemented yet** — even when the physical
  link is up, nothing in this repo currently routes Buildbarn or any other pod traffic
  over it. The gap is real and unaddressed, not just currently broken.

This separation means heavy East-West traffic between `ghost` and `exo-0` doesn't need to
compete with control-plane/API traffic on the shared Ethernet LAN — but only once both
the physical link is restored and an actual traffic-steering mechanism (e.g. a dedicated
CNI route entry, a NetworkAttachmentDefinition + Multus, or an /etc/hosts-style override
pointing Buildbarn's frontend/worker Service traffic at the thunderbolt0 IPs) is built.
Neither exists today.

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
