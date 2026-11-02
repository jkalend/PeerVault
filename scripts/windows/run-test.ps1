#requires -Version 5.1
<#
.SYNOPSIS
  Clean-Windows RecoveryKit verify/restore drill.

.DESCRIPTION
  Runs from the read-only PEERVAULT_TEST volume in a new Windows VM. It uses
  the PEERVAULT_SECRET volume for the repository password, verifies the
  RecoveryKit, restores into a destination that must not exist, and compares
  every expected file against the independent oracle.

  This is experimental synthetic-data qualification code. The serial result is
  the only result channel; the VM shuts down after writing it.
#>
$ErrorActionPreference = 'Stop'
$result = [ordered]@{
    verify_exit = $null
    verify = $null
    restore_exit = $null
    restore = $null
    expected_file_count = $null
    file_count = $null
    hash_mismatches = @()
    ok = $false
}

function Invoke-Captured {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    # A native command that writes stderr must not be treated as a terminating
    # PowerShell error before its exit code and output are recorded.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $FilePath @Arguments 2>&1
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previous
    }
    if ($null -eq $code) {
        $code = -1
    }
    return [pscustomobject]@{
        ExitCode = [int]$code
        Output = (($output | ForEach-Object { $_.ToString() }) -join "`n")
    }
}

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try {
            $bytes = $sha.ComputeHash($stream)
            return ([BitConverter]::ToString($bytes) -replace '-', '').ToLowerInvariant()
        }
        finally {
            $sha.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

try {
    $testVolume = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'PEERVAULT_TEST' } | Select-Object -First 1
    $secretVolume = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'PEERVAULT_SECRET' } | Select-Object -First 1
    if (-not $testVolume -or [string]::IsNullOrWhiteSpace($testVolume.DriveLetter)) {
        throw 'PEERVAULT_TEST volume missing or has no drive letter'
    }
    if (-not $secretVolume -or [string]::IsNullOrWhiteSpace($secretVolume.DriveLetter)) {
        throw 'PEERVAULT_SECRET volume missing or has no drive letter'
    }

    $testRoot = "$($testVolume.DriveLetter):"
    $secretRoot = "$($secretVolume.DriveLetter):"
    $expectedPath = Join-Path $testRoot 'expected.json'
    $expected = Get-Content -LiteralPath $expectedPath -Raw | ConvertFrom-Json
    $expectedEntries = @($expected.PSObject.Properties)
    $result.expected_file_count = $expectedEntries.Count
    if ($expectedEntries.Count -ne 4) {
        throw "expected exactly four fixture files, found $($expectedEntries.Count)"
    }

    $work = 'C:\peervault'
    $destination = 'C:\restored'
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    if (Test-Path -LiteralPath $destination) {
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    Copy-Item -LiteralPath (Join-Path $testRoot 'peervault.exe') -Destination $work -Force
    Copy-Item -LiteralPath (Join-Path $testRoot 'restic.exe') -Destination $work -Force

    $binary = Join-Path $work 'peervault.exe'
    $backend = Join-Path $work 'restic.exe'
    $password = Join-Path $secretRoot 'secret.txt'

    $verify = Invoke-Captured -FilePath $binary -Arguments @(
        'recovery-kit', 'verify',
        '--kit', (Join-Path $testRoot 'kit'),
        '--repository', (Join-Path $testRoot 'repository'),
        '--backend', $backend,
        '--password-file', $password
    )
    $result.verify_exit = $verify.ExitCode
    $result.verify = $verify.Output

    $restore = Invoke-Captured -FilePath $binary -Arguments @(
        'restore',
        '--repository', (Join-Path $testRoot 'repository'),
        '--backend', $backend,
        '--snapshot', 'a077e7233ee2a3cfce01abbdb5783a6eb9242ec99a39d0218943ad0407602323',
        '--destination', $destination,
        '--password-file', $password
    )
    $result.restore_exit = $restore.ExitCode
    $result.restore = $restore.Output

    $bad = @()
    if (Test-Path -LiteralPath $destination) {
        $restoredFiles = @(Get-ChildItem -LiteralPath $destination -Recurse -File)
        $result.file_count = $restoredFiles.Count
        foreach ($property in $expectedEntries) {
            $relative = [string]$property.Name
            if ([string]::IsNullOrWhiteSpace($relative) -or
                [IO.Path]::IsPathRooted($relative) -or
                $relative -match '(^|[\\/])\.\.([\\/]|$)') {
                $bad += "invalid path $relative"
                continue
            }
            $path = Join-Path -Path $destination -ChildPath $relative
            $item = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
            if (-not $item -or $item.PSIsContainer) {
                $bad += "missing $relative"
                continue
            }
            $size = $item.Length
            $hash = Get-Sha256Hex -Path $path
            if ($size -ne [int64]$property.Value.size -or $hash -ne ([string]$property.Value.sha256).ToLowerInvariant()) {
                $bad += "mismatch $relative"
            }
        }
    }
    else {
        $result.file_count = 0
        $bad += 'restored tree missing'
    }
    $result.hash_mismatches = @($bad)
    $result.ok = (
        $result.verify_exit -eq 0 -and
        $result.restore_exit -eq 0 -and
        $bad.Count -eq 0 -and
        $result.expected_file_count -eq 4 -and
        $result.file_count -eq $result.expected_file_count
    )
}
catch {
    $result.ok = $false
    $result.error = $_.Exception.Message
}

$json = $result | ConvertTo-Json -Depth 6
Set-Content -Path 'C:\result.json' -Value $json -Encoding UTF8
try {
    $port = New-Object System.IO.Ports.SerialPort COM1,115200,None,8,one
    $port.Open()
    $port.WriteLine('PEERVAULT_RESULT_BEGIN')
    $port.WriteLine($json)
    $port.WriteLine('PEERVAULT_RESULT_END')
    $port.Close()
}
catch {
    # The VM still shuts down; the absence of a serial result is a harness failure.
}
Start-Sleep -Seconds 2
& shutdown.exe /s /t 5
