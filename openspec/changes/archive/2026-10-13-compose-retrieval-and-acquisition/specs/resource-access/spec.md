## ADDED Requirements

### Requirement: Source and acquired paths have distinct representations and lifetimes
Native and CLI access SHALL distinguish source material paths from acquired-content paths. Source descriptors SHALL report file, directory or Git repository representation and exact provenance; a bare repository SHALL not be represented as a checked-out directory. Native source access SHALL offer a non-serializable guard coordinating refresh and cleanup. Acquired access SHALL offer output lifetime protection or explicit caller ownership. Bare path lookups SHALL not extend either lifetime or perform updates. A native temporary acquired result SHALL retain any dependency guards needed for continued use.

#### Scenario: Inspect repository then use an acquired folder
- **WHEN** a caller accesses a retrieved Git repository and acquires its plugin subpath
- **THEN** source_path refers to repository data and the acquisition path refers to the copied plugin files
- **AND** each result identifies its ownership and whether a live guard is held

#### Scenario: CLI requests a temporary path without an owner
- **WHEN** a caller requests bare temporary retrieval output that would expire when the CLI exits
- **THEN** the CLI rejects it with guidance to use a child-consumer workflow or caller-owned destination

### Requirement: Retrieved-material execution retains source lifetime
The CLI SHALL provide run-retrieved FILE -- PROGRAM ARG... with literal argument forwarding, source-path and representation metadata in the child environment, inherited standard streams and child exit status. It SHALL retain cached source access or temporary material through the supported native child lifetime, including unexpected wrapper exit. It SHALL not expose a writable cache capability or silently substitute an acquired output. Failure to prepare or start the child SHALL leave no live orphan protection.

#### Scenario: Read original ZIP in a child
- **WHEN** a caller runs a consumer on a temporary ZIP retrieval
- **THEN** the child can read the original archive until its protected lifetime ends without an extracted output or persistent cache

### Requirement: Temporary acquired files have a process consumer workflow
The CLI SHALL provide run-acquired FILE -- PROGRAM ARG... composing retrieval and acquisition before starting a consumer. It SHALL expose acquired files through SAUCEPAN_RESOURCE_DIR and preserve literal arguments, standard streams and child exit status. Temporary copies and required parent guards SHALL survive the supported child lifetime, including unexpected wrapper exit. It SHALL reject a bare temporary acquisition result that would expire on CLI exit and SHALL clean up failed launches without orphan protection.

#### Scenario: Use a temporary Git subpath without persistent caching
- **WHEN** a caller runs an acquired consumer using temporary Git retrieval and temporary subpath acquisition
- **THEN** the child receives the copied subpath for its protected lifetime
- **AND** no managed retrieval, output or claim records are created
