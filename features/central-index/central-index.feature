Feature: Central-index precedence

  Scenario: A shadowed index warns on every resolution
    Given two registered indexes describe the same manifest-less target
    When I install the target twice in separate workspaces
    Then each resolution warns that the later index is shadowed
    And each installation uses the earlier index manifest
