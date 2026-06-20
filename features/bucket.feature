Feature: Bucket management

  Scenario: List with no buckets registered
    Given no buckets are registered
    When I run bucket list
    Then stdout contains "no buckets registered"

  Scenario: Add a bucket URL
    When I run bucket add <url>
    Then the URL appears in .saucepan/buckets.json
    And stdout contains "bucket added"

  Scenario: Adding the same URL twice fails
    Given a bucket is already registered
    When I run bucket add <same-url>
    Then the process fails
    And stderr contains "already registered"

  Scenario: Remove a registered bucket
    Given a bucket is registered
    When I run bucket remove <url>
    Then the URL is removed from .saucepan/buckets.json
    And stdout contains "bucket removed"

  Scenario: Remove an unregistered URL fails
    When I run bucket remove <unknown-url>
    Then the process fails
    And stderr contains "not found"
