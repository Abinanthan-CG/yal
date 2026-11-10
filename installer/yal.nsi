; YAL - NSIS installer.
;
; Built by `cargo xtask dist`, which passes VERSION, ARCH, NSIS_ARCH, CHANNEL
; and OUTFILE on the command line.
;
; The install is per user: it never asks for administrator rights, and it never
; writes outside the user's own profile.

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

; NSIS resolves relative paths against this script's own directory, which is
; `installer/`. The staged files and the icon live in the repository root, one
; level up, so every relative path here starts with `..`.

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef ARCH
  !define ARCH "x64"
!endif
!ifndef NSIS_ARCH
  !define NSIS_ARCH "x64"
!endif
!ifndef CHANNEL
  !define CHANNEL "stable"
!endif
!ifndef OUTFILE
  !define OUTFILE "yal-setup-${VERSION}-${ARCH}.exe"
!endif
!ifndef STAGE
  !define STAGE "..\dist\stage-${ARCH}"
!endif

!define PRODUCT "YAL"
!define PUBLISHER "YAL community port"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\YAL-${CHANNEL}"
!define PROTOCOL "tinycast"

; A line break in a message box.
!define NL "\r\n"

Name "${PRODUCT} ${VERSION} (${ARCH})"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\yal"
InstallDirRegKey HKCU "${UNINSTKEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=1033 "FileDescription" "${PRODUCT} launcher"
VIAddVersionKey /LANG=1033 "LegalCopyright" "AGPL-3.0-only"
VIAddVersionKey /LANG=1033 "OriginalFilename" "yal.exe"

!define MUI_ABORTWARNING
!define MUI_ICON "..\assets\yal.ico"
!define MUI_UNICON "..\assets\yal.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\yal.exe"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "YAL" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; The protocol is registered per user, so no administrator rights are needed
  ; and the app can be launched from a link in any browser.
  WriteRegStr HKCU "Software\Classes\${PROTOCOL}" "" "URL:${PRODUCT} Protocol"
  WriteRegStr HKCU "Software\Classes\${PROTOCOL}" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\${PROTOCOL}\DefaultIcon" "" "$INSTDIR\yal.exe,0"
  WriteRegStr HKCU "Software\Classes\${PROTOCOL}\shell\open\command" "" '"$INSTDIR\yal.exe" "%1"'

  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\yal.exe,0"
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1

  WriteRegStr HKCU "Software\YAL" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\YAL" "Version" "${VERSION}"
  WriteRegStr HKCU "Software\YAL" "Channel" "${CHANNEL}"
SectionEnd

Section "Start menu shortcut" SecStartMenu
  CreateDirectory "$SMPROGRAMS\YAL"
  CreateShortcut "$SMPROGRAMS\YAL\YAL.lnk" "$INSTDIR\yal.exe" "" "$INSTDIR\yal.exe" 0
  CreateShortcut "$SMPROGRAMS\YAL\Uninstall YAL.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\YAL.lnk" "$INSTDIR\yal.exe" "" "$INSTDIR\yal.exe" 0
SectionEnd

Section "Launch at login" SecAutostart
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "YAL" '"$INSTDIR\yal.exe" --autostart'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "The YAL launcher and its helper processes."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecStartMenu} "Adds YAL to the Start menu."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Adds a YAL shortcut to the desktop."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutostart} "Starts YAL when you sign in."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInit
  ; A running instance holds the portable data folder, so close it first.
  IfFileExists "$INSTDIR\yal.exe" 0 +2
  ExecWait '"$INSTDIR\yal.exe" --quit-running' $0
FunctionEnd

Function un.onInit
  ; The uninstaller must not delete the files it is running from.
  SetShellVarContext current
FunctionEnd

Section "Uninstall"
  ; The program files go. The user's data stays unless they ask for it to go,
  ; because losing a configuration is worse than leaving a small folder behind.
  Delete "$INSTDIR\yal.exe"
  Delete "$INSTDIR\yal-indexer.exe"
  Delete "$INSTDIR\yal-ocr.exe"
  Delete "$INSTDIR\yal-dictation.exe"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$INSTDIR\portable"
  Delete "$INSTDIR\yal.ico"
  Delete "$INSTDIR\yal.png"
  RMDir /r "$INSTDIR\locales"
  RMDir /r "$INSTDIR\resources"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\YAL\YAL.lnk"
  Delete "$SMPROGRAMS\YAL\Uninstall YAL.lnk"
  RMDir "$SMPROGRAMS\YAL"
  Delete "$DESKTOP\YAL.lnk"

  DeleteRegKey HKCU "Software\Classes\${PROTOCOL}"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "YAL"

  ; The default answer is "keep the data", both for a silent uninstall and
  ; for a user who presses Enter without reading. Losing a configuration is
  ; worse than leaving a small folder behind.
  MessageBox MB_YESNO "Remove your YAL data as well?${NL}${NL}Your settings, quicklinks, snippets and history live in %APPDATA%\yal. Choosing No keeps them for the next install." /SD IDNO IDYES remove_data IDNO keep_data
  remove_data:
    RMDir /r "$APPDATA\yal"
    RMDir /r "$APPDATA\YAL"
    RMDir /r "$LOCALAPPDATA\yal"
    RMDir /r "$LOCALAPPDATA\YAL"
    Goto done
  keep_data:
  done:
  DeleteRegKey HKCU "Software\YAL"
SectionEnd
