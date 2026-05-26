"""
Custom step definitions for GNOME Shell smoke tests.

common_steps provides: Application is running, Item found/not found,
Left/Right click, Key combo, Press key, Type text, Run and save command output,
Last command output, Wait N seconds.

Custom steps here cover:
- GNOME Shell accessibility check (retrying via context.sandbox.shell)
- Activities overview state, search bar content.

NOTE: We do NOT redefine 'Application "{name}" is running' — behave raises
AmbiguousStep when a literal step conflicts with an existing wildcard step.
Instead we use a distinct step name: 'GNOME Shell is accessible via AT-SPI'.

Step patterns sourced from: modehnal/GNOMETerminalAutomation steps.py
dogtail API: root.application(), Node.findChild(), Node.child(roleName=)
"""
from time import sleep

from behave import step
from dogtail import tree
from dogtail.rawinput import pressKey
from qecore.common_steps import *  # noqa: F401,F403


def _shell_snapshot(shell, max_children=15):
    top_level = [(child.roleName, child.name, child.showing) for child in shell.children[:max_children]]
    panels = shell.findChildren(lambda node: node.roleName == "panel")
    panel_children = []
    toggle_names = []
    if panels:
        panel_children = [
            (child.roleName, child.name, child.showing) for child in panels[0].children[:max_children]
        ]
        toggle_names = [toggle.name for toggle in panels[0].findChildren(lambda node: node.roleName == "toggle button")]
    return (
        f"top-level={top_level}\n"
        f"panel-children={panel_children}\n"
        f"toggle-names={toggle_names}"
    )


def _find_panel_with_retry(context, attempts=6, delay=1.0):
    shell = context.sandbox.shell
    last_snapshot = _shell_snapshot(shell)
    for attempt in range(1, attempts + 1):
        panels = shell.findChildren(lambda node: node.roleName == "panel")
        if panels:
            context.panel = panels[0]
            return panels[0]
        last_snapshot = _shell_snapshot(shell)
        if attempt < attempts:
            sleep(delay)
    raise AssertionError(
        f"Panel (role='panel') not found in gnome-shell after {attempts * delay:.1f}s.\n"
        f"{last_snapshot}"
    )


def _wait_for_toggle(panel, predicate, description, attempts=6, delay=1.0):
    toggle_info = []
    for attempt in range(1, attempts + 1):
        toggles = panel.findChildren(lambda node: node.roleName == "toggle button")
        toggle_info = [(toggle.name, toggle.roleName, toggle.showing) for toggle in toggles]
        match = next((toggle for toggle in toggles if predicate(toggle)), None)
        if match is not None:
            return match
        if attempt < attempts:
            sleep(delay)
    raise AssertionError(
        f"{description} not found after {attempts * delay:.1f}s.\n"
        f"All panel toggles: {toggle_info}"
    )


@step("Dump panel children to log")
def dump_panel_children(context) -> None:
    """Print the full gnome-shell AT-SPI tree to stdout (Argo logs).
    Helps discover clock/system-status area roles and names in Bluefin GNOME.
    """
    try:
        shell = context.sandbox.shell
        print("=== GNOME-SHELL AT-SPI TREE ===", flush=True)
        def _dump(node, depth=0, max_depth=3):
            prefix = "  " * depth
            print(f"{prefix}role={node.roleName!r:20} name={node.name!r:30} showing={node.showing}", flush=True)
            if depth < max_depth:
                for c in node.children[:30]:
                    _dump(c, depth + 1, max_depth)
        _dump(shell, max_depth=3)
        print("=== END AT-SPI TREE ===", flush=True)
    except Exception as exc:  # noqa: BLE001
        print(f"dump_panel_children failed: {exc}", flush=True)


@step("Dump gnome-shell AT-SPI tree to results")
def dump_atspi_tree(context) -> None:
    """Write the gnome-shell AT-SPI node tree to /tmp/results/atspi_tree.txt.

    Called from the first smoke scenario while the session is live, so the
    Wayland session and AT-SPI bus are both active.
    """
    import os
    lines = []
    shell = context.sandbox.shell
    def _write_tree(node, depth=0, max_depth=4):
        prefix = "  " * depth
        lines.append(f"{prefix}role={node.roleName!r:25} name={node.name!r} showing={node.showing}")
        if depth < max_depth:
            for gc in node.children[:40]:
                _write_tree(gc, depth + 1, max_depth)
    _write_tree(shell, max_depth=4)
    os.makedirs("/tmp/results", exist_ok=True)
    with open("/tmp/results/atspi_tree.txt", "w") as f:
        f.write("\n".join(lines))
    print(f"AT-SPI tree written: {len(lines)} lines (depth=4)", flush=True)



@step("GNOME Shell is accessible via AT-SPI")
def gnome_shell_is_accessible(context) -> None:
    """Retrying gnome-shell AT-SPI check via qecore's built-in shell getter.

    The common 'Application "{name}" is running' step calls is_open() which
    does not work for gnome-shell (compositor, not a regular window).
    context.sandbox.shell uses qecore's own retry path and is the recommended
    way to access gnome-shell per qecore docs.
    """
    last_exc = None
    for attempt in range(6):   # up to 30 s total
        try:
            shell = context.sandbox.shell
            assert shell is not None, "gnome-shell not registered in AT-SPI tree"
            return
        except Exception as exc:   # noqa: BLE001
            last_exc = exc
            sleep(5)
    raise AssertionError(
        f"gnome-shell not accessible via AT-SPI after 30 s: {last_exc}"
    )


@step('Panel is present in AT-SPI tree')
def panel_is_present(context) -> None:
    """Verify the GNOME Shell top bar panel is accessible.
    Searches by role='panel' — does NOT depend on accessible-name, which
    varies across GNOME versions (may be empty, 'panel', 'top-bar', etc.).
    """
    _find_panel_with_retry(context)


@step('Clock toggle is visible in top bar')
def clock_toggle_visible(context) -> None:
    """Verify the clock toggle button is visible in the panel.
    GNOME 47+ accessible-name for the clock is the formatted time string
    (e.g. '7:14 PM' or 'Sunday 25 May, 7:14 PM'), NOT the literal 'clock'.
    We match by role and exclude 'Activities' and known system-menu names.
    """
    import re
    panel = _find_panel_with_retry(context)
    # dogtail.config.searchShowingOnly = True (set in before_all) makes the
    # implicit `.showing` filter redundant here.
    SYSTEM_NAMES = {"Activities", "System", "System Menu", "System menu"}
    time_re = re.compile(r'\d{1,2}:\d{2}|clock', re.IGNORECASE)
    clock = _wait_for_toggle(
        panel,
        lambda toggle: toggle.name and toggle.name not in SYSTEM_NAMES and time_re.search(toggle.name),
        "Clock toggle (time-pattern in accessible-name)",
    )
    context.clock_toggle = clock
    print(f"Clock toggle found: name={clock.name!r}", flush=True)


@step('System menu toggle is visible in top bar')
def system_menu_toggle_visible(context) -> None:
    """Verify the system menu / quick-settings toggle is visible.
    In GNOME 47/48 the accessible-name is 'System' (not 'System menu').
    Also accepts 'System menu' for forward compatibility.
    """
    panel = _find_panel_with_retry(context)
    CANDIDATE_NAMES = {"System", "System menu", "System Menu"}
    system = _wait_for_toggle(
        panel,
        lambda toggle: toggle.name in CANDIDATE_NAMES,
        f"System menu toggle (looked for {sorted(CANDIDATE_NAMES)})",
    )
    context.system_toggle = system
    print(f"System menu toggle found: name={system.name!r}", flush=True)


@step('Last command output stripped "is" "{expected}"')
def last_command_output_stripped_is(context, expected) -> None:
    """Compare last command output after stripping whitespace/newlines.

    grep -c always appends a trailing newline; use this step instead of
    'Last command output "is"' when the command output has trailing whitespace.
    Supports qecore versions that use last_command_output or last_run_output.
    """
    # qecore 4.16 stores under command_stdout; older versions used last_command_output
    actual = (
        getattr(context, 'command_stdout', None)
        or getattr(context, 'last_command_output', None)
        or getattr(context, 'last_run_output', None)
        or ""
    ).strip()
    assert actual == expected, (
        f"\nWanted output: '{expected}'\nActual output: '{actual}'"
    )


# ── Shell.Eval helpers (GNOME 50: uinput Super + AT-SPI toggle click broken) ──

def _shell_eval(js: str) -> str:
    """Run JS in GNOME Shell and return stdout. Requires unsafe_mode=true."""
    import subprocess
    r = subprocess.run(
        ['gdbus', 'call', '--session',
         '--dest', 'org.gnome.Shell',
         '--object-path', '/org/gnome/Shell',
         '--method', 'org.gnome.Shell.Eval',
         js],
        capture_output=True, text=True, timeout=5,
    )
    if r.returncode != 0:
        stderr = r.stderr.strip() or r.stdout.strip() or "unknown gdbus failure"
        raise AssertionError(f"Shell.Eval failed for {js!r}: {stderr[:200]}")
    print(f"Shell.Eval({js!r}) → {r.stdout.strip()}", flush=True)
    return r.stdout


def _shell_eval_inner(js: str) -> str:
    """Return the inner JS-result string from Shell.Eval's (bs) GVariant tuple.

    gdbus output format: ``(true, 'value')`` — the outer bool is the DBus
    call-success flag, NOT the JS result.  ``'true' in out.lower()`` is a
    false-positive trap because the wrapper always contains 'true'.
    This helper extracts only the inner quoted value.
    """
    import re
    out = _shell_eval(js)
    m = re.search(r"\((?:true|false),\s*'(.*)'\)", out.strip())
    if m is None:
        raise AssertionError(f"Unexpected Shell.Eval output for {js!r}: {out.strip()!r}")
    inner = m.group(1)
    if inner.startswith('"') and inner.endswith('"'):
        return inner[1:-1]
    return inner


@step('Open Activities overview via Shell.Eval')
def open_overview_eval(context) -> None:
    _shell_eval('Main.overview.show()')
    sleep(1)


@step('Close Activities overview via Shell.Eval')
def close_overview_eval(context) -> None:
    _shell_eval('Main.overview.hide()')
    sleep(0.5)


@step('Open Quick Settings via Shell.Eval')
def open_quick_settings_eval(context) -> None:
    # menu.toggle() is stable across GNOME 49/50
    _shell_eval('Main.panel.statusArea.quickSettings.menu.toggle()')
    sleep(0.5)


@step('Quick Settings panel is open via Shell.Eval')
def quick_settings_open_eval(context) -> None:
    inner = _shell_eval_inner('Main.panel.statusArea.quickSettings.menu.isOpen.toString()')
    assert inner == 'true', f"Quick Settings not open — Shell.Eval inner: {inner!r}"


@step('Quick Settings panel is closed via Shell.Eval')
def quick_settings_closed_eval(context) -> None:
    for _ in range(8):
        inner = _shell_eval_inner('Main.panel.statusArea.quickSettings.menu.isOpen.toString()')
        if inner == 'false':
            return
        sleep(0.5)
    raise AssertionError(f"Quick Settings still open after 4s — Shell.Eval inner: {inner!r}")


@step('Open date menu via Shell.Eval')
def open_date_menu_eval(context) -> None:
    # menu.toggle() is stable across GNOME 49/50; _toggleMenu() is GNOME 50+ only
    _shell_eval('Main.panel.statusArea.dateMenu.menu.toggle()')
    sleep(0.5)


@step('Close Quick Settings via Shell.Eval')
def close_quick_settings_eval(context) -> None:
    # close(0) = BoxPointer.PopupAnimation.NONE — explicit close, not toggle
    _shell_eval('Main.panel.statusArea.quickSettings.menu.close(0)')
    sleep(0.5)


@step('Close date menu via Shell.Eval')
def close_date_menu_eval(context) -> None:
    _shell_eval('Main.panel.statusArea.dateMenu.menu.close(0)')
    sleep(0.5)


@step('Date menu panel is open via Shell.Eval')
def date_menu_open_eval(context) -> None:
    inner = _shell_eval_inner('Main.panel.statusArea.dateMenu.menu.isOpen.toString()')
    assert inner == 'true', f"Date menu not open — Shell.Eval inner: {inner!r}"


@step('Date menu panel is closed via Shell.Eval')
def date_menu_closed_eval(context) -> None:
    for _ in range(8):
        inner = _shell_eval_inner('Main.panel.statusArea.dateMenu.menu.isOpen.toString()')
        if inner == 'false':
            return
        sleep(0.5)
    raise AssertionError(f"Date menu still open after 4s — Shell.Eval inner: {inner!r}")


@step('Set overview search text to "{text}" via Shell.Eval')
def set_overview_search_eval(context, text) -> None:
    """Populate overview search bar via GNOME Shell JS.
    uinput typing is broken on these VMs — use Shell.Eval instead.
    set_text() fires St.Entry::changed which the SearchController is connected to;
    _onSearchChanged() is a private method removed in GNOME 50 and must not be called.
    """
    _shell_eval(f'Main.overview.searchEntry.set_text("{text}")')
    sleep(0.5)


@step("Overview is open")
def overview_is_open(context) -> None:
    inner = ""
    for _ in range(8):
        inner = _shell_eval_inner('Main.overview.visible.toString()')
        if inner == "true":
            return
        sleep(0.5)
    raise AssertionError(f"Activities overview did not open after 4s — Shell.Eval inner: {inner!r}")


@step("Overview is closed")
def overview_is_closed(context) -> None:
    inner = ""
    for _ in range(8):
        inner = _shell_eval_inner('Main.overview.visible.toString()')
        if inner == "false":
            return
        sleep(0.5)
    raise AssertionError(f"Activities overview is still showing after 4s — Shell.Eval inner: {inner!r}")


@step('Overview search bar contains "{text}"')
def overview_search_bar_contains(context, text) -> None:
    shell = context.sandbox.shell
    last_entries = []
    for _ in range(8):
        entries = shell.findChildren(lambda n: n.roleName == "text")
        last_entries = [(entry.name, getattr(entry, "text", "")) for entry in entries[:10]]
        if entries and text in entries[0].text:
            return
        sleep(0.5)
    raise AssertionError(
        f"Search bar text entry did not contain {text!r} after 4s.\n"
        f"Visible text nodes: {last_entries}"
    )


def _find_application(*candidate_names):
    for candidate in candidate_names:
        try:
            app = tree.root.application(candidate)
            if app is not None:
                return app
        except Exception:  # noqa: BLE001
            pass
    return None


@step("Launch first overview search result via Enter")
def launch_first_overview_result(context) -> None:
    pressKey("Return")
    sleep(2)


@step("Files application is open")
def files_application_is_open(context) -> None:
    app = _find_application("org.gnome.Nautilus", "Files", "nautilus")
    assert app is not None, "Files application did not appear in the AT-SPI tree"
    context.active_application = app


@step("Settings application is open")
def settings_application_is_open(context) -> None:
    app = _find_application("org.gnome.Settings", "gnome-control-center", "Settings")
    assert app is not None, "Settings application did not appear in the AT-SPI tree"
    context.active_application = app


@step("Close active application window")
def close_active_application_window(context) -> None:
    app = getattr(context, "active_application", None)
    assert app is not None, "No active application stored on context"
    frames = app.findChildren(lambda n: n.roleName == "frame")
    assert frames, f"No application frame found for {app.name!r}"
    frames[0].keyCombo("<Alt>F4")
    sleep(1)
