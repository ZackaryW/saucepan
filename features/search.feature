Feature: Search buckets with jq

  Scenario: No buckets registered
    Given no buckets are registered
    When I run search '<filter>'
    Then stdout contains "no buckets registered"

  Scenario: Filter matches a stub
    Given a bucket is registered with a stub named "my-lib"
    When I run search '.name == "my-lib"'
    Then stdout contains "my-lib"

  Scenario: Filter matches nothing
    Given a bucket is registered with stubs
    When I run search '.name == "nonexistent"'
    Then stdout contains "no matches"

  Scenario: Custom jq binary path is used
    Given saucepan.toml sets jq = "<path-to-jq>"
    And a bucket is registered
    When I run search '<filter>'
    Then the custom jq binary is used
