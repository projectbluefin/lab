"""
Custom step definitions for developer suite tests.

common_steps covers: Start/Close application, Item found/not found,
Key combo, Press key, Type text, Run and save command output.

Custom steps here:
  - Make sure window is focused for wayland testing (port from GNOMETerminalAutomation)
  - Terminal output in ptyxis contains <text>
  - Ptyxis has N tabs
  - No Flatpak missing-runtime error
  - Podman Desktop containers navigation and error-dialog checks
"""
import os
from time import sleep

from behave import step
from dogtail.rawinput import pressKey
from dogtail.tree import root
from qecore.common_steps import *  # noqa: F401,F403


@step("Make sure window is focused for wayland testing")
def make_sure_window_is_focused(context) -> None:
    # Pattern from GNOMETerminalAutomation steps.py — prevents input race on Wayland
    sleep(2)
    if context.sandbox.session_type == "wayland":
        context.ptyxis.instance.children[0].click()


@step('Terminal output in ptyxis contains "{text}"')
def terminal_output_contains(context, text) -> None:
    # Ptyxis terminal widget uses roleName "terminal" (VTE-backed)
    terminal_widget = context.ptyxis.instance.child(roleName="terminal")
    assert text in terminal_widget.text, (
        f"Terminal output does not contain '{text}'"
    )


@step('Ptyxis has "{number}" tabs')
def ptyxis_has_n_tabs(context, number) -> None:
    # Tab bar uses roleName "page tab list"
    tab_lists = context.ptyxis.instance.findChildren(
        lambda n: n.roleName == "page tab list" and n.showing
    )
    assert tab_lists, "Ptyxis tab bar not found"
    tab_list = tab_lists[0]
    tabs = tab_list.findChildren(lambda n: n.roleName == "page tab")
    assert len(tabs) == int(number), (
        f"Expected {number} tabs, found {len(tabs)}"
    )


@step('No Flatpak missing-runtime error for "{flatpak_id}"')
def no_flatpak_missing_runtime_error(context, flatpak_id) -> None:
    # Checks journalctl for Flatpak runtime-missing errors (regression: dakota#430)
    import subprocess
    journal_since = os.environ.get("TEST_JOURNAL_SINCE")
    cmd = ["journalctl", "-b", "--no-pager", "-g", f"{flatpak_id}.*runtime.*missing"]
    if journal_since:
        cmd.extend(["--since", journal_since])
    result = subprocess.run(
        cmd,
        capture_output=True, text=True,
    )
    assert result.returncode != 0 or result.stdout.strip() == "", (
        f"Flatpak runtime-missing error found for {flatpak_id}:\n{result.stdout}"
    )


def _podman_desktop_instance(context):
    app = getattr(context.podman_desktop, "instance", None)
    assert app is not None, "Podman Desktop is not running"
    return app


@step("Podman Desktop shows a Containers navigation item")
def podman_desktop_shows_containers_navigation_item(context) -> None:
    app = _podman_desktop_instance(context)
    containers = app.findChildren(
        lambda node: node.showing
        and "container" in (node.name or "").lower()
        and node.roleName in ("list item", "push button", "link")
    )
    assert containers, "Containers navigation item not found in Podman Desktop"


@step("No Podman Desktop permission error dialog appears")
def no_podman_desktop_permission_error_dialog_appears(context) -> None:
    dialogs = root.findChildren(
        lambda node: node.roleName == "dialog"
        and node.showing
        and any(
            word in (node.name or "").lower()
            for word in ("permission", "error", "denied", "failed")
        )
    )
    assert not dialogs, (
        "Permission error dialog on Podman Desktop launch: "
        f"{[dialog.name for dialog in dialogs]}"
    )
