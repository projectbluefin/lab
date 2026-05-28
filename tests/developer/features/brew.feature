@developer_suite @brew
Feature: Homebrew bootstrap coverage
  Validates Homebrew bootstrap, PATH integration, and profile configuration.

  @brew_setup
  Scenario: Homebrew bootstrap service completed successfully
    * Homebrew bootstrap service completed successfully

  @brew_path
  Scenario: Homebrew binary is available on PATH
    * Homebrew binary is available on PATH

  @brew_doctor
  Scenario: Homebrew doctor completes without unexpected warnings
    * Homebrew doctor completes without unexpected warnings

  @brew_profile
  Scenario: Homebrew profile integration is configured
    * Homebrew profile integration is configured

  @brewfile @wip
  Scenario: Homebrew installs a formula from a fixture Brewfile
    * A fixture Brewfile exists at "/tmp/bluefin-test-Brewfile" with formula "jq"
    * Running "brew bundle install" with the fixture Brewfile succeeds
    * The formula "jq" is installed via Homebrew

  @brewfile @idempotent @wip
  Scenario: Running brew bundle install twice produces no unexpected drift
    * A fixture Brewfile exists at "/tmp/bluefin-test-Brewfile" with formula "jq"
    * Running "brew bundle install" with the fixture Brewfile succeeds
    * Running "brew bundle install" a second time exits cleanly with no changes
