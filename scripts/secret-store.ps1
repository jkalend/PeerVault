param(
    [Parameter(Mandatory = $true)] [ValidateSet("Store", "Remove")] [string]$Action,
    [string]$SecretFile,
    [Parameter(Mandatory = $true)] [string]$ProtectedFile
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Security
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name

if ($Action -eq "Remove") {
    Remove-Item -LiteralPath $ProtectedFile -Force -ErrorAction SilentlyContinue
    Write-Output "Removed DPAPI-protected secret $ProtectedFile"
    exit 0
}

if (-not $SecretFile -or -not (Test-Path -LiteralPath $SecretFile -PathType Leaf)) {
    throw "SecretFile must be an existing owner-only secret file"
}
$bytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $SecretFile).Path)
try {
    if ($bytes.Length -lt 1 -or $bytes.Length -gt 8192) {
        throw "secret must be 1..=8192 bytes"
    }
    $protected = [Security.Cryptography.ProtectedData]::Protect(
        $bytes, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    $ProtectedFile = [IO.Path]::GetFullPath($ProtectedFile)
    [IO.File]::WriteAllBytes($ProtectedFile, $protected)
    & icacls $ProtectedFile /inheritance:r /grant:r "$($identity):F" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "icacls failed for the protected secret" }
    Write-Output "Stored DPAPI-protected secret for $identity at $ProtectedFile"
} finally {
    [Array]::Clear($bytes, 0, $bytes.Length)
}
