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
    $wslCargoHome = Convert-CargoHomeToWslPath
    $commit = (git rev-parse HEAD).Trim()
    $quotedRepoRoot = $wslRepoRoot.Replace("'", "'\''")
    $quotedCommit = $commit.Replace("'", "'\''")
    $linuxEnv = "VERSION='$Version'"
    if (-not [string]::IsNullOrWhiteSpace($wslCargoHome)) {
        $quotedCargoHome = $wslCargoHome.Replace("'", "'\''")
        $linuxEnv += " CARGO_HOME='$quotedCargoHome'"
    }

    $tempScript = [System.IO.Path]::GetTempFileName()
    $wslScript = ConvertTo-WslPath $tempScript
    $script = @"
set -euo pipefail
export PATH="`$HOME/.cargo/bin:`$PATH"
tmp_dir=`$(mktemp -d)
trap 'rm -rf "`$tmp_dir"' EXIT
git clone --quiet '$quotedRepoRoot' "`$tmp_dir/repo"
cd "`$tmp_dir/repo"
git checkout --quiet '$quotedCommit'
git submodule update --init --recursive
$linuxEnv bash scripts/build-release-linux.sh
mkdir -p '$quotedRepoRoot/dist'
cp "dist/cptool-v$Version-linux-x86_64.tar.gz" '$quotedRepoRoot/dist/'
"@
    try {
        [System.IO.File]::WriteAllText($tempScript, ($script -replace "`r`n", "`n"), [System.Text.Encoding]::ASCII)
        wsl bash $wslScript
        if ($LASTEXITCODE -ne 0) {
            throw "Linux release build failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Remove-Item -Force -LiteralPath $tempScript -ErrorAction SilentlyContinue
    }
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

function Convert-CargoHomeToWslPath {
    $cargoHome = $env:CARGO_HOME
    if ([string]::IsNullOrWhiteSpace($cargoHome)) {
        $cargoHome = Join-Path $env:USERPROFILE ".cargo"
    }
    if (-not (Test-Path -LiteralPath $cargoHome)) {
        return ""
    }
    return ConvertTo-WslPath $cargoHome
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
