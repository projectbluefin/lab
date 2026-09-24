---
name: kubevirt-lifecycle
description: >
  KubeVirt VM lifecycle: disks, scheduling, teardown.
---

## Core Process

### 1. containerDisk capacity

**Critical `containerDisk.json` requirement:** When building a custom containerDisk from `scratch` (using `buildah`), KubeVirt *requires* a `/containerDisk.json` file at the root of the image to declare the disk capacity:
```json
{"volumes":[{"image":"disk.qcow2", "capacity":"25Gi"}]}
```
Without this file, `virt-controller` attempts to fetch the uncompressed size from the registry API. If that fails (e.g. against an insecure local registry without proper credentials), KubeVirt defaults the pod's `ephemeral-storage` limit to 50M. This results in the virt-launcher pod being evicted immediately upon extracting the disk image, causing a `No disk capacity` error. Always inject this JSON metadata during the build step.

**Zot data loss:** The `zot-writable` pod (port 30500) can lose its manifest
index on a pod or k3s restart even though blobs may still exist. Check the
registry through its API before running the pipeline; rebuild if the manifest
is unavailable. Do not inspect the host path with workstation SSH.

### 2. Native bootc OCI boot — what is and isn't possible

**KubeVirt cannot boot a bootc OCI image directly without disk preparation.** This is a hard
constraint of the current KubeVirt architecture. Summary of what was verified:

**What the bootc OCI image contains (verified on `bluefin:testing`):**
- Kernel: `/usr/lib/modules/<version>/vmlinuz`  (e.g. `7.0.12-201.fc44.x86_64`)
- Initramfs: `/usr/lib/modules/<version>/initramfs.img`
- These paths are accessible via `KubeVirt kernelBoot.container`

**Why `kernelBoot.container` alone is not enough:**
- `kernelBoot.container` extracts kernel + initramfs from an OCI image and passes them to QEMU
  as `-kernel`/`-initrd` — it does NOT provide a root filesystem
- The bootc/ostree initramfs requires `root=UUID=<uuid>` and `ostree=/ostree/boot.1/default/<hash>/0`
  — both set by `bootc install to-disk` at disk creation time
- Without an ostree-deployed root disk the VM fails to mount `/` and panics

**Why `containerDisk` cannot use the raw bootc OCI image:**
- KubeVirt `containerDisk` expects a disk image file at `/disk/` inside the OCI image (raw or qcow2)
- A bootc OCI image contains OS filesystem layers, not a disk image — KubeVirt rejects it
- CDI `DataVolume` registry source has the same constraint

**Verified boot cmdline structure (reference for debugging):**
```
BOOT_IMAGE=(hd0,gpt3)/boot/ostree/default-<hash>/vmlinuz-7.0.12-201.fc44.x86_64
root=UUID=<disk-uuid> rw selinux=0 ostree=/ostree/boot.1/default/<hash>/0
```

**Diagnosing kernel/initramfs paths in a running VM (guest-agent):**
```bash
kubectl exec -n <ns> <virt-launcher-pod> -c compute -- \
  virsh qemu-agent-command 1 \
  '{"execute":"guest-exec","arguments":{"path":"cat","arg":["/proc/cmdline"],"capture-output":true}}'
# Decode result: base64 -d <<< <out-data>
```

### 3. Required KubeVirt feature gates

Two feature gates must be enabled in the `kubevirt` CR. If VM creation fails with
`feature gate is not enabled in kubevirt-config`, this is cluster drift — fix via GitOps:

```bash
kubectl patch kubevirt kubevirt -n kubevirt --type=merge --patch='
{
  "spec": {
    "configuration": {
      "developerConfiguration": {
        "featureGates": ["HostDisk", "ExperimentalIgnitionSupport"]
      }
    }
  }
}'
```

Persist this in `manifests/` so ArgoCD maintains it.

### 4. VM node scheduling

**No hostDisk VMs remain.** VMs use containerDisk or PVC — they schedule freely on any KubeVirt-capable node.
PVC-backed VMs use a `local-path` RWO PVC; KubeVirt auto-schedules on the PVC's node.

**nodeSelector and hostNetwork are NOT required for kubectl workflow steps.**
Pod IPs are routable across nodes via flannel and `kubectl exec` goes through
the API server. Workflow storage must use PVCs rather than hostPath volumes.

KubeVirt capacity is whatever nodes are currently online with
`kubevirt.io/schedulable: "true"` and `virt-handler` running.
No Argo global parallelism cap — Kubernetes pod scheduling is the
real backpressure, so pods queue naturally when node RAM is exhausted.

### 5. Teardown — always via onExit, never skip

Every pipeline must include an `onExit` teardown that deletes the KubeVirt VM
object: `kubectl delete vm "${VM}" -n "${NS}"`.

Teardown deletes the VM and any PVC-backed disk through the Kubernetes API; it
must not rely on a host-local file or node placement.

Orphaned VMs (from force-deleted workflows) are cleaned by the `orphan-vm-cleanup`
CronWorkflow every 2 hours.

### 6. Checking for stuck VMs

```bash
just list-vms
# Expected output when idle: empty (no VMs)
```

If VMs are stuck `Terminating`:
```bash
# Delete the virt-launcher pod and let reconciliation finish
kubectl delete pod -n <namespace> -l kubevirt.io/vm=<vm-name> --force
```

### 7. Node inotify limits — required for KubeVirt

KubeVirt virt-handler, containerd, and podman together consume thousands of inotify
watches. When exhausted, VM boot fails silently and container
errors appear. The `inotify-tuning` DaemonSet in `manifests/` raises limits on all nodes:

```
fs.inotify.max_user_watches=1048576
fs.inotify.max_user_instances=512
```

If you see VM boot timeouts that aren't explained by disk or network issues, check:
```bash
cat /proc/sys/fs/inotify/max_user_watches   # should be >= 1048576
```

The DaemonSet applies this on every node restart. Do not remove it.
