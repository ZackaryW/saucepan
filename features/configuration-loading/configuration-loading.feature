Feature: Configuration loading

  Scenario: Missing configuration is a public configuration error
    Given a workspace without saucepan.toml
    When I run the list command
    Then the command exits with code 3
    And stderr names saucepan.toml
