param(
    [ValidateSet("windows", "linux", "all")]
    [string]$Target = "all",
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($Version)) {
    $versionLine = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $versionLine) {
        throw "Could not read package version from Cargo.toml"
    }
    $Version = $versionLine.Matches[0].Groups[1].Value
}

$dist = Join-Path $repoRoot "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null

function Build-WindowsPackage {
    $name = "cptool-v$Version-windows-x86_64"
    $packageDir = Join-Path $dist $name
    $zipPath = Join-Path $dist "$name.zip"
    $targetDir = Join-Path $repoRoot "target\release-windows"

    if (Test-Path $packageDir) {
        Remove-Item -Recurse -Force -LiteralPath $packageDir
    }
    if (Test-Path $zipPath) {
        Remove-Item -Force -LiteralPath $zipPath
    }

    $oldTargetDir = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = $targetDir
        cargo build --release
    }
    finally {
        if ($null -eq $oldTargetDir) {
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        } else {
            $env:CARGO_TARGET_DIR = $oldTargetDir
        }
    }

    New-Item -ItemType Directory -Force -Path $packageDir | Out-Null
    Copy-Item -LiteralPath (Join-Path $targetDir "release\cptool.exe") -Destination (Join-Path $packageDir "cptool.exe")
    Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination (Join-Path $packageDir "README.md")
    Compress-Archive -Path $packageDir -DestinationPath $zipPath -CompressionLevel Optimal

    & (Join-Path $packageDir "cptool.exe") --version
    Write-Host "created $zipPath"
}

function Build-LinuxPackage {
    $wslRepoRoot = ConvertTo-WslPath $repoRoot
    if ([string]::IsNullOrWhiteSpace($wslRepoRoot)) {
        throw "Could not translate repository path for WSL"
    }
    $quotedRepoRoot = $wslRepoRoot.Replace("'", "'\''")
    wsl bash -lc "cd '$quotedRepoRoot' && VERSION='$Version' bash scripts/build-release-linux.sh"
}

function ConvertTo-WslPath {
    param([string]$Path)

    if ($Path -match '^([A-Za-z]):\\(.*)$') {
        $drive = $Matches[1].ToLowerInvariant()
        $relative = $Matches[2] -replace '\\', '/'
        return "/mnt/$drive/$relative"
    }

    return (& wsl wslpath -a $Path).Trim()
}

function Write-Checksums {
    $files = Get-ChildItem $dist -File |
        Where-Object { $_.Name -like "cptool-v$Version-*" } |
        Sort-Object Name
    if ($files.Count -eq 0) {
        return
    }

    $lines = foreach ($file in $files) {
        $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName
        "$($hash.Hash.ToLower())  $($file.Name)"
    }
    $checksumPath = Join-Path $dist "SHA256SUMS.txt"
    $lines | Set-Content -Encoding ascii -Path $checksumPath
    Write-Host "created $checksumPath"
}

switch ($Target) {
    "windows" { Build-WindowsPackage }
    "linux" { Build-LinuxPackage }
    "all" {
        Build-WindowsPackage
        Build-LinuxPackage
    }
}

Write-Checksums
