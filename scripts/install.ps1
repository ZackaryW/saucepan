# Saucepan official installer; rendered from the release commit by scripts/release.py.
# Release-Tag: @SAUCEPAN_RELEASE_TAG@
# Release-Commit: @SAUCEPAN_RELEASE_COMMIT@
[CmdletBinding()]
param(
    [string]$Version = '@SAUCEPAN_RELEASE_TAG@',
    [string]$Root = (Join-Path ([Environment]::GetFolderPath('UserProfile')) '.saucepan'),
    [string]$Operation,
    [switch]$MigrateUnmanaged,
    [string]$ExpectedSha256,
    [string]$TestReleaseBase,
    [string]$TestGithubApi
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$PSNativeCommandUseErrorActionPreference = $false
$scratch = $null
$exitCode = 1
$previousEncoding = [Console]::OutputEncoding
try {
    if ($PSVersionTable.PSVersion -lt [Version]'5.1') { throw 'PowerShell 5.1 or newer is required' }
    if ($Version -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$') { throw 'Select an exact release with -Version vMAJOR.MINOR.PATCH' }
    if ([string]::IsNullOrWhiteSpace($Root)) { throw 'Root must not be empty' }
    if ($MigrateUnmanaged -and (-not $Operation -or $ExpectedSha256 -notmatch '^[0-9a-fA-F]{64}$')) { throw 'Migration requires -Operation UUID and -ExpectedSha256 HASH' }
    if (-not $MigrateUnmanaged -and $ExpectedSha256) { throw 'ExpectedSha256 requires MigrateUnmanaged' }
    if (-not $Operation) { $Operation = [Guid]::NewGuid().ToString() }
    $parsedOperation = [Guid]::Empty
    if (-not [Guid]::TryParseExact($Operation, 'D', [ref]$parsedOperation)) { throw 'Operation must be a UUID' }
    $architecture = $env:PROCESSOR_ARCHITEW6432
    if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
    $target = switch ($architecture) {
        'AMD64' { 'x86_64-pc-windows-msvc' }
        'ARM64' { 'aarch64-pc-windows-msvc' }
        'x86' { 'i686-pc-windows-msvc' }
        default { throw "Unsupported Windows architecture: $architecture" }
    }
    $base = 'https://github.com/ZackaryW/saucepan/releases/download'
    if ($TestReleaseBase) {
        if ($TestReleaseBase -notmatch '^http://(?:127\.0\.0\.1|localhost):[0-9]+(?:/[^?#]*)?$') { throw 'TestReleaseBase must be explicit loopback HTTP' }
        $base = $TestReleaseBase.TrimEnd('/')
    }
    if ($TestGithubApi -and $TestGithubApi -notmatch '^http://(?:127\.0\.0\.1|localhost):[0-9]+$') { throw 'TestGithubApi must be explicit loopback HTTP' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $scratch = Join-Path ([IO.Path]::GetTempPath()) ('saucepan-setup-' + [Guid]::NewGuid().ToString())
    $rootPath = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    $stagePath = [IO.Path]::GetFullPath($scratch)
    if ($stagePath.StartsWith($rootPath + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or $stagePath -eq $rootPath) { throw 'Installer staging must be outside the selected store' }
    [IO.Directory]::CreateDirectory($scratch) | Out-Null
    [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
    [Console]::Error.WriteLine("Saucepan setup operation: $Operation")
    $asset = "saucepan-$Version-$target.zip"
    $archivePath = Join-Path $scratch $asset
    $sumsPath = Join-Path $scratch 'SHA256SUMS'
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$Version/SHA256SUMS" -OutFile $sumsPath -TimeoutSec 120
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$Version/$asset" -OutFile $archivePath -TimeoutSec 120
    $matching = @(Get-Content -LiteralPath $sumsPath | Where-Object { $_ -match ('^[0-9a-fA-F]{64}  ' + [regex]::Escape($asset) + '$') })
    if ($matching.Count -ne 1) { throw 'Expected exactly one checksum for the selected archive' }
    $expected = $matching[0].Substring(0, 64)
    $hasher = [Security.Cryptography.SHA256]::Create()
    $archiveStream = [IO.File]::OpenRead($archivePath)
    try { $actual = [BitConverter]::ToString($hasher.ComputeHash($archiveStream)).Replace('-', '') }
    finally { $archiveStream.Dispose(); $hasher.Dispose() }
    if ($actual -ne $expected) { throw 'Release archive checksum mismatch' }
    $zip = [IO.Compression.ZipFile]::OpenRead($archivePath)
    try {
        $allowed = @('saucepan.exe', 'saucepan-shim.exe', 'LICENSE')
        $names = @($zip.Entries | ForEach-Object { $_.FullName })
        if ($names.Count -ne 3 -or @($names | Select-Object -Unique).Count -ne 3) { throw 'Archive has missing, extra or duplicate entries' }
        foreach ($entry in $zip.Entries) {
            if ($allowed -cnotcontains $entry.FullName -or $entry.Length -le 0 -or $entry.Length -gt 536870912) { throw 'Invalid archive entry or size' }
            $kind = ($entry.ExternalAttributes -shr 16) -band 61440
            if ($kind -ne 0 -and $kind -ne 32768) { throw 'Archive contains a nonregular entry' }
        }
        foreach ($entry in $zip.Entries) {
            $reader = $entry.Open()
            $writer = [IO.File]::Open((Join-Path $scratch $entry.FullName), [IO.FileMode]::CreateNew)
            try {
                $buffer = New-Object byte[] 65536
                [long]$copied = 0
                while (($count = $reader.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $copied += $count
                    if ($copied -gt $entry.Length -or $copied -gt 536870912) { throw 'Archive entry exceeds its declared size' }
                    $writer.Write($buffer, 0, $count)
                }
                if ($copied -ne $entry.Length) { throw 'Archive entry is incomplete' }
                $writer.Flush($true)
            } finally { $reader.Dispose(); $writer.Dispose() }
        }
    } finally { $zip.Dispose() }
    $manager = Join-Path $scratch 'saucepan.exe'
    $shim = Join-Path $scratch 'saucepan-shim.exe'
    $help = & $manager setup --help 2>&1
    if ($LASTEXITCODE -ne 0 -or ($help -join "`n") -notmatch '--migrate-unmanaged') { throw 'Selected manager lacks managed setup; obtain a setup-capable release' }
    $arguments = @('--root', $Root)
    if ($TestGithubApi) { $arguments += @('--test-github-api', $TestGithubApi) }
    $arguments += @('setup', $shim, '--tag', $Version, '--operation', $Operation)
    if ($MigrateUnmanaged) { $arguments += @('--migrate-unmanaged', '--expected-sha256', $ExpectedSha256) }
    & $manager @arguments
    $exitCode = $LASTEXITCODE
    if ($exitCode -eq 0) { [Console]::Error.WriteLine("Add the managed bin directory to PATH yourself: $(Join-Path $Root 'bin')") }
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
} finally {
    [Console]::OutputEncoding = $previousEncoding
    if ($scratch -and (Test-Path -LiteralPath $scratch)) {
        # Only this explicitly created child of the temp directory is removed.
        $resolvedScratch = [IO.Path]::GetFullPath($scratch)
        $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
        if ($resolvedScratch.StartsWith($tempParent + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -and [IO.Path]::GetFileName($resolvedScratch).StartsWith('saucepan-setup-')) {
            try { Remove-Item -LiteralPath $resolvedScratch -Recurse -Force }
            catch { [Console]::Error.WriteLine("Could not remove installer scratch: $resolvedScratch") }
        }
    }
}
exit $exitCode
