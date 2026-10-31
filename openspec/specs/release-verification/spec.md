# release-verification Specification

## Purpose
Publish a Saucepan release only after its exact packaged artifacts pass acceptance on every verified platform, and withdraw a failing candidate without affecting consumers that resolve published releases.

## Requirements

### Requirement: Candidates are staged as draft releases
A version tag SHALL produce a draft release containing the complete validated artifact set: binary archives, SDK archives, SDK manifest, both installers and `SHA256SUMS`. Artifacts SHALL be compiled only in the build stage. A draft SHALL NOT be resolvable through the public release download URLs, `releases/latest` or `releases/tags/<tag>`, so official installers, Saucepan setup and self-update, and Powerspec cannot obtain an unverified candidate.

#### Scenario: Candidate is not installable before verification
- **WHEN** a tag has been built and staged but verification has not finished
- **THEN** `setup --tag <tag>`, `self-update --tag <tag>` and the official installer for that tag cannot download it

### Requirement: Verification uses the staged artifacts without compiling
Staging SHALL confirm that the draft's assets are byte-identical to the staged set, and each verification job SHALL run acceptance against that staged set without invoking a compiler and without write access to the repository. Verification SHALL run the host platform's own archive on Windows x64 from PowerShell, Windows x64 from Git Bash, macOS arm64 and Linux x64. Packaged targets without a verification runner SHALL be reported as not verified.

#### Scenario: Tested bytes are the shipped bytes
- **WHEN** verification passes on a platform
- **THEN** the archive it exercised has the digest recorded for that asset in the draft's `SHA256SUMS`

#### Scenario: Unverified targets are reported
- **WHEN** a candidate includes Windows ARM64 and macOS x86_64 archives
- **THEN** the verification summary lists them as packaged but not verified

### Requirement: Promotion publishes the draft unchanged
When every verification job passes, the draft SHALL be published without rebuilding, re-uploading or renaming assets, preserving the prerelease flag derived from the tag. A stable release SHALL become the latest release. Promotion SHALL NOT run when any verification job failed, was skipped or was cancelled.

#### Scenario: Pinned digests stay valid
- **WHEN** a consumer recorded the installer digest from the verified candidate
- **THEN** the published installer has the same digest

#### Scenario: Promotion interrupted
- **WHEN** publishing the draft fails
- **THEN** the release remains a draft, the tag is not revoked, and rerunning promotion completes it

### Requirement: Failed candidates are revoked
When the build, staging or any verification job fails or is cancelled before promotion, the workflow SHALL revoke the candidate: delete the draft release if one exists, create the tag `revoked/<tag>-<short-commit>` at the candidate commit, delete the version tag, and keep every verification report as a workflow artifact. Revocation SHALL refuse to act on a release that is already published.

#### Scenario: One platform fails
- **WHEN** macOS verification fails while other platforms are still running
- **THEN** the remaining verification jobs are cancelled, the draft is deleted, the version tag is moved to its `revoked/` name, and the run summary names macOS and the failing scenario

#### Scenario: Published release is never revoked
- **WHEN** revocation runs against a tag whose release is already published
- **THEN** it fails without deleting the release or moving the tag

#### Scenario: Version is retried after revocation
- **WHEN** a fix is committed and the same version tag is pushed again
- **THEN** the pipeline builds, stages and verifies the new commit, and the earlier attempt remains reachable through its `revoked/` tag

### Requirement: Failures are reproducible from the reports
Every verification job SHALL upload its acceptance report and identify the platform, launch environment, artifact digests and failing scenario. The run summary SHALL give the command that reproduces the failure locally against the same downloaded artifacts.

#### Scenario: Local reproduction
- **WHEN** a developer downloads the revoked candidate's artifacts and runs the reproduction command from the summary on the same platform
- **THEN** acceptance runs the failing scenario against those exact archives without building
