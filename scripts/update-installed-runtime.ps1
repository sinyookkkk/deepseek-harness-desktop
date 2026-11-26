# Replace an installed app's seed runtime and, when it carries the same
# version, its recorded runtime with a rebuilt dsh executable.
#
# The shell prefers the runtime recorded in current.json, so replacing only
# resources/runtime does not update an existing installation. Close the app
# before running this script; Windows locks the recorded executable while it
# runs.

$ErrorActionPreference = 'Stop'

$appDirectory = Split-Path -Parent $PSScriptRoot
$sourceDirectory = Join-Path $appDirectory 'dist-exe'
$executableName = 'deepseek-harness-sdk-runtime-win-x64.exe'
$sidecarName = 'deepseek-harness-sdk-runtime-win-x64-rg.exe'
$installDirectory = Join-Path $env:LOCALAPPDATA 'DeepSeek Harness'
$stateDirectory = Join-Path $env:LOCALAPPDATA 'ai.deepseek.harness.desktop-tauri'

if ($args.Count -gt 0) {
    throw "update-installed-runtime: unexpected arguments: $($args -join ' ')"
}

$sourceExe = Join-Path $sourceDirectory $executableName
$sourceSidecar = Join-Path $sourceDirectory $sidecarName
if (-not (Test-Path -LiteralPath $sourceExe -PathType Leaf)) {
    throw "Rebuilt runtime is missing at $sourceExe. Run scripts/prepare-runtime.ps1 or copy a dist-exe build first."
}
if (-not (Test-Path -LiteralPath $sourceSidecar -PathType Leaf)) {
    throw "Rebuilt runtime sidecar is missing at $sourceSidecar."
}
if (-not (Test-Path -LiteralPath $installDirectory -PathType Container)) {
    throw "Installed app is missing at $installDirectory; pass a portable install directory by editing this script."
}

function Assert-NoProcessUnder([string]$root) {
    $running = Get-CimInstance Win32_Process | Where-Object {
        $_.ExecutablePath -and $_.ExecutablePath.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase)
    }
    if ($running) {
        $ids = ($running | ForEach-Object ProcessId) -join ', '
        throw "Close the desktop app first; these processes are still running under ${root}: $ids"
    }
}

Assert-NoProcessUnder $installDirectory
Assert-NoProcessUnder $stateDirectory

$version = (& $sourceExe --version | Select-Object -First 1).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "$sourceExe --version failed with exit code $LASTEXITCODE."
}
if ($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
    throw "$sourceExe --version reported an invalid version: $version"
}

$seedDirectory = Join-Path $installDirectory 'resources\runtime'
New-Item -ItemType Directory -Force -Path $seedDirectory | Out-Null
Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $seedDirectory $executableName) -Force
Copy-Item -LiteralPath $sourceSidecar -Destination (Join-Path $seedDirectory $sidecarName) -Force

$recorded = $false
$pointerFile = Join-Path $stateDirectory 'runtimes\current.json'
if (Test-Path -LiteralPath $pointerFile -PathType Leaf) {
    $pointer = Get-Content -LiteralPath $pointerFile -Raw | ConvertFrom-Json
    if ($pointer.version -ne $version) {
        Write-Host "update-installed-runtime: seed updated to $version; the recorded runtime is $($pointer.version) and was left unchanged."
    }
    else {
        $runtimeDirectory = Join-Path $stateDirectory "runtimes\$version"
        if (Test-Path -LiteralPath $runtimeDirectory -PathType Container) {
            Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $runtimeDirectory 'dsh.exe') -Force
            Copy-Item -LiteralPath $sourceSidecar -Destination (Join-Path $runtimeDirectory 'dsh-rg.exe') -Force
            $recorded = $true
        }
    }
}

# Prove both payload copies match the source before reporting success.
$copies = @(
    @{ Expected = $sourceExe; Actual = (Join-Path $seedDirectory $executableName) },
    @{ Expected = $sourceSidecar; Actual = (Join-Path $seedDirectory $sidecarName) }
)
if ($recorded) {
    $copies += @{ Expected = $sourceExe; Actual = (Join-Path (Join-Path $stateDirectory "runtimes\$version") 'dsh.exe') }
    $copies += @{ Expected = $sourceSidecar; Actual = (Join-Path (Join-Path $stateDirectory "runtimes\$version") 'dsh-rg.exe') }
}
foreach ($copy in $copies) {
    $expected = (Get-FileHash -LiteralPath $copy.Expected -Algorithm SHA256).Hash
    $actual = (Get-FileHash -LiteralPath $copy.Actual -Algorithm SHA256).Hash
    if ($expected -ne $actual) {
        throw "Copied runtime does not match $($copy.Expected): $($copy.Actual)"
    }
}

$where = if ($recorded) { 'the seed and recorded runtime' } else { 'the seed runtime' }
Write-Host "update-installed-runtime: updated $where to $version; reopen DeepSeek Harness."
