Feature: Python SDK command and entity contract

  Scenario: Bucket pin is exposed by the SDK
    Given an SDK workspace and a repository-target index tagged v1
    When I add the index through the SDK at ref v1
    Then the SDK bucket collection records ref v1 and its resolved commit

  Scenario: Bucket refresh is exposed by the SDK
    Given an SDK workspace with an unpinned registered repository-target index
    When the index advances and I refresh it through the SDK
    Then the SDK bucket collection records the refreshed commit

  Scenario: SDK entities expose central-index additions
    Given CLI state containing an index-sourced sauce and a bucket stub with extra fields
    When I read sauces and bucket stubs through the SDK
    Then the sauce exposes its index manifest source
    And the bucket stub exposes its extra fields
