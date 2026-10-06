---
name: kubevirt-vms
description: >
  KubeVirt ephemeral VM lifecycle in the lab: VM provisioning, boot checks,
  teardown. Use when writing boot-test templates, debugging VM boot failures,
  or working with KubeVirt manifests.
metadata:
  context7-sources:
    - /kubevirt/kubevirt
    - /kubevirt/user-guide
    - /kubevirt/containerized-data-importer
---

# KubeVirt VMs — lab Skill

## When to Use

- Editing `bluefin-server-boot-test.yaml` or other boot-test VM templates
- Running or debugging the Dakota VM boot test pod (`dakota-vm-boot-test.yaml`)
- Debugging VM boot timeouts
- Enabling a new KubeVirt feature gate
- Understanding why a VM is stuck `Terminating`

## When NOT to Use

- Argo Workflows YAML syntax issues → `argo-workflows.md`
- ArgoCD sync problems → `gitops-argocd.md`

## Core Process

- [VM lifecycle](vm-lifecycle.md) — disks, scheduling, teardown.

## Dakota VM boot test pod (plain-root lane)

The Dakota image under test cannot boot through KubeVirt containerDisk or standard `bootc install to-disk` due to upstream bootc/composefs splitstream limitations. Instead, `dakota-vm-boot-test.yaml` runs direct QEMU/KVM kernel boot inside an ephemeral privileged Kubernetes pod whose container image is the Dakota image under test.

The Dakota container ships all required tooling: `qemu-system-x86_64`, `qemu-img`, `skopeo`, `ssh`, `ssh-keygen`, and `e2fsprogs` (`mkfs.ext4`).

### When to Use
- Validating Dakota image contents and runtime services (kernel, systemd, networking, gVisor `runsc`, `krun`/`libkrun` microVMs, `fuse2fs`, `gocryptfs`).
- Testing PR images built by `just run-bst-build <ref>` and pushed to the local Zot registry (`192.168.1.102:30500/dakota:<tag>`).
- Submitting on-demand via `just run-dakota-vm-boot image="192.168.1.102:30500/dakota:<tag>"`.

### Architecture & Mechanics
1. **Unpack without loop devices**:
   - `skopeo copy --src-tls-verify=false --dest-decompress docker://$IMG oci:/work/oci:img`
   - Unpack every layer from the OCI manifest into `/work/root` using `tar -xpf ... --xattrs --xattrs-include='*' --numeric-owner`.
   - Populate an ext4 disk image directly from `/work/root` via `truncate -s 40G disk.raw; mkfs.ext4 -q -L root -d /work/root disk.raw`. This populates the filesystem cleanly without kernel loop devices or partition tables.
2. **Credential and service injection**:
   - Enable `sshd.service` via symlink in `/etc/systemd/system/multi-user.target.wants/`.
   - Generate host keys (`ssh-keygen -A -f root`) and root keypair (`ssh-keygen -t ed25519 -f key -N ""`).
   - Inject root public key into `/var/roothome/.ssh/authorized_keys` (mode 0600).
3. **Direct kernel boot under QEMU/KVM**:
   - Copy `vmlinuz` and `initramfs.img` from `/usr/lib/modules/<version>/`.
   - Boot QEMU with `-machine q35,accel=kvm -cpu host -m 8192 -smp 6`, `-kernel vmlinuz -initrd initramfs.img -append "root=/dev/vda rw systemd.firstboot=no console=ttyS0,115200"`.
   - Forward host port 2222 to guest port 22 (`-netdev user,id=net0,hostfwd=tcp:127.0.0.1:2222-:22 -device virtio-net-pci,netdev=net0`).
4. **Execution & Evidence**:
   - Poll SSH readiness on `127.0.0.1:2222` as `root` (typically ready in ~20 seconds).
   - Stream evidence commands into the VM over SSH and capture outputs.
   - Expected degradation: `systemctl is-system-running` reports `degraded` solely because `ublue-boot-timeout.service` fails due to the absence of a systemd-boot ESP on a plain-root disk. This is expected and normal for this test lane.

### Failure Modes and Invariants (Timeless)
- **Plain-root runtime scope**: This lane validates image *contents and runtime behavior*, not the bootloader or ostree deployment update mechanics.
- **`bootc install to-disk` ostree backend failure**: Standard ostree installation fails with `bootupd is required for ostree-based installs` because bootc requires bootupd for systemd-boot ostree setups. Furthermore, no BLS entries or boot symlinks are created; even with synthetic symlinks, systemd detects `Running in initrd` post-switch-root and loops until service start limits force emergency mode. Not viable for Dakota.
- **`bootc install to-disk --composefs-backend` failure**: Dakota's native composefs installation path (bootc 1.16.13) fails with `Pulling image into composefs repository: Unexpected EOF in splitstream` across docker, containers-storage, and decompressed OCI layouts on both ext4 and btrfs (upstream: `bootc-dev/bootc#1703` and `composefs/composefs-rs#210`). The plain-root lane is the supported path until upstream resolves splitstream composefs import.
- **`bcvk ephemeral` dependency gap**: `bcvk ephemeral` requires host `virtiofsd`. The Dakota image does not ship `virtiofsd` and upstream publishes no standalone binary releases, preventing in-pod bcvk use without external package layering.
- **Nested KVM acceleration**: Lab nodes (AMD) run with `kvm_amd nested=1`. QEMU requires `-cpu host` to expose virtualization flags to the guest, enabling nested microVM runtimes like `libkrun`/`krun` and gVisor `runsc` inside the VM.
- **Script delivery**: Feeding long shell scripts via `kubectl run -i` stdin races against container startup. Workflows must inline scripts into Argo templates or mount them from ConfigMaps.

## Console-driven VM control (imperative path)

The KubeStellar Console ServiceAccount can start/stop/restart VMs
imperatively via `patch virtualmachines` (ClusterRole
`kubestellar-console-lab-surfaces`, see `console-dashboard/SKILL.md`).
Verified live (2026-07-25): a probe VM was started and stopped via
`kubectl patch vm <name> --as=system:serviceaccount:kubestellar-console:kubestellar-console
--type=merge -p '{"spec":{"running":true}}'`; the VMI scheduled on exo-0
with no pinning.

- `spec.running` is deprecated (warning emitted); prefer
  `spec.runStrategy: Always|Halted` in new manifests. Both work today.
- Serial/graphical access: `virtctl console <vm>` / `virtctl vnc <vm>`
  from a workstation with cluster access — the Console GUI does not
  proxy VNC; virtctl is the documented fallback.
- VM *definitions* remain GitOps (manifests in git); only runtime
  start/stop/restart is imperative. Ephemeral test VMs created by
  workflows are exempt (owned by the workflow, cleaned by onExit and
  `orphan-vm-cleanup`).

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "I'll keep the VM up between runs to save time." | No persistent test VMs. The `orphan-vm-cleanup` CronWorkflow will delete it. |
| "The teardown step can be optional." | A missing `onExit` handler leaks VMs and disk clones on failure. Always required. |
| "VMs must pin to ghost." | No VM type requires a ghost pin. VMs use containerDisk or PVC. Adding a node requires no YAML changes. |
| "Workflow pods need hostNetwork + nodeSelector: ghost to reach VMs." | False. Pod IPs route across nodes via flannel. kubectl exec works from any node. hostNetwork was a workaround for broken exo-1 flannel — not a KubeVirt requirement. |
| "The zot image from yesterday is still there." | Zot-writable loses its index.json on pod restart. Always check before running the pipeline. |
| "HostDisk feature gate is probably already on." | Verify with `kubectl get kubevirt kubevirt -n kubevirt -o jsonpath='{.spec.configuration}'`. Don't assume. |
| "inotify limits are a kernel concern, not a k8s concern." | KubeVirt virt-handler + containerd exhaust defaults at scale. The `inotify-tuning` DaemonSet is required. |

## Red Flags

- A VM provision template with `nodeSelector: kubernetes.io/hostname: ghost` — no VM type requires this anymore (no hostDisk VMs remain)
- An `onExit` handler that doesn't delete the VM object
- Hardcoded IPs in VM templates (use pod IP from `kubectl get pod -l kubevirt.io/vm=...`)
- **Any hostPath for VM disks** — use a containerDisk or PVC instead
- `registry.k8s.io/kubectl` used as image for a step that needs bash — it is distroless, use `cgr.dev/chainguard/kubectl:latest-dev`
- VM boot timeout with no disk or network explanation — check `cat /proc/sys/fs/inotify/max_user_watches` (should be >= 1048576)
- VM goes `Stopped` with `FailedCreate` and `metadata.labels: must be no more than 63 characters` — VM name exceeds Kubernetes label-value limit. Use short, unique names such as `{{workflow.name}}-{{item}}`.
- Orphaned VMs from a prior workflow consuming ghost resources — run `just list-vms` before submitting a new run; delete orphans with `kubernetes-mcp-resources_delete` if present.
- **VM immediately fails with `No disk capacity`** — virt-launcher was evicted because its `ephemeral-storage` limit was exceeded during disk extraction. This usually means `disk.Capacity == nil` in KubeVirt because the containerDisk image is missing the `/containerDisk.json` metadata file. See [VM lifecycle](vm-lifecycle.md) section 1.

## Verification

Before merging any VM provisioning change:

- [ ] No VM provision template has `nodeSelector: kubernetes.io/hostname: ghost` — no hostDisk VMs remain; all types float freely
- [ ] No `hostNetwork: true` or `nodeSelector: ghost` on kubectl workflow step pods — flannel handles routing
- [ ] `onExit` teardown deletes VM object; containerDisk teardown is VM-delete only; PVC-backed teardown also deletes the PVC
- [ ] Feature gates checked if adding a new VM capability
- [ ] `just list-vms` shows empty after workflow completion
- [ ] VM disks use containerDisk or PVC volumes; no VM workflow relies on hostPath storage
- [ ] No hardcoded IPs — pod IP derived at runtime via `kubectl get pod`
- [ ] **containerDisk builds**: Image contains `/containerDisk.json` with the correct `capacity` explicitly declared (prevents `No disk capacity` limits)
