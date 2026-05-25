"""
Custom step definitions for GNOME Shell smoke tests.

common_steps provides: Application is running, Item found/not found,
Left/Right click, Key combo, Press key, Type text, Run and save command output,
Last command output, Wait N seconds.

Custom steps here cover:
- Application "gnome-shell" is running (override: retrying via context.sandbox.shell)
- Activities overview state, search bar content.

Step patterns sourced from: modehnal/GNOMETerminalAutomation steps.py
dogtail API: root.application(), Node.findChild(), Node.child(roleName=)
"""
from time import sleep

from behave import step
from dogtail import tree
from qecore.common_steps import *  # noqa: F401,F403


@step('Application "gnome-shell" is running')
def application_gnome_shell_is_running(context) -> None:
    """Override qecore common_step with a retrying AT-SPI check.

    The common_step calls dogtail.tree.root.application() once; it races
    against AT-SPI registration after GDM restarts.  context.sandbox.shell
    uses qecore's own retry logic which is more reliable here.
    """
    last_exc = None
    for attempt in range(6):   # up to 30 s total
        try:
            shell = context.sandbox.shell
            assert shell is not None, "gnome-shell is not registered in AT-SPI tree"
            return
        except Exception as exc:   # noqa: BLE001
            last_exc = exc
            sleep(5)
    raise AssertionError(
        f"gnome-shell not accessible via AT-SPI after 30 s: {last_exc}"
    )


@step("Overview is open")
def overview_is_open(context) -> None:
    # AT-SPI: gnome-shell exposes an "overview" named child when open
    shell = tree.root.application("gnome-shell")
    overview = shell.findChild(
        lambda n: n.name == "overview" and n.showing,
        requireResult=True,
    )
    assert overview is not None, "Activities overview did not open"


@step("Overview is closed")
def overview_is_closed(context) -> None:
    shell = tree.root.application("gnome-shell")
    results = shell.findChildren(
        lambda n: n.name == "overview" and n.showing,
    )
    assert len(results) == 0, "Activities overview is still showing"


@step('Overview search bar contains "{text}"')
def overview_search_bar_contains(context, text) -> None:
    shell = tree.root.application("gnome-shell")
    # search entry lives inside the overview
    entry = shell.findChild(
        lambda n: n.roleName == "text" and n.showing,
        requireResult=True,
    )
    assert text in entry.text, f"Search bar text '{entry.text}' does not contain '{text}'"
