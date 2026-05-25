"""
Phase 1 — Notification System Smoke Test
Validates GNOME notification infrastructure is functional.
"""
import subprocess
import time

import pytest
from dogtail.rawinput import pressKey
from dogtail.tree import root


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Dismiss transient GNOME Shell UI before and after each test."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


class TestNotifications:
    """Validate notification delivery and shell stability."""

    def test_notify_send_works(self):
        """notify-send must succeed without errors."""
        result = subprocess.run(
            [
                "notify-send",
                "--app-name=bluefin-test",
                "Test Notification",
                "QA pipeline smoke test",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        assert result.returncode == 0, f"notify-send failed: {result.stderr}"

        shell = root.application("gnome-shell")
        assert shell is not None, "gnome-shell not found in AT-SPI tree"

    def test_no_notification_daemon_errors(self):
        """No notification daemon crashes in boot journal."""
        shell = root.application("gnome-shell")
        assert shell is not None, "gnome-shell not found in AT-SPI tree"
        result = subprocess.run(
            [
                "journalctl",
                "-b",
                "--no-pager",
                "-u",
                "gnome-shell",
                "--grep",
                "notification.*error|GDBus.*Error",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        errors = [
            line for line in result.stdout.splitlines()
            if "error" in line.lower() and "notification" in line.lower()
        ]
        assert len(errors) == 0, (
            "Notification errors in gnome-shell journal:\n" + "\n".join(errors)
        )
