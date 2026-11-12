; Installeur Windows de MediaStar (Inno Setup 6).
; Lancé par package.ps1, qui passe /DVersion=… /DStage=… /DOutDir=… :
;   Stage  : dossier préparé (mediastar.exe, libmpv-2.dll, MediaInfo.dll, licences\)
;   OutDir : dossier où écrire MediaStar-<version>-windows-x64-setup.exe

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Stage
  #define Stage "..\..\..\..\dist\stage-windows"
#endif
#ifndef OutDir
  #define OutDir "..\..\..\..\dist"
#endif

[Setup]
AppId={{D72A7795-3C9A-4E6F-B9A2-6D16299E760E}
AppName=MediaStar
AppVersion={#Version}
AppVerName=MediaStar {#Version}
AppPublisher=Pierrick
AppPublisherURL=https://github.com/acetayls/mediastar
DefaultDirName={autopf}\MediaStar
DefaultGroupName=MediaStar
DisableProgramGroupPage=yes
; Installation pour tous (droits administrateur) ou pour l'utilisateur seul, au choix.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutDir}
OutputBaseFilename=MediaStar-{#Version}-windows-x64-setup
SetupIconFile=..\..\assets\icons\mediastar.ico
UninstallDisplayIcon={app}\mediastar.exe
LicenseFile={#Stage}\licences\LICENCES.txt
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Fermer MediaStar s'il tourne pendant une mise à jour.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Stage}\mediastar.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\libmpv-2.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\MediaInfo.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\licences\*"; DestDir: "{app}\licences"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\MediaStar"; Filename: "{app}\mediastar.exe"
Name: "{autodesktop}\MediaStar"; Filename: "{app}\mediastar.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\mediastar.exe"; Description: "{cm:LaunchProgram,MediaStar}"; Flags: nowait postinstall skipifsilent
