; Compile with ISCC /DAppVersion=X.Y.Z /DBinaryPath=... /DOutputPath=... reclaim.iss
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef BinaryPath
  #error BinaryPath is required
#endif
#ifndef OutputPath
  #define OutputPath "..\..\dist"
#endif
[Setup]
AppId={{0EC19857-83F3-4B19-9239-A900C15E1661}
AppName=Reclaim
AppVersion={#AppVersion}
AppPublisher=Reclaim contributors
AppPublisherURL=https://github.com/sudo-py-dev/reclaim
AppUpdatesURL=https://github.com/sudo-py-dev/reclaim/releases
DefaultDirName={localappdata}\Programs\Reclaim
DefaultGroupName=Reclaim
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputPath}
OutputBaseFilename=reclaim-windows-x86_64-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\..\assets\icons\reclaim.ico
LicenseFile=..\..\LICENSE
UninstallDisplayIcon={app}\reclaim.exe
CloseApplications=yes
RestartApplications=no
SetupMutex=ReclaimSetup
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
[Files]
Source: "{#BinaryPath}"; DestDir: "{app}"; DestName: "reclaim.exe"; Flags: ignoreversion
Source: "installed-by-inno"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\fonts\DejaVu-LICENSE.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\..\assets\fonts\FontAwesome-LICENSE.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
[Icons]
Name: "{group}\Reclaim"; Filename: "{app}\reclaim.exe"
Name: "{autodesktop}\Reclaim"; Filename: "{app}\reclaim.exe"; Tasks: desktopicon
[Run]
Filename: "{app}\reclaim.exe"; Description: "Launch Reclaim"; Flags: nowait postinstall skipifsilent
