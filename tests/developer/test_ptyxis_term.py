"""
Phase 2 — Terminal and Developer Environment Tests
Validates Ptyxis launch, Homebrew path, and shell environment on Bluefin.
Fixtures (launch_ptyxis, launch_ghostty) are in conftest.py.
"""
import os
import time
import subprocess
from dogtail.rawinput import pressKey, typeText


class TestPtyxisTerminal:

    def test_ptyxis_launches(self, launch_ptyxis):
        """Ptyxis window must appear in the AT-SPI tree."""
        assert launch_ptyxis is not None

    def test_ptyxis_window_gains_focus(self, launch_ptyxis):
        """The Ptyxis frame must have keyboard focus."""
        frame = launch_ptyxis.child(roleName="frame")
        assert frame is not None, "Ptyxis frame not found"
        assert frame.focused or frame.active, "Ptyxis window did not gain focus"

    def test_ptyxis_accepts_keyboard_input(self, launch_ptyxis):
        """Typing into Ptyxis must not raise an error."""
        typeText("echo hello-bluefin\n")
        time.sleep(1.0)
        # No assertion on output — tmt collects the shell session log


class TestBrew:

    def test_brew_in_path(self, launch_ptyxis):
        """brew must be resolvable in the default Ptyxis shell PATH."""
        typeText("which brew > /tmp/tmt-brew-path.txt 2>&1\n")
        time.sleep(1.0)
        result = subprocess.run(
            ["cat", "/tmp/tmt-brew-path.txt"],
            capture_output=True, text=True
        )
        assert "/homebrew" in result.stdout or "/home/linuxbrew" in result.stdout, \
            f"brew not found in PATH: {result.stdout.strip()}"

    def test_brew_version(self, launch_ptyxis):
        """brew --version must exit 0 and print a version string."""
        typeText("brew --version > /tmp/tmt-brew-version.txt 2>&1\n")
        time.sleep(2.0)
        result = subprocess.run(
            ["cat", "/tmp/tmt-brew-version.txt"],
            capture_output=True, text=True
        )
        assert "Homebrew" in result.stdout, \
            f"Unexpected brew output: {result.stdout.strip()}"


class TestPodman:

    def test_podman_available(self):
        """podman must be installed and return a zero exit code."""
        result = subprocess.run(
            ["podman", "--version"],
            capture_output=True, text=True, timeout=10
        )
        assert result.returncode == 0, f"podman --version failed: {result.stderr}"
        assert "podman" in result.stdout.lower()

    def test_podman_run_hello(self):
        """podman run hello-world must complete successfully."""
        result = subprocess.run(
            ["podman", "run", "--rm", "hello-world"],
            capture_output=True, text=True, timeout=60
        )
        assert result.returncode == 0, f"podman run hello-world failed:\n{result.stderr}"


class TestPtyxisRegressions:
    """Regression tests for known Ptyxis bugs."""

    def test_no_vulkan_spam_in_journal(self, launch_ptyxis):
        """Ptyxis must not flood the journal with vkAcquireNextImageKHR errors (regression: bluefin#4620)."""
        # Open, use, and resize the terminal to trigger rendering
        typeText("echo resize-test\n")
        time.sleep(0.5)
        frame = launch_ptyxis.child(roleName="frame")
        # Resize by changing window size via AT-SPI actions if available
        try:
            frame.resizeTo(800, 600)
            time.sleep(1.0)
            frame.resizeTo(1200, 800)
            time.sleep(1.0)
        except Exception:
            pass  # resize not available in all AT-SPI implementations

        journal_since = os.environ.get("TEST_JOURNAL_SINCE", "1 minute ago")
        result = subprocess.run(
            ["journalctl", "-b", "--no-pager", "-g", "vkAcquireNextImageKHR",
             "--since", journal_since],
            capture_output=True, text=True, timeout=10
        )
        vulkan_errors = [l for l in result.stdout.splitlines()
                         if "vkAcquireNextImageKHR" in l]
        assert len(vulkan_errors) < 5, \
            f"Ptyxis flooding journal with Vulkan errors ({len(vulkan_errors)} lines):\n" \
            + "\n".join(vulkan_errors[:10])

