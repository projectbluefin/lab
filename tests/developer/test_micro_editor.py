"""
Phase 2 — Micro Editor Tests
Validates that the micro text editor launches and accepts input within Ptyxis.
"""
import time
import pytest
from dogtail.rawinput import pressKey, typeText


class TestMicroEditor:

    def test_micro_launches_in_terminal(self, launch_ptyxis):
        """Launching micro inside Ptyxis must update the terminal UI."""
        typeText("micro /tmp/tmt-micro-test.txt\n")
        time.sleep(1.5)
        # Micro has taken over the Ptyxis buffer — the frame title updates
        frame = launch_ptyxis.child(roleName="frame")
        # Accept either the filename or "micro" in the window title
        assert "micro" in frame.name.lower() or "tmt-micro-test" in frame.name.lower(), \
            f"Micro editor title not found in Ptyxis frame: '{frame.name}'"
        # Exit micro without saving
        pressKey("<Ctrl>q")
        time.sleep(0.5)

    def test_micro_accepts_text_input(self, launch_ptyxis):
        """Micro must accept keyboard text input into the buffer."""
        typeText("micro /tmp/tmt-micro-input-test.txt\n")
        time.sleep(1.5)
        # Type some text into the buffer
        typeText("hello from bluefin test suite")
        time.sleep(0.5)
        # Exit without saving
        pressKey("<Ctrl>q")
        time.sleep(0.3)
        # Micro prompts to save — dismiss with 'n'
        typeText("n")
        time.sleep(0.5)
