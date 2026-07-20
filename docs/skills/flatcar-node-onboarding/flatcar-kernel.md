---
name: flatcar-kernel
description: >
  Custom Flatcar kernel lifecycle, Nebraska packages, and bare-metal builds.
---

# Flatcar Kernel Lifecycle

## Kernel Lifecycle (3-node simple mode)

Use one update group (`GROUP=stable`) for all Flatcar nodes. Keep canary behavior as
an operational gate: **exo-0 must validate first for 24h** before the same package is
considered promoted for the rest of the cluster.

This keeps config simple while preserving staged rollout discipline.

| Key | Meaning |
|---|---|
| `candidate-version` | Kernel version currently under exo-0 canary gate |
| `candidate-package` | Nebraska package filename for the current candidate |
| `candidate-created-at` | RFC3339 timestamp when the current candidate entered the gate |
| `gate-status` | `pending`, `pass`, or `fail` for the current candidate |
| `stable-version` | Last promoted known-good kernel version |
| `stable-package` | Nebraska package filename for the last promoted stable kernel |
| `validation-marker-node` | Manual/runtime validation marker node name (`exo-0` for the canary gate fallback path) |
| `validation-marker-status` | Manual/runtime validation marker status (`pass` when no labeled workflow marker exists) |
| `validation-marker-version` | Candidate version validated by the manual/runtime marker |
| `validation-marker-created-at` | RFC3339 timestamp for the manual/runtime validation marker |

### Promotion policy

1. Build and register a new candidate package via `flatcar-kernel-build`.
2. Validate on exo-0 for 24h:
   - exo-0 remains `Ready`
   - `flatcar-update` pods remain healthy
   - one successful update/reboot validation completes on exo-0, recorded either as a labeled successful workflow or as the explicit `validation-marker-*` ConfigMap keys
3. Promote by keeping the new package as the active stable target.
4. On failure, roll back by re-pointing to the last-known-good package/version.

### Operator checks

```bash
kubectl get configmap flatcar-kernel-lifecycle-state -n argo -o yaml
argo cron list -n argo | grep flatcar-kernel-gate
argo submit -n argo --from workflowtemplate/flatcar-kernel-gate
```

### Exo-0 7.1 verification (ponytail path)

```bash
# Node kernel (cluster truth)
kubectl get node exo-0 -o jsonpath='{.status.nodeInfo.kernelVersion}{"\n"}'

# Nebraska package list (latest entries)
curl -s "http://<lab-ip>:30802/api/v1/apps/e96281a6-d1af-4bde-9a0a-97b76e56dc57/packages" | jq '.[-5:]'

# Confirm update.conf on exo-0
POD=$(kubectl get pods -n flatcar-update -l app=flatcar-update-configurator -o wide \
  | awk '/exo-0/ {print $1; exit}')
kubectl exec -n flatcar-update "$POD" -- nsenter --target 1 --mount -- cat /etc/flatcar/update.conf
```

---

## Node Addition Checklist (copy-paste for each new node)

```
[ ] Node has Flatcar Container Linux installed (any supported version)
[ ] k3s agent joined (kubectl get nodes shows new node as Ready)
[ ] flatcar-update-configurator DaemonSet pod Running on new node
[ ] kubectl exec nsenter confirms /etc/flatcar/update.conf is correct
[ ] Nebraska logs show first processEvent for this machineId
[ ] (Optional) flatcar-kernel-build workflow run to register new kernel package
[ ] exo-0 canary gate passes 24h (Ready, healthy flatcar-update pods, reboot validation)
[ ] kubectl get nodes -o wide confirms node is Running with expected kernel
```

---

## Bare-Metal Custom Kernel Builds

When the KubeVirt / Argo VM pipeline (`flatcar-kernel-build.yaml`) is blocked by resource constraints, TTY errors, or Portage overlay mapping bugs, build the custom kernel directly on a bare-metal Flatcar host (such as `exo-0`).

### Core Process

1. **Stop k3s to free resources**:
   ```bash
   sudo systemctl disable --now k3s-agent
   ```

2. **Setup workspace and clone build tools**:
   ```bash
   mkdir -p ~/work && cd ~/work
   git clone --filter=blob:none https://github.com/flatcar/scripts.git
   git clone https://github.com/projectbluefin/lab.git
   cd scripts
   git checkout flatcar-4593  # Match the running Stable branch
   ```

3. **Vendor the local overlay**:
   ```bash
   OVERLAY_DST=sdk_container/src/third_party/coreos-overlay/sys-kernel
   OVERLAY_SRC=~/work/lab/flatcar/kernel-overlay/sys-kernel
   rsync -av "$OVERLAY_SRC"/ "$OVERLAY_DST"/
   ```

4. **Prepare the overlay and kernel defconfig**:
   - Upstream Linux 7.1.1 compiles cleanly with an empty `UNIPATCH_LIST` inside `sys-kernel/coreos-sources-7.1.1.ebuild`. Do not include stale 6.12 patches in 7.1.1.
   - Seed a 7.1 defconfig:
     `cp sdk_container/src/third_party/coreos-overlay/sys-kernel/coreos-modules/files/amd64_defconfig-6.12 sdk_container/src/third_party/coreos-overlay/sys-kernel/coreos-modules/files/amd64_defconfig-7.1`

5. **Generate the SDK command script inside `sdk_container/tmp`**:
   The host's `sdk_container/` directory is mapped to `/mnt/host/source/` inside the container. Command files must be written under `${PWD}/sdk_container/tmp/` on the host to be visible inside the container at `/mnt/host/source/tmp/`.
   ```bash
   mkdir -p sdk_container/tmp
   cat > sdk_container/tmp/inside-sdk.sh <<'EOF'
   #!/usr/bin/env bash
   set -euo pipefail
   KVER=7.1.1
   # Patch kernel-2.eclass to accept EAPI 7/8
   ECLASS=/mnt/host/source/src/third_party/portage-stable/eclass/kernel-2.eclass
   if [ -f "$ECLASS" ] && grep -q '2|3|4|5|6)$' "$ECLASS"; then
     sudo sed -i 's/2|3|4|5|6)$/2|3|4|5|6|7|8)/' "$ECLASS"
   fi
   # Ebuild manifest and emerge
   cd /mnt/host/source/src/third_party/coreos-overlay/sys-kernel/coreos-sources
   ebuild "coreos-sources-${KVER}.ebuild" manifest
   setup_board --board=amd64-usr --default --force
   emerge-amd64-usr -v sys-kernel/coreos-sources sys-kernel/coreos-modules sys-kernel/coreos-kernel
   # Build image update payload
   /mnt/host/source/src/scripts/build_packages --board=amd64-usr
   /mnt/host/source/src/scripts/build_image --board=amd64-usr prod
   EOF
   chmod +x sdk_container/tmp/inside-sdk.sh
   ```

6. **Run the container without `-t` in background**:
   Do **not** use the TTY `-t` flag when running via `nohup` or background tasks, or docker will crash with `the input device is not a TTY` (exit code 137).
   ```bash
   ./run_sdk_container -- /mnt/host/source/tmp/inside-sdk.sh
   ```

7. **Stage and apply update**:
   The build outputs `flatcar_production_update.gz` to `../build/images/amd64-usr/developer-latest/`. Mount this to a local nginx container, append `SERVER=http://127.0.0.1:8080/` to `/etc/flatcar/update.conf`, trigger `sudo update_engine_client -update`, and reboot.

### Red Flags

- `mkdir: cannot create directory /var/home`: On Flatcar, the home directory is `/home/jorge/`, not `/var/home/jorge/`.
- `/mnt/host/source/tmp/inside-sdk.sh: No such file or directory`: Script was written to `/tmp/` on the host instead of `sdk_container/tmp/`.
- `docker run exits with status 137` on background start: Remove the `-t` TTY flag from `run_sdk_container` invocation.
- `Unable to dry-run patch unipatch failure`: Stale 6.12 patch files are missing or do not apply to 7.1. Set `UNIPATCH_LIST=""` in `coreos-sources-7.1.1.ebuild`.


---

