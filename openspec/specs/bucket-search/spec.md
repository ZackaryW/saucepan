## Purpose

Define jq-backed public search over registered bucket entries, including empty inputs, arbitrary filters, and executable selection.

## Requirements

### Requirement: Search handles an empty registry
Search SHALL succeed without invoking jq when no buckets are registered and SHALL report that condition.

#### Scenario: No buckets registered
- **WHEN** a user runs `search` with an empty bucket registry
- **THEN** Saucepan reports that no buckets are registered

### Requirement: Search returns entries selected by jq
Search SHALL stream registered bucket entries through the configured jq filter, return matching values, and report when the filter matches nothing.

#### Scenario: Filter matches an entry
- **WHEN** a registered bucket contains an entry selected by the user's jq filter
- **THEN** Saucepan writes the selected value to stdout

#### Scenario: Filter matches nothing
- **WHEN** registered buckets contain no value selected by the user's jq filter
- **THEN** Saucepan reports that there are no matches

### Requirement: Search uses the configured jq executable
Search SHALL use the workspace's configured jq executable and SHALL surface launch failures through the stable error contract.

#### Scenario: Custom jq executable
- **WHEN** a workspace configures a custom jq path and the user searches
- **THEN** Saucepan invokes that path instead of the default executable
