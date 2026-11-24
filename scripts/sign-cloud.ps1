# Sign one executable with Azure Trusted Signing, or leave it unsigned when the
# signing inputs are absent. Tauri invokes this through
# bundle.windows.signCommand for the application binary and the NSIS installer.
# Non-.exe files (the NSIS plugin DLLs Tauri also passes) are skipped.
#
# Signing inputs (all required together):
#   AZURE_TENANT_ID
#   AZURE_CLIENT_ID
#   AZURE_CLIENT_SECRET
#   AZURE_TRUSTED_SIGNING_ENDPOINT   e.g. https://weu.codesigning.azure.net
#   AZURE_TRUSTED_SIGNING_ACCOUNT
#   AZURE_TRUSTED_SIGNING_PROFILE
# Optional:
#   AZURE_TRUSTED_SIGNING_DESCRIPTION (default: DeepSeek Harness)
#   DSH_DESKTOP_REQUIRE_SIGNING=1 turns missing inputs into a build failure.

param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Path
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "sign-cloud: file not found: $Path"
}
if ([IO.Path]::GetExtension($Path).ToLowerInvariant() -ne '.exe') {
    Write-Host "sign-cloud: skipping non-executable $Path"
    exit 0
}

$required = $env:DSH_DESKTOP_REQUIRE_SIGNING -eq '1'
$names = @(
    'AZURE_TENANT_ID',
    'AZURE_CLIENT_ID',
    'AZURE_CLIENT_SECRET',
    'AZURE_TRUSTED_SIGNING_ENDPOINT',
    'AZURE_TRUSTED_SIGNING_ACCOUNT',
    'AZURE_TRUSTED_SIGNING_PROFILE'
)
$missing = @($names | Where-Object { [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($_)) })
if ($missing.Count -gt 0) {
    $message = "Cloud signing is not configured; missing: $($missing -join ', ')."
    if ($required) { throw $message }
    Write-Warning "$message Leaving $(Split-Path -Leaf $Path) unsigned."
    exit 0
}

$cli = Get-Command trusted-signing-cli -ErrorAction SilentlyContinue
if ($null -eq $cli) {
    $message = 'trusted-signing-cli is not installed.'
    if ($required) { throw $message }
    Write-Warning "$message Leaving $(Split-Path -Leaf $Path) unsigned."
    exit 0
}

$description = $env:AZURE_TRUSTED_SIGNING_DESCRIPTION
if ([string]::IsNullOrWhiteSpace($description)) { $description = 'DeepSeek Harness' }

& $cli.Source `
    -e $env:AZURE_TRUSTED_SIGNING_ENDPOINT `
    -a $env:AZURE_TRUSTED_SIGNING_ACCOUNT `
    -c $env:AZURE_TRUSTED_SIGNING_PROFILE `
    -d $description `
    $Path
if ($LASTEXITCODE -ne 0) {
    throw "trusted-signing-cli failed for $Path with exit code $LASTEXITCODE."
}
