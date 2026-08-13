Feature: GitHub target matching

  Scenario: Equivalent target spellings match without changing provenance
    Given a manifest-less repository indexed with an equivalent trailing-separator target
    When I install the repository target without the trailing separator
    Then installation succeeds with the index manifest identity
    And machine-readable state retains the original repository target
