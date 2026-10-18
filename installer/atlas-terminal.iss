; =====================================================================
; Atlas Terminal — Modern Inno Setup Installer Script
; =====================================================================

#ifndef MyAppVersion
#define MyAppVersion "0.1.3"
#endif

#define MyAppName "Atlas Terminal"
#define MyAppPublisher "Atlas Terminal Team"
#define MyAppURL "https://github.com/akrram0/atlas-terminal"
#define MyAppExeName "atlas-terminal.exe"
#define MyAppId "{{B4C26E92-3B47-4E28-86F0-55E04F2F125E}"

[Setup]
; Unique application identity for seamless in-place upgrades
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases

; Per-user install: no administrator privileges / UAC prompt required
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\AtlasTerminal
DefaultGroupName=Atlas Terminal
DisableProgramGroupPage=yes

; Wizard configuration
DisableWelcomePage=no
LicenseFile=..\LICENSE
DisableDirPage=no

; Visuals & Branding
SetupIconFile=..\assets\atlas.ico
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
WizardStyle=modern

; Output configuration
OutputDir=..\dist
OutputBaseFilename=AtlasTerminal-Setup-{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes

; Process termination for clean in-place updates
CloseApplications=yes
CloseApplicationsFilter={#MyAppExeName},atlas-native.exe

; Broadcast environment changes when PATH is updated
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "addtopath"; Description: "Add Atlas Terminal to PATH (current user)"; GroupDescription: "System integration:"
Name: "contextmenu"; Description: "Add ""Open Atlas Terminal here"" to Explorer context menu"; GroupDescription: "System integration:"

[Files]
; Main binary
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; Documentation & license
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
; Bundled assets (icons, fonts)
Source: "..\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Always create Start Menu shortcut
Name: "{userprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"
; Optional Desktop shortcut
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Context menu for folders (clicking on a folder in Explorer)
Root: HKCU; Subkey: "Software\Classes\Directory\shell\AtlasTerminal"; ValueType: string; ValueData: "Open Atlas Terminal here"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\AtlasTerminal"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\AtlasTerminal\command"; ValueType: string; ValueData: """{app}\{#MyAppExeName}"" ""%V"""; Flags: uninsdeletekey; Tasks: contextmenu

; Context menu for folder background (clicking whitespace inside a folder)
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\AtlasTerminal"; ValueType: string; ValueData: "Open Atlas Terminal here"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\AtlasTerminal"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\AtlasTerminal\command"; ValueType: string; ValueData: """{app}\{#MyAppExeName}"" ""%V"""; Flags: uninsdeletekey; Tasks: contextmenu

; Context menu for drives (e.g. C:\)
Root: HKCU; Subkey: "Software\Classes\Drive\shell\AtlasTerminal"; ValueType: string; ValueData: "Open Atlas Terminal here"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Drive\shell\AtlasTerminal"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Drive\shell\AtlasTerminal\command"; ValueType: string; ValueData: """{app}\{#MyAppExeName}"" ""%V"""; Flags: uninsdeletekey; Tasks: contextmenu

[Run]
; Interactive launch option on wizard finish page
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
; Silent auto-relaunch if /relaunch parameter was supplied
Filename: "{app}\{#MyAppExeName}"; Flags: nowait skipifnotsilent; Check: ShouldRelaunch

[Code]
const
  EnvironmentKey = 'Environment';

function ShouldRelaunch: Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
  begin
    if CompareText(ParamStr(I), '/relaunch') = 0 then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

procedure AddPathToEnv(PathToAdd: String);
var
  CurrentPath: String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', CurrentPath) then
    CurrentPath := '';

  if Pos(';' + Uppercase(PathToAdd) + ';', ';' + Uppercase(CurrentPath) + ';') = 0 then
  begin
    if (CurrentPath <> '') and (CurrentPath[Length(CurrentPath)] <> ';') then
      CurrentPath := CurrentPath + ';';
    CurrentPath := CurrentPath + PathToAdd;
    RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', CurrentPath);
  end;
end;

procedure RemovePathFromEnv(PathToRemove: String);
var
  CurrentPath, Target: String;
begin
  if RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', CurrentPath) then
  begin
    Target := PathToRemove;
    StringChange(CurrentPath, ';' + Target + ';', ';');
    StringChange(CurrentPath, ';' + Target, '');
    StringChange(CurrentPath, Target + ';', '');
    StringChange(CurrentPath, Target, '');
    StringChange(CurrentPath, ';;', ';');
    if (Length(CurrentPath) > 0) and (CurrentPath[1] = ';') then
      Delete(CurrentPath, 1, 1);
    if (Length(CurrentPath) > 0) and (CurrentPath[Length(CurrentPath)] = ';') then
      Delete(CurrentPath, Length(CurrentPath), 1);
    RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', CurrentPath);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then
    begin
      AddPathToEnv(ExpandConstant('{app}'));
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
  AppDir: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\atlas-terminal');
    if DirExists(DataDir) then
    begin
      if SuppressibleMsgBox('Also delete your settings and data?' + #13#10 + DataDir, mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) = IDYES then
      begin
        DelTree(DataDir, True, True, True);
      end;
    end;
    RemovePathFromEnv(ExpandConstant('{app}'));
  end
  else if CurUninstallStep = usPostUninstall then
  begin
    AppDir := ExpandConstant('{app}');
    if (Pos('AtlasTerminal', AppDir) > 0) and DirExists(AppDir) then
    begin
      DelTree(AppDir, True, True, True);
    end;
  end;
end;
