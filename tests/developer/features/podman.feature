@developer_suite
Feature: Podman Desktop smoke tests
  Validates Podman Desktop Flatpak launches and shows main UI.
  Regression for dakota#430 (Podman Desktop Flatpak missing dependency).

  @podman_desktop @launch @regression @dakota_430
  Scenario: Podman Desktop Flatpak launches without missing dependency error (dakota#430)
    * Start application "podman_desktop" via "command"
    * Wait until "Podman Desktop" "frame" appears in "podman_desktop"
    * Application "podman_desktop" is running
    * No Flatpak missing-runtime error for "io.podman_desktop.PodmanDesktop"

  @podman_desktop @ui
  Scenario: Podman Desktop main window shows Dashboard
    * Start application "podman_desktop" via "command"
    * Wait until "Podman Desktop" "frame" appears in "podman_desktop"
    * Item "Dashboard" "label" is "showing" in "podman_desktop"

  @podman_desktop @close
  Scenario: Podman Desktop closes cleanly
    * Start application "podman_desktop" via "command"
    * Wait until "Podman Desktop" "frame" appears in "podman_desktop"
    * Close application "podman_desktop" via "shortcut"
    * Application "podman_desktop" is no longer running
