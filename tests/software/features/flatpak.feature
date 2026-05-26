@software_suite
Feature: gnome-software (Bazaar) smoke tests
  Validates Bazaar launches and core UI elements are accessible.
  Regression coverage for bluefin#4062 and #4471.

  Background:
    * Start application "software" via "command"
    * Wait until "Software" "frame" appears in "software"

  @software @launch
  Scenario: Bazaar launches and main window is visible
    * Application "software" is running
    * Item "Software" "frame" is "showing" in "software"

  @software @navigation
  Scenario: Explore tab is present and accessible
    * Item "Explore" "toggle button" is "showing" in "software"

  @software @navigation
  Scenario: Installed tab is present and accessible
    * Item "Installed" "toggle button" is "showing" in "software"

  @software @navigation
  Scenario: Clicking Installed tab shows installed apps list
    * Left click "Installed" "toggle button" in "software"
    * Wait until "Installed" "page tab" appears in "software"

  @software @search
  Scenario: Search bar accepts input and returns results
    * Left click "Search" "toggle button" in "software"
    * Type text: "Firefox" with uinput
    * Wait until "Firefox" "label" appears in "software"

  @software @search
  Scenario: Searching for Flatseal returns a visible result
    * Search for "Flatseal" in Bazaar
    * Bazaar search shows result "Flatseal"

  @software @search
  Scenario: Flatseal detail page exposes an Install button
    * Search for "Flatseal" in Bazaar
    * Open Bazaar search result "Flatseal"
    * Bazaar install button is accessible

  @software @regression @bluefin_4062
  Scenario: Bazaar launch does not show the login keyring modal (bluefin#4062)
    * No keyring dialog is visible for Bazaar

  @software @stability
  Scenario: Installed tab is reachable without Bazaar crashing
    * Left click "Installed" "toggle button" in "software"
    * Run and save command output: "journalctl -b --no-pager --since=\"${TEST_JOURNAL_SINCE:-1 minute ago}\" -g 'gnome-software.*segfault\|gnome-software.*abort' | grep -c . || echo 0"
    * Last command output "is" "0"

  @software @regression @bluefin_4471
  Scenario: No gnome-software coredump on Explore page load (bluefin#4471)
    * Left click "Explore" "toggle button" in "software"
    * Wait 2 seconds before action
    * Run and save command output: "coredumpctl list gnome-software --no-pager --since=\"${TEST_JOURNAL_SINCE:-1 minute ago}\" 2>&1 | grep -c 'gnome-software' || echo 0"
    * Last command output "is" "0"

  @software @close
  Scenario: Bazaar closes cleanly via shortcut
    * Close application "software" via "shortcut"
    * Application "software" is no longer running
