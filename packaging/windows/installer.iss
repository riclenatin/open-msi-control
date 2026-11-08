; Inno Setup 6 script for the Open MSI Control installer.
;
; Built by packaging/build-release.ps1, which passes the workspace version and the cargo
; release directory:
;   ISCC.exe /DAppVersion=0.1.0 /DBinDir=C:\...\target\release packaging\windows\installer.iss
;
; The output name open-msi-control-<version>-setup.exe is read by the in-app update checker;
; keep it in sync with build-release.ps1 and the release workflow.

#ifndef AppVersion
  #error Pass the version: ISCC.exe /DAppVersion=x.y.z installer.iss
#endif
#ifndef BinDir
  #define BinDir "target\release"
#endif

#define AppName "Open MSI Control"
#define AppExeName "open-msi-control.exe"
; Same name as crates/app/src/autostart.rs (TASK_NAME).
#define AutostartTask "open-msi-control"

[Setup]
; Fixed for the lifetime of the product: upgrades and the uninstaller are matched by it.
AppId={{2D5DC448-4964-41F0-87E3-5104CEE4F57F}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=open-msi-control contributors
AppPublisherURL=https://github.com/riclenatin/open-msi-control
AppSupportURL=https://github.com/riclenatin/open-msi-control/issues
AppUpdatesURL=https://github.com/riclenatin/open-msi-control/releases
; Paths below are relative to the repository root.
SourceDir=..\..
OutputDir=dist
OutputBaseFilename=open-msi-control-{#AppVersion}-setup
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
SetupIconFile=crates\app\resources\app.ico
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
LicenseFile=LICENSE
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
; Backs up PrepareToInstall below: force-close anything still holding the installed files.
CloseApplications=force
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BinDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BinDir}\msi-probe.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
; Post-install entries run as the original (non-elevated) user, and CreateProcess cannot start
; an exe whose manifest requires elevation (ERROR_ELEVATION_REQUIRED). ShellExecute can: it
; shows the UAC prompt, and the app then runs for the signed-in user, not the installing admin.
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent shellexec

; User settings are kept on uninstall.
[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#AppExeName} /F"; Flags: runhidden waituntilterminated; RunOnceId: "StopApp"
; Fails harmlessly when "Start with Windows" was never enabled.
Filename: "{sys}\schtasks.exe"; Parameters: "/Delete /TN {#AutostartTask} /F"; Flags: runhidden waituntilterminated; RunOnceId: "DeleteAutostartTask"

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  { Stop a running instance (often hidden in the tray) before files are replaced, so the
    wizard does not stop at "applications are using files". Fails harmlessly if not running. }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM {#AppExeName} /F', '', SW_HIDE,
    ewWaitUntilTerminated, ResultCode);
  Result := '';
end;
