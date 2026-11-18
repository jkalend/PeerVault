param(
    [Parameter(Mandatory = $true)] [string]$Core,
    [Parameter(Mandatory = $true)] [string]$Config,
    [Parameter(Mandatory = $true)] [string]$State,
    [string]$TaskName = "PeerVault scheduled tick",
    [int]$EveryMinutes = 15,
    [string]$User = "$env:USERDOMAIN\$env:USERNAME",
    [string]$ProtectedSecret,
    [string]$SecretPath,
    [string]$RunnerScript,
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

if ($ProtectedSecret) {
    $ProtectedSecret = (Resolve-Path -LiteralPath $ProtectedSecret -ErrorAction Stop).Path
    if (-not $SecretPath) { throw "SecretPath is required with ProtectedSecret" }
    $SecretPath = [IO.Path]::GetFullPath($SecretPath)
    if (-not (Test-Path -LiteralPath (Split-Path -Parent $SecretPath) -PathType Container)) {
        throw "SecretPath directory must exist"
    }
    if (-not $RunnerScript) { $RunnerScript = Join-Path $PSScriptRoot "task-run.ps1" }
    $RunnerScript = (Resolve-Path -LiteralPath $RunnerScript -ErrorAction Stop).Path
    $action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument (
        "-NoProfile -ExecutionPolicy Bypass -File `"$RunnerScript`" " +
        "-Core `"$Core`" -Config `"$Config`" -State `"$State`" " +
        "-ProtectedSecret `"$ProtectedSecret`" -SecretPath `"$SecretPath`"")
    $secretSource = "DPAPI current-user secret; runtime file removed after each tick"
} else {
    $action = New-ScheduledTaskAction -Execute $Core -Argument "schedule-tick --config `"$Config`" --state `"$State`""
    $secretSource = "job config password_file"
}
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
        SecretSource = $secretSource
    } | ConvertTo-Json
    exit 0
}

Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger `
    -Principal $principal -Settings $settings -Force | Out-Null
Write-Output "Registered scheduled task '$TaskName' every $EveryMinutes minute(s) as $User (S4U)."
Write-Output "The task runs the core schedule-tick; scheduled secrets stay in the job config's password_file."
