@smoke_suite
Feature: Browser workflow smoke tests
  Firefox is not AT-SPI accessible in headless sessions, so these scenarios
  validate browser launch via process inspection instead of the accessibility
  tree. setup-titan-fixtures must install Firefox first.

  @browser @launch @wip
  Scenario: Firefox launches from its desktop entry
    * Run and save command output: "sh -c 'gtk-launch org.mozilla.firefox >/dev/null 2>&1 || flatpak run org.mozilla.firefox >/dev/null 2>&1 &'"
    * Application "firefox" process is running
    * Kill application "firefox" process

  @browser @open_url @wip
  Scenario: xdg-open launches Firefox for a local file URL
    * Open URL "file:///etc/os-release" via xdg-open
    * Application "firefox" process is running
    * Kill application "firefox" process
