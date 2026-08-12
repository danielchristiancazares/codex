[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Step {
    param([string]$Message)

    Write-Host "==> $Message"
}

function Invoke-Checked {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$FilePath exited with code $LASTEXITCODE."
    }
}

function Get-FileSha256 {
    param(
        [string]$Path
    )

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Install-Binaries {
    param(
        [System.Collections.IDictionary]$Binaries,
        [string]$PackageDir,
        [string]$InstallDir,
        [scriptblock]$Verify
    )

    $stagingSuffix = "$PID.$([System.Guid]::NewGuid().ToString("N"))"
    $operations = [System.Collections.Generic.List[object]]::new()
    foreach ($relativeSource in $Binaries.Keys) {
        $source = Join-Path $PackageDir $relativeSource
        $destination = Join-Path $InstallDir $Binaries[$relativeSource]
        $stagedPath = "$destination.installing.$stagingSuffix"

        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Built package is missing $source."
        }
        if (Test-Path -LiteralPath $destination -PathType Container) {
            throw "Install destination is a directory: $destination."
        }
        if (Test-Path -LiteralPath $stagedPath) {
            throw "Staging path already exists: $stagedPath."
        }

        $operations.Add([pscustomobject]@{
            Source = $source
            Destination = $destination
            StagedPath = $stagedPath
        })
    }

    try {
        foreach ($operation in $operations) {
            Copy-Item `
                -LiteralPath $operation.Source `
                -Destination $operation.StagedPath

            if ((Get-FileSha256 $operation.Source) -ne (Get-FileSha256 $operation.StagedPath)) {
                throw "Staged binary does not match the build output: $($operation.Source)."
            }
        }

        foreach ($operation in $operations) {
            Move-Item `
                -LiteralPath $operation.StagedPath `
                -Destination $operation.Destination `
                -Force
        }

        & $Verify

        foreach ($operation in $operations) {
            if ((Get-FileSha256 $operation.Source) -ne (Get-FileSha256 $operation.Destination)) {
                throw "Installed binary does not match the build output: $($operation.Destination)."
            }
        }
    } finally {
        foreach ($operation in $operations) {
            if (Test-Path -LiteralPath $operation.StagedPath) {
                Remove-Item -LiteralPath $operation.StagedPath -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

if ($env:OS -ne "Windows_NT") {
    throw "This script is intended for Windows."
}

$repoRoot = $PSScriptRoot
Set-Location $repoRoot

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$target = switch ($architecture) {
    "X64" { "x86_64-pc-windows-msvc" }
    "Arm64" { "aarch64-pc-windows-msvc" }
    default { throw "Unsupported Windows architecture: $architecture" }
}

$pythonCommand = Get-Command python -ErrorAction SilentlyContinue
if ($null -eq $pythonCommand) {
    $pythonCommand = Get-Command python3 -ErrorAction SilentlyContinue
}
if ($null -eq $pythonCommand) {
    throw "Python 3 is required to build the Codex package."
}

$python = $pythonCommand.Source
$versionScript = 'import tomllib; print(tomllib.load(open("codex-rs/Cargo.toml", "rb"))["workspace"]["package"]["version"])'
$version = (& $python -c $versionScript).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($version)) {
    throw "Could not read the Codex workspace version."
}

$packageDir = Join-Path $repoRoot "dist\codex-package-$target"
$cargoHome = if ([string]::IsNullOrWhiteSpace($env:CARGO_HOME)) {
    Join-Path $env:USERPROFILE ".cargo"
} else {
    $env:CARGO_HOME
}
$installDir = Join-Path $cargoHome "bin"

Write-Step "Building Codex $version for $target"
Invoke-Checked -FilePath $python -Arguments @(
    "scripts/build_codex_package.py",
    "--target", $target,
    "--cargo-profile", "release",
    "--package-dir", $packageDir,
    "--force"
)

Write-Step "Installing Windows binaries in $installDir"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$binaries = [ordered]@{
    "codex-resources\codex-command-runner.exe" = "codex-command-runner.exe"
    "codex-resources\codex-windows-sandbox-setup.exe" = "codex-windows-sandbox-setup.exe"
    "bin\codex-code-mode-host.exe" = "codex-code-mode-host.exe"
    "bin\codex.exe" = "codex.exe"
}

$codexPath = Join-Path $installDir "codex.exe"
Install-Binaries `
    -Binaries $binaries `
    -PackageDir $packageDir `
    -InstallDir $installDir `
    -Verify {
        Write-Step "Verifying installation"
        $reportedVersionLines = @(& $codexPath --version)
        $exitCode = $LASTEXITCODE
        $reportedVersion = ($reportedVersionLines -join [Environment]::NewLine).Trim()
        if ($exitCode -ne 0) {
            throw "$codexPath exited with code $exitCode."
        }
        if ($reportedVersion -notlike "*$version*") {
            throw "Installed Codex reported '$reportedVersion'; expected version $version."
        }
        Write-Host $reportedVersion
    }

Write-Host "==> Installed Codex $version"
Write-Host "==> Installed at $codexPath"
