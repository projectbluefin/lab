"""
Shared pytest fixtures for developer phase tests.
Provides launch_ptyxis and launch_ghostty fixtures used across
test_ptyxis_term.py, test_micro_editor.py, and test_dakota_terminal.py.
"""
import time
import pytest
from dogtail.tree import root
from dogtail.rawinput import pressKey, typeText


@pytest.fixture
def launch_ptyxis():
    """Launch Ptyxis via Activities search and return the window node."""
    pressKey("super")
    time.sleep(1.5)
    typeText("Ptyxis")
    time.sleep(1.0)
    pressKey("Return")
    time.sleep(2.0)
    terminal = root.application("ptyxis")
    assert terminal is not None, "Ptyxis did not launch"
    yield terminal
    try:
        terminal.child(roleName="frame").keyCombo("<Alt>F4")
        time.sleep(0.5)
    except Exception:
        pass


@pytest.fixture
def launch_ghostty():
    """Launch Ghostty via Activities search (Dakota builds)."""
    pressKey("super")
    time.sleep(1.5)
    typeText("Ghostty")
    time.sleep(1.0)
    pressKey("Return")
    time.sleep(2.0)
    terminal = root.application("ghostty")
    assert terminal is not None, "Ghostty did not launch"
    yield terminal
    try:
        terminal.child(roleName="frame").keyCombo("<Alt>F4")
        time.sleep(0.5)
    except Exception:
        pass
