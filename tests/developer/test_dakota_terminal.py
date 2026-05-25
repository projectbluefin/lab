"""
Dakota Terminal Tests — Phase 2 (Developer Tooling)
Tests Ghostty terminal on Dakota builds (ghcr.io/projectbluefin/dakota).
Skipped automatically when BLUEFIN_VARIANT != dakota.
"""
import os
import time
import pytest
import subprocess
from dogtail.rawinput import typeText

# Skip entire module if not running against Dakota
pytestmark = pytest.mark.skipif(
    os.environ.get("BLUEFIN_VARIANT", "bluefin") != "dakota",
    reason="Dakota-specific tests — set BLUEFIN_VARIANT=dakota to run"
)


class TestGhosttyTerminal:

    def test_ghostty_launches(self, launch_ghostty):
        """Ghostty window must appear in the AT-SPI tree."""
        assert launch_ghostty is not None

    def test_ghostty_window_gains_focus(self, launch_ghostty):
        """The Ghostty frame must have keyboard focus."""
        frame = launch_ghostty.child(roleName="frame")
        assert frame is not None, "Ghostty frame not found"
        assert frame.focused or frame.active, "Ghostty window did not gain focus"

    def test_ghostty_accepts_keyboard_input(self, launch_ghostty):
        """Typing into Ghostty must not raise an error."""
        typeText("echo hello-dakota\n")
        time.sleep(1.0)


class TestDakotaBrew:

    def test_brew_in_path(self, launch_ghostty):
        """brew must be resolvable in the default Ghostty shell PATH."""
        typeText("which brew > /tmp/tmt-dakota-brew-path.txt 2>&1\n")
        time.sleep(1.0)
        result = subprocess.run(
            ["cat", "/tmp/tmt-dakota-brew-path.txt"],
            capture_output=True, text=True
        )
        assert "/homebrew" in result.stdout or "/home/linuxbrew" in result.stdout, \
            f"brew not found in PATH on Dakota: {result.stdout.strip()}"

    def test_brew_version(self, launch_ghostty):
        """brew --version must exit 0 and print a version string."""
        typeText("brew --version > /tmp/tmt-dakota-brew-version.txt 2>&1\n")
        time.sleep(2.0)
        result = subprocess.run(
            ["cat", "/tmp/tmt-dakota-brew-version.txt"],
            capture_output=True, text=True
        )
        assert "Homebrew" in result.stdout, \
            f"Unexpected brew output on Dakota: {result.stdout.strip()}"


class TestDakotaPodman:

    def test_podman_available(self):
        """podman must be installed and return a zero exit code on Dakota."""
        result = subprocess.run(
            ["podman", "--version"],
            capture_output=True, text=True, timeout=10
        )
        assert result.returncode == 0, f"podman --version failed: {result.stderr}"
        assert "podman" in result.stdout.lower()

    def test_podman_run_hello(self):
        """podman run hello-world must complete successfully on Dakota."""
        result = subprocess.run(
            ["podman", "run", "--rm", "hello-world"],
            capture_output=True, text=True, timeout=60
        )
        assert result.returncode == 0, \
            f"podman run hello-world failed on Dakota:\n{result.stderr}"
