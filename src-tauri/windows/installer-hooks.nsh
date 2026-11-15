; Included by Tauri's NSIS template (bundle.windows.nsis.installerHooks).

; The Welcome and Finish titles get three lines, not two: "Completing Cogit Setup" in MUI2's
; 12 pt bold wrapped past the second line and was cut at large scaling.
!define MUI_WELCOMEFINISHPAGE_TITLE_3LINES

; The header bitmap has the icon at its right edge: show it there, the page title on the left.
!define MUI_HEADERIMAGE_RIGHT

; F-946: "Open in Cogit" on a folder and on a folder's background, and the install folder on
; PATH. This installer is per-user (installMode currentUser), so both go under HKCU; the MSI
; writes the same under HKLM (wix-integration.wxs). Macros, not defines: this file is included
; before MAINBINARYNAME is defined, and a macro body is read where it is inserted.
!include WinMessages.nsh

!macro COGIT_WRITE_MENU KEY
  WriteRegStr HKCU "Software\Classes\${KEY}\shell\Cogit" "" "Open in Cogit"
  WriteRegStr HKCU "Software\Classes\${KEY}\shell\Cogit" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr HKCU "Software\Classes\${KEY}\shell\Cogit\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%V$\""
!macroend

; Only an entry that runs this install's cogit.exe goes: another copy's stays.
!macro COGIT_DELETE_MENU KEY
  ReadRegStr $R0 HKCU "Software\Classes\${KEY}\shell\Cogit\command" ""
  ${If} $R0 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%V$\""
    DeleteRegKey HKCU "Software\Classes\${KEY}\shell\Cogit"
  ${EndIf}
!macroend

; PATH is not edited here: an NSIS string holds 1024 characters (NSIS_MAX_STRLEN), so reading
; a longer PATH into a variable, appending and writing it back would cut it, and the EnVar
; plugin has the same limit. cogit.exe edits HKCU\Environment\Path itself, whole and with its
; REG_EXPAND_SZ type (crates/desktop_entry/src/path_env.rs); the installer then tells running
; programs (Explorer) that the environment changed. A failure leaves PATH as it was and the
; install goes on: PATH is a convenience, Cogit runs without it.
!macro COGIT_PATH FLAG
  ClearErrors
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" ${FLAG}' $R0
  ${If} ${Errors}
  ${OrIf} $R0 <> 0
    DetailPrint "Cogit could not change the user PATH ($R0)"
  ${EndIf}
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro COGIT_WRITE_MENU "Directory"
  !insertmacro COGIT_WRITE_MENU "Directory\Background"
  !insertmacro COGIT_PATH "--add-to-path"
!macroend

; Before the files go: cogit.exe is still there to take its folder off PATH.
!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro COGIT_PATH "--remove-from-path"
  !insertmacro COGIT_DELETE_MENU "Directory"
  !insertmacro COGIT_DELETE_MENU "Directory\Background"
!macroend
