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

function Install-PackageFiles {
    param(
        [System.Collections.IDictionary]$Files,
        [string]$PackageDir,
        [string]$InstallDir,
        [scriptblock]$Verify
    )

    $stagingSuffix = "$PID.$([System.Guid]::NewGuid().ToString("N"))"
    $operations = [System.Collections.Generic.List[object]]::new()
    foreach ($relativeSource in $Files.Keys) {
        $source = Join-Path $PackageDir $relativeSource
        $destination = Join-Path $InstallDir $Files[$relativeSource]
        $stagedPath = "$destination.installing.$stagingSuffix"

        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Built package is missing $source."
        }
        if (Test-Path -LiteralPath $destination -PathType Container) {
            throw "Install destination is a directory: $destination."
        }
        if (-not (Test-Path -LiteralPath $destination -PathType Leaf)) {
            throw "Installed package is missing $destination."
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
                throw "Staged file does not match the build output: $($operation.Source)."
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
                throw "Installed file does not match the build output: $($operation.Destination)."
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
$targetInfo = switch ($architecture) {
    "X64" {
        [pscustomobject]@{
            Target = "x86_64-pc-windows-msvc"
            PlatformPackage = "codex-win32-x64"
        }
    }
    "Arm64" {
        [pscustomobject]@{
            Target = "aarch64-pc-windows-msvc"
            PlatformPackage = "codex-win32-arm64"
        }
    }
    default { throw "Unsupported Windows architecture: $architecture" }
}
$target = $targetInfo.Target

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
$npmCommand = Get-Command npm.cmd -ErrorAction SilentlyContinue
if ($null -eq $npmCommand) {
    throw "npm is required to locate the installed Codex executables."
}

$npmRootLines = @(& $npmCommand.Source root --global)
$npmRoot = ($npmRootLines -join [Environment]::NewLine).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($npmRoot)) {
    throw "Could not locate the global npm package directory."
}

$codexPackageDir = Join-Path $npmRoot "@openai\codex"
$nestedPlatformPackageDir = Join-Path $codexPackageDir "node_modules\@openai\$($targetInfo.PlatformPackage)"
$hoistedPlatformPackageDir = Join-Path $npmRoot "@openai\$($targetInfo.PlatformPackage)"
if (Test-Path -LiteralPath $nestedPlatformPackageDir -PathType Container) {
    $platformPackageDir = $nestedPlatformPackageDir
} elseif (Test-Path -LiteralPath $hoistedPlatformPackageDir -PathType Container) {
    $platformPackageDir = $hoistedPlatformPackageDir
} else {
    throw "Could not find the installed @openai/$($targetInfo.PlatformPackage) package."
}
$installDir = Join-Path $platformPackageDir "vendor\$target"

Write-Step "Building Codex $version for $target"
Invoke-Checked -FilePath $python -Arguments @(
    "scripts/build_codex_package.py",
    "--target", $target,
    "--cargo-profile", "release",
    "--package-dir", $packageDir,
    "--force"
)

Write-Step "Replacing installed Codex executables in $installDir"
$packageFiles = [ordered]@{
    "bin\codex.exe" = "bin\codex.exe"
    "bin\codex-code-mode-host.exe" = "bin\codex-code-mode-host.exe"
    "codex-resources\codex-command-runner.exe" = "codex-resources\codex-command-runner.exe"
    "codex-resources\codex-windows-sandbox-setup.exe" = "codex-resources\codex-windows-sandbox-setup.exe"
    "codex-path\rg.exe" = "codex-path\rg.exe"
    "codex-package.json" = "codex-package.json"
}

$codexPath = Join-Path $installDir "bin\codex.exe"
Install-PackageFiles `
    -Files $packageFiles `
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
