param([Parameter(Mandatory)][string]$AppDir,[Parameter(Mandatory)][string]$DataDir)
$ErrorActionPreference = 'Stop'
$marker = Join-Path $DataDir 'pending-upgrade-backup'
if (!(Test-Path -LiteralPath $marker -PathType Leaf)) { throw 'No saved upgrade is available' }
$backup = [IO.File]::ReadAllText($marker).Trim()
if (!$backup.StartsWith((Join-Path $DataDir 'upgrade-backup-'),[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid backup path' }
$service = Get-Service HomeHub -ErrorAction SilentlyContinue
if ($service -and $service.Status -ne 'Stopped') { Stop-Service HomeHub; (Get-Service HomeHub).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45)) }
$current = Join-Path $DataDir ('failed-upgrade-' + [DateTime]::UtcNow.ToString('yyyyMMddHHmmssfff'))
New-Item -ItemType Directory -Path $current -ErrorAction Stop | Out-Null
foreach ($name in @('hub.db','hub.db-wal','hub.db-shm','config.json')) {
    $path = Join-Path $DataDir $name
    if (Test-Path -LiteralPath $path -PathType Leaf) { Copy-Item -LiteralPath $path -Destination $current -ErrorAction Stop }
}
foreach ($name in @('hh-service.exe','hh-session.exe','hh-tray.exe','hh-desktop.exe','hh-tools.exe')) {
    $source = Join-Path $backup $name
    if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-Item -LiteralPath $source -Destination (Join-Path $AppDir $name) -Force -ErrorAction Stop }
}
foreach ($name in @('hub.db','hub.db-wal','hub.db-shm','config.json')) {
    $source = Join-Path $backup $name
    $target = Join-Path $DataDir $name
    if (Test-Path -LiteralPath $source -PathType Leaf) {
        Copy-Item -LiteralPath $source -Destination $target -Force -ErrorAction Stop
    } elseif (Test-Path -LiteralPath $target -PathType Leaf) {
        Remove-Item -LiteralPath $target -Force -ErrorAction Stop
    }
}
if ($service) { Start-Service HomeHub }
