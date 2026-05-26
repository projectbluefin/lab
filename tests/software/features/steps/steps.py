"""
Custom step definitions for software suite.

Custom steps here cover Bazaar-specific dogtail flows that common_steps does
not provide: search results, detail pages, install-button assertions, and the
bluefin#4062 keyring-modal regression.
"""
from time import sleep

from behave import step
from dogtail.rawinput import pressKey, typeText
from dogtail.tree import root

from qecore.common_steps import *  # noqa: F401,F403


def _software_instance(context):
    app = getattr(context.software, "instance", None)
    assert app is not None, "Bazaar is not running"
    return app


def _visible_search_entry(app):
    entries = app.findChildren(
        lambda node: node.roleName in ("entry", "text") and node.showing
    )
    assert entries, "Bazaar search entry not found"
    return entries[0]


def _matching_results(app, text):
    needle = text.lower()
    return app.findChildren(
        lambda node: node.showing
        and node.roleName in ("list item", "push button", "label", "link")
        and needle in (node.name or "").lower()
    )


@step('Search for "{text}" in Bazaar')
def search_for_text_in_bazaar(context, text) -> None:
    app = _software_instance(context)
    search_buttons = app.findChildren(
        lambda node: node.roleName == "toggle button"
        and (node.name or "").strip() == "Search"
        and node.showing
    )
    assert search_buttons, "Bazaar Search toggle button not found"
    search_buttons[0].click()
    sleep(1)

    entry = _visible_search_entry(app)
    entry.click()
    sleep(0.5)
    entry.keyCombo("<Ctrl>A")
    pressKey("BackSpace")
    typeText(text)
    pressKey("Return")
    sleep(3)


@step('Bazaar search shows result "{text}"')
def bazaar_search_shows_result(context, text) -> None:
    app = _software_instance(context)
    results = _matching_results(app, text)
    assert results, f'Bazaar search did not show a result for "{text}"'
    context.software_search_results = results


@step('Open Bazaar search result "{text}"')
def open_bazaar_search_result(context, text) -> None:
    app = _software_instance(context)
    results = getattr(context, "software_search_results", None) or _matching_results(
        app, text
    )
    assert results, f'Bazaar search result "{text}" not found'
    results[0].click()
    sleep(2)


@step("Bazaar install button is accessible")
def bazaar_install_button_is_accessible(context) -> None:
    app = _software_instance(context)
    install_buttons = app.findChildren(
        lambda node: node.roleName == "push button"
        and (node.name or "").strip().lower() == "install"
        and node.showing
    )
    assert install_buttons, "Bazaar Install button not found on the detail page"
    assert install_buttons[0].sensitive, "Bazaar Install button is not clickable"


@step("No keyring dialog is visible for Bazaar")
def no_keyring_dialog_is_visible_for_bazaar(context) -> None:
    dialogs = root.findChildren(
        lambda node: node.roleName == "dialog"
        and "keyring" in (node.name or "").lower()
        and node.showing
    )

    keyring_app = None
    try:
        keyring_app = root.application("gnome-keyring-ask")
    except Exception:  # noqa: BLE001
        keyring_app = None

    assert not dialogs, (
        "Unexpected keyring dialog while Bazaar is running: "
        f"{[dialog.name for dialog in dialogs]}"
    )
    assert keyring_app is None, "gnome-keyring-ask appeared during Bazaar launch"
