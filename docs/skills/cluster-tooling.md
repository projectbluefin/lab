---
name: cluster-tooling
description: "Cluster management tools for the lab: kubectl, k3s, zot, external-secrets, and K8sGPT. Use when managing cluster state, installing cluster add-ons, configuring the OCI registry, or running cluster analysis through MCP."
metadata:
  type: reference
  context7-sources:
    - /helm/helm
    - /k3s-io/k3s
    - /project-zot/zot
    - /external-secrets/external-secrets
    - /k8sgpt-ai/k8sgpt
---

# Cluster Tooling — lab

## Mandatory first step

Before any kubectl, k3s, or K8sGPT operation, look up the current API via Context7:

```
resolve-library-id "/k3s-io/k3s" → get-library-docs
resolve-library-id "/k8sgpt-ai/k8sgpt" → get-library-docs
```

Do not guess flags, chart schema, or MCP method names. The K8sGPT MCP server exposes `analyze`, `cluster-info`, `list-resources`, `get-resource`, `list-namespaces`, `get-logs`, `list-events`, `list-filters`, `add-filters`, `remove-filters`, `list-integrations`, and `config`; verify the current docs before wiring it into a client.

## Tool roles

| Tool | Role |
|------|------|
| `k3s` | Lightweight Kubernetes — cluster runtime |
| `kubectl` | Direct cluster inspection and apply |
| `zot` | OCI registry for test artifacts |
| `external-secrets` | Pulls secrets from vault into k8s Secrets |
| `k8sgpt` | Cluster analysis / MCP troubleshooting bridge |

## Key references

- Cluster topology: `AGENTS.md`
- Bootstrap procedure: `docs/bootstrap.md`
- Recovery: `docs/skills/k3s-cluster-ops` (user skill, load before any cluster recovery)
- K8sGPT MCP config: `~/.copilot/mcp-config.json` on this machine, with `k8sgpt serve --mcp` or `--mcp --mcp-http` as the client target

## K8sGPT usage notes

- Use `k8sgpt analyze --explain` for broad triage.
- Narrow with `--filter=Pod`, `--filter=Deployment`, or `--namespace=<ns>`.
- For assistant integration, prefer the MCP server mode (`k8sgpt serve --mcp`) and register it in Copilot/Claude-style MCP configs.
- Verified source: `/k8sgpt-ai/k8sgpt`

## NVMe and PCIe Power Management Quirks on Strix Halo (e.g., exo-0)

On Strix Halo platforms, DRAM-less NVMe controllers (like the Innogrit IG5220 / RainierQX) can experience active I/O timeouts (`QID 32 timeout`) and uninterruptible sleep state (`D` state) due to aggressive PCIe ASPM and APST power state transitions.

### 1. Diagnosis
Check for timeouts or blocked processes in kernel logs or list active file-locks on raw block devices:
```bash
# Check dmesg for NVMe errors
dmesg | grep -i nvme

# Find lock holders on raw disk
fuser -v /dev/nvme1n1
```

### 2. Remediation Workflow
Follow this precise order of operations to disable power management and reset the device at runtime without a host reboot:

1. **Set PCIe ASPM Policy to Performance**:
   ```bash
   echo performance > /sys/module/pcie_aspm/parameters/policy
   ```
2. **Reset the NVMe Controller**:
   ```bash
   echo 1 > /sys/class/nvme/nvme1/device/reset
   ```
3. **Disable Autonomous Power State Transition (APST)** on the drive:
   ```bash
   nvme set-feature -f 0x0c -V 0 /dev/nvme1
   ```
4. **Settle Udev Triggers**:
   ```bash
   udevadm settle
   ```

### 3. Optimal Formatting and Mounting
Always use optimal Btrfs parameters for SSD storage to maximize performance over the 40G USB4 link:
```bash
# Format skipping slow full-disk discard
mkfs.btrfs -f -K /dev/nvme1n1

# Optimal mount options
mount -o rw,noatime,compress=zstd:3,ssd,discard=async,space_cache=v2 /dev/nvme1n1 /var/mnt/ghost-data
```

## BuildStream 2.x Distributed Builds and Caching

BuildStream 2.x has migrated to standard Remote Execution API (REAPI) Content Addressable Storage (CAS) protocols, replacing legacy python-based daemons with lightweight, high-performance, single-binary REAPI caching servers (e.g. `bazel-remote`).

### 1. REAPI CAS Backend (`bazel-remote`)
- **Image**: `quay.io/bazel-remote/bazel-remote`
- **Deployment**: Pinned to the high-speed Btrfs storage node (`exo-0` via `/var/mnt/ghost-data`).
- **Configuration**: Exposes gRPC on port `9092` and HTTP/1.1 on port `8080`.

### 2. Client-Side `buildbox-casd` Mandate
- BuildStream 2.x **cannot** initialize any remote cache (gRPC or HTTP) unless `buildbox-casd` is installed and available in the client's `PATH`. Standard python/pip container environments will fail with connection or type errors.
- Always use the mirrored `bst2` image (`192.168.1.102:30500/bst2:<tag>`) as the build-container runner, which includes both `buildbox-casd` and BuildStream 2.7.0.

### 3. Client-Side Configuration Layout
In `buildstream.conf`, project-specific remote caches must be nested within the project dictionary under the `servers:` list. Using an incorrect structure causes configuration type errors:

```yaml
projects:
  bst-prototype:
    artifacts:
      servers:
      - url: grpc://bst-artifact-server:9092
        push: true
```

### 4. YAML Scripting and Indentation Safety
When generating `buildstream.conf` dynamically inside an Argo YAML script block:
- **Avoid multiline heredocs (`cat << EOF`)**: Indented heredocs preserve spaces unless processed carefully, while non-indented lines violate YAML script structure.
- **Prefer explicit sequential writes**: Use `echo "..." > file` and `echo "..." >> file` for structured text generation. This completely eliminates YAML indentation parse bugs and is 100% robust.


