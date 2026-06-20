Feature: Install a sauce

  Scenario: No sources enabled fails with clear error
    Given saucepan.toml has no source sections
    When I run install <name>
    Then the process fails
    And stderr contains "no sources enabled"

  Scenario: Already in local index skips fetch
    Given [local] is enabled
    And the sauce is already in .saucepan/index.json
    When I run install <name>
    Then stdout contains "already installed"
    And no git operations are performed

  Scenario: Install from github source clones into github/<name>/
    Given [github] is enabled with binary = "git"
    And a local git repo exists with a valid sauce.json
    When I run install <repo-path>
    Then github/<sanitized-name>/ exists in the workspace
    And .saucepan/index.json contains a github entry for <name>
    And stdout contains "installed"

  Scenario: Install from github source with custom manifest name
    Given [github] has manifest = "my-manifest.json"
    And the repo contains my-manifest.json instead of sauce.json
    When I run install <repo-path>
    Then the manifest is read from my-manifest.json
    And the sauce is registered in .saucepan/index.json

  Scenario: Install from customgit source clones into customgit/<name>/
    Given [customgit] is enabled with a base url and binary = "git"
    And a local git repo exists at <base-url>/<name>
    When I run install <name>
    Then customgit/<sanitized-name>/ exists in the workspace
    And .saucepan/index.json contains a customgit entry

  Scenario: All sources fail produces a clear error
    Given [github] is enabled
    And the repo does not exist
    When I run install <name>
    Then the process fails
    And stderr contains "could not install"
