"""
Phase 1 — GNOME System Monitor smoke tests.
Validates tab visibility and basic process listing.
"""
from __future__ import annotations

import time

import pytest
from dogtail.rawinput import pressKey, typeText
from dogtail.tree import root


def _find_application(*candidate_names: str):
    for candidate in candidate_names:
        try:
            app = root.application(candidate)
            if app is not None:
                return app
        except Exception:  # noqa: BLE001
            pass
    return None


def _wait_for_application(*candidate_names: str, timeout: float = 12.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        app = _find_application(*candidate_names)
        if app is not None:
            return app
        time.sleep(0.5)
    raise AssertionError(f"Application did not appear in the AT-SPI tree: {candidate_names}")


def _matching_nodes(container, *needles: str):
    lowered = [needle.lower() for needle in needles]
    return container.findChildren(
        lambda node: bool(node.showing)
        and bool(node.name)
        and any(needle in node.name.lower() for needle in lowered)
    )


def _find_frame(app):
    frames = app.findChildren(lambda node: node.roleName == "frame")
    assert frames, f"No application frame found for {app.name!r}"
    return frames[0]


def _close_application(app) -> None:
    _find_frame(app).keyCombo("<Alt>F4")
    time.sleep(1.0)
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Dismiss transient shell UI before and after each test."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture
def launch_system_monitor():
    """Launch System Monitor from Activities and close it after the test."""
    pressKey("super")
    time.sleep(1.5)
    typeText("System Monitor")
    time.sleep(1.0)
    pressKey("Return")
    time.sleep(3.0)

    app = _wait_for_application("org.gnome.SystemMonitor", "gnome-system-monitor", "System Monitor")
    yield app
    _close_application(app)


class TestSystemMonitor:
    """Validate key System Monitor tabs and process visibility."""

    def test_system_monitor_exposes_core_tabs(self, launch_system_monitor):
        """Processes, Resources, and File Systems tabs must be visible."""
        for tab_name in ("Processes", "Resources", "File Systems"):
            assert _matching_nodes(launch_system_monitor, tab_name), (
                f"System Monitor did not expose the {tab_name!r} tab"
            )

    def test_processes_tab_shows_core_processes(self, launch_system_monitor):
        """Processes tab must surface at least one stable desktop/system process."""
        process_matches = _matching_nodes(
            launch_system_monitor,
            "gnome-shell",
            "systemd",
            "dbus",
        )
        assert process_matches, "System Monitor did not surface expected core processes"
