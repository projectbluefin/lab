"""
Flatcar OS Smoke Tests — Phase 1
Validates that Flatcar boots healthy and core services are running.
Runs over SSH via tmt on a KubeVirt VM.
"""
import subprocess


def run(cmd, **kwargs):
    """Run a command on the Flatcar VM; tmt provides the SSH context."""
    return subprocess.run(cmd, shell=True, capture_output=True, text=True, **kwargs)


class TestSystemdHealth:
    def test_systemd_is_running(self):
        r = run("systemctl is-system-running")
        assert r.returncode == 0 or "degraded" in r.stdout, \
            f"systemd not running: {r.stdout.strip()}"

    def test_no_failed_units(self):
        r = run("systemctl list-units --state=failed --no-legend")
        assert r.stdout.strip() == "", f"Failed units:\n{r.stdout}"

    def test_sshd_active(self):
        r = run("systemctl is-active sshd")
        assert "active" in r.stdout


class TestContainerRuntime:
    def test_docker_or_containerd_running(self):
        r = run("systemctl is-active docker containerd 2>/dev/null | grep -q active; echo $?")
        assert r.stdout.strip() == "0", "No container runtime active"

    def test_docker_hello_world(self):
        r = run("docker run --rm hello-world 2>&1", timeout=60)
        assert r.returncode == 0, f"docker hello-world failed:\n{r.stdout}\n{r.stderr}"


class TestNetworking:
    def test_network_online(self):
        r = run("systemctl is-active systemd-networkd")
        assert "active" in r.stdout

    def test_dns_resolves(self):
        r = run("getent hosts github.com")
        assert r.returncode == 0, "DNS resolution failed"


class TestFlatcarVersion:
    def test_os_release(self):
        r = run("cat /etc/os-release")
        assert "Flatcar" in r.stdout, f"Not Flatcar: {r.stdout}"
        assert "Container Linux" in r.stdout or "flatcar" in r.stdout.lower()

    def test_kernel_version_reported(self):
        r = run("uname -r")
        assert r.returncode == 0
        assert len(r.stdout.strip()) > 0
