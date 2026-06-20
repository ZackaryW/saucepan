Feature: Configuration loading

  Scenario: Missing saucepan.toml fails with clear error
    Given a folder with no saucepan.toml
    When I run any subcommand
    Then the process fails
    And stderr contains "saucepan.toml"

  Scenario: Invalid TOML fails with clear error
    Given saucepan.toml contains invalid TOML
    When I run any subcommand
    Then the process fails
    And stderr contains "invalid saucepan.toml"

  Scenario: Empty config is valid (no sources enabled)
    Given saucepan.toml is empty
    When I run list
    Then the process succeeds

  Scenario: [local] presence enables local source
    Given saucepan.toml contains [local]
    When I run install <name>
    Then the local index is consulted

  Scenario: jq = "<path>" overrides the jq binary used for search
    Given saucepan.toml sets jq = "/custom/jq"
    When I run search
    Then that binary is invoked instead of "jq"
