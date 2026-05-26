"""
Phase 1 — Image-contract smoke checks.
Verifies low-risk Bluefin platform guarantees on the booted image.
"""
import json
import subprocess

import pytest


class TestImageContract:
    """Verify core image-based contract signals on the running system."""

    @staticmethod
    def _run(*args: str, timeout: int = 15) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _bootc_status_json(self) -> str:
        result = self._run("bootc", "status", "--format=json")
        if result.returncode == 0:
            return result.stdout

        stderr = (result.stderr or "").lower()
        if "must be executed as the root user" in stderr:
            sudo_result = self._run("sudo", "-n", "bootc", "status", "--format=json")
            if sudo_result.returncode == 0:
                return sudo_result.stdout
            result = sudo_result

        raise AssertionError(
            "bootc status failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def _bootc_status_verbose(self) -> str:
        result = self._run("bootc", "status", "--verbose", timeout=30)
        if result.returncode == 0:
            return result.stdout

        stderr = (result.stderr or "").lower()
        if "must be executed as the root user" in stderr:
            sudo_result = self._run("sudo", "-n", "bootc", "status", "--verbose", timeout=30)
            if sudo_result.returncode == 0:
                return sudo_result.stdout
            result = sudo_result

        raise AssertionError(
            "bootc status --verbose failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def test_bootc_reports_a_booted_deployment(self):
        """`bootc status` must succeed and report a booted deployment."""
        data = json.loads(self._bootc_status_json())
        status = data.get("status")
        assert isinstance(status, dict), (
            "bootc status JSON missing top-level 'status' object:\n"
            f"{json.dumps(data, indent=2)}"
        )
        assert status.get("booted") is not None, (
            "bootc status JSON did not report a booted deployment:\n"
            f"{json.dumps(data, indent=2)}"
        )

    def test_bootc_verbose_status_is_non_empty(self):
        """`bootc status --verbose` must emit human-readable deployment evidence."""
        output = self._bootc_status_verbose()
        assert output.strip(), "bootc status --verbose returned empty output"
        assert any(token in output.lower() for token in ("booted", "image", "deployment")), (
            "bootc status --verbose did not expose recognizable deployment details:\n"
            f"{output}"
        )

    def test_ostree_booted_marker_exists(self):
        """Image-based Bluefin hosts must expose the ostree boot marker."""
        result = self._run("test", "-e", "/run/ostree-booted", timeout=5)
        assert result.returncode == 0, "/run/ostree-booted marker is missing"

    def test_usr_mount_is_read_only_when_not_unlocked(self):
        """`/usr` should remain read-only unless an ostree unlock overlay is active."""
        result = self._run("findmnt", "--target", "/usr", "--output", "OPTIONS", "-n", timeout=10)
        assert result.returncode == 0, (
            "findmnt failed for /usr:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        options = [option.strip() for option in result.stdout.strip().split(",") if option.strip()]
        if "rw" in options:
            pytest.skip(
                "/usr is mounted rw because an ostree unlock overlay is active; "
                "this smoke assertion only applies to the locked image."
            )

        assert "ro" in options, (
            "/usr is not mounted read-only:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def test_sysroot_mount_is_read_only(self):
        """`/sysroot` should stay read-only for an image-based host."""
        result = self._run("findmnt", "--target", "/sysroot", "--output", "OPTIONS", "-n", timeout=10)
        assert result.returncode == 0, (
            "findmnt failed for /sysroot:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        options = [option.strip() for option in result.stdout.strip().split(",") if option.strip()]
        assert "ro" in options, (
            "/sysroot is not mounted read-only:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
