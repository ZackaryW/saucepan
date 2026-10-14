# Spec Delta

## ADDED Requirements

### Requirement: Explicit pins independently override output reclaim eligibility
Reclaim SHALL preserve every output named by an active verified pin and report active-pin protection for otherwise eligible entries. Pin protection SHALL apply under history none or any latest-count policy without changing the computed revision window. Releasing one pin SHALL leave other pins, live leases, active bindings, pending lifecycle candidates and surviving retention demands effective. Plain claims and group registrations SHALL remain reclaimable unless a separate protection applies. Pinning SHALL not rewrite immutable output policies or provenance, create a new content entry or prevent independent source-cache maintenance.

#### Scenario: Pinned output falls outside revision history
- **WHEN** a pinned old output falls outside a latest-one window
- **THEN** reclaim reports its pin protection and leaves its bytes while still reclaiming eligible unprotected entries

#### Scenario: Final pin is released while a lease is alive
- **WHEN** the final durable pin is released but a live lease still uses the output
- **THEN** reclaim continues deferring deletion until that lease and all other applicable protection end
