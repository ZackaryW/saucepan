## MODIFIED Requirements

### Requirement: Source visibility is explicit verified administration
Trusted local administration SHALL configure source public discovery by explicitly adopting a source_visibility_policy UUID through native and CLI interfaces. Missing visibility policy SHALL mean private. Adoption SHALL check the source attachment revision and preserve source identity, provider definition and discovery cache, protecting the changed metadata using the store-selected integrity mode. Registration retries SHALL preserve the configured references. App sessions and caller catalogs SHALL NOT publish source visibility. A visibility-setting convenience command SHALL register an immutable definition and adopt its reference atomically rather than maintaining a second boolean authority; query results SHALL retain the derived public-discovery boolean. Sources in a supported flat store with no visibility attachment SHALL remain private. Earlier nested-policy and legacy stores SHALL remain unsupported. Basic stores SHALL retain the same visibility behavior without keyring access.

#### Scenario: Publish and withdraw
- **WHEN** an administrator publishes and then withdraws a source
- **THEN** subsequent anonymous queries include and then omit it without changing its UUID or claims

#### Scenario: Visibility is independent from application restrictions
- **WHEN** an application has private claim scopes and a source adopts a public discovery policy
- **THEN** anonymous source queries can discover that source but cannot discover the private claims or gain acquisition permission
