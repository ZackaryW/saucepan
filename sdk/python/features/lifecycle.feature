Feature: Manage installed sauces through the SDK
  Scenario: Install a sauce without a ref
    Given a GitHub sauce repository at version "1.0.0"
    When I install the repository without a ref
    Then the returned sauce is named "my-lib"
    And the returned sauce has no requested ref
    And the returned sauce records the repository commit

  Scenario: Install a sauce with a ref
    Given a GitHub sauce repository at version "1.0.0"
    And the repository commit is tagged "v1"
    When I install the repository with ref "v1"
    Then the returned sauce is named "my-lib"
    And the returned sauce requested ref is "v1"
    And the returned sauce records the repository commit

  Scenario: Update an installed sauce
    Given a GitHub sauce repository at version "1.0.0"
    And the repository is installed through the SDK
    And the repository advances to version "2.0.0"
    When I update the installed sauce
    Then the same sauce reports version "2.0.0"
    And the returned sauce records the repository commit

  Scenario: Uninstall an installed sauce
    Given a GitHub sauce repository at version "1.0.0"
    And the repository is installed through the SDK
    When I uninstall the installed sauce
    Then the workspace has no installed sauces
