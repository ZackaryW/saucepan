Feature: Safe sauce uninstall

  Scenario: Uninstall removes managed state but preserves local sources
    Given one managed sauce and one local sauce are installed
    When I uninstall both sauces by manifest name
    Then both entries are absent from the local index
    And the managed checkout is removed
    And the local source path remains
