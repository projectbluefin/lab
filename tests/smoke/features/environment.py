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
import re
import subprocess
import sys
import time
import traceback

from dogtail.config import config as dogtail_config
from qecore.sandbox import TestSandbox
from qecore.common_steps import *  # noqa: F401,F403 — registers all common @step definitions


def before_all(context) -> None:
    # searchShowingOnly=True: all dogtail searches implicitly filter to .showing
    # nodes — removes need for redundant `.showing` predicates in step code.
    dogtail_config.searchShowingOnly = True

    # Give GDM/GNOME Shell time to start the session
    time.sleep(5)

    # Enable unsafe_mode to expose clock and system-status in AT-SPI tree.
    # Hard-fail if all attempts fail — tests downstream depend on this and
    # silently proceeding hides the real cause of later AT-SPI lookup failures.
    last_err = "unknown"
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
            last_err = f"exit {r.returncode}: {r.stderr.decode('utf-8', errors='replace')[:200]}"
            print(f"unsafe_mode attempt {attempt+1} failed ({last_err})", flush=True)
            time.sleep(2)
        except Exception as e:  # noqa: BLE001
            last_err = str(e)
            print(f"unsafe_mode attempt {attempt+1} failed: {e}", flush=True)
            time.sleep(2)
    else:
        raise RuntimeError(
            f"unsafe_mode activation failed after 3 attempts ({last_err}). "
            "Clock and system-menu toggles will not be reachable via AT-SPI."
        )

    # Poll until a clock-like toggle (time string in name) appears in AT-SPI.
    # Accepting "any non-Activities toggle" silently masked a broken shell setup
    # where unsafe_mode hadn't taken effect — see issue #5.
    from dogtail import tree as dtree
    time_re = re.compile(r'\d{1,2}:\d{2}|clock', re.IGNORECASE)
    deadline = time.time() + 15
    while time.time() < deadline:
        try:
            shell = dtree.root.application('gnome-shell')
            panels = shell.findChildren(lambda n: n.roleName == 'panel')
            if panels:
                toggles = panels[0].findChildren(
                    lambda n: n.roleName == 'toggle button')
                toggle_names = [t.name for t in toggles]
                if any(n and time_re.search(n) for n in toggle_names):
                    print(f"Panel toggles ready: {toggle_names}", flush=True)
                    break
        except Exception as e:  # noqa: BLE001
            print(f"AT-SPI poll: {e}", flush=True)
        time.sleep(1)
    else:
        # Do not raise — some sessions are slow to populate the panel and the
        # individual @step checks will surface the failure with proper context.
        print("WARNING: clock toggle not visible after 15s — step-level checks will diagnose", flush=True)

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
