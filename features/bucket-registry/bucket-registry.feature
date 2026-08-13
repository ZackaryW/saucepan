Feature: Bucket registry

  Scenario: Pin a repository-target index
    Given a repository-target index tagged v1
    When I register the index at ref v1
    Then the registry records ref v1 and its resolved commit

  Scenario: Refresh an unpinned repository-target index
    Given an unpinned registered repository-target index
    When the index advances and I refresh it
    Then the registry records the refreshed commit
