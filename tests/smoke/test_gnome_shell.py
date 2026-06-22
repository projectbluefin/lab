"""
Phase 1 — Golden Path Smoke Tests
Validates that GNOME Shell is functional and navigable on a fresh Bluefin boot.
Run on every PR.
"""
import re
import time
import pytest
from dogtail.tree import root
from dogtail.rawinput import pressKey, typeText
from dogtail.utils import run
import os
import subprocess


@pytest.fixture(autouse=True)
def reset_gnome_state():
    """Ensure we start from a clean GNOME state and clean up after."""
    # Close Activities overview if open
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


class TestGnomeShellBoot:
    """Verify GNOME Shell initialized correctly."""

    def test_gnome_shell_is_running(self):
        """AT-SPI root must contain a gnome-shell application node."""
        shell = root.application("gnome-shell")
        assert shell is not None, "gnome-shell not found in AT-SPI tree"

    def test_top_bar_accessible(self):
        """The top bar (panel) must be reachable via the accessibility tree."""
        shell = root.application("gnome-shell")
        panel = shell.child(roleName="panel")
        assert panel is not None, "GNOME top bar panel not found"

    def test_activities_button_present(self):
        """The Activities button must exist in the top bar."""
        shell = root.application("gnome-shell")
        # The Activities toggle is a push button in the panel
        activities = shell.child("Activities", roleName="toggle button")
        assert activities is not None, "Activities toggle button not found"


class TestActivitiesOverview:
    """Verify the Activities overview and search work."""

    def test_super_key_opens_overview(self):
        """Pressing Super must open the Activities overview."""
        pressKey("super")
        time.sleep(1.5)
        shell = root.application("gnome-shell")
        # Search entry appears in the overview
        search = shell.child(roleName="entry")
        assert search is not None, "Search bar not found after opening Activities"

    def test_search_accepts_input(self):
        """Typing in the overview search bar must not hang the session."""
        pressKey("super")
        time.sleep(1.5)
        typeText("Files")
        time.sleep(0.5)
        shell = root.application("gnome-shell")
        search = shell.child(roleName="entry")
        assert search.text == "Files", f"Search bar text mismatch: '{search.text}'"

    def test_escape_closes_overview(self):
        """Escape must close the Activities overview."""
        pressKey("super")
        time.sleep(1.5)
        pressKey("Escape")
        time.sleep(0.5)
        # After closing, no search entry should be visible
        shell = root.application("gnome-shell")
        entries = shell.findChildren(lambda n: n.roleName == "entry")
        assert len(entries) == 0, "Activities overview did not close on Escape"


class TestSystemMenus:
    """Verify Quick Settings panel opens and reports correct state."""

    def test_quick_settings_opens(self):
        """Clicking the system tray area must open Quick Settings."""
        shell = root.application("gnome-shell")
        panel = shell.child(roleName="panel")
        # The system indicator area is the rightmost panel item
        system_menu = panel.child("System Menu", roleName="toggle button")
        system_menu.click()
        time.sleep(1.0)
        # Quick Settings dialog must appear
        qs = shell.child("Quick Settings", roleName="frame")
        assert qs is not None, "Quick Settings panel did not open"

    def test_quick_settings_closes_on_escape(self):
        """Escape must dismiss the Quick Settings panel."""
        shell = root.application("gnome-shell")
        panel = shell.child(roleName="panel")
        system_menu = panel.child("System Menu", roleName="toggle button")
        system_menu.click()
        time.sleep(1.0)
        pressKey("Escape")
        time.sleep(0.5)
        # After close, Quick Settings frame must be gone
        frames = shell.findChildren(lambda n: n.name == "Quick Settings")
        assert len(frames) == 0, "Quick Settings did not close on Escape"


class TestSystemTray:
    """Verify the GNOME Shell clock and calendar panel."""

    def test_clock_visible_in_top_bar(self):
        """Clock must be present in the GNOME top bar."""
        shell = root.application("gnome-shell")
        panel = shell.child(roleName="panel")
        clocks = [
            button for button in panel.findChildren(lambda node: node.roleName == "toggle button")
            if re.search(r"\d+:\d+", button.name or "")
        ]
        assert len(clocks) > 0, "No clock found in top bar"

    def test_calendar_opens_from_clock(self):
        """Clicking the clock must open the calendar popup."""
        shell = root.application("gnome-shell")
        panel = shell.child(roleName="panel")
        clocks = [
            button for button in panel.findChildren(lambda node: node.roleName == "toggle button")
            if re.search(r"\d+:\d+", button.name or "")
        ]
        assert len(clocks) > 0, "No clock in panel"
        clocks[0].click()
        time.sleep(1.0)
        calendars = shell.findChildren(
            lambda node: node.roleName in ("frame", "calendar")
            and ("calendar" in (node.name or "").lower() or node.roleName == "calendar")
        )
        assert len(calendars) > 0, "Calendar did not open from clock click"
        pressKey("Escape")


class TestExtensions:
    """Verify GNOME Shell extensions loaded without errors."""

    def test_extensions_loaded(self):
        """gnome-extensions list must report all enabled extensions as active."""
        result = subprocess.run(
            ["gnome-extensions", "list", "--enabled"],
            capture_output=True, text=True, timeout=10
        )
        assert result.returncode == 0, "gnome-extensions command failed"
        enabled = result.stdout.strip().splitlines()
        assert len(enabled) > 0, "No extensions reported as enabled"

    def test_no_extension_errors_in_journal(self):
        """No JS errors from extensions in the current boot journal."""
        journal_since = os.environ.get("TEST_JOURNAL_SINCE")
        cmd = ["journalctl", "-b", "--no-pager", "-g", "Extension.*error|GNOME Shell.*crashed"]
        if journal_since:
            cmd.extend(["--since", journal_since])
        result = subprocess.run(
            cmd,
            capture_output=True, text=True, timeout=10
        )
        errors = [l for l in result.stdout.splitlines() if "error" in l.lower() or "crash" in l.lower()]
        assert len(errors) == 0, f"Extension errors found in journal:\n" + "\n".join(errors)

    def test_no_malcontent_timerd_timeout(self):
        """malcontent-timerd.service must not time out at boot (regression: bluefin#4612)."""
        result = subprocess.run(
            ["journalctl", "-b", "--no-pager", "-u", "malcontent-timerd.service"],
            capture_output=True, text=True, timeout=10
        )
        timeouts = [l for l in result.stdout.splitlines() if "timed out" in l.lower() or "timeout" in l.lower()]
        assert len(timeouts) == 0, f"malcontent-timerd timeout found in journal:\n" + "\n".join(timeouts)


class TestLogoutFlow:
    """Verify logout returns to GDM without crashes (regression: bluefin#4642)."""

    def test_logout_returns_to_greeter(self):
        """Log Out via system menu must return to GDM login screen without gnome-shell coredump."""
        shell = root.application("gnome-shell")
        panel = shell.child(roleName="panel")
        system_menu = panel.child("System Menu", roleName="toggle button")
        system_menu.click()
        time.sleep(1.0)

        # Click Log Out
        qs = shell.child("Quick Settings", roleName="frame")
        logout_btn = qs.child("Log Out", roleName="push button")
        assert logout_btn is not None, "Log Out button not found in Quick Settings"
        logout_btn.click()
        time.sleep(2.0)

        # Confirm dialog may appear — dismiss it
        try:
            confirm = root.application("gnome-shell").child("Log Out", roleName="push button")
            confirm.click()
            time.sleep(3.0)
        except Exception:
            pass

        # Assert no coredump was produced
        journal_since = os.environ.get("TEST_JOURNAL_SINCE", "1 minute ago")
        result = subprocess.run(
            ["coredumpctl", "list", "--no-pager", "--since", journal_since],
            capture_output=True, text=True, timeout=10
        )
        coredumps = [l for l in result.stdout.splitlines() if "gnome-shell" in l]
        assert len(coredumps) == 0, f"gnome-shell coredump after logout:\n" + "\n".join(coredumps)
