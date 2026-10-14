# Spec Delta

## ADDED Requirements

### Requirement: Group preparation coordinates participating claim selections
Application-owned group preparation SHALL apply existing exact-match, covered-view, scope, context and policy-adoption rules across persisted claims and the complete requested set. Compatible duplicate or covered members SHALL share a claim and retain distinct named views; fresh compatible containing intents SHALL be resolved independently of input ordering. An existing narrower claim SHALL NOT be silently replaced by a broader owner. Distinct explicit scopes SHALL retain independent claim identities. Successful preparation SHALL publish participating claim changes, registry membership and the selected group generation atomically, incrementing the affected application revision once. Unrelated claims, context and active bindings SHALL be preserved. Preparation SHALL NOT imply activation, claim release or policy adoption. Failure and concurrent release/uninstall SHALL not create partial claims or resurrect removed ownership.

#### Scenario: Duplicate intents in one new group
- **WHEN** a new group has two compatible same-scope intents for the same selection
- **THEN** successful preparation creates one claim and both names resolve through it

#### Scenario: Existing ownership rejects broadening
- **WHEN** a group requests a parent folder while that scope already owns a narrower claim
- **THEN** preparation fails under existing overlap rules without replacing the claim or publishing a partial group

#### Scenario: A multi-claim publication succeeds
- **WHEN** two independent scoped claims are updated by one group preparation
- **THEN** both selections and the group generation become visible together with one application revision increment and no active-binding change
