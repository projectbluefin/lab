"""
Phase 1 — Core desktop application smoke tests.
Validates real Files and Settings workflows via AT-SPI.
"""
from __future__ import annotations

import os
import subprocess
import time
import uuid
from pathlib import Path

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


def _matching_nodes(container, *needles: str):
    lowered = [needle.lower() for needle in needles]
    return container.findChildren(
        lambda node: bool(node.showing)
        and bool(node.name)
        and any(needle in node.name.lower() for needle in lowered)
    )


def _wait_for_named_node(container, *needles: str, timeout: float = 10.0, interactive_only: bool = False):
    deadline = time.time() + timeout
    while time.time() < deadline:
        matches = _matching_nodes(container, *needles)
        if interactive_only:
            matches = [
                node
                for node in matches
                if node.roleName in {"push button", "table cell", "list item", "icon", "link", "page tab"}
                or bool(getattr(node, "actions", []))
            ]
        if matches:
            return matches[0]
        time.sleep(0.5)
    raise AssertionError(f"Timed out waiting for visible node matching: {needles}")


def _activate_node(node) -> None:
    try:
        node.click()
        return
    except Exception:  # noqa: BLE001
        pass

    try:
        node.doActionNamed("click")
        return
    except Exception:  # noqa: BLE001
        pass

    try:
        node.doActionNamed("activate")
        return
    except Exception as exc:  # noqa: BLE001
        raise AssertionError(f"Unable to activate node {node.name!r}") from exc


def _expected_os_tokens() -> list[str]:
    pretty_name = os.environ.get("PRETTY_NAME", "").strip()
    if not pretty_name:
        result = subprocess.run(
            ["bash", "-lc", ". /etc/os-release && printf '%s' \"$PRETTY_NAME\""],
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        pretty_name = result.stdout.strip()

    if not pretty_name:
        return ["Bluefin", "Fedora"]

    tokens = [part.strip("()") for part in pretty_name.replace('"', "").split()]
    preferred = [token for token in tokens if token.lower() not in {"linux", "desktop"}]
    return preferred[:3] or ["Bluefin", "Fedora"]


def _downloads_dir() -> Path:
    result = subprocess.run(
        ["xdg-user-dir", "DOWNLOAD"],
        capture_output=True,
        text=True,
        timeout=5,
        check=False,
    )
    if result.returncode == 0 and result.stdout.strip():
        return Path(result.stdout.strip())
    return Path.home() / "Downloads"


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Dismiss transient shell UI before and after each test."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture
def launch_files():
    """Launch Files in the home directory and close it after the test."""
    subprocess.run(["nautilus", "--new-window", str(Path.home())], check=True, timeout=10)
    time.sleep(3.0)

    app = _wait_for_application("org.gnome.Nautilus", "Files", "nautilus")
    yield app
    _close_application(app)


@pytest.fixture
def launch_settings():
    """Launch Settings and close it after the test."""
    subprocess.run(["gnome-control-center"], check=True, timeout=10)
    time.sleep(3.0)

    app = _wait_for_application("org.gnome.Settings", "gnome-control-center", "Settings")
    yield app
    _close_application(app)


@pytest.fixture
def files_search_fixture():
    """Create a disposable searchable fixture under Downloads."""
    downloads_dir = _downloads_dir()
    downloads_dir.mkdir(parents=True, exist_ok=True)

    unique_name = f"copilot-files-fixture-{uuid.uuid4().hex[:8]}"
    fixture_dir = downloads_dir / unique_name
    fixture_dir.mkdir()
    (fixture_dir / "evidence.txt").write_text("testing-lab files workflow fixture\n", encoding="utf-8")

    yield {
        "downloads_dir": downloads_dir,
        "name": unique_name,
        "path": fixture_dir,
    }

    if fixture_dir.exists():
        subprocess.run(["rm", "-rf", str(fixture_dir)], check=False, timeout=10)


class TestCoreDesktopApplications:
    """Validate core GNOME applications required for a usable desktop."""

    def test_files_navigates_searches_and_opens_result(self, launch_files, files_search_fixture):
        """Files must navigate to Downloads, search a fixture, and open it."""
        frame = _find_frame(launch_files)

        frame.keyCombo("<Control>l")
        time.sleep(0.5)
        typeText(str(files_search_fixture["downloads_dir"]))
        pressKey("Return")
        time.sleep(2.0)

        _wait_for_named_node(launch_files, "Downloads")

        frame.keyCombo("<Control>f")
        time.sleep(0.5)
        typeText(files_search_fixture["name"])
        time.sleep(2.0)

        result_node = _wait_for_named_node(
            launch_files,
            files_search_fixture["name"],
            interactive_only=True,
        )
        _activate_node(result_node)
        time.sleep(2.0)

        pressKey("Escape")
        time.sleep(1.0)
        _wait_for_named_node(launch_files, files_search_fixture["name"], "evidence.txt")

    def test_settings_navigates_about_and_appearance_panels(self, launch_settings):
        """Settings must expose Bluefin identity and allow navigation to another stable panel."""
        os_identity = _expected_os_tokens()

        about_entry = _wait_for_named_node(launch_settings, "About", "System", interactive_only=True)
        _activate_node(about_entry)
        time.sleep(1.5)

        assert _matching_nodes(launch_settings, "Operating System"), (
            "Settings About/System panel did not expose an 'Operating System' label"
        )
        assert _matching_nodes(launch_settings, *os_identity), (
            "Settings About/System panel did not expose Bluefin OS identity.\n"
            f"Expected one of: {os_identity}"
        )

        appearance_entry = _wait_for_named_node(launch_settings, "Appearance", interactive_only=True)
        _activate_node(appearance_entry)
        time.sleep(1.5)

        assert _matching_nodes(launch_settings, "Style", "Accent Color", "Background"), (
            "Settings Appearance panel did not expose expected stable content"
        )
