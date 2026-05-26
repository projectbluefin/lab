"""
Phase 1 — Browser integration smoke tests.
Validates the default browser and xdg-open desktop integration.
"""
from __future__ import annotations

import subprocess
import time
import uuid
from pathlib import Path

import pytest
from dogtail.rawinput import pressKey
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


def _wait_for_application(*candidate_names: str, timeout: float = 15.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        app = _find_application(*candidate_names)
        if app is not None:
            return app
        time.sleep(0.5)
    raise AssertionError(f"Browser application did not appear in the AT-SPI tree: {candidate_names}")


def _find_frame(app):
    frames = app.findChildren(lambda node: node.roleName == "frame")
    assert frames, f"No application frame found for {app.name!r}"
    return frames[0]


def _close_application(app) -> None:
    frame = _find_frame(app)
    frame.keyCombo("<Alt>F4")
    time.sleep(1.0)
    pressKey("Escape")
    time.sleep(0.5)


def _browser_candidates(desktop_entry: str) -> list[str]:
    stripped = desktop_entry.removesuffix(".desktop")
    return [
        desktop_entry,
        stripped,
        stripped.replace("-", " "),
        "firefox",
        "Firefox",
        "org.mozilla.firefox",
        "Navigator",
        "epiphany",
        "Web",
        "org.gnome.Epiphany",
    ]


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Dismiss transient shell UI before and after each test."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture
def browser_fixture_page():
    """Create a deterministic local HTML page for browser coverage."""
    fixture_path = Path("/tmp") / f"testing-lab-browser-{uuid.uuid4().hex[:8]}.html"
    title = "Testing Lab Browser Fixture"
    body = "Default browser integration is working."
    fixture_path.write_text(
        (
            "<!doctype html>\n"
            "<html>\n"
            "  <head>\n"
            f"    <title>{title}</title>\n"
            "  </head>\n"
            "  <body>\n"
            f"    <main><h1>{title}</h1><p>{body}</p></main>\n"
            "  </body>\n"
            "</html>\n"
        ),
        encoding="utf-8",
    )
    yield {
        "title": title,
        "body": body,
        "path": fixture_path,
        "url": fixture_path.as_uri(),
    }
    fixture_path.unlink(missing_ok=True)


class TestBrowserDesktopIntegration:
    """Validate the default browser and desktop-opened local content."""

    def test_default_browser_is_configured(self):
        """A default browser desktop entry must be configured for the session."""
        result = subprocess.run(
            ["xdg-settings", "get", "default-web-browser"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert result.returncode == 0, (
            "xdg-settings get default-web-browser failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        assert result.stdout.strip(), "No default browser desktop entry was configured"

    def test_default_browser_opens_local_fixture_via_xdg_open(self, browser_fixture_page):
        """xdg-open must hand a local HTML page to the configured browser."""
        result = subprocess.run(
            ["xdg-settings", "get", "default-web-browser"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert result.returncode == 0, (
            "xdg-settings get default-web-browser failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
        desktop_entry = result.stdout.strip()
        assert desktop_entry, "No default browser desktop entry was configured"

        subprocess.run(
            ["xdg-open", browser_fixture_page["url"]],
            check=True,
            timeout=10,
        )
        time.sleep(3.0)

        app = _wait_for_application(*_browser_candidates(desktop_entry))
        try:
            frame = _find_frame(app)
            visible_names = [
                node.name for node in app.findChildren(lambda node: bool(node.showing) and bool(node.name))
            ]
            joined = "\n".join(visible_names + [frame.name or ""])
            assert browser_fixture_page["title"] in joined, (
                "The browser did not expose the local fixture title in AT-SPI-visible UI.\n"
                f"Visible names:\n{joined}"
            )
        finally:
            _close_application(app)
