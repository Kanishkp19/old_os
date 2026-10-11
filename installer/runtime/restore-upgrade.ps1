param([Parameter(Mandatory)][string]$AppDir,[Parameter(Mandatory)][string]$DataDir)
$ErrorActionPreference = 'Stop'
$marker = Join-Path $DataDir 'pending-upgrade-backup'
if (!(Test-Path -LiteralPath $marker -PathType Leaf)) { throw 'No saved upgrade is available' }
$backup = [IO.Path]::GetFullPath([IO.File]::ReadAllText($marker).Trim())
if ([IO.Path]::GetDirectoryName($backup) -ne [IO.Path]::GetFullPath($DataDir).TrimEnd('\') -or
    [IO.Path]::GetFileName($backup) -notmatch '^upgrade-backup-[0-9a-f]{32}$' -or
    !(Test-Path -LiteralPath (Join-Path $backup 'app\hh-service.exe') -PathType Leaf)) {
    throw 'Invalid upgrade backup'
}
$service = Get-Service -Name HomeHub -ErrorAction SilentlyContinue
if ($service -and $service.Status -ne 'Stopped') {
    Stop-Service -Name HomeHub -ErrorAction Stop
    (Get-Service -Name HomeHub).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45))
}
foreach ($name in @('hh-session','hh-tray','hh-desktop')) {
    Get-Process -Name $name -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq (Join-Path $AppDir ($name + '.exe')) } |
        Stop-Process -ErrorAction Stop
}
$id = [guid]::NewGuid().ToString('N')
$appStage = $AppDir + '.rollback-stage-' + $id
$failedApp = $AppDir + '.failed-' + $id
$failedData = Join-Path $DataDir ('failed-upgrade-' + $id)
$dbStage = Join-Path $DataDir ('hub.db.rollback-stage-' + $id)
$configStage = Join-Path $DataDir ('config.json.rollback-stage-' + $id)
Copy-Item -LiteralPath (Join-Path $backup 'app') -Destination $appStage -Recurse -ErrorAction Stop
if (Test-Path -LiteralPath (Join-Path $backup 'hub.db') -PathType Leaf) {
    Copy-Item -LiteralPath (Join-Path $backup 'hub.db') -Destination $dbStage -ErrorAction Stop
}
if (Test-Path -LiteralPath (Join-Path $backup 'config.json') -PathType Leaf) {
    Copy-Item -LiteralPath (Join-Path $backup 'config.json') -Destination $configStage -ErrorAction Stop
}
New-Item -ItemType Directory -Path $failedData -ErrorAction Stop | Out-Null
try {
    if (Test-Path -LiteralPath $AppDir -PathType Container) { [IO.Directory]::Move($AppDir,$failedApp) }
    [IO.Directory]::Move($appStage,$AppDir)
    foreach ($name in @('hub.db','hub.db-wal','hub.db-shm','config.json')) {
        $target = Join-Path $DataDir $name
        if (Test-Path -LiteralPath $target -PathType Leaf) {
            [IO.File]::Move($target,(Join-Path $failedData $name))
        }
    }
    if (Test-Path -LiteralPath $dbStage -PathType Leaf) { [IO.File]::Move($dbStage,(Join-Path $DataDir 'hub.db')) }
    if (Test-Path -LiteralPath $configStage -PathType Leaf) { [IO.File]::Move($configStage,(Join-Path $DataDir 'config.json')) }
    if ($service) {
        Start-Service -Name HomeHub -ErrorAction Stop
        (Get-Service -Name HomeHub).WaitForStatus('Running',[TimeSpan]::FromSeconds(30))
    }
    Remove-Item -LiteralPath $marker -Force -ErrorAction Stop
} catch {
    # Leave the service stopped and the marker intact for another recovery attempt.
    if ($service -and (Get-Service -Name HomeHub).Status -ne 'Stopped') {
        Stop-Service -Name HomeHub -ErrorAction SilentlyContinue
        (Get-Service -Name HomeHub).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45))
    }
    if (Test-Path -LiteralPath $failedApp -PathType Container) {
        if (Test-Path -LiteralPath $AppDir -PathType Container) { [IO.Directory]::Move($AppDir,($AppDir + '.incomplete-' + $id)) }
        [IO.Directory]::Move($failedApp,$AppDir)
    }
    foreach ($name in @('hub.db','hub.db-wal','hub.db-shm','config.json')) {
        $target = Join-Path $DataDir $name
        $failed = Join-Path $failedData $name
        if (Test-Path -LiteralPath $failed -PathType Leaf) {
            if (Test-Path -LiteralPath $target -PathType Leaf) { [IO.File]::Move($target,($target + '.incomplete-' + $id)) }
            [IO.File]::Move($failed,$target)
        }
    }
    throw
}
