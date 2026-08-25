[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion -ge [Version]"7.3") {
    $PSNativeCommandUseErrorActionPreference = $true
}

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
    param([string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Assert-PathWithin {
    param(
        [string]$Path,
        [string]$Root
    )

    $resolvedPath = [IO.Path]::GetFullPath($Path).TrimEnd("\")
    $resolvedRoot = [IO.Path]::GetFullPath($Root).TrimEnd("\")
    $isWithin = $resolvedPath.Equals(
        $resolvedRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    ) -or $resolvedPath.StartsWith(
        "$resolvedRoot\",
        [System.StringComparison]::OrdinalIgnoreCase
    )
    if (-not $isWithin) {
        throw "Refusing to modify $resolvedPath because it is outside $resolvedRoot."
    }
}

function Assert-ExecutablesMatch {
    param(
        [string]$SourceDir,
        [string]$DestinationDir,
        [System.Collections.IDictionary]$Executables
    )

    foreach ($sourceRelativePath in $Executables.Keys) {
        $sourcePath = Join-Path $SourceDir $sourceRelativePath
        $destinationPath = Join-Path $DestinationDir $Executables[$sourceRelativePath]
        if (-not (Test-Path -LiteralPath $destinationPath -PathType Leaf)) {
            throw "$DestinationDir is missing $($Executables[$sourceRelativePath])."
        }
        if ((Get-FileSha256 $sourcePath) -ne (Get-FileSha256 $destinationPath)) {
            throw "$destinationPath does not match $sourcePath."
        }
    }
}

function Get-CodexVersion {
    param([string]$CodexPath)

    $reportedVersionLines = @(& $CodexPath --version)
    $exitCode = $LASTEXITCODE
    $reportedVersion = ($reportedVersionLines -join [Environment]::NewLine).Trim()
    if ($exitCode -ne 0) {
        throw "$CodexPath exited with code $exitCode."
    }

    return $reportedVersion
}

function Prepend-PathEntry {
    param(
        [AllowNull()]
        [string]$PathValue,
        [string]$Entry
    )

    $needle = $Entry.TrimEnd("\")
    $segments = @($Entry)
    if (-not [string]::IsNullOrWhiteSpace($PathValue)) {
        $segments += $PathValue.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries) |
            Where-Object { $_.TrimEnd("\") -ine $needle }
    }
    return $segments -join ";"
}

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) {
    throw "This script is intended for Windows."
}

$repoRoot = $PSScriptRoot
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

$cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($null -eq $cargoCommand) {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
}
if ($null -eq $cargoCommand) {
    throw "cargo is required to build the Codex release executables."
}

$npmCommand = Get-Command npm.cmd -ErrorAction SilentlyContinue
if ($null -eq $npmCommand) {
    throw "npm is required to locate the global Codex installation."
}

$npmRootLines = @(& $npmCommand.Source root --global)
$npmRoot = ($npmRootLines -join [Environment]::NewLine).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($npmRoot)) {
    throw "Could not locate the global npm package directory."
}
$npmPrefixLines = @(& $npmCommand.Source prefix --global)
$npmPrefix = ($npmPrefixLines -join [Environment]::NewLine).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($npmPrefix)) {
    throw "Could not locate the global npm command directory."
}

$codexPackageDir = Join-Path $npmRoot "@openai\codex"
$codexPackageMetadataPath = Join-Path $codexPackageDir "package.json"
if (-not (Test-Path -LiteralPath $codexPackageMetadataPath -PathType Leaf)) {
    throw "Could not find the global @openai/codex npm package. Run npm install -g @openai/codex first."
}
$codexPackageMetadata = Get-Content -LiteralPath $codexPackageMetadataPath -Raw | ConvertFrom-Json
if ($codexPackageMetadata.name -cne "@openai/codex") {
    throw "$codexPackageMetadataPath does not describe the @openai/codex package."
}

$nestedPlatformPackageDir = Join-Path $codexPackageDir "node_modules\@openai\$($targetInfo.PlatformPackage)"
$hoistedPlatformPackageDir = Join-Path $npmRoot "@openai\$($targetInfo.PlatformPackage)"
if (Test-Path -LiteralPath $nestedPlatformPackageDir -PathType Container) {
    $platformPackageDir = $nestedPlatformPackageDir
} elseif (Test-Path -LiteralPath $hoistedPlatformPackageDir -PathType Container) {
    $platformPackageDir = $hoistedPlatformPackageDir
} else {
    throw "Could not find the installed @openai/$($targetInfo.PlatformPackage) npm package."
}

$installDir = Join-Path $platformPackageDir "vendor\$target"
if (-not (Test-Path -LiteralPath $installDir -PathType Container)) {
    throw "Could not find the npm Codex package location at $installDir."
}
$codexShim = Join-Path $npmPrefix "codex.cmd"
if (-not (Test-Path -LiteralPath $codexShim -PathType Leaf)) {
    throw "Could not find the npm Codex command shim at $codexShim."
}

Assert-PathWithin -Path $codexPackageDir -Root $npmRoot
Assert-PathWithin -Path $platformPackageDir -Root $npmRoot
Assert-PathWithin -Path $installDir -Root $platformPackageDir

$codexRsRoot = Join-Path $repoRoot "codex-rs"
$targetDir = Join-Path $codexRsRoot "target"
$releaseDir = Join-Path $targetDir "release"
$executables = [ordered]@{
    "codex.exe" = "bin\codex.exe"
    "codex-code-mode-host.exe" = "bin\codex-code-mode-host.exe"
    "codex-command-runner.exe" = "codex-resources\codex-command-runner.exe"
    "codex-windows-sandbox-setup.exe" = "codex-resources\codex-windows-sandbox-setup.exe"
}
Assert-PathWithin -Path $releaseDir -Root $targetDir

Write-Step "Building Codex release executables"
Push-Location $codexRsRoot
try {
    Invoke-Checked -FilePath $cargoCommand.Source -Arguments @(
        "build",
        "--release",
        "--target-dir", $targetDir,
        "--bin", "codex",
        "--bin", "codex-code-mode-host",
        "--bin", "codex-command-runner",
        "--bin", "codex-windows-sandbox-setup"
    )
} finally {
    Pop-Location
}

foreach ($sourceRelativePath in $executables.Keys) {
    $builtPath = Join-Path $releaseDir $sourceRelativePath
    if (-not (Test-Path -LiteralPath $builtPath -PathType Leaf)) {
        throw "Cargo did not build $builtPath."
    }

    $installedPath = Join-Path $installDir $executables[$sourceRelativePath]
    if (-not (Test-Path -LiteralPath $installedPath -PathType Leaf)) {
        throw "The npm Codex package is missing $installedPath."
    }
}
$builtCodexPath = Join-Path $releaseDir "codex.exe"
$version = Get-CodexVersion -CodexPath $builtCodexPath

$installSuffix = "$PID.$([Guid]::NewGuid().ToString('N'))"
$installParent = Split-Path -Parent $installDir
$stagingDir = Join-Path $installParent ".$target.installing.$installSuffix"
$backupDir = Join-Path $installParent ".$target.backup.$installSuffix"
Assert-PathWithin -Path $stagingDir -Root $installParent
Assert-PathWithin -Path $backupDir -Root $installParent

if (Test-Path -LiteralPath $stagingDir) {
    throw "Staging path already exists: $stagingDir."
}
if (Test-Path -LiteralPath $backupDir) {
    throw "Backup path already exists: $backupDir."
}

$backupCreated = $false
$replacementInstalled = $false
try {
    Write-Step "Staging Codex $version beside the npm package"
    Copy-Item -LiteralPath $installDir -Destination $stagingDir -Recurse
    foreach ($sourceRelativePath in $executables.Keys) {
        Copy-Item `
            -LiteralPath (Join-Path $releaseDir $sourceRelativePath) `
            -Destination (Join-Path $stagingDir $executables[$sourceRelativePath]) `
            -Force
    }
    Assert-ExecutablesMatch `
        -SourceDir $releaseDir `
        -DestinationDir $stagingDir `
        -Executables $executables
    $stagedCodexPath = Join-Path $stagingDir "bin\codex.exe"
    $stagedVersion = Get-CodexVersion -CodexPath $stagedCodexPath
    if ($stagedVersion -cne $version) {
        throw "$stagedCodexPath reported '$stagedVersion'; expected '$version'."
    }

    Write-Step "Replacing npm package at $installDir"
    Move-Item -LiteralPath $installDir -Destination $backupDir
    $backupCreated = $true
    Move-Item -LiteralPath $stagingDir -Destination $installDir
    $replacementInstalled = $true

    Assert-ExecutablesMatch `
        -SourceDir $releaseDir `
        -DestinationDir $installDir `
        -Executables $executables
    $installedCodexPath = Join-Path $installDir "bin\codex.exe"
    $reportedVersion = Get-CodexVersion -CodexPath $installedCodexPath
    if ($reportedVersion -cne $version) {
        throw "$installedCodexPath reported '$reportedVersion'; expected '$version'."
    }
    $shimVersion = Get-CodexVersion -CodexPath $codexShim
    if ($shimVersion -cne $version) {
        throw "$codexShim reported '$shimVersion'; expected '$version'."
    }
} catch {
    $installFailure = $_
    if ($backupCreated) {
        try {
            if ($replacementInstalled -and (Test-Path -LiteralPath $installDir)) {
                Assert-PathWithin -Path $installDir -Root $platformPackageDir
                Remove-Item -LiteralPath $installDir -Recurse -Force
            }
            if (Test-Path -LiteralPath $backupDir) {
                Move-Item -LiteralPath $backupDir -Destination $installDir
                $backupCreated = $false
            }
        } catch {
            throw "Installing the local build failed, and restoring the npm package also failed. The backup remains at $backupDir. Original error: $installFailure"
        }
    }
    throw $installFailure
} finally {
    if (Test-Path -LiteralPath $stagingDir) {
        Assert-PathWithin -Path $stagingDir -Root $installParent
        Remove-Item -LiteralPath $stagingDir -Recurse -Force
    }
}

if ($backupCreated -and (Test-Path -LiteralPath $backupDir)) {
    Assert-PathWithin -Path $backupDir -Root $installParent
    try {
        Remove-Item -LiteralPath $backupDir -Recurse -Force
    } catch {
        Write-Warning "The local build is installed, and the previous npm package remains at $backupDir because it is still in use."
    }
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$updatedUserPath = Prepend-PathEntry -PathValue $userPath -Entry $npmPrefix
if ($updatedUserPath -cne $userPath) {
    [Environment]::SetEnvironmentVariable("Path", $updatedUserPath, "User")
}
$env:Path = Prepend-PathEntry -PathValue $env:Path -Entry $npmPrefix

Write-Host $reportedVersion
Write-Host "==> npm command: $codexShim"
Write-Host "==> Replaced npm package: $installDir"
