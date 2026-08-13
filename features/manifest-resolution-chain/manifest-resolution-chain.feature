Feature: Ordered manifest resolution

  Scenario: A central index supplies a missing repository manifest
    Given a manifest-less repository and a registered index with a pinned manifest
    When I install the manifest-less repository
    Then installation succeeds with the index manifest identity
    And machine-readable state records index manifest provenance
