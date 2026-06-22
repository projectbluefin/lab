"""
Phase 1 — Extension runtime smoke checks.
Validates enabled extension metadata and installed-path signals beyond list-only smoke.
"""
from __future__ import annotations

import subprocess
from pathlib import Path


class TestExtensionRuntime:
    """Verify enabled extensions expose queryable metadata and installed files."""

    @staticmethod
    def _run(*args: str, timeout: int = 15) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _enabled_extensions(self) -> list[str]:
        result = self._run("gnome-extensions", "list", "--enabled")
        assert result.returncode == 0, (
            "gnome-extensions list --enabled failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        extensions = [line.strip() for line in result.stdout.splitlines() if line.strip()]
        assert extensions, "No enabled GNOME extensions were reported"
        return extensions

    def test_enabled_extensions_have_queryable_metadata(self):
        """Each enabled extension must return metadata via gnome-extensions info."""
        for extension in self._enabled_extensions():
            result = self._run("gnome-extensions", "info", extension)
            assert result.returncode == 0, (
                f"gnome-extensions info failed for {extension}:\n"
                f"stdout:\n{result.stdout}\n"
                f"stderr:\n{result.stderr}"
            )
            lowered = result.stdout.lower()
            assert any(token in lowered for token in ("name:", "path:", "state:")), (
                f"Extension info output for {extension} did not expose expected metadata:\n"
                f"{result.stdout}"
            )

    def test_enabled_extensions_resolve_to_installed_paths(self):
        """Enabled extension metadata should resolve to an installed extension path."""
        for extension in self._enabled_extensions():
            result = self._run("gnome-extensions", "info", extension)
            assert result.returncode == 0, (
                f"gnome-extensions info failed for {extension}:\n"
                f"stdout:\n{result.stdout}\n"
                f"stderr:\n{result.stderr}"
            )

            path_line = next(
                (line for line in result.stdout.splitlines() if line.lower().startswith("path:")),
                None,
            )
            assert path_line is not None, f"Extension info for {extension} did not include a Path line"

            extension_path = Path(path_line.split(":", 1)[1].strip())
            assert extension_path.exists(), (
                f"Extension path from gnome-extensions info does not exist for {extension}: {extension_path}"
            )

    def test_enabled_extensions_report_active_state(self):
        """Enabled extensions must report an active/enabled style state in metadata."""
        for extension in self._enabled_extensions():
            result = self._run("gnome-extensions", "info", extension)
            assert result.returncode == 0, (
                f"gnome-extensions info failed for {extension}:\n"
                f"stdout:\n{result.stdout}\n"
                f"stderr:\n{result.stderr}"
            )
            lowered = result.stdout.lower()
            assert any(token in lowered for token in ("active", "enabled", "state:")), (
                f"Extension info for {extension} did not report an enabled/active state:\n"
                f"{result.stdout}"
            )
