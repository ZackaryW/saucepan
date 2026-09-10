Feature: Python SDK central-store contract

  Scenario: Acquired entries belong to the calling app view
    Given two registered apps and a local source
    When the first app acquires the source through the Python SDK
    Then only the first app sees the acquired entry
    And its current view verifies through the Python SDK

  Scenario: Settings changes keep the caller token stable
    Given two registered apps and a local source
    When the first app changes its settings through the Python SDK
    Then its original caller token reads the new settings
