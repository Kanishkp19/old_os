param([Parameter(Mandatory)][string]$AppDir,[Parameter(Mandatory)][string]$DataDir)
$ErrorActionPreference = 'Stop'
$service = Get-Service -Name HomeHub -ErrorAction SilentlyContinue
if ($service) {
    if ($service.Status -ne 'Stopped') {
        Stop-Service -Name HomeHub -ErrorAction Stop
        (Get-Service -Name HomeHub).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45))
    }
}
if (!(Test-Path -LiteralPath $AppDir -PathType Container)) { exit 0 }
New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
$backup = Join-Path $DataDir ('upgrade-backup-' + [DateTime]::UtcNow.ToString('yyyyMMddHHmmss'))
New-Item -ItemType Directory -Path $backup -ErrorAction Stop | Out-Null
foreach ($name in @('hh-service.exe','hh-session.exe','hh-tray.exe','hh-desktop.exe','hh-tools.exe')) {
    $source = Join-Path $AppDir $name
    if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-Item -LiteralPath $source -Destination $backup -ErrorAction Stop }
}
foreach ($name in @('hub.db','hub.db-wal','hub.db-shm','config.json')) {
    $source = Join-Path $DataDir $name
    if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-Item -LiteralPath $source -Destination $backup -ErrorAction Stop }
}
[IO.File]::WriteAllText((Join-Path $DataDir 'pending-upgrade-backup'),$backup)
