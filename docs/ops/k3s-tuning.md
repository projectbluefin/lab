# k3s Control Plane Tuning — ghost

Canonical reference for tuning the k3s server on ghost.
Applied once via `/etc/rancher/k3s/config.yaml` and a service restart.
This is not a GitOps manifest — it lives outside ArgoCD's reconciliation scope.

> [!CAUTION]
> This is a private maintainer/operator procedure for host-level maintenance,
> not a public agent recipe. Do not SSH from a workstation to `ghost` or
> `exo-0`; use the approved private maintenance channel. Routine cluster
> inspection and workload control remain API-driven through `just`, `argo`, and
> `kubectl`.

---

## Why

k3s ships with defaults sized for general-purpose clusters. Ghost runs a
five-node homelab cluster (one control-plane + two current workers + up to three
planned Framework desktops) under a KubeVirt + Argo Workflows workload that
creates and destroys many objects quickly. Three defaults cause measurable waste:

**API server watch-cache** is unbounded. Every informer — and ArgoCD, Argo
Workflows, and KubeVirt each run several — reserves cache memory proportional to
object churn. A bounded cache of 100 entries per resource type is sufficient for
this object volume and dramatically reduces idle RSS.

**etcd** does not compact or snapshot automatically. On a busy cluster, the
keyspace grows without limit until etcd slows or hits its quota. Periodic
compaction every hour and a 4 GiB quota cap keep the database healthy without
manual intervention.

**Controller-manager** defaults to 20 concurrent sync workers for Deployments
and ReplicaSets. For five nodes that is 4x more goroutines than useful, burning
CPU and scheduler overhead during Argo burst phases.

---

## Configuration

Place the following block in `/etc/rancher/k3s/config.yaml` on ghost. The file
is cumulative — if it already exists, merge rather than replace.

```yaml
# /etc/rancher/k3s/config.yaml on ghost
# Apply once; requires: sudo systemctl restart k3s
# Workers reconnect automatically within ~30s.

kube-apiserver-arg:
  - "default-watch-cache-size=100"
  - "event-ttl=30m"
  - "max-requests-inflight=100"
  - "max-mutating-requests-inflight=50"
  - "profiling=false"

kube-controller-manager-arg:
  - "leader-elect=false"
  - "node-monitor-period=60s"
  - "node-monitor-grace-period=180s"
  - "concurrent-deployment-syncs=2"
  - "concurrent-replicaset-syncs=2"
  - "concurrent-statefulset-syncs=1"
  - "concurrent-gc-syncs=2"

etcd-arg:
  - "auto-compaction-mode=periodic"
  - "auto-compaction-retention=1h"
  - "quota-backend-bytes=4294967296"
  - "heartbeat-interval=250"
  - "election-timeout=2500"

etcd-snapshot-schedule-cron: "0 */12 * * *"
etcd-snapshot-retention: 5
etcd-snapshot-compress: true
```

**Argument notes:**

| Argument | Value | Rationale |
|---|---|---|
| `default-watch-cache-size` | 100 | Caps per-resource watch-cache entries; still fast for informers on a small cluster |
| `event-ttl` | 30m | Default 1h; events are noisy and short-lived — Argo logs are the durable record |
| `max-requests-inflight` | 100 | Default 400; adequate for five nodes + Argo burst |
| `max-mutating-requests-inflight` | 50 | Default 200; adequate for VM create/delete cycles |
| `profiling` | false | Saves a small amount of CPU; no pprof endpoint needed in production |
| `leader-elect` | false | Single control-plane; disabling saves lease churn |
| `node-monitor-grace-period` | 180s | 3× the monitor period; avoids false `NotReady` on slow VM create phases |
| `concurrent-*-syncs` | 1–2 | Right-sized for five nodes; reduces goroutine pressure during Argo bursts |
| `auto-compaction-retention` | 1h | Compact every hour; prevents keyspace bloat from ephemeral VM objects |
| `quota-backend-bytes` | 4 GiB | Hard cap; etcd logs a warning at 80%; alerts before it blocks writes |
| `heartbeat-interval` | 250ms | Default 100ms; reduces etcd election noise on a LAN |
| `election-timeout` | 2500ms | 10× heartbeat; standard ratio for stable single-node etcd |

---

## Apply Procedure

1. In the approved private host-maintenance session (not workstation SSH), edit
   the file:

   ```
   sudo nano /etc/rancher/k3s/config.yaml
   ```

   Merge the block above. If the file does not exist, create it.

2. Restart k3s:

   ```
   sudo systemctl restart k3s
   ```

   The API server is unavailable for roughly 10–15 seconds while etcd and the
   API server restart. Workers lose contact and reconnect automatically; expect
   all nodes `Ready` again within 30 seconds.

3. Verify:

   ```
   kubectl get nodes
   ```

   All nodes should report `Ready`. If a worker does not recover within 60
   seconds, check `journalctl -u k3s-agent -n 50` on that worker.

4. Confirm etcd compaction is active:

   ```
   kubectl -n kube-system logs -l component=etcd --tail=20 | grep compact
   ```

   You should see periodic compaction log lines within the first hour.

---

## What Was Skipped and Why

**`--watch-cache=false`** — Disabling the watch cache entirely forces every
informer to bypass the in-memory cache and read directly from etcd. ArgoCD,
KubeVirt, and Argo Workflows each hold persistent watches across many resource
types; disabling the cache would significantly increase etcd read load and
latency. Bounding the cache size (`default-watch-cache-size=100`) achieves the
memory goal without the etcd penalty.

**Disabling specific controllers** (`--controllers=-attachdetach,-pv-binder`) —
KubeVirt requires both the `attachdetach` and `pv-binder` controllers to manage
disk attachment for VMs. These cannot be disabled without breaking VM
provisioning.

**Cilium CNI migration** — Replacing Flannel with Cilium would reduce kube-proxy
overhead and add eBPF-based network policy. For a five-node homelab under this
workload, the operational complexity of a full CNI migration is not justified by
the marginal gain. Revisit if the cluster grows beyond ten nodes or network
policy becomes a requirement.

**`--disable-kube-proxy`** — Paired with Cilium; not applicable while Flannel is
the CNI.

---

## Pod MTU 9000 — both nodes

Cross-node pod traffic (CAS between BuildGrid workers and Buildbarn, Zot pushes)
is forwarded over `thunderbolt0`. flannel host-gw takes its MTU from the default
route interface (`enp191s0`, 1500), and thunderbolt-net is packet-rate bound at
that size. Measured with `thunderbolt0` TSO off:

| Path | MTU 1500 | MTU 9000 |
|---|---|---|
| host → host over `thunderbolt0` | 5.2 Gb/s | 8.5 Gb/s |
| pod → pod over `thunderbolt0` | 4.8 Gb/s | 8.6 Gb/s |
| host → host over `enp191s0` | 2.4 Gb/s | — |

Each node carries, outside GitOps:

```yaml
# /etc/rancher/k3s/config.yaml (ghost: k3s, exo-0: k3s-agent)
flannel-cni-conf: /etc/rancher/k3s/flannel-cni.conflist
```

`/etc/rancher/k3s/flannel-cni.conflist` is the k3s-generated conflist with
`"mtu": 9000` added to the flannel `delegate`. `usb4-link-monitor` keeps
`thunderbolt0` at MTU 9000. Traffic that leaves through `enp191s0` (internet,
LAN, USB4-down fallback) gets ICMP fragmentation-needed from the host, so TCP
path-MTU discovery shrinks it to 1500.

Apply without killing running pods: write both files, `systemctl restart k3s`
(or `k3s-agent`; running containers survive), `ip link set cni0 mtu 9000`,
then roll only the workloads that should get 9000-byte veths. Pods created
after the restart get MTU 9000 automatically. Do not use `k3s-killall.sh` for
this: it kills the control plane, Zot, and BuildGrid's Postgres. A new node
needs the same two files before it carries pod traffic over USB4.

---

## Container task limit — both nodes

k3s runs kubelet and containerd with the systemd cgroup driver, so every
container is a transient `cri-containerd-<id>.scope` under its pod slice.
Kubernetes sets no pids limit, so systemd gives each scope `DefaultTasksMax`:
15% of `min(kernel.pid_max, kernel.threads-max)`, 76549 on ghost and 76120 on
exo-0. That is the only binding limit: the pod slice is `max`, `kubepods.slice`
is `threads-max`, and the k3s unit's `TasksMax` does not cover pods. A GNOME
recc fan-out holds one casd thread per in-flight RPC plus every recc and
compiler process in the BuildGrid worker container; it reached the limit,
`buildbox-worker` died with `Resource temporarily unavailable`, and all 32
running actions on the node failed.

A kubelet `podPidsLimit` cannot raise this (it sets the pod slice, not the
scope), and the manifest has no field for it. A prefix drop-in for the
container scopes raises the per-container limit to half of the node's task
budget, leaving the other half for the host and the desktop session:

```bash
# On each node (ghost, exo-0); no k3s restart, no pod restart.
sudo install -d -m 0755 /etc/systemd/system/cri-containerd-.scope.d
printf '[Scope]\nTasksMax=50%%\n' | sudo tee /etc/systemd/system/cri-containerd-.scope.d/50-tasks-max.conf
sudo systemctl daemon-reload
```

`daemon-reload` applies the drop-in to running containers as well as new
ones; the percentage is relative to the same `min(pid_max, threads-max)`.
Verify on the node that every container scope picked it up:

```bash
systemctl list-units --plain --no-legend 'cri-containerd-*.scope' | awk '{print $1}' \
  | xargs systemctl show -p TasksMax | grep TasksMax | sort | uniq -c
# ghost: TasksMax=255164, exo-0: TasksMax=253736 (was 76549 / 76120)
```

buildbox has no option that caps this from inside the pod: `--concurrent-jobs`
bounds actions, not the recc compiles one action fans out, and casd's
`--num-io-threads`/`--num-digest-threads` size fixed pools, not its per-RPC
server threads.

---

## Framework Desktop Nodes

Workers join via the standard `K3S_URL` / `K3S_TOKEN` registration and are
immediately schedulable, and the BuildGrid worker DaemonSet gives them a bot.
They are not first-class build nodes until four manual steps are done:

1. The flannel MTU files from [Pod MTU 9000](#pod-mtu-9000--both-nodes).
   Without them the node mints MTU 1500 pods on a 9000 network.
2. A static `ip rule ... to <peer pod CIDR> lookup 40` route over
   `thunderbolt0` on every host pair. Otherwise its cross-node traffic stays on 2.5GbE.
3. A peer entry in `manifests/usb4-link-monitor.yaml`. Its `case` exits on
   unknown nodes, so the node never gets a `usb4-link=up` label.
4. The container task limit drop-in from
   [Container task limit](#container-task-limit--both-nodes). Without it the
   BuildGrid worker dies at 15% of the node's tasks during a recc fan-out.

Do not add node selectors to steer workloads toward local disks. Define an
explicit non-root local-path mapping for the node, then let
`WaitForFirstConsumer` and the Kubernetes scheduler co-locate the PVC consumer
with its selected volume.
