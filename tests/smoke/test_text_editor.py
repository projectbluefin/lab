"""
Phase 1 — GNOME Text Editor smoke tests.
Validates save and reopen workflows for a core desktop editor.
"""
from __future__ import annotations

import subprocess
import time
import uuid
from pathlib import Path

import pytest
from dogtail.rawinput import pressKey, typeText
from dogtail.tree import root


def _find_application(*candidate_names: str):
    for candidate in candidate_names:
        try:
            app = root.application(candidate)
            if app is not None:
                return app
        except Exception:  # noqa: BLE001
            pass
    return None


def _wait_for_application(*candidate_names: str, timeout: float = 12.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        app = _find_application(*candidate_names)
        if app is not None:
            return app
        time.sleep(0.5)
    raise AssertionError(f"Application did not appear in the AT-SPI tree: {candidate_names}")


def _find_frame(app):
    frames = app.findChildren(lambda node: node.roleName == "frame")
    assert frames, f"No application frame found for {app.name!r}"
    return frames[0]


def _close_application(app) -> None:
    _find_frame(app).keyCombo("<Alt>F4")
    time.sleep(1.0)
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture(autouse=True)
def reset_shell_state():
    """Dismiss transient shell UI before and after each test."""
    pressKey("Escape")
    time.sleep(0.5)
    yield
    pressKey("Escape")
    time.sleep(0.5)


@pytest.fixture
def editor_target_file():
    """Create a disposable editor target path."""
    path = Path("/tmp") / f"testing-lab-editor-{uuid.uuid4().hex[:8]}.txt"
    path.write_text("", encoding="utf-8")
    yield path
    path.unlink(missing_ok=True)


@pytest.fixture
def launch_text_editor(editor_target_file: Path):
    """Launch GNOME Text Editor against a disposable file."""
    subprocess.run(["gnome-text-editor", str(editor_target_file)], check=True, timeout=10)
    time.sleep(3.0)

    app = _wait_for_application("org.gnome.TextEditor", "Text Editor", "gnome-text-editor")
    yield app
    _close_application(app)


class TestTextEditor:
    """Validate core editor save and reopen flows."""

    def test_text_editor_saves_typed_content(self, launch_text_editor, editor_target_file: Path):
        """Typed content must save to disk via Ctrl+S."""
        expected = "testing-lab text editor smoke"
        typeText(expected)
        time.sleep(0.5)
        _find_frame(launch_text_editor).keyCombo("<Control>s")
        time.sleep(1.5)

        content = editor_target_file.read_text(encoding="utf-8")
        assert expected in content, f"Saved editor file did not contain expected text: {content!r}"

    def test_text_editor_reopens_existing_file(self, editor_target_file: Path):
        """Reopening a saved file must surface its filename in the editor UI."""
        expected = "testing-lab reopen check"
        editor_target_file.write_text(expected, encoding="utf-8")

        subprocess.run(["gnome-text-editor", str(editor_target_file)], check=True, timeout=10)
        time.sleep(3.0)
        app = _wait_for_application("org.gnome.TextEditor", "Text Editor", "gnome-text-editor")
        try:
            frame = _find_frame(app)
            visible_names = [node.name for node in app.findChildren(lambda node: bool(node.showing) and bool(node.name))]
            joined = "\n".join(visible_names + [frame.name or ""])
            assert editor_target_file.name in joined, (
                "Reopened file name was not visible in the text editor UI.\n"
                f"Visible names:\n{joined}"
            )
        finally:
            _close_application(app)
