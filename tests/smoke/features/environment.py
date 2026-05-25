"""
Smoke test environment — qecore TestSandbox for GNOME Shell.

Pattern sourced from: modehnal/GNOMETerminalAutomation features/environment.py
qecore source: gitlab.com/dogtail/qecore

qecore-headless (invoked by the Argo runner) handles:
  - DBUS_SESSION_BUS_ADDRESS
  - WAYLAND_DISPLAY / XDG_RUNTIME_DIR
  - gnome-ponytail-daemon activation
  - AT-SPI bus bridge
"""
import sys
import traceback

from qecore.sandbox import TestSandbox
from qecore.common_steps import *  # noqa: F401,F403 — registers all common @step definitions


def before_all(context) -> None:
    import time
    import subprocess

    # Give GDM/GNOME Shell time to start the session
    time.sleep(5)

    # Enable unsafe_mode to expose clock and system-status in AT-SPI tree
    for attempt in range(3):
        try:
            r = subprocess.run(
                ['gdbus', 'call', '--session',
                 '--dest', 'org.gnome.Shell',
                 '--object-path', '/org/gnome/Shell',
                 '--method', 'org.gnome.Shell.Eval',
                 'global.context.unsafe_mode = true'],
                capture_output=True, timeout=5,
            )
            if r.returncode == 0:
                print(f"unsafe_mode set (attempt {attempt+1})", flush=True)
                break
            print(f"unsafe_mode attempt {attempt+1} failed (exit {r.returncode}): {r.stderr.decode()[:200]}", flush=True)
            time.sleep(2)
        except Exception as e:  # noqa: BLE001
            print(f"unsafe_mode attempt {attempt+1} failed: {e}", flush=True)
            time.sleep(2)

    # Poll until clock + system toggles appear in AT-SPI (up to 15s)
    from dogtail import tree as dtree
    deadline = time.time() + 15
    while time.time() < deadline:
        try:
            shell = dtree.root.application('gnome-shell')
            panels = shell.findChildren(lambda n: n.roleName == 'panel')
            if panels:
                toggles = panels[0].findChildren(
                    lambda n: n.roleName == 'toggle button' and n.showing)
                toggle_names = [t.name for t in toggles]
                print(f"Panel toggles: {toggle_names}", flush=True)
                # Need more than just Activities + Show Apps
                non_activities = [t for t in toggles if t.name != 'Activities']
                if len(non_activities) >= 1:
                    print("Clock/System toggles visible — proceeding", flush=True)
                    break
        except Exception as e:  # noqa: BLE001
            print(f"AT-SPI poll: {e}", flush=True)
        time.sleep(1)
    else:
        print("WARNING: clock/system toggles not found after 15s — proceeding anyway", flush=True)

    # Initialize sandbox
    try:
        context.sandbox = TestSandbox("gnome-shell", context=context)
        context.sandbox.attach_faf = False
        context.sandbox.production = False
        context.shell = context.sandbox.shell
    except Exception as error:
        print(f"Environment error: before_all: {error}", flush=True)
        context.failed_setup = traceback.format_exc()


def before_scenario(context, scenario) -> None:
    # Initialize qecore command output attributes (attribute name varies by version)
    # qecore 4.16: command_stdout; older: last_command_output
    context.command_stdout = ""
    context.last_command_output = ""
    try:
        context.sandbox.before_scenario(context, scenario)
    except Exception:
        tb = traceback.format_exc()
        print(f"HOOK_ERROR in before_scenario:\n{tb}", flush=True)
        sys.exit(1)


def after_scenario(context, scenario) -> None:
    context.sandbox.after_scenario(context, scenario)


def after_step(context, step) -> None:
    """Print full traceback for errored steps — needed because behave JSON
    serialises error_message as empty when the exception has no str()."""
    if step.status.name in ("error", "failed") and step.exception is not None:
        print(
            f"\nSTEP_ERROR [{step.name!r}]: "
            f"{type(step.exception).__name__}: {step.exception}",
            flush=True,
        )
        traceback.print_exception(
            type(step.exception),
            step.exception,
            step.exception.__traceback__,
            file=sys.stderr,
        )


def after_all(context) -> None:
    """Dump gnome-shell AT-SPI tree to results for node name discovery.
    Runs after the last scenario while the session is still active enough
    for the sandbox to have a valid shell handle.
    """
    try:
        import os
        if os.path.exists("/tmp/results/atspi_tree.txt"):
            return  # already written by after_scenario
        shell = context.sandbox.shell
        lines = []
        for child in shell.children[:60]:
            lines.append(f"role={child.roleName!r:30} name={child.name!r}")
            for gc in child.children[:20]:
                lines.append(f"  role={gc.roleName!r:30} name={gc.name!r}")
        os.makedirs("/tmp/results", exist_ok=True)
        with open("/tmp/results/atspi_tree.txt", "w") as f:
            f.write("\n".join(lines))
    except Exception:   # noqa: BLE001
        pass
