param(
    [string]$Version = "",
    [ValidateSet("windows", "linux", "all")]
    [string]$Target = "all",
    [string]$Repo = "panic-0/cptool",
    [switch]$Draft,
    [switch]$Prerelease,
    [switch]$SkipChecks,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
Set-Location $repoRoot

$Python = $env:PYTHON
if (-not $Python) {
    $Python = "python"
}

function Get-CargoVersion {
    $versionLine = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $versionLine) {
        throw "Could not read package version from Cargo.toml"
    }
    return $versionLine.Matches[0].Groups[1].Value
}

function Get-GhPath {
    $command = Get-Command gh -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $commonPaths = @(
        "C:\Program Files\GitHub CLI\gh.exe",
        "C:\Program Files (x86)\GitHub CLI\gh.exe"
    )
    foreach ($path in $commonPaths) {
        if (Test-Path -LiteralPath $path) {
            return $path
        }
    }

    throw "GitHub CLI not found. Install gh or add it to PATH."
}

function Assert-CleanWorktree {
    $status = git status --porcelain
    if (-not [string]::IsNullOrWhiteSpace($status)) {
        throw "Working tree is not clean. Commit or stash changes before releasing.`n$status"
    }
}

function Assert-CommandOk {
    param([scriptblock]$Command)
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE"
    }
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-CargoVersion
}

$cargoVersion = Get-CargoVersion
if ($Version -ne $cargoVersion) {
    throw "Requested version $Version does not match Cargo.toml version $cargoVersion"
}

$tag = "v$Version"
$gh = Get-GhPath

Assert-CleanWorktree

$existingTag = git tag --list $tag
if (-not [string]::IsNullOrWhiteSpace($existingTag)) {
    throw "Local tag $tag already exists"
}

$remoteTag = git ls-remote --tags origin $tag
if (-not [string]::IsNullOrWhiteSpace($remoteTag)) {
    throw "Remote tag $tag already exists on origin"
}

$releaseViewSucceeded = $false
try {
    & $gh release view $tag --repo $Repo *> $null
    $releaseViewSucceeded = ($LASTEXITCODE -eq 0)
}
catch {
    $releaseViewSucceeded = $false
}
if ($releaseViewSucceeded) {
    throw "GitHub release $tag already exists in $Repo"
}

if (-not $SkipChecks) {
    Assert-CommandOk { python scripts/check.py }
}

if (-not $SkipBuild) {
    Assert-CommandOk {
        & $Python (Join-Path $scriptDir "build_release.py") `
            --usage-name "python scripts/build_release.py" `
            --target $Target `
            --version $Version
    }
}

$assets = Get-ChildItem -Path (Join-Path $repoRoot "dist") -File |
    Where-Object { $_.Name -like "cptool-v$Version-*" -or $_.Name -eq "SHA256SUMS.txt" } |
    Sort-Object Name

if ($assets.Count -eq 0) {
    throw "No release assets found in dist/. Run without -SkipBuild or build artifacts first."
}

$currentBranch = git branch --show-current
if ([string]::IsNullOrWhiteSpace($currentBranch)) {
    throw "Cannot release from detached HEAD"
}

Assert-CommandOk { git push origin $currentBranch }
Assert-CommandOk { git tag -a $tag -m "cptool $tag" }
Assert-CommandOk { git push origin $tag }

$releaseArgs = @(
    "release", "create", $tag,
    "--repo", $Repo,
    "--title", "cptool $tag",
    "--generate-notes"
)
if ($Draft) {
    $releaseArgs += "--draft"
}
if ($Prerelease) {
    $releaseArgs += "--prerelease"
}
foreach ($asset in $assets) {
    $releaseArgs += $asset.FullName
}

Assert-CommandOk { & $gh @releaseArgs }
Write-Host "released $tag to $Repo"
