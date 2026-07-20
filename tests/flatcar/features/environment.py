"""
Flatcar test environment — plain behave, no qecore.

Flatcar has no GNOME desktop; tests run in the Argo runner pod and
issue SSH commands into the Flatcar VM. VM_IP is injected by the runner
as an environment variable.
"""
import os


def before_all(context) -> None:
    context.vm_ip = os.environ["FLATCAR_VM_IP"]
    context.ssh_key = os.environ.get("SSH_KEY_PATH", "/etc/ssh/test-key/id_ed25519")
    context.ssh_user = os.environ.get("SSH_USER", "core")


