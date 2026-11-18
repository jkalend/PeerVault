param(
    [Parameter(Mandatory = $true)] [string]$Core,
    [Parameter(Mandatory = $true)] [string]$Config,
    [Parameter(Mandatory = $true)] [string]$State,
    [Parameter(Mandatory = $true)] [string]$ProtectedSecret,
    [Parameter(Mandatory = $true)] [string]$SecretPath
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Security
$Core = (Resolve-Path -LiteralPath $Core -ErrorAction Stop).Path
$Config = (Resolve-Path -LiteralPath $Config -ErrorAction Stop).Path
$ProtectedSecret = (Resolve-Path -LiteralPath $ProtectedSecret -ErrorAction Stop).Path
$State = [IO.Path]::GetFullPath($State)
$SecretPath = [IO.Path]::GetFullPath($SecretPath)
$directory = Split-Path -Parent $SecretPath
if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
    throw "SecretPath directory must exist: $directory"
}
if (Test-Path -LiteralPath $SecretPath) {
    throw "SecretPath already exists; refusing to overwrite a secret file"
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$plain = [Security.Cryptography.ProtectedData]::Unprotect(
    [IO.File]::ReadAllBytes($ProtectedSecret), $null,
    [Security.Cryptography.DataProtectionScope]::CurrentUser)
$code = 1
try {
    if ($plain.Length -lt 1 -or $plain.Length -gt 8192) {
        throw "decrypted secret is outside the supported size"
    }
    [IO.File]::WriteAllBytes($SecretPath, $plain)
    & icacls $SecretPath /inheritance:r /grant:r "$($identity):F" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "icacls failed for the runtime secret" }
    $env:PEERVAULT_PASSWORD_FILE = $SecretPath
    & $Core schedule-tick --config $Config --state $State
    $code = $LASTEXITCODE
} finally {
    Remove-Item -LiteralPath $SecretPath -Force -ErrorAction SilentlyContinue
    Remove-Item Env:PEERVAULT_PASSWORD_FILE -ErrorAction SilentlyContinue
    [Array]::Clear($plain, 0, $plain.Length)
}
exit $code
