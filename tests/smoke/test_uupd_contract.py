"""
Phase 1 — uupd contract smoke checks.
Validates that Bluefin exposes update-orchestration configuration and timer signals.
"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path


class TestUupdContract:
    """Verify deterministic uupd orchestration signals on the guest."""

    @staticmethod
    def _run(*args: str, timeout: int = 15) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _uupd_config(self) -> dict:
        config_path = Path("/etc/uupd/config.json")
        assert config_path.exists(), "/etc/uupd/config.json is missing"
        return json.loads(config_path.read_text(encoding="utf-8"))

    def test_uupd_config_json_exists_and_is_structured(self):
        """uupd must ship a JSON config with module-oriented structure."""
        config = self._uupd_config()
        assert isinstance(config, dict), "uupd config.json did not parse to a JSON object"

        modules = config.get("modules")
        assert isinstance(modules, list) and modules, (
            "uupd config.json did not expose a non-empty 'modules' list:\n"
            f"{json.dumps(config, indent=2)}"
        )
        assert any(
            module in modules
            for module in ("container", "ostree", "policy", "signature", "composefs", "format")
        ), (
            "uupd config modules did not include any expected Bluefin orchestration module:\n"
            f"{json.dumps(config, indent=2)}"
        )

    def test_uupd_timer_unit_is_present(self):
        """The guest must expose a systemd timer unit for uupd orchestration."""
        result = self._run("systemctl", "show", "uupd.timer", "--property=LoadState,UnitFileState,ActiveState")
        assert result.returncode == 0, (
            "systemctl show uupd.timer failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        fields = dict(
            line.split("=", 1)
            for line in result.stdout.splitlines()
            if "=" in line
        )
        assert fields.get("LoadState") == "loaded", (
            "uupd.timer was not loaded:\n"
            f"{result.stdout}"
        )
        assert fields.get("UnitFileState") in {"enabled", "static", "generated", "indirect"}, (
            "uupd.timer did not expose an expected systemd unit-file state:\n"
            f"{result.stdout}"
        )

    def test_uupd_config_mentions_policy_or_staging_behaviour(self):
        """uupd config should expose policy or staging-related module configuration."""
        config = self._uupd_config()
        serialized = json.dumps(config).lower()
        assert any(token in serialized for token in ("policy", "stage", "bootc", "update", "container")), (
            "uupd config did not expose recognizable policy/staging-related signals:\n"
            f"{json.dumps(config, indent=2)}"
        )
