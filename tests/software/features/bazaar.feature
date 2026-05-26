@software_suite
Feature: Bazaar (GNOME Software) update and install workflows

  Background: Bazaar is available
    * Start "org.gnome.Software" via shell
    * Application "org.gnome.Software" is opened

  @bazaar @launch
  Scenario: Bazaar launches and shows main view
    * Application "org.gnome.Software" is opened
    * Close "org.gnome.Software"

  @bazaar @updates
  Scenario: Bazaar updates tab is accessible
    * Application "org.gnome.Software" is opened
    * Activate "Updates" in "org.gnome.Software"
    * Close "org.gnome.Software"
