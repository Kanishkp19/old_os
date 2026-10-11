param([Parameter(Mandatory)][string]$AppDir,[Parameter(Mandatory)][string]$DataDir)
$ErrorActionPreference = 'Stop'
if (!(Test-Path -LiteralPath $AppDir -PathType Container)) { exit 0 }
$serviceExe = Join-Path $AppDir 'hh-service.exe'
if (!(Test-Path -LiteralPath $serviceExe -PathType Leaf)) { throw 'Existing service binary is missing; upgrade cannot be backed up' }
New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
$marker = Join-Path $DataDir 'pending-upgrade-backup'
if (Test-Path -LiteralPath $marker) { throw 'An earlier upgrade backup is pending recovery' }
$backup = Join-Path $DataDir ('upgrade-backup-' + [guid]::NewGuid().ToString('N'))
$stage = $backup + '.staging'
$service = Get-Service -Name HomeHub -ErrorAction SilentlyContinue
$wasRunning = $service -and $service.Status -ne 'Stopped'
$completed = $false
try {
    if ($wasRunning) {
        Stop-Service -Name HomeHub -ErrorAction Stop
        (Get-Service -Name HomeHub).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45))
    }
    foreach ($name in @('hh-session','hh-tray','hh-desktop')) {
        Get-Process -Name $name -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq (Join-Path $AppDir ($name + '.exe')) } |
            Stop-Process -ErrorAction Stop
    }
    New-Item -ItemType Directory -Path $stage -ErrorAction Stop | Out-Null
    Copy-Item -LiteralPath $AppDir -Destination (Join-Path $stage 'app') -Recurse -ErrorAction Stop
    $db = Join-Path $DataDir 'hub.db'
    if (Test-Path -LiteralPath $db -PathType Leaf) {
        & $serviceExe --data-dir $DataDir --backup-database (Join-Path $stage 'hub.db')
        if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath (Join-Path $stage 'hub.db') -PathType Leaf)) {
            throw 'Could not make a consistent SQLite database snapshot'
        }
    }
    $config = Join-Path $DataDir 'config.json'
    if (Test-Path -LiteralPath $config -PathType Leaf) { Copy-Item -LiteralPath $config -Destination $stage -ErrorAction Stop }
    Move-Item -LiteralPath $stage -Destination $backup -ErrorAction Stop
    $markerStage = $marker + '.staging'
    [IO.File]::WriteAllText($markerStage,$backup)
    [IO.File]::Move($markerStage,$marker)
    $completed = $true
} finally {
    if (!$completed) {
        Remove-Item -LiteralPath $stage,$backup,($marker + '.staging') -Recurse -Force -ErrorAction SilentlyContinue
        if ($wasRunning) { Start-Service -Name HomeHub -ErrorAction SilentlyContinue }
    }
}
