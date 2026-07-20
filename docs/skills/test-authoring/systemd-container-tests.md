---
name: systemd-container-tests
description: >
  Native systemd E2E tests in a privileged disposable Kubernetes Pod (no VM).
---

### 11. Native-systemd E2E Testing

`run-systemd-container-tests` validates native systemd behavior inside a
scheduler-managed Kubernetes target Pod, without KubeVirt, a disk artifact, or
nested Podman. It creates a privileged disposable target Pod with systemd as
PID 1; qecore and Behave run inside that target, never under Argo emissary
PID 1.

A full desktop smoke suite is still blocked by GNOME session handoff inside the
container target, so do not claim that all desktop suites pass there. System-level
and headless-qecore suites are the current working target; desktop GNOME Shell
suites remain under investigation.

#### Core Containerized Testing Rules:
1. **Native systemd boundary:** Create the target with an Argo `resource`
   template and set its owner reference to the Workflow. The runner waits for
   `systemctl is-system-running`, `dbus`, and `systemd-logind` before invoking
   qecore, and deletes the target in an EXIT trap.
2. **Bounded privileged runtime:** Keep privilege confined to the target Pod.
   Request 2 CPU, 4 Gi memory, and 20 Gi ephemeral storage, with limits of
   4 CPU, 8 Gi memory, and 40 Gi ephemeral storage. Do not add a node pin,
   VMI, raw-disk build, or containerDisk step:
   ```yaml
   securityContext:
     privileged: true
     runAsUser: 0
     allowPrivilegeEscalation: true
   ```
3. **Resolver repair:** The memory-backed `/run` emptyDir volume breaks the
   image's `/etc/resolv.conf` symlink (it typically points below `/run`). The
   runner must copy its own Kubernetes-provided `/etc/resolv.conf` into the
   target before any `git clone` or `pip install`, and replace the dangling
   symlink with that file.
4. **Autologin test user:** The ephemeral `bluefin-test` account must have a
   non-expired shadow `lastchg` value for GDM PAM autologin to succeed. A zero
   or expired value forces a password change and blocks login before qecore
   starts. Set `lastchg` to the current day (or a recent value) when preparing
   the target image.
5. **Pip bootstrapping:** Minimal bootc target images do not always include
   `pip`. Bootstrap it with `python3 -m ensurepip --default-pip`, then install
   qecore, dogtail, and Behave inside the disposable target.

#### Verified session findings

- A privileged target Pod running systemd as PID 1 is viable for native-systemd
  E2E tests.
- `/run` must be an `emptyDir` for systemd, but that invalidates the
  `/etc/resolv.conf` symlink. Copy the runner's live resolver into the target and
  overwrite the broken symlink before network-dependent setup.
- **Do not pre-start `gnome-ponytail-daemon`** in the target. `qecore` starts and
  manages the daemon itself; an existing instance will collide with the session
  it tries to create.
- The disposable `bluefin-test` user needs a valid, non-expired shadow `lastchg`
  entry. Without it, GDM autologin fails and qecore cannot reach the desktop.
- `qecore` does **not** propagate arbitrary suite-level environment variables to
  its spawned user script. Persist any inputs the suite needs (image references,
  branch names, secrets paths) in a file the target can read, and have the test
  runner or environment read that file instead of relying on env propagation.

