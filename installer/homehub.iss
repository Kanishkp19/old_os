; Home Hub installer — Inno Setup 6 script.
; Builds HomeHubSetup.exe which:
;   1. installs hh-service.exe + dashboard assets,
;   2. creates %ProgramData%\HomeHub (CA + DB live here),
;   3. registers hh-service as an auto-start Windows service,
;   4. adds Private-profile firewall rules for 47800/47802/5353 ONLY,
;   5. on uninstall: stops the service and asks before deleting data.
;
; Build: iscc installer/homehub.iss   (run from the repo root)

#define AppName "Home Hub"
#define AppVersion "0.1.0"
#define AppPublisher "Home Hub Project"

[Setup]
AppId={{8F3A1C2E-5B7D-4E1F-9A2C-HomeHub00001}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\HomeHub
DefaultGroupName=Home Hub
OutputDir=installer\out
OutputBaseFilename=HomeHubSetup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

[Files]
Source: "hub\target\release\hh-service.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "hub\target\release\hh-tools.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "hub\dashboard\*"; DestDir: "{app}\dashboard"; Flags: ignoreversion recursesubdirs
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
; CA key, hub.db, logs. NOT deleted on uninstall unless the user opts in.
Name: "{commonappdata}\HomeHub"

[Icons]
Name: "{group}\Home Hub Dashboard"; Filename: "http://127.0.0.1:47801/"

[Run]
; Register + start the service (auto-start, restarts on failure).
Filename: "sc.exe"; Parameters: "create HomeHub binPath= ""{app}\hh-service.exe"" start= auto DisplayName= ""Home Hub"""; Flags: runhidden waituntilterminated
Filename: "sc.exe"; Parameters: "failure HomeHub reset= 86400 actions= restart/5000/restart/10000/restart/30000"; Flags: runhidden waituntilterminated
; Firewall: Private profile only, LAN-only service. Dashboard (47801) is
; loopback and needs NO rule; pairing (47802) is time-boxed by the app.
Filename: "netsh.exe"; Parameters: "advfirewall firewall add rule name=""Home Hub API"" dir=in action=allow protocol=TCP localport=47800 profile=private program=""{app}\hh-service.exe"""; Flags: runhidden waituntilterminated
Filename: "netsh.exe"; Parameters: "advfirewall firewall add rule name=""Home Hub Pairing"" dir=in action=allow protocol=TCP localport=47802 profile=private program=""{app}\hh-service.exe"""; Flags: runhidden waituntilterminated
Filename: "netsh.exe"; Parameters: "advfirewall firewall add rule name=""Home Hub mDNS"" dir=in action=allow protocol=UDP localport=5353 profile=private program=""{app}\hh-service.exe"""; Flags: runhidden waituntilterminated
Filename: "sc.exe"; Parameters: "start HomeHub"; Flags: runhidden waituntilterminated
Filename: "http://127.0.0.1:47801/"; Description: "Open the Home Hub dashboard"; Flags: postinstall shellexec skipifsilent

[UninstallRun]
Filename: "sc.exe"; Parameters: "stop HomeHub"; Flags: runhidden waituntilterminated
Filename: "sc.exe"; Parameters: "delete HomeHub"; Flags: runhidden waituntilterminated
Filename: "netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Home Hub API"""; Flags: runhidden waituntilterminated
Filename: "netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Home Hub Pairing"""; Flags: runhidden waituntilterminated
Filename: "netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Home Hub mDNS"""; Flags: runhidden waituntilterminated

[Code]
// Never delete user data silently (AGENTS.md §1): ask before removing the
// data directory, defaulting to KEEP.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
  Choice: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{commonappdata}\HomeHub');
    if DirExists(DataDir) then
    begin
      Choice := MsgBox(
        'Keep your Home Hub data (photos and files you received)?' + #13#10 +
        'Choose Yes to keep everything. Choose No to delete it permanently.',
        mbConfirmation, MB_YESNO or MB_DEFBUTTON1);
      if Choice = IDNO then
        DelTree(DataDir, True, True, True);
    end;
  end;
end;
