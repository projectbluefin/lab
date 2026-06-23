"""
Custom step definitions for gnome suite tests.

common_steps covers: Start/Close application, Item found/not found,
Key combo, Press key, Type text, Run and save command output.

Custom steps here:
  - Make sure window is focused for wayland testing
  - Terminal output in ptyxis contains <text>
  - Ptyxis has N tabs
"""
from time import sleep

from behave import step
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
    sleep(1)  # wait for Ptyxis to render the new tab bar after key input
    tab_lists = context.ptyxis.instance.findChildren(
        lambda n: n.roleName == "page tab list" and n.showing
    )
    assert tab_lists, "Could not find a visible Ptyxis tab list"
    tabs = tab_lists[0].findChildren(lambda n: n.roleName == "page tab")
    assert len(tabs) == int(number), (
        f"Expected {number} tabs, found {len(tabs)}"
    )
