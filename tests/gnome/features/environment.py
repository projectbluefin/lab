"""
GNOME test environment — qecore TestSandbox for Ptyxis and other GNOME upstream apps.

AT-SPI app names confirmed in tests:
  - Ptyxis: root.application("ptyxis")

Pattern: modehnal/GNOMETerminalAutomation features/environment.py
"""
import os
import subprocess
import sys
import traceback

from qecore.sandbox import TestSandbox
from qecore.common_steps import *  # noqa: F401,F403


def before_all(context) -> None:
    try:
        context.sandbox = TestSandbox("ptyxis", context=context)
        context.sandbox.attach_faf = False
        context.sandbox.production = False

        context.ptyxis = context.sandbox.get_application(
            name="ptyxis",
            a11y_app_name="ptyxis",
            desktop_file_name="org.gnome.Ptyxis.desktop",
        )
        context.ptyxis.exit_shortcut = "<Alt>F4"

        # Read test start time written by workflow before behave started (issue #6)
        _start_time_file = "/tmp/results/test-start-time.txt"
        if os.path.exists(_start_time_file):
            with open(_start_time_file) as _f:
                context.test_start_time = _f.read().strip()
        else:
            context.test_start_time = subprocess.run(
                ["date", "--iso-8601=seconds"], capture_output=True, text=True
            ).stdout.strip()
    except Exception as error:
        print(f"Environment error: before_all: {error}")
        context.failed_setup = traceback.format_exc()
        return


def before_scenario(context, scenario) -> None:
    if getattr(context, 'failed_setup', None) is not None:
        scenario.skip(f"Suite setup failed: {context.failed_setup}")
        return
    try:
        context.sandbox.before_scenario(context, scenario)
    except Exception:
        context.embed("text/plain", traceback.format_exc(), "Before Scenario Error")
        sys.exit(1)


def after_scenario(context, scenario) -> None:
    context.sandbox.after_scenario(context, scenario)
