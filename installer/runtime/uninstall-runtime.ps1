param(
    [Parameter(Mandatory)][string]$AppDir,
    [Parameter(Mandatory)][string]$DataDir,
    [ValidateSet('yes','no')][string]$KeepData = 'yes'
)
$ErrorActionPreference = 'Stop'
Unregister-ScheduledTask -TaskName 'HomeHub User Session' -Confirm:$false -ErrorAction SilentlyContinue
foreach ($name in @('hh-desktop','hh-tray','hh-session')) {
    Get-Process -Name $name -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq (Join-Path $AppDir ($name + '.exe')) } |
        Stop-Process -ErrorAction Stop
}
$service = Get-Service -Name HomeHub -ErrorAction SilentlyContinue
if ($service) {
    if ($service.Status -ne 'Stopped') {
        Stop-Service -Name HomeHub -ErrorAction Stop
        (Get-Service -Name HomeHub).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45))
    }
    & sc.exe delete HomeHub | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not remove the Home Hub service' }
}
foreach ($name in @('Home Hub API','Home Hub Pairing','Home Hub Discovery','Home Hub Screen')) {
    Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction Stop
}
if ($KeepData -eq 'no') {
    $expected = [IO.Path]::GetFullPath((Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'HomeHub')).TrimEnd('\')
    if ([IO.Path]::GetFullPath($DataDir).TrimEnd('\') -ne $expected) { throw 'Refusing to remove data outside ProgramData\HomeHub' }
    if (Test-Path -LiteralPath $DataDir -PathType Container) { Remove-Item -LiteralPath $DataDir -Recurse -Force -ErrorAction Stop }
}
# Keep-data leaves the Hub database, CA, library, and private app data in place.
