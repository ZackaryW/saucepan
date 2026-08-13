Feature: Search registered buckets

  Scenario: A jq filter returns a matching bucket entry
    Given a registered local bucket containing my-lib
    When I search for the entry named my-lib
    Then stdout contains the my-lib entry
