; Canonical Inno Setup 6 package. Build only from release/prepare.ps1 output.
#define AppName "Home Hub"
#define AppVersion "0.1.0"

[Setup]
AppId={{8F3A1C2E-5B7D-4E1F-9A2C-HomeHub00001}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Home Hub Project
DefaultDirName={autopf}\HomeHub
DefaultGroupName=Home Hub
OutputDir=out
OutputBaseFilename=HomeHubSetup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
UninstallDisplayIcon={app}\hh-desktop.exe
CloseApplications=yes
RestartApplications=no

[Files]
Source: "staging\hh-service.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\hh-session.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\hh-tray.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\hh-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\hh-tools.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\MicrosoftEdgeWebView2RuntimeInstallerX64.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall
Source: "..\hub\dashboard\*"; DestDir: "{app}\dashboard"; Flags: ignoreversion recursesubdirs
Source: "runtime\install-runtime.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "runtime\start-user.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "runtime\restore-upgrade.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "runtime\uninstall-runtime.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "runtime\pre-upgrade.ps1"; Flags: dontcopy
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
Name: "{commonappdata}\HomeHub"

[Icons]
Name: "{group}\Home Hub"; Filename: "{app}\hh-desktop.exe"

[Run]
Filename: "{app}\hh-desktop.exe"; Description: "Open Home Hub"; Flags: postinstall nowait skipifsilent runasoriginaluser

[UninstallRun]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File ""{app}\uninstall-runtime.ps1"" -AppDir ""{app}"""; Flags: runhidden waituntilterminated

[Code]
var OwnerPage:TInputQueryWizardPage;

procedure InitializeWizard;
begin
  OwnerPage:=CreateInputQueryPage(wpSelectDir,'Home Hub owner','Choose the Windows account that will use Home Hub','This account can administer the Hub. If Windows asked for a different administrator password, enter your normal Windows account here as COMPUTER\user or DOMAIN\user.');
  OwnerPage.Add('Windows account:',False);
  OwnerPage.Values[0]:=GetUserNameString;
end;

function GetOwnerAccount(Param:String):String;
begin
  Result:=Trim(OwnerPage.Values[0]);
end;

function PrepareToInstall(var NeedsRestart:Boolean):String;
var Code:Integer; Script:String;
begin
  Result:='';
  if Trim(OwnerPage.Values[0])='' then begin Result:='Choose the Windows account that will administer Home Hub.'; exit; end;
  if Pos('"',OwnerPage.Values[0])>0 then begin Result:='Windows account contains an invalid character.'; exit; end;
  ExtractTemporaryFile('pre-upgrade.ps1');
  Script:=ExpandConstant('{tmp}\pre-upgrade.ps1');
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),'-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+Script+'" -AppDir "'+ExpandConstant('{app}')+'" -DataDir "'+ExpandConstant('{commonappdata}\HomeHub')+'"','',SW_HIDE,ewWaitUntilTerminated,Code) or (Code<>0) then
    Result:='Could not prepare the existing Home Hub installation. Its data was left in place.';
end;

procedure CurStepChanged(CurStep:TSetupStep);
var Code:Integer; Params:String;
begin
  if CurStep=ssPostInstall then begin
    if not Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebView2RuntimeInstallerX64.exe'),'/silent /install','',SW_HIDE,ewWaitUntilTerminated,Code) or (Code<>0) then
      RaiseException('WebView2 could not be installed. Home Hub setup cannot continue.');
    Params:='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+ExpandConstant('{app}\install-runtime.ps1')+'" -AppDir "'+ExpandConstant('{app}')+'" -DataDir "'+ExpandConstant('{commonappdata}\HomeHub')+'" -OwnerAccount "'+GetOwnerAccount('')+'"';
    if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),Params,'',SW_HIDE,ewWaitUntilTerminated,Code) or (Code<>0) then
      RaiseException('Home Hub could not finish installation. The saved upgrade is retained for recovery.');
  end;
end;

; User data is retained on uninstall, including library, CA, DB, private app
; data in the owner profile and independently stored second copies.
