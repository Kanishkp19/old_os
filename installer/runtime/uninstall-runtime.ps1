param([Parameter(Mandatory)][string]$AppDir)
$ErrorActionPreference = 'Continue'
Unregister-ScheduledTask -TaskName 'HomeHub User Session' -Confirm:$false -ErrorAction SilentlyContinue
foreach ($name in @('hh-tray','hh-session')) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq (Join-Path $AppDir ($name + '.exe')) } | Stop-Process -ErrorAction SilentlyContinue
}
Stop-Service -Name HomeHub -ErrorAction SilentlyContinue
& sc.exe delete HomeHub | Out-Null
foreach ($name in @('Home Hub API','Home Hub Pairing','Home Hub Discovery','Home Hub Screen')) { Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue | Remove-NetFirewallRule }
# Data, library and per-user private apps remain intact by default.
