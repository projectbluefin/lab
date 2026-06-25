# Agentic Lab Reference Architecture — Technical Specification

Date: 2026-06-25

## 1. Goal & Scope
This document specifies the highly optimized technical implementation of the Project Bluefin homelab. It serves as the authoritative blueprint for automated agents and human operators to configure, tune, and maintain the cluster. 

The primary design principle is **zero hardware waste**. By aggressively minimizing control plane and Kubernetes overhead, we ensure that maximum CPU, RAM, and disk I/O are allocated directly to container builds and KubeVirt test VMs.

---

## 2. Cluster Topology & Node Specifications

The lab consists of a 3-node physical mesh utilizing custom-tailored nodes:

| Host | IP | Role | Specs | OS / Kernel |
|---|---|---|---|---|
| `ghost` | `192.168.1.102` | Control Plane + KubeVirt Compute | Ryzen AI MAX+ 395, 16c/32t, 64GB RAM | Bluefin / 6.11+ |
| `exo-0` | `192.168.1.171` | Dedicated k3s Worker | Framework Desktop, 4 TB ZFS NVMe | Flatcar 4593.2.3 |
| `exo-2` | `192.168.1.171` | Dedicated k3s Worker | Framework Desktop, 4 TB ZFS NVMe | Flatcar 4593.2.3 |
| `exo-1` | `192.168.1.239` | Opt-in k3s Worker (Laptop) | 22c, 16GB RAM | Dakota / 6.11+ |
| `hamilton` | `192.168.1.225` | Opt-in k3s Worker (Workstation) | Ryzen 7 5800X, 32GB RAM | Bluefin / 6.11+ |

---

## 3. Host-Level Control Plane Tuning (k3s)

To prevent etcd and API server bloat under high object churn (rapid VM creation/destruction), the `/etc/rancher/k3s/config.yaml` on `ghost` is optimized to enforce strict resource boundaries:

```yaml
# /etc/rancher/k3s/config.yaml on ghost
# Apply once; requires: sudo systemctl restart k3s

kube-apiserver-arg:
  - "default-watch-cache-size=100"      # Binds watch-cache memory allocation
  - "event-ttl=30m"                    # Reduces noisy event retention (default 1h)
  - "max-requests-inflight=100"
  - "max-mutating-requests-inflight=50"
  - "profiling=false"

kube-controller-manager-arg:
  - "leader-elect=false"               # Single control-plane saves lease churn
  - "node-monitor-period=60s"
  - "node-monitor-grace-period=180s"    # Prevents transient NotReady states during VM boot
  - "concurrent-deployment-syncs=2"     # Bounded concurrency to save CPU cycles
  - "concurrent-replicaset-syncs=2"
  - "concurrent-statefulset-syncs=1"
  - "concurrent-gc-syncs=2"

etcd-arg:
  - "auto-compaction-mode=periodic"
  - "auto-compaction-retention=1h"     # Immediate hourly compaction
  - "quota-backend-bytes=4294967296"    # Strict 4 GiB quota backend bytes
  - "heartbeat-interval=250"
  - "election-timeout=2500"

etcd-snapshot-schedule-cron: "0 */12 * * *"
etcd-snapshot-retention: 5
etcd-snapshot-compress: true
```

---

## 4. Storage Architecture: Local CoW Reflink

Networked or replicated storage layers (like Longhorn or OpenEBS) introduce overhead that degrades test VM boot performance. This reference architecture relies entirely on native, bare-metal-speed local storage:

### 1. Local-Path Provisioner
All dynamic volume claims utilize `local-path-provisioner` pinned to a node's physical NVMe drive. This ensures zero network replication latency.

### 2. Copy-on-Write (CoW) Golden Disks
To launch VMs instantly, we store a "golden" base disk image (`disk.raw`) on the host's physical partition. 
* On workflow initiation, KubeVirt boots VMs using the golden disk.
* For each individual test VM, we perform a **btrfs or ZFS `reflink` clone** which takes roughly **24 milliseconds**:
  ```bash
  cp --reflink=always /var/tmp/bluefin-golden/testing/disk.raw /var/tmp/knuckle-test/disk.raw
  ```
* Reflink clones consume zero extra disk space at creation, executing instant, lightweight copy-on-write writes that isolate tests from each other.

---

## 5. Networking Subsystem Specification

### 1. Flannel host-gw Flat L2
Pod and VM networking runs entirely on Flannel's `host-gw` backend.
* **Mechanism:** Pod-to-pod routing occurs directly via IP routing tables on the host, bypassing VXLAN, encapsulation, or WireGuard encryption overhead.
* **Requirement:** All physical nodes must be dual-homed or directly connected to the flat L2 subnet `192.168.1.0/24`.

### 2. USB4 Daisy-Chain Mesh (thunderbolt-net)
To prevent build and layer traffic from saturating the primary 2.5 GbE control-plane interface, a secondary switchless **40 Gbps USB4 mesh** connects the nodes:
* Nodes are daisy-chained using physical USB4 cables.
* Kernel drivers bind the physical interfaces to virtual IP adapters using the `thunderbolt-net` module.
* K3s Flannel is forced to route inter-node data-plane traffic over this high-speed mesh by overriding `--flannel-iface` to bind to the Thunderbolt net interface:
  ```bash
  --flannel-iface=thunderbolt0
  ```
* **Kernel Prerequisite:** Host machines must run a modern Linux kernel (6.8+) supporting stable Thunderbolt routing topologies and daisy-chaining.

---

## 6. OCI Layer Caching & containerd Mirroring

To make container builds and bootc pulls extremely fast, the homelab hosts a local OCI cache:

### 1. Local Zot Registry Configuration
* **Writable Target (Port 30500):** Acts as the local storage destination for built containerDisks and ISO outputs.
* **Pull-Through Cache (Port 30501):** Intercepts requests for upstreams (`ghcr.io`, `docker.io`, `quay.io`, `registry.fedoraproject.org`, `registry.k8s.io`, `cgr.dev`). Any image pulled by any pod is cached on the host's high-speed NVMe and served locally on subsequent hits.

### 2. containerd hosts.toml Mirror Routing
All cluster nodes execute a DaemonSet (`manifests/registry-mirror-config.yaml`) that configures containerd to route pull requests through the local Zot cache:

```toml
# Example host config write under /etc/containerd/certs.d/ghcr.io/hosts.toml
server = "https://ghcr.io"

[host."http://192.168.1.102:30501/ghcr"]
  capabilities = ["pull", "resolve"]
  skip_verify = true
```

Since Fedora-based images (Bluefin, LTS, Aurora, Bazzite, Dakota) share massive base layers, this caching model reduces download times from minutes to milliseconds, relying on the host's ZFS ARC to serve layer cache hits directly from RAM.

---

## 7. VM Lifecycle & Test Runner Integration

All VMs are strictly ephemeral, executing under a deterministic lifecycle orchestrated by Argo Workflows:

```
[Argo Submit] ────► [reflink Disk Clone (24ms)] ────► [KubeVirt VMI Spawn]
                                                            │
                                                            ▼
[Test Run Complete] ◄──── [behave + dogtail AT-SPI] ◄──── [Wait for SSH]
        │
        ▼
[onExit: Teardown VMI + Delete Disk]
```

### 1. SSH Access & accessCredentials Injection
Rather than mutating disk images to inject SSH credentials (which violates bootc's immutable user-space guarantees), we leverage KubeVirt's native `accessCredentials` with `qemuGuestAgent`:
* A public key secret (`bluefin-test-ssh-pubkey`) is declared via GitOps.
* KubeVirt injects the key into the running VM's authorized keys table dynamically at boot via the QEMU guest agent.
* Workflows can immediately SSH using the corresponding private key without host disk mutations.

### 2. Behave + dogtail GUI Test Execution
1. The test runner pod starts headless Wayland via `qecore-headless`:
   ```bash
   qecore-headless --session-type wayland --session-desktop gnome
   ```
2. The runner automates the desktop using `dogtail` AT-SPI tree traversal.
3. Coordinates for cursor clicks on Wayland surfaces are bridged dynamically by `gnome-ponytail-daemon`.

---

## 8. Test Reports & Pages Publishing

On test completion, results are synthesized into human-readable outputs and published:
* **Screenshot Harvesting:** Wayland desktop captures (`.png`) are harvested by the runner pod and pushed to GHCR as OCI image layers.
* **Pages Rebuild:** A cron workflow pulls the latest OCI layers, extracts the reports, and triggers a static build to update the public-facing testing website.
