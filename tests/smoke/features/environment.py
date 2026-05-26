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
import traceback

from dogtail.config import config as dogtail_config
from qecore.sandbox import TestSandbox
from qecore.common_steps import *  # noqa: F401,F403 — registers all common @step definitions


def before_all(context) -> None:
    # searchShowingOnly=True: all dogtail searches implicitly filter to .showing
    # nodes — removes need for redundant `.showing` predicates in step code.
    dogtail_config.searchShowingOnly = True
    try:
        result = subprocess.run(
            [
                "gdbus",
                "call",
                "--session",
                "--dest",
                "org.gnome.Shell",
                "--object-path",
                "/org/gnome/Shell",
                "--method",
                "org.gnome.Shell.Eval",
                "global.context.unsafe_mode === true",
            ],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except Exception as error:  # noqa: BLE001
        raise RuntimeError(f"unsafe_mode verification failed: {error}") from error
    if result.returncode != 0:
        stderr = result.stderr.strip() or result.stdout.strip() or "unknown gdbus failure"
        raise RuntimeError(f"unsafe_mode verification failed: {stderr[:200]}")
    match = re.search(r"\((?:true|false),\s*'([^']*)'\)", result.stdout.strip())
    if match is None or match.group(1) != "true":
        raise RuntimeError(
            "unsafe_mode was not enabled by run-gnome-tests readiness checks. "
            f"Got: {result.stdout.strip()[:200]}"
        )

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
