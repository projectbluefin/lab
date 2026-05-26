"""
Phase 1 — ostree filesystem smoke tests.
Validates deployment directories and ostree admin visibility.
"""
from __future__ import annotations

import subprocess
from pathlib import Path


class TestOstreeFilesystem:
    """Verify ostree deployment filesystem signals on the guest."""

    @staticmethod
    def _run(*args: str, timeout: int = 20) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def test_sysroot_ostree_deploy_directory_exists(self):
        """Image-based hosts must expose the sysroot ostree deployment directory."""
        deploy_path = Path("/sysroot/ostree/deploy")
        assert deploy_path.exists(), "/sysroot/ostree/deploy is missing"
        assert any(deploy_path.iterdir()), "/sysroot/ostree/deploy did not contain any deployment entries"

    def test_ostree_admin_status_lists_deployments(self):
        """ostree admin status must return visible deployment information."""
        result = self._run("ostree", "admin", "status")
        assert result.returncode == 0, (
            "ostree admin status failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        lowered = result.stdout.lower()
        assert any(token in lowered for token in ("deploy", "version", "checksum", "ostree://", "*")), (
            "ostree admin status did not expose recognizable deployment details:\n"
            f"{result.stdout}"
        )

    def test_usr_path_resides_under_ostree_sysroot(self):
        """The booted /usr path should resolve under the ostree-managed sysroot tree."""
        result = self._run("readlink", "-f", "/usr")
        assert result.returncode == 0, (
            "readlink -f /usr failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        resolved = result.stdout.strip()
        assert resolved.startswith("/usr") or resolved.startswith("/sysroot"), (
            "Resolved /usr path did not look ostree-managed:\n"
            f"{resolved}"
        )
