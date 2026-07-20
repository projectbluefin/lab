---
name: flatcar-recovery
description: >
  Recovery backdoor using virt-handler and host recovery procedures for Flatcar nodes.
---

## Host and Service Recovery Backdoor via KubeVirt DaemonSet (SSH-Refused Recovery)

When SSH access is refused on a node (e.g., `sshd.service` is deactivated or crashed) and direct console/SSH access is unavailable, you can execute privileged commands directly on the host using the pre-existing, highly-privileged `virt-handler` pod on that node.

This works because `virt-handler` runs with `hostPID: true`, `hostNetwork: true`, `privileged: true`, and as the `root` user (UID 0).

### Core Process

1. **Locate the running `virt-handler` pod on the target node**:
   ```bash
   kubectl get pods -n kubevirt -o wide | grep virt-handler | grep <node-name>
   ```
   *Example output*: `virt-handler-c7wtg` on `exo-0`

2. **Execute a command using `nsenter` from inside the pod**:
   - Check `sshd` status:
     ```bash
     kubectl exec -n kubevirt <virt-handler-pod-name> -c virt-handler -- nsenter -t 1 -m -- systemctl status sshd
     ```
   - Start/enable `sshd`:
     ```bash
     kubectl exec -n kubevirt <virt-handler-pod-name> -c virt-handler -- nsenter -t 1 -m -- systemctl start sshd
     ```
   - Diagnose journalctl:
     ```bash
     kubectl exec -n kubevirt <virt-handler-pod-name> -c virt-handler -- nsenter -t 1 -m -- journalctl -u sshd -n 100 --no-pager
     ```

### Systemd-Resolved / D-Bus Deadlock Recovery

If `systemd-resolved` enters a failed state (`Failed with result 'protocol'`) due to a `dbus-broker` restart, DNS resolution will fail globally on the node, breaking container image pulls.

**Remediation**:
1. Check DNS resolution locally:
   ```bash
   resolvectl query google.com
   ```
2. If it is hung or failing, restart `systemd-resolved` using the backdoor:
   ```bash
   kubectl exec -n kubevirt <virt-handler-pod-name> -c virt-handler -- nsenter -t 1 -m -- systemctl restart systemd-resolved
   ```

---

