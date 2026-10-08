param([string]$WebView2Installer)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$staging = Join-Path $root 'installer\staging'
if (!$WebView2Installer) { $WebView2Installer = Join-Path $root 'installer\deps\MicrosoftEdgeWebView2RuntimeInstallerX64.exe' }
if (!(Test-Path -LiteralPath $WebView2Installer -PathType Leaf)) { throw 'Place the official x64 Evergreen WebView2 standalone installer in installer\deps first.' }
$webviewSignature = Get-AuthenticodeSignature -LiteralPath $WebView2Installer
if ($webviewSignature.Status -ne 'Valid' -or $webviewSignature.SignerCertificate.Subject -notmatch 'Microsoft Corporation') { throw 'The WebView2 installer must have a valid Microsoft signature.' }
Push-Location (Join-Path $root 'hub')
try { & cargo build --release -p hh-service -p hh-session -p hh-tray -p hh-tools; if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed' } } finally { Pop-Location }
Push-Location (Join-Path $root 'desktop')
try {
    if (Test-Path -LiteralPath 'package-lock.json') { & npm ci } else { & npm install --no-audit --no-fund }
    if ($LASTEXITCODE -ne 0) { throw 'Desktop dependency install failed' }
    & npm run desktop:build
    if ($LASTEXITCODE -ne 0) { throw 'Desktop release build failed' }
} finally { Pop-Location }
New-Item -ItemType Directory -Path $staging -Force | Out-Null
foreach ($name in @('hh-service','hh-session','hh-tray','hh-tools')) {
    Copy-Item -LiteralPath (Join-Path $root ('hub\target\release\' + $name + '.exe')) -Destination (Join-Path $staging ($name + '.exe')) -Force
}
Copy-Item -LiteralPath (Join-Path $root 'desktop\src-tauri\target\release\hh-desktop.exe') -Destination (Join-Path $staging 'hh-desktop.exe') -Force
Copy-Item -LiteralPath $WebView2Installer -Destination (Join-Path $staging 'MicrosoftEdgeWebView2RuntimeInstallerX64.exe') -Force
$signPfx = [Environment]::GetEnvironmentVariable('HOMEHUB_SIGN_PFX')
$signPassword = [Environment]::GetEnvironmentVariable('HOMEHUB_SIGN_PASSWORD')
if ($signPfx -and $signPassword) {
    foreach ($file in (Get-ChildItem -LiteralPath $staging -Filter 'hh-*.exe')) {
        & signtool.exe sign /fd SHA256 /f $signPfx /p $signPassword $file.FullName
        if ($LASTEXITCODE -ne 0) { throw ('Signing failed: ' + $file.Name) }
    }
}
Push-Location (Join-Path $root 'installer')
try { & ISCC.exe 'homehub.iss'; if ($LASTEXITCODE -ne 0) { throw 'Inno Setup build failed' } } finally { Pop-Location }
if ($signPfx -and $signPassword) {
    foreach ($file in (Get-ChildItem -LiteralPath (Join-Path $root 'installer\out') -Filter 'HomeHubSetup-*.exe')) {
        & signtool.exe sign /fd SHA256 /f $signPfx /p $signPassword $file.FullName
        if ($LASTEXITCODE -ne 0) { throw ('Installer signing failed: ' + $file.Name) }
    }
}
Get-ChildItem -LiteralPath (Join-Path $root 'installer\out') -Filter 'HomeHubSetup-*.exe' | ForEach-Object { Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName }
