#requires -Version 5.1
<#
.SYNOPSIS
  Build the read-only PEERVAULT_TEST ISO for the Windows clean-client drill.

.DESCRIPTION
  The source directory must contain the complete test-media payload:
  autounattend.xml, run-test.ps1, peervault.exe, restic.exe, expected.json,
  repository/ and kit/. The separate PEERVAULT_SECRET ISO is not built here; it
  must be supplied separately and contain secret.txt.

  This is a Windows-host helper for experimental synthetic-data qualification.
  It uses the built-in IMAPI2FS COM API and does not require external ISO tools.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$SourceDirectory,
    [Parameter(Mandatory = $true)][string]$OutputIso,
    [string]$VolumeLabel = 'PEERVAULT_TEST',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
$output = [IO.Path]::GetFullPath($OutputIso)
$required = @(
    'autounattend.xml',
    'run-test.ps1',
    'peervault.exe',
    'restic.exe',
    'expected.json',
    'repository',
    'kit'
)
foreach ($name in $required) {
    $path = Join-Path $source $name
    if (-not (Test-Path -LiteralPath $path)) {
        throw "test-media source is missing $name"
    }
}
if ([string]::IsNullOrWhiteSpace($VolumeLabel) -or $VolumeLabel.Length -gt 32) {
    throw 'VolumeLabel must be 1-32 characters'
}
if (Test-Path -LiteralPath $output) {
    if (-not $Force) {
        throw "output ISO already exists: $output (use -Force to replace it)"
    }
    Remove-Item -LiteralPath $output -Force
}

Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
public static class PeerVaultIsoStream {
    public static void Save(object stream, string path) {
        IStream source = (IStream)stream;
        using (FileStream file = File.Create(path)) {
            byte[] buffer = new byte[1024 * 1024];
            IntPtr read = Marshal.AllocHGlobal(4);
            try {
                int count;
                do {
                    source.Read(buffer, buffer.Length, read);
                    count = Marshal.ReadInt32(read);
                    if (count > 0) {
                        file.Write(buffer, 0, count);
                    }
                } while (count > 0);
            }
            finally {
                Marshal.FreeHGlobal(read);
            }
        }
    }
}
'@

$fsi = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
$fsi.FileSystemsToCreate = 3
$fsi.VolumeName = $VolumeLabel
$fsi.Root.AddTree($source, $false)
$result = $fsi.CreateResultImage()
[PeerVaultIsoStream]::Save($result.ImageStream, $output)
if (-not (Test-Path -LiteralPath $output) -or (Get-Item -LiteralPath $output).Length -eq 0) {
    throw "ISO creation did not produce a non-empty file: $output"
}
Write-Output "created $output ($((Get-Item -LiteralPath $output).Length) bytes, volume $VolumeLabel)"
