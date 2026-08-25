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

function Assert-ExecutablesReplaceable {
    param(
        [string]$InstallDir,
        [System.Collections.IDictionary]$Executables
    )

    $replaceabilityChecks = [System.Collections.Generic.List[System.IO.FileStream]]::new()
    try {
        foreach ($destinationRelativePath in $Executables.Values) {
            $installedPath = Join-Path $InstallDir $destinationRelativePath
            try {
                $replaceabilityChecks.Add(
                    [System.IO.File]::Open(
                        $installedPath,
                        [System.IO.FileMode]::Open,
                        [System.IO.FileAccess]::ReadWrite,
                        [System.IO.FileShare]::None
                    )
                )
            } catch {
                throw "Cannot replace $installedPath. Close any process using it and try again. $($_.Exception.Message)"
            }
        }
    } finally {
        foreach ($replaceabilityCheck in $replaceabilityChecks) {
            $replaceabilityCheck.Dispose()
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
            RustyV8ArchiveSha256 = "732ec5da4243aa166799780c8519a5eea6f32f6e47657a323342794dc3c239d6"
        }
    }
    "Arm64" {
        [pscustomobject]@{
            Target = "aarch64-pc-windows-msvc"
            PlatformPackage = "codex-win32-arm64"
            RustyV8ArchiveSha256 = "54722842af36b74248c403ff531254efac6ff65d281198bab0c6350fc1188ad4"
        }
    }
    default { throw "Unsupported Windows architecture: $architecture" }
}
$target = $targetInfo.Target
$rustyV8ReleaseUrl = "https://github.com/openai/codex/releases/download/rusty-v8-v150.4.0"
$rustyV8ArchivePath = Join-Path `
    $repoRoot `
    "rusty_v8_ptrcomp_sandbox_release_$target.lib.gz"
$rustyV8BindingPath = Join-Path `
    $repoRoot `
    "src_binding_ptrcomp_sandbox_release_$target.rs"
$rustyV8Artifacts = [ordered]@{
    $rustyV8ArchivePath = $targetInfo.RustyV8ArchiveSha256
    $rustyV8BindingPath = "dabf78ba1faac127660db9862b1d0354175c71b8db2d4fcb5bacbd9c93576b16"
}
foreach ($artifactPath in $rustyV8Artifacts.Keys) {
    $artifactName = Split-Path -Leaf $artifactPath
    if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
        throw "Missing $artifactPath. Download it from $rustyV8ReleaseUrl/$artifactName."
    }
    $actualSha256 = Get-FileSha256 -Path $artifactPath
    $expectedSha256 = $rustyV8Artifacts[$artifactPath]
    if ($actualSha256 -ine $expectedSha256) {
        throw "$artifactPath has SHA-256 $actualSha256; expected $expectedSha256."
    }
}

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

foreach ($destinationRelativePath in $executables.Values) {
    $installedPath = Join-Path $installDir $destinationRelativePath
    if (-not (Test-Path -LiteralPath $installedPath -PathType Leaf)) {
        throw "The npm Codex package is missing $installedPath."
    }
}

Write-Step "Building Codex release executables"
$previousRustyV8Archive = [Environment]::GetEnvironmentVariable(
    "RUSTY_V8_ARCHIVE",
    "Process"
)
$previousRustyV8Binding = [Environment]::GetEnvironmentVariable(
    "RUSTY_V8_SRC_BINDING_PATH",
    "Process"
)
try {
    [Environment]::SetEnvironmentVariable(
        "RUSTY_V8_ARCHIVE",
        $rustyV8ArchivePath,
        "Process"
    )
    [Environment]::SetEnvironmentVariable(
        "RUSTY_V8_SRC_BINDING_PATH",
        $rustyV8BindingPath,
        "Process"
    )
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
} finally {
    [Environment]::SetEnvironmentVariable(
        "RUSTY_V8_ARCHIVE",
        $previousRustyV8Archive,
        "Process"
    )
    [Environment]::SetEnvironmentVariable(
        "RUSTY_V8_SRC_BINDING_PATH",
        $previousRustyV8Binding,
        "Process"
    )
}

foreach ($sourceRelativePath in $executables.Keys) {
    $builtPath = Join-Path $releaseDir $sourceRelativePath
    if (-not (Test-Path -LiteralPath $builtPath -PathType Leaf)) {
        throw "Cargo did not build $builtPath."
    }
}
$builtCodexPath = Join-Path $releaseDir "codex.exe"
$version = Get-CodexVersion -CodexPath $builtCodexPath

$installSuffix = "$PID.$([Guid]::NewGuid().ToString('N'))"
$installParent = Split-Path -Parent $installDir
$stagingDir = Join-Path $installParent ".$target.installing.$installSuffix"
Assert-PathWithin -Path $stagingDir -Root $installParent

if (Test-Path -LiteralPath $stagingDir) {
    throw "Staging path already exists: $stagingDir."
}

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

    Assert-ExecutablesReplaceable -InstallDir $installDir -Executables $executables

    Write-Step "Replacing npm package at $installDir"
    foreach ($sourceRelativePath in $executables.Keys) {
        Move-Item `
            -LiteralPath (Join-Path $stagingDir $executables[$sourceRelativePath]) `
            -Destination (Join-Path $installDir $executables[$sourceRelativePath]) `
            -Force
    }

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
} finally {
    if (Test-Path -LiteralPath $stagingDir) {
        Assert-PathWithin -Path $stagingDir -Root $installParent
        Remove-Item -LiteralPath $stagingDir -Recurse -Force
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
