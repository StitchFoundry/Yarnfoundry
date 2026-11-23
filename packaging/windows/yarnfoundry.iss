; Inno Setup script for the YarnFoundry Windows installer.
; Built by packaging/windows/build.ps1, which passes /DAppVersion=x.y.z and /DSourceExe=<path>.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\target\release\yarnfoundry.exe"
#endif

[Setup]
AppId={{6B0F4D0E-3B7A-4C57-9C1E-5A0E8F2B7D41}
AppName=YarnFoundry
AppVersion={#AppVersion}
AppPublisher=StitchFoundry
AppPublisherURL=https://github.com/StitchFoundry/Yarnfoundry
DefaultDirName={autopf}\YarnFoundry
DefaultGroupName=YarnFoundry
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE-MIT
OutputDir=..\..\dist
OutputBaseFilename=YarnFoundry-{#AppVersion}-windows-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
WizardStyle=modern
UninstallDisplayName=YarnFoundry

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "yarnfoundry.exe"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\YarnFoundry"; Filename: "{app}\yarnfoundry.exe"
Name: "{autodesktop}\YarnFoundry"; Filename: "{app}\yarnfoundry.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Run]
Filename: "{app}\yarnfoundry.exe"; Description: "Launch YarnFoundry"; Flags: nowait postinstall skipifsilent
