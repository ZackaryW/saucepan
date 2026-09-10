## Purpose

Provide sourceable POSIX shell functions for Saucepan's central-store CLI without introducing a second configuration, storage, or authority implementation.

## Requirements

### Requirement: Central-store command wrappers
The library SHALL expose named functions for initialization, registration, configuration, acquisition, view retrieval/verification, path lookup, mirroring, history, snapshots, and shared executable lookup, plus a generic command function. Callers SHALL own JSON input files and output parsing.

#### Scenario: Acquire and inspect content
- **WHEN** a shell caller registers an app and supplies a recipe file
- **THEN** acquisition and subsequent view/history commands return the core's JSON unchanged

### Requirement: Literal forwarding and shell state preservation
The library SHALL use quoted arguments without eval or assembled command strings and SHALL isolate invocation variables in a subshell. It SHALL preserve the caller's cwd, options, and traps.

#### Scenario: Literal context
- **WHEN** an app name or filename contains spaces or shell metacharacters
- **THEN** the executable receives it as one literal argument

### Requirement: External executable and explicit context
Environment variables SHALL select the supplied executable, app, marker, authoritative mode, and optional test root/key. The default executable SHALL reside under the user's `.saucepan/bin`. The library SHALL NOT download binaries, register apps implicitly, refresh tokens, or read local TOML settings.

#### Scenario: Incomplete test configuration
- **WHEN** only one of the test root/key pair is supplied or the key is malformed
- **THEN** the wrapper emits a diagnostic and exits 64 without opening the real user store

### Requirement: Preserve native results
The wrapper SHALL preserve CLI stdout, stderr, and exit status; invalid wrapper configuration SHALL exit 64. Its runtime SHALL require only a POSIX shell and the compatible executable.

#### Scenario: Invalid proof
- **WHEN** the core rejects a marker or saved view
- **THEN** the shell caller receives the core's failure status and diagnostic

#### Scenario: Windows caller
- **WHEN** the library is used on Windows
- **THEN** it runs in a POSIX shell such as Git Bash with a compatible executable supplied or installed at the default path
