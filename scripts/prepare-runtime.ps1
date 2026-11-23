# Write the bundled seed dsh runtime into resources/runtime.
#
# A local dist-exe build wins; otherwise the latest published public PyPI
# win_amd64 runtime wheel is downloaded and verified by its SHA-256 digest.
# Set DSH_DESKTOP_RUNTIME_VERSION to pin a published version.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$appDirectory = Split-Path -Parent $PSScriptRoot
$parentDirectory = Split-Path -Parent $appDirectory
# This application is either apps/desktop-tauri in the harness repository or
# the repository root of a standalone checkout.
$repositoryRoot = if ((Split-Path -Leaf $appDirectory) -eq 'desktop-tauri' -and (Split-Path -Leaf $parentDirectory) -eq 'apps') {
    Split-Path -Parent $parentDirectory
}
else {
    $appDirectory
}
$destination = Join-Path $appDirectory 'resources\runtime'
$executableName = 'deepseek-harness-sdk-runtime-win-x64.exe'
$sidecarName = 'deepseek-harness-sdk-runtime-win-x64-rg.exe'

function Copy-Runtime([string]$sourceDirectory) {
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item (Join-Path $sourceDirectory $executableName) $destination -Force
    Copy-Item (Join-Path $sourceDirectory $sidecarName) $destination -Force
    Remove-Item (Join-Path $destination '.placeholder') -Force -ErrorAction SilentlyContinue
}

$localRuntime = Join-Path $repositoryRoot "dist-exe\$executableName"
if (Test-Path $localRuntime) {
    $localSidecar = Join-Path $repositoryRoot "dist-exe\$sidecarName"
    if (-not (Test-Path $localSidecar)) {
        throw "Local runtime sidecar is missing: $localSidecar"
    }
    Copy-Runtime (Join-Path $repositoryRoot 'dist-exe')
    $source = 'dist-exe'
}
else {
    $package = 'deepseek-harness-runtime-bin'
    $pinned = $env:DSH_DESKTOP_RUNTIME_VERSION
    $metadataUri = if ($pinned) {
        "https://pypi.org/pypi/$package/$pinned/json"
    }
    else {
        "https://pypi.org/pypi/$package/json"
    }
    $metadata = Invoke-RestMethod -Uri $metadataUri
    $release = $metadata.info.version
    $files = if ($pinned) { $metadata.urls } else { $metadata.releases.$release }
    $wheel = $files | Where-Object { $_.filename -like '*win_amd64.whl' } | Select-Object -First 1
    if (-not $wheel) {
        throw "PyPI release $release of $package has no win_amd64 wheel."
    }

    $temporary = Join-Path $env:TEMP ("dsh-desktop-runtime-" + [guid]::NewGuid())
    try {
        New-Item -ItemType Directory -Force -Path $temporary | Out-Null
        $wheelPath = Join-Path $temporary $wheel.filename
        Invoke-WebRequest -Uri $wheel.url -OutFile $wheelPath
        $actual = (Get-FileHash -Algorithm SHA256 $wheelPath).Hash.ToLowerInvariant()
        if ($actual -ne $wheel.digests.sha256) {
            throw "Runtime wheel checksum mismatch for $($wheel.url): expected $($wheel.digests.sha256), got $actual."
        }
        $zipPath = Join-Path $temporary 'runtime.zip'
        Copy-Item $wheelPath $zipPath
        Expand-Archive -Path $zipPath -DestinationPath (Join-Path $temporary 'wheel')
        $runtimeDirectory = Join-Path $temporary 'wheel\deepseek_harness_runtime\runtime'
        Copy-Runtime $runtimeDirectory
        $source = "PyPI $package $release"
    }
    finally {
        Remove-Item -Recurse -Force $temporary -ErrorAction SilentlyContinue
    }
}

$seed = Join-Path $destination $executableName
if (-not (Test-Path (Join-Path $destination $sidecarName))) {
    throw "Runtime sidecar is missing beside $seed."
}
$version = & $seed --version
if ($LASTEXITCODE -ne 0) {
    throw "$seed --version failed with exit code $LASTEXITCODE."
}
$size = [math]::Round((Get-Item $seed).Length / 1MB, 1)
Write-Host "prepare-runtime: $source -> $destination ($size MB, version $version)"
