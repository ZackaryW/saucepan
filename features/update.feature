Feature: Update an installed sauce

  Scenario: Update a sauce not in the index fails
    Given the sauce is not installed
    When I run update <name>
    Then the process fails
    And stderr contains "not installed"

  Scenario: Update a local sauce fails with clear error
    Given the sauce is installed from local source
    When I run update <name>
    Then the process fails
    And stderr contains "local sauces do not support update"

  Scenario: Update a github sauce pulls latest and refreshes index
    Given the sauce is installed from github source
    And a newer version exists in the repo
    When I run update <name>
    Then .saucepan/index.json reflects the new version
    And stdout contains "updated"

  Scenario: Update a customgit sauce pulls latest and refreshes index
    Given the sauce is installed from customgit source
    When I run update <name>
    Then .saucepan/index.json is refreshed
    And stdout contains "updated"
