param(
    [string]$TaskName = "PeerVault scheduled tick"
)
$ErrorActionPreference = "Stop"
Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction Stop
Write-Output "Removed scheduled task '$TaskName'. Repository data and credentials were not deleted."
