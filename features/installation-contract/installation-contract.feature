Feature: Installation failure contract

  Scenario: Exhausted manifest resolution remains not found
    Given a manifest-less repository and no matching central index
    When I install the manifest-less repository
    Then the command exits with code 1
    And stderr reports that no root manifest was found
