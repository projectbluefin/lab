"""
Phase 2 — Podman Desktop Integration Test
Validates Podman Desktop Flatpak launches and shows containers panel.
Regression: projectbluefin/dakota#430
"""
import subprocess
import time

import pytest
from dogtail.rawinput import pressKey, typeText
from dogtail.tree import root


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Start and end each test with GNOME Shell overlays dismissed."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture
def launch_podman_desktop():
    """Launch Podman Desktop from the overview and close it after each test."""
    app_info = subprocess.run(
        ["flatpak", "info", "io.podman_desktop.PodmanDesktop"],
        capture_output=True,
        text=True,
        timeout=10,
    )
    if app_info.returncode != 0:
        pytest.skip("Podman Desktop not installed")

    pressKey("super")
    time.sleep(1.5)
    typeText("Podman Desktop")
    time.sleep(1.0)
    pressKey("Return")
    time.sleep(5.0)

    app = root.application("podman-desktop")
    assert app is not None, "Podman Desktop not found in AT-SPI tree after launch"

    yield app

    try:
        app.child(roleName="frame").keyCombo("<Alt>F4")
        time.sleep(1.0)
    except Exception:
        pass
    pressKey("Escape")
    time.sleep(0.5)


class TestPodmanDesktop:
    """Validate Podman Desktop basic accessibility and startup health."""

    def test_podman_desktop_launches(self, launch_podman_desktop):
        """Podman Desktop must appear in AT-SPI tree."""
        assert launch_podman_desktop is not None

    def test_containers_panel_accessible(self, launch_podman_desktop):
        """Containers navigation item must be accessible."""
        containers = launch_podman_desktop.findChildren(
            lambda node: "container" in (node.name or "").lower()
            and node.roleName in ("list item", "push button", "link")
        )
        assert len(containers) > 0, "Containers panel not found in Podman Desktop"

    def test_no_permission_error_on_connect(self, launch_podman_desktop):
        """No permission error dialog must appear on first launch (regression: dakota#430)."""
        time.sleep(2.0)
        error_dialogs = root.findChildren(
            lambda node: node.roleName == "dialog"
            and any(
                word in (node.name or "").lower()
                for word in ("permission", "error", "denied", "failed")
            )
        )
        assert len(error_dialogs) == 0, (
            "Permission error dialog on Podman Desktop launch: "
            f"{[dialog.name for dialog in error_dialogs]}"
        )
