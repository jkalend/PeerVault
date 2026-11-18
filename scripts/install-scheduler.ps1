param(
    [Parameter(Mandatory = $true)] [string]$Core,
    [Parameter(Mandatory = $true)] [string]$Config,
    [Parameter(Mandatory = $true)] [string]$State,
    [string]$TaskName = "PeerVault scheduled tick",
    [int]$EveryMinutes = 15,
    [string]$User = "$env:USERDOMAIN\$env:USERNAME",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$Core = (Resolve-Path -LiteralPath $Core -ErrorAction Stop).Path
$Config = (Resolve-Path -LiteralPath $Config -ErrorAction Stop).Path
$State = [IO.Path]::GetFullPath($State)
if (-not (Test-Path -LiteralPath $Core -PathType Leaf)) { throw "Core executable not found: $Core" }
if (-not (Test-Path -LiteralPath $Config -PathType Leaf)) { throw "Job config not found: $Config" }
if ($EveryMinutes -lt 1 -or $EveryMinutes -gt 1440) { throw "EveryMinutes must be 1..1440" }
$stateDirectory = Split-Path -Parent $State
if (-not $stateDirectory -or -not (Test-Path -LiteralPath $stateDirectory -PathType Container)) {
    throw "State directory must exist: $stateDirectory"
}

$action = New-ScheduledTaskAction -Execute $Core -Argument "schedule-tick --config `"$Config`" --state `"$State`""
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) `
    -RepetitionInterval (New-TimeSpan -Minutes $EveryMinutes) `
    -RepetitionDuration (New-TimeSpan -Days 3650)
$principal = New-ScheduledTaskPrincipal -UserId $User -LogonType S4U -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Hours 6) -AllowStartIfOnBatteries:$false `
    -DontStopIfGoingOnBatteries:$false

if ($DryRun) {
    [pscustomobject]@{
        TaskName = $TaskName
        Core = $Core
        Argument = $action.Arguments
        User = $User
        EveryMinutes = $EveryMinutes
        LogonType = "S4U (no stored password)"
        SecretSource = "job config password_file"
    } | ConvertTo-Json
    exit 0
}

Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger `
    -Principal $principal -Settings $settings -Force | Out-Null
Write-Output "Registered scheduled task '$TaskName' every $EveryMinutes minute(s) as $User (S4U)."
Write-Output "The task runs the core schedule-tick; scheduled secrets stay in the job config's password_file."
