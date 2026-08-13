Feature: Sauce update

  Scenario: An index-sourced update adopts the current index ref
    Given an installed index-sourced sauce pinned by the index to v1
    And the registered index now supplies manifest version 2.0.0 at ref v2
    When I update the installed sauce
    Then the installed sauce reports version 2.0.0
    And its resolved commit is the v2 commit
    And its manifest provenance still identifies the registered index
