param(
    [Parameter(Mandatory = $true)] [string]$Core,
    [Parameter(Mandatory = $true)] [string]$Config,
    [Parameter(Mandatory = $true)] [string]$State,
    [Parameter(Mandatory = $true)] [string]$ProtectedSecret,
    [string]$SecretPath
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Security
$Core = (Resolve-Path -LiteralPath $Core -ErrorAction Stop).Path
$Config = (Resolve-Path -LiteralPath $Config -ErrorAction Stop).Path
$ProtectedSecret = (Resolve-Path -LiteralPath $ProtectedSecret -ErrorAction Stop).Path
$State = [IO.Path]::GetFullPath($State)
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name

if (-not $SecretPath) {
    $runDirectory = Join-Path $env:LOCALAPPDATA "PeerVault\run"
    if (-not (Test-Path -LiteralPath $runDirectory -PathType Container)) {
        New-Item -ItemType Directory -Force -Path $runDirectory | Out-Null
    }
    # The runtime directory is dedicated to one owner; remove inherited access.
    & icacls $runDirectory /inheritance:r /grant:r "$($identity):(OI)(CI)F" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "icacls failed for the runtime secret directory" }
    $SecretPath = Join-Path $runDirectory ("secret-" + [Guid]::NewGuid().ToString("N") + ".txt")
} else {
    $SecretPath = [IO.Path]::GetFullPath($SecretPath)
    $directory = Split-Path -Parent $SecretPath
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
        throw "SecretPath directory must exist: $directory"
    }
}
if (Test-Path -LiteralPath $SecretPath) {
    throw "SecretPath already exists; refusing to overwrite a secret file"
}

$plain = [Security.Cryptography.ProtectedData]::Unprotect(
    [IO.File]::ReadAllBytes($ProtectedSecret), $null,
    [Security.Cryptography.DataProtectionScope]::CurrentUser)
$code = 1
try {
    if ($plain.Length -lt 1 -or $plain.Length -gt 8192) {
        throw "decrypted secret is outside the supported size"
    }
    $security = New-Object System.Security.AccessControl.FileSecurity
    $security.SetAccessRuleProtection($true, $false)
    $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
        $identity, "FullControl", "Allow")
    $security.AddAccessRule($rule)
    $stream = New-Object System.IO.FileStream(
        $SecretPath,
        [IO.FileMode]::CreateNew,
        [System.Security.AccessControl.FileSystemRights]::FullControl,
        [IO.FileShare]::Read,
        4096,
        [IO.FileOptions]::None,
        $security)
    try {
        $stream.Write($plain, 0, $plain.Length)
        $stream.Flush($true)
    } finally {
        $stream.Close()
    }
    $env:PEERVAULT_PASSWORD_FILE = $SecretPath
    & $Core schedule-tick --config $Config --state $State
    $code = $LASTEXITCODE
} finally {
    Remove-Item -LiteralPath $SecretPath -Force -ErrorAction SilentlyContinue
    Remove-Item Env:PEERVAULT_PASSWORD_FILE -ErrorAction SilentlyContinue
    [Array]::Clear($plain, 0, $plain.Length)
}
exit $code
