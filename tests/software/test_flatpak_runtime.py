"""
Phase 3 — Installed Flatpak runtime smoke tests.
Validates that Bazaar reflects real system-installed Flatpaks and exposes runtime actions.
"""
from __future__ import annotations

import subprocess
import time

import pytest
from dogtail.rawinput import pressKey, typeText
from dogtail.tree import root


def _matching_nodes(container, *needles: str):
    lowered = [needle.lower() for needle in needles if needle]
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


def _system_flatpak_apps() -> list[dict[str, str]]:
    result = subprocess.run(
        ["flatpak", "list", "--system", "--app", "--columns=application,name"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        raise AssertionError(
            "flatpak list --system --app failed:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    apps = []
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        parts = [part.strip() for part in line.split("\t")]
        if len(parts) == 1:
            application = parts[0]
            display_name = parts[0].split(".")[-1]
        else:
            application, display_name = parts[0], parts[1] or parts[0]
        apps.append(
            {
                "application": application,
                "display_name": display_name,
                "short_name": application.split(".")[-1],
            }
        )
    return apps


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Dismiss transient shell UI before and after each test."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture
def launch_software_center():
    """Launch GNOME Software (Bazaar) via Activities and return the window."""
    pressKey("super")
    time.sleep(1.5)
    typeText("Software")
    time.sleep(1.0)
    pressKey("Return")
    time.sleep(3.0)
    app = root.application("gnome-software")
    assert app is not None, "GNOME Software did not launch"
    yield app
    try:
        app.child(roleName="frame").keyCombo("<Alt>F4")
        time.sleep(0.5)
    except Exception:  # noqa: BLE001
        pass


class TestInstalledFlatpakRuntime:
    """Validate Bazaar installed-app runtime affordances for system Flatpaks."""

    def test_bazaar_installed_tab_shows_a_real_system_flatpak(self, launch_software_center):
        """Installed tab must surface at least one system-scoped Flatpak app from the CLI."""
        apps = _system_flatpak_apps()
        assert apps, "No system Flatpak applications were listed by flatpak list --system --app"

        installed_tab = _wait_for_named_node(launch_software_center, "Installed", interactive_only=True)
        _activate_node(installed_tab)
        time.sleep(2.0)

        matches = []
        for app in apps:
            matches = _matching_nodes(
                launch_software_center,
                app["display_name"],
                app["short_name"],
            )
            if matches:
                break

        assert matches, (
            "Bazaar Installed tab did not expose any system Flatpak app that flatpak list reported.\n"
            f"CLI apps: {[app['application'] for app in apps[:10]]}"
        )

    def test_installed_flatpak_detail_exposes_runtime_actions(self, launch_software_center):
        """An installed system Flatpak detail page must expose launch/remove style actions."""
        apps = _system_flatpak_apps()
        assert apps, "No system Flatpak applications were listed by flatpak list --system --app"

        target = apps[0]
        installed_tab = _wait_for_named_node(launch_software_center, "Installed", interactive_only=True)
        _activate_node(installed_tab)
        time.sleep(2.0)

        app_row = _wait_for_named_node(
            launch_software_center,
            target["display_name"],
            target["short_name"],
            timeout=15.0,
            interactive_only=True,
        )
        _activate_node(app_row)
        time.sleep(2.0)

        action = _wait_for_named_node(
            launch_software_center,
            "Open",
            "Launch",
            "Remove",
            "Uninstall",
            timeout=10.0,
            interactive_only=True,
        )
        assert action.sensitive, f"Installed Flatpak action button is not clickable: {action.name!r}"
