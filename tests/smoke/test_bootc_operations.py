"""
Phase 1 — bootc operations smoke checks.
Validates safe bootc command paths and ostree deployment visibility.
"""
from __future__ import annotations

import json
import subprocess


class TestBootcOperations:
    """Verify deterministic bootc command behavior without mutating the guest."""

    @staticmethod
    def _run(*args: str, timeout: int = 30) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _run_bootc(self, *args: str) -> subprocess.CompletedProcess[str]:
        result = self._run("bootc", *args)
        if result.returncode == 0:
            return result

        stderr = (result.stderr or "").lower()
        if "must be executed as the root user" in stderr:
            sudo_result = self._run("sudo", "-n", "bootc", *args)
            if sudo_result.returncode == 0:
                return sudo_result
            result = sudo_result

        raise AssertionError(
            f"bootc {' '.join(args)} failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def test_bootc_upgrade_check_completes(self):
        """A dry-run upgrade check must execute without mutating the host."""
        result = self._run_bootc("upgrade", "--check")
        output = f"{result.stdout}\n{result.stderr}".lower()
        assert any(token in output for token in ("update", "cached", "available", "no updates", "latest")), (
            "bootc upgrade --check did not expose recognizable upgrade-check output:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def test_bootc_status_json_exposes_staged_or_cached_keys(self):
        """bootc status JSON should expose staged-update visibility fields when available."""
        result = self._run_bootc("status", "--format=json")
        data = json.loads(result.stdout)
        status = data.get("status")
        assert isinstance(status, dict), (
            "bootc status JSON missing top-level 'status' object:\n"
            f"{json.dumps(data, indent=2)}"
        )
        assert any(key in status for key in ("staged", "cachedUpdate", "rollback", "booted")), (
            "bootc status JSON did not expose staged/cached deployment visibility keys:\n"
            f"{json.dumps(data, indent=2)}"
        )

    def test_ostree_admin_status_reports_a_deployment(self):
        """ostree admin status must report at least one deployment checksum/ref."""
        result = self._run("ostree", "admin", "status", timeout=20)
        assert result.returncode == 0, (
            "ostree admin status failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        lowered = result.stdout.lower()
        assert any(token in lowered for token in ("*", "ostree://", "checksum", "deployments:", "version:")), (
            "ostree admin status did not expose recognizable deployment output:\n"
            f"{result.stdout}"
        )
