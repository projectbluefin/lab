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
    # Give GNOME Shell a moment to settle after qecore-headless restarts GDM.
    # Without this, get_application() races against AT-SPI bus initialization.
    time.sleep(8)
    try:
        context.sandbox = TestSandbox("gnome-shell", context=context)
        context.sandbox.attach_faf = False          # no ABRT integration in lab
        context.sandbox.production = False          # disable screencast/journal embeds locally

        # gnome-shell is always running — register without desktop_file_exists
        context.shell = context.sandbox.get_application(
            name="gnome-shell",
            a11y_app_name="gnome-shell",
            desktop_file_exists=False,
        )
    except Exception as error:
        print(f"Environment error: before_all: {error}")
        context.failed_setup = traceback.format_exc()


def before_scenario(context, scenario) -> None:
    try:
        context.sandbox.before_scenario(context, scenario)
    except Exception:
        context.embed("text/plain", traceback.format_exc(), "Before Scenario Error")
        sys.exit(1)


def after_scenario(context, scenario) -> None:
    context.sandbox.after_scenario(context, scenario)
