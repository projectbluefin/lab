"""
Phase 1 — Integrity contract smoke checks.
Validates composefs fallback and signature-policy signals without mutating the guest.
"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path


class TestIntegrityContract:
    """Verify low-risk integrity signals on the running image."""

    @staticmethod
    def _run(*args: str, timeout: int = 15) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def test_prepare_root_conf_exists_and_mentions_integrity_path(self):
        """prepare-root fallback config must be present and mention composefs or related policy."""
        config_path = Path("/etc/prepare-root.conf")
        assert config_path.exists(), "/etc/prepare-root.conf is missing"

        content = config_path.read_text(encoding="utf-8")
        lowered = content.lower()
        assert any(token in lowered for token in ("composefs", "ostree", "transient", "verity")), (
            "/etc/prepare-root.conf did not expose expected integrity/fallback signals:\n"
            f"{content}"
        )

    def test_container_signature_policy_json_is_present(self):
        """Container signature policy must exist and expose transport/default rules."""
        policy_path = Path("/etc/containers/policy.json")
        assert policy_path.exists(), "/etc/containers/policy.json is missing"

        policy = json.loads(policy_path.read_text(encoding="utf-8"))
        assert isinstance(policy, dict), "policy.json did not parse to a JSON object"
        assert "transports" in policy or "default" in policy, (
            "policy.json did not expose transports/default policy keys:\n"
            f"{json.dumps(policy, indent=2)}"
        )

    def test_usr_mount_or_prepare_root_reports_composefs_path(self):
        """Either the live /usr mount or prepare-root config should expose composefs expectations."""
        result = self._run("findmnt", "--target", "/usr", "--output", "FSTYPE,OPTIONS", "-n", timeout=10)
        assert result.returncode == 0, (
            "findmnt failed for /usr:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        mount_info = result.stdout.lower()
        if "composefs" in mount_info:
            return

        prepare_root = Path("/etc/prepare-root.conf").read_text(encoding="utf-8").lower()
        assert "composefs" in prepare_root, (
            "Neither /usr mount info nor /etc/prepare-root.conf exposed composefs expectations:\n"
            f"findmnt:\n{result.stdout}\nprepare-root.conf:\n{prepare_root}"
        )
