"""
Phase 1 — Flatpak smoke checks.
Regression: ublue-os/bluefin#4403
"""
import subprocess


class TestFlatpakHealth:
    """Verify system Flatpak metadata is healthy on the running image."""

    @staticmethod
    def _run_flatpak(*args: str, timeout: int = 120) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["flatpak", *args],
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def test_flatpak_repair_system_exits_zero(self):
        """`flatpak repair --system` must succeed on a fresh session."""
        result = self._run_flatpak("repair", "--system")
        assert result.returncode == 0, (
            "flatpak repair failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def test_system_flatpak_remotes_are_configured(self):
        """System Flatpak remotes must be queryable and non-empty."""
        result = self._run_flatpak("remotes", "--system", "--columns=name")
        assert result.returncode == 0, (
            "flatpak remotes --system failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        remotes = [line.strip() for line in result.stdout.splitlines() if line.strip()]
        assert remotes, (
            "No system Flatpak remotes were configured.\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    def test_system_flatpak_apps_are_visible(self):
        """System-scoped Flatpak applications must be discoverable."""
        result = self._run_flatpak("list", "--system", "--app", "--columns=application")
        assert result.returncode == 0, (
            "flatpak list --system --app failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

        applications = [line.strip() for line in result.stdout.splitlines() if line.strip()]
        assert applications, (
            "No system Flatpak applications were listed.\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
