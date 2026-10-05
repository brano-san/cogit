; Included by Tauri's NSIS template (bundle.windows.nsis.installerHooks).

; The Welcome and Finish titles get three lines, not two: "Completing Cogit Setup" in MUI2's
; 12 pt bold wrapped past the second line and was cut at large scaling.
!define MUI_WELCOMEFINISHPAGE_TITLE_3LINES

; The header bitmap has the icon at its right edge: show it there, the page title on the left.
!define MUI_HEADERIMAGE_RIGHT
