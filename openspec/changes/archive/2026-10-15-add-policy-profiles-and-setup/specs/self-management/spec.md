# Spec Delta

## ADDED Requirements

### Requirement: First setup is explicit and supports profile-prepared release requests
Saucepan SHALL document and support first setup beginning with an explicitly obtained release manager and matching built shim. The flow SHALL explicitly initialize the selected root, register ordinary profile-expanded installation policies and GitHub release source/filter records, and invoke the existing bootstrap operation. Profile choice SHALL not clone a repository, choose a different identity/scope contract, alter verification modes or replace normal platform selection. The default root SHALL retain existing user-home .saucepan resolution. Setup instructions SHALL describe adding its bin directory to PATH without editing shell profiles or environment settings automatically. Installing a language SDK SHALL NOT perform these steps.

#### Scenario: Basic setup without a keyring
- **WHEN** a caller explicitly initializes a basic root and bootstraps from a GitHub release request with registered explicit-update installation refs
- **THEN** the existing release acquisition, version probe, managed claim and stable shim flow succeeds without a keyring requirement

#### Scenario: SDK package installed without a manager
- **WHEN** a user installs or imports a language SDK with no manager present
- **THEN** no Saucepan root, binary, source, claim or launcher is created and attempted manager operations report the missing executable

### Requirement: Profile acceptance exercises the real self-management boundary
Profile-assisted self-management acceptance SHALL use the actual built manager and shim, an isolated basic root and a controlled GitHub release fixture. It SHALL verify expanded definition identities, launch through the shim, an explicit update to distinguishable executable bytes, failed-candidate preservation and reclaim protection. Historical mock installations and real user roots SHALL remain untouched. Platform and native-keyring evidence SHALL be reported only for checks actually run.

#### Scenario: Profile-assisted release update fails validation
- **WHEN** an isolated installation acquired through for-install policies encounters an invalid new release
- **THEN** the existing selected claim and binding remain usable through the actual shim and the failed operation is inspectable
