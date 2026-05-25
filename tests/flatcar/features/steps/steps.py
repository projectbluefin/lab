"""
Step definitions for Flatcar boot tests.

Steps issue SSH commands to the Flatcar VM via subprocess.
Connection details come from context (set in environment.py before_all).

No qecore, no dogtail — plain behave.
"""
import subprocess

from behave import step


def _ssh(context, command: str) -> subprocess.CompletedProcess:
    """Run a command on the Flatcar VM and return the completed process."""
    return subprocess.run(
        [
            "ssh",
            "-i", context.ssh_key,
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "ConnectTimeout=10",
            f"{context.ssh_user}@{context.vm_ip}",
            command,
        ],
        capture_output=True,
        text=True,
    )


@step("Flatcar VM is reachable over SSH")
def flatcar_vm_is_reachable(context) -> None:
    result = _ssh(context, "echo ok")
    assert result.returncode == 0, (
        f"Cannot reach Flatcar VM at {context.vm_ip}: {result.stderr}"
    )
    context.last_ssh_result = result


@step('Run SSH command: "{command}"')
def run_ssh_command(context, command) -> None:
    context.last_ssh_result = _ssh(context, command)


@step('SSH command output "is" "{expected}"')
def ssh_output_is(context, expected) -> None:
    actual = context.last_ssh_result.stdout.strip()
    assert actual == expected, f"Expected '{expected}', got '{actual}'"


@step('SSH command output is not "{val1}" and not "{val2}"')
def ssh_output_not_degraded(context, val1, val2) -> None:
    actual = context.last_ssh_result.stdout.strip()
    assert actual not in (val1, val2), (
        f"System state is '{actual}' — expected neither '{val1}' nor '{val2}'"
    )


@step('SSH command return code is "{expected_code}"')
def ssh_return_code_is(context, expected_code) -> None:
    actual = context.last_ssh_result.returncode
    assert actual == int(expected_code), (
        f"SSH command exited {actual}, expected {expected_code}\n"
        f"stdout: {context.last_ssh_result.stdout}\n"
        f"stderr: {context.last_ssh_result.stderr}"
    )
