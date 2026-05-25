"""
Phase 3 — Software Management and Flatpak UI Tests
Validates Bazaar (gnome-software) search and install flow via AT-SPI.
"""
import subprocess
import time
import pytest
from dogtail.tree import root
from dogtail.rawinput import pressKey, typeText


@pytest.fixture
def launch_software_center():
    """Launch GNOME Software (Bazaar) via Activities and return the window."""
    pressKey("super")
    time.sleep(1.5)
    typeText("Software")
    time.sleep(1.0)
    pressKey("Return")
    time.sleep(3.0)  # gnome-software is slow to initialize
    app = root.application("gnome-software")
    assert app is not None, "GNOME Software did not launch"
    yield app
    app.child(roleName="frame").keyCombo("<Alt>F4")
    time.sleep(0.5)


class TestFlatpakUI:

    def test_software_center_launches(self, launch_software_center):
        """GNOME Software must appear in the AT-SPI tree."""
        assert launch_software_center is not None

    def test_software_center_has_search(self, launch_software_center):
        """A search entry must be present in the GNOME Software UI."""
        search = launch_software_center.child(roleName="entry")
        assert search is not None, "No search entry found in GNOME Software"

    def test_flatpak_search_returns_results(self, launch_software_center):
        """Searching for 'Flatseal' must return at least one result."""
        search = launch_software_center.child(roleName="entry")
        search.click()
        typeText("Flatseal")
        pressKey("Return")
        time.sleep(3.0)
        # Results appear as list items or tiles
        results = launch_software_center.findChildren(
            lambda n: n.roleName in ("list item", "push button") and "Flatseal" in (n.name or "")
        )
        assert len(results) > 0, "No Flatseal results found in GNOME Software search"

    def test_install_button_accessible(self, launch_software_center):
        """The Install button for a search result must be in the AT-SPI tree.
        Does NOT click Install — validates accessibility mapping only."""
        search = launch_software_center.child(roleName="entry")
        search.click()
        typeText("Flatseal")
        pressKey("Return")
        time.sleep(3.0)
        # Click the first result to open the app detail page
        results = launch_software_center.findChildren(
            lambda n: "Flatseal" in (n.name or "")
        )
        assert len(results) > 0, "Flatseal result not found"
        results[0].click()
        time.sleep(2.0)
        # The Install button must exist on the detail page
        install_btn = launch_software_center.child("Install", roleName="push button")
        assert install_btn is not None, "Install button not found in AT-SPI tree"
        assert install_btn.sensitive, "Install button is not sensitive (clickable)"


class TestBazaarRegressions:
    """Regression tests for known Bazaar (gnome-software) bugs."""

    def test_no_keyring_modal_on_launch(self, launch_software_center):
        """Bazaar must not trigger 'Unlock Login Keyring' modal on launch (regression: bluefin#4062)."""
        time.sleep(2.0)
        # Check for the keyring unlock dialog
        try:
            keyring_dialog = root.application("gnome-keyring-ask")
            assert keyring_dialog is None, "Keyring unlock modal appeared on Bazaar launch"
        except Exception:
            pass  # application() raises if not found — that's the correct (passing) behaviour

        # Also check for any dialog titled "Unlock Login Keyring"
        dialogs = root.findChildren(
            lambda n: n.roleName == "dialog" and "keyring" in (n.name or "").lower()
        )
        assert len(dialogs) == 0, f"Unexpected keyring dialog: {[d.name for d in dialogs]}"

    def test_no_lingering_high_cpu_after_close(self):
        """gnome-software process must exit cleanly after the window is closed (regression: bluefin#4471)."""
        # Launch
        pressKey("super")
        time.sleep(1.5)
        typeText("Software")
        time.sleep(1.0)
        pressKey("Return")
        time.sleep(3.0)
        app = root.application("gnome-software")
        assert app is not None, "GNOME Software did not launch"

        # Close via Alt+F4
        app.child(roleName="frame").keyCombo("<Alt>F4")
        time.sleep(3.0)

        # Assert no gnome-software process remains
        result = subprocess.run(
            ["pgrep", "-x", "gnome-software"],
            capture_output=True, text=True
        )
        assert result.returncode != 0, \
            f"gnome-software still running after window close (PIDs: {result.stdout.strip()})"
