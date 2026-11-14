; Snowfall's installer: per user, no administrator rights, English or Russian.
; Built by .github/workflows/release.yml as `iscc /DVersion=X.Y.Z installer\Snowfall.iss`, after
; `cargo build --release`; the result is dist\Snowfall-Setup.exe.

#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{6C1C8B0E-3F7A-4E2B-9D0B-5A7E2F1C9A41}
AppName=Snowfall
AppVersion={#Version}
AppVerName=Snowfall {#Version}
AppPublisher=The Snowfall contributors
AppPublisherURL=https://github.com/dsahakyan15/snowfall
AppSupportURL=https://github.com/dsahakyan15/snowfall/issues
AppUpdatesURL=https://github.com/dsahakyan15/snowfall/releases
; Per user, in the user's own programs folder: no UAC prompt.
PrivilegesRequired=lowest
DefaultDirName={userpf}\Snowfall
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
OutputDir=..\dist
OutputBaseFilename=Snowfall-Setup
SetupIconFile=..\windows\assets\Snowfall.ico
UninstallDisplayIcon={app}\Snowfall.exe
UninstallDisplayName=Snowfall
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; Snowfall is closed by the code below, not by the Restart Manager.
CloseApplications=no
LicenseFile=..\LICENSE-MIT

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[Files]
Source: "..\target\release\Snowfall.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Snowfall"; Filename: "{app}\Snowfall.exe"

[Run]
Filename: "{app}\Snowfall.exe"; Description: "{cm:LaunchProgram,Snowfall}"; Flags: nowait postinstall

[Registry]
; Nothing is written here at install; Snowfall writes its settings and autostart entry itself. Uninstalling removes both.
Root: HKCU; Subkey: "Software\Snowfall"; Flags: uninsdeletekey dontcreatekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Snowfall"; Flags: uninsdeletevalue dontcreatekey

[UninstallDelete]
; The log.
Type: filesandordirs; Name: "{localappdata}\Snowfall"

[Code]
const
  WM_CLOSE = $0010;

function FindWindowByClass(ClassName: String; WindowName: Integer): HWND;
  external 'FindWindowW@user32.dll stdcall';

{ Asks a running Snowfall to quit, as its menu's Quit does, and waits for it to go. }
procedure CloseSnowfall();
var
  Window: HWND;
  Waited: Integer;
begin
  Window := FindWindowByClass('SnowfallController', 0);
  if Window = 0 then
    Exit;
  PostMessage(Window, WM_CLOSE, 0, 0);
  Waited := 0;
  while (FindWindowByClass('SnowfallController', 0) <> 0) and (Waited < 5000) do
  begin
    Sleep(100);
    Waited := Waited + 100;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseSnowfall();
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    CloseSnowfall();
end;
