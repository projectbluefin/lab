"""
Phase 2 — Homebrew bootstrap smoke tests.
Validates brew-setup service visibility and shell integration.
"""
from __future__ import annotations

import subprocess


class TestBrewBootstrap:
    """Verify Homebrew bootstrap and shellenv exposure."""

    @staticmethod
    def _run(*args: str, timeout: int = 20) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _brew_setup_status(self) -> subprocess.CompletedProcess[str]:
        user_result = self._run(
            "systemctl",
            "--user",
            "show",
            "brew-setup.service",
            "--property=LoadState,ActiveState,Result,UnitFileState",
        )
        if user_result.returncode == 0:
            return user_result

        system_result = self._run(
            "systemctl",
            "show",
            "brew-setup.service",
            "--property=LoadState,ActiveState,Result,UnitFileState",
        )
        return system_result

    def test_brew_setup_service_is_present(self):
        """brew-setup.service must be present in either the user or system manager."""
        result = self._brew_setup_status()
        assert result.returncode == 0, (
            "brew-setup.service was not visible to systemctl:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        fields = dict(
            line.split("=", 1)
            for line in result.stdout.splitlines()
            if "=" in line
        )
        assert fields.get("LoadState") == "loaded", (
            "brew-setup.service was not loaded:\n"
            f"{result.stdout}"
        )
        assert fields.get("Result", "success") in {"success", "done", ""}, (
            "brew-setup.service did not report a successful result:\n"
            f"{result.stdout}"
        )

    def test_brew_is_available_in_login_shell(self):
        """brew must resolve from a standard login shell."""
        result = self._run("bash", "-lc", "command -v brew && brew --prefix", timeout=30)
        assert result.returncode == 0, (
            "brew was not available in a login shell:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        output = result.stdout.strip()
        assert "/homebrew" in output or "/home/linuxbrew" in output, (
            "brew resolved, but the reported prefix was unexpected:\n"
            f"{output}"
        )

    def test_brew_shellenv_outputs_prefix_exports(self):
        """brew shellenv must emit export lines for shell integration."""
        result = self._run("bash", "-lc", "brew shellenv", timeout=30)
        assert result.returncode == 0, (
            "brew shellenv failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        lowered = result.stdout.lower()
        assert "export" in lowered and "brew" in lowered, (
            "brew shellenv did not emit recognizable shell integration output:\n"
            f"{result.stdout}"
        )
