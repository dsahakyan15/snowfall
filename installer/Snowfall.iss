; Snowfall's installer: per user, no administrator rights, English or Russian.
; Built by .github/workflows/release.yml as `iscc /DVersion=X.Y.Z installer\Snowfall.iss`, after
; `cargo build --release`, with Inno Setup 6.3 or later; the result is dist\Snowfall-Setup.exe.

#ifndef Version
  #define Version "0.0.0"
#endif
; The file version has numbers only: 1.2.0 for 1.2.0-beta.1.
#define NumericVersion Copy(Version, 1, Pos("-", Version + "-") - 1)

[Setup]
AppId={{6C1C8B0E-3F7A-4E2B-9D0B-5A7E2F1C9A41}
AppName=Snowfall
AppVersion={#Version}
AppVerName=Snowfall {#Version}
VersionInfoVersion={#NumericVersion}
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

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.StillRunning=Snowfall is still running. Quit it from its snowflake icon in the notification area, then run Setup again.
ru.StillRunning=Snowfall всё ещё работает. Закройте его через значок снежинки в области уведомлений и запустите установку снова.

[Files]
Source: "..\target\release\Snowfall.exe"; DestDir: "{app}"; Flags: ignoreversion
; Snowfall is MIT OR Apache-2.0 at the user's option; both texts go along, with the notices.
Source: "..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Snowfall"; Filename: "{app}\Snowfall.exe"

[Run]
Filename: "{app}\Snowfall.exe"; Description: "{cm:LaunchProgram,Snowfall}"; Flags: nowait postinstall

[Registry]
; Snowfall writes its own settings. This copy takes over starting at login unless the person turned that off, as a
; portable copy run earlier may have registered itself.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Snowfall"; ValueData: """{app}\Snowfall.exe"""; Check: OpensAtLogin
; Uninstalling removes the settings; the start-at-login entry goes in the code below, if it is this copy's.
Root: HKCU; Subkey: "Software\Snowfall"; Flags: uninsdeletekey dontcreatekey

[UninstallDelete]
; The log.
Type: filesandordirs; Name: "{localappdata}\Snowfall"

[Code]
const
  WM_CLOSE = $0010;
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';

{ Asks a running Snowfall to quit, as its menu's Quit does, and waits for it to go. Returns whether it is gone. }
function CloseSnowfall(): Boolean;
var
  Window: HWND;
  Waited: Integer;
begin
  Window := FindWindowByClassName('SnowfallController');
  if Window <> 0 then
    PostMessage(Window, WM_CLOSE, 0, 0);
  Waited := 0;
  while (FindWindowByClassName('SnowfallController') <> 0) and (Waited < 5000) do
  begin
    Sleep(100);
    Waited := Waited + 100;
  end;
  Result := FindWindowByClassName('SnowfallController') = 0;
end;

{ Open at login is on unless the person turned it off. }
function OpensAtLogin(): Boolean;
var
  Value: Cardinal;
begin
  Result := not RegQueryDWordValue(HKCU, 'Software\Snowfall', 'openAtLogin', Value) or (Value <> 0);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  if CloseSnowfall() then
    Result := ''
  else
    Result := CustomMessage('StillRunning');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command: String;
begin
  if CurUninstallStep = usUninstall then
    CloseSnowfall();
  { The start-at-login entry goes only if it starts this copy, not a portable one elsewhere. }
  if (CurUninstallStep = usPostUninstall) and RegQueryStringValue(HKCU, RunKey, 'Snowfall', Command)
    and (Pos(Lowercase(ExpandConstant('{app}')), Lowercase(Command)) > 0) then
    RegDeleteValue(HKCU, RunKey, 'Snowfall');
end;
