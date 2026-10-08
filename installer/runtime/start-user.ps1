param([Parameter(Mandatory)][string]$AppDir,[Parameter(Mandatory)][string]$DataDir)
$ErrorActionPreference = 'Stop'
foreach ($name in @('hh-session','hh-tray')) {
    if (!(Get-Process -Name $name -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq (Join-Path $AppDir ($name + '.exe')) })) {
        Start-Process -FilePath (Join-Path $AppDir ($name + '.exe')) -ArgumentList @('--data-dir',('"' + $DataDir + '"')) -WindowStyle Hidden
    }
}
