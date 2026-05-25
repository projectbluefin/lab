"""
Software test environment — qecore TestSandbox for gnome-software (Bazaar).

Regressions: bluefin#4062, #4471.
"""
import sys
import traceback

from qecore.sandbox import TestSandbox
from qecore.common_steps import *  # noqa: F401,F403


def before_all(context) -> None:
    try:
        context.sandbox = TestSandbox("gnome-software", context=context)
        context.sandbox.attach_faf = False
        context.sandbox.production = False

        context.software = context.sandbox.get_application(
            name="gnome-software",
            a11y_app_name="gnome-software",
            desktop_file_name="org.gnome.Software.desktop",
        )
        context.software.exit_shortcut = "<Ctrl>Q"
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
