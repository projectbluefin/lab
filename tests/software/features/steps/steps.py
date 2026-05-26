"""
Custom step definitions for software suite.

common_steps covers the low-level lifecycle and UI actions; custom aliases here
map the Bazaar workflow wording onto those shared qecore patterns.
"""

from behave import step
from qecore.common_steps import *  # noqa: F401,F403


def _require_bazaar(app_id: str) -> None:
    assert app_id == "org.gnome.Software", f"Unsupported application id: {app_id}"


@step('Start "{app_id}" via shell')
def start_app_via_shell(context, app_id) -> None:
    _require_bazaar(app_id)
    context.execute_steps('* Start application "software" via "command"')


@step('Application "{app_id}" is opened')
def application_is_opened(context, app_id) -> None:
    _require_bazaar(app_id)
    context.execute_steps(
        '\n'.join(
            [
                '* Application "software" is running',
                '* Wait until "Software" "frame" appears in "software"',
            ]
        )
    )


@step('Close "{app_id}"')
def close_app(context, app_id) -> None:
    _require_bazaar(app_id)
    context.execute_steps(
        '\n'.join(
            [
                '* Close application "software" via "shortcut"',
                '* Application "software" is no longer running',
            ]
        )
    )


@step('Activate "{label}" in "{app_id}"')
def activate_view(context, label, app_id) -> None:
    _require_bazaar(app_id)
    context.execute_steps(
        '\n'.join(
            [
                f'* Left click "{label}" "toggle button" in "software"',
                f'* Wait until "{label}" "page tab" appears in "software"',
            ]
        )
    )
