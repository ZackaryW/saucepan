Feature: Uninstall a sauce

  Scenario: Uninstall a github sauce removes its managed checkout
    Given the sauce is installed from github source
    When I run uninstall <manifest-name>
    Then the sauce is removed from .saucepan/index.json
    And its checkout beneath github/ is removed

  Scenario: Uninstall a customgit sauce removes its managed checkout
    Given the sauce is installed from customgit source
    When I run uninstall <manifest-name>
    Then the sauce is removed from .saucepan/index.json
    And its checkout beneath customgit/ is removed

  Scenario: Uninstall a local sauce preserves its source path
    Given the sauce is installed from local source
    When I run uninstall <manifest-name>
    Then the sauce is removed from .saucepan/index.json
    And its local source path still exists

  Scenario: Uninstall recovers from a missing managed checkout
    Given a managed sauce remains indexed after its checkout was removed
    When I run uninstall <manifest-name>
    Then the stale index entry is removed
    And the process succeeds

  Scenario: Uninstall an unknown sauce reports not found
    Given the sauce is not installed
    When I run uninstall <manifest-name>
    Then the process fails with exit code 1
    And the index is unchanged
