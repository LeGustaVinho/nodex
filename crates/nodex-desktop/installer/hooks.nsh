; Nodex installer hooks for Tauri's NSIS template: Explorer's "Open with Nodex" on folders and
; on a folder's background (per user, HKCU).
; Nodex keeps its data in %APPDATA%\<identifier> and %LOCALAPPDATA%\<identifier>, which the
; template's own "Delete the application data" option removes.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\Directory\shell\Nodex" "" "Open with Nodex"
  WriteRegStr HKCU "Software\Classes\Directory\shell\Nodex" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr HKCU "Software\Classes\Directory\shell\Nodex\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Nodex" "" "Open with Nodex"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Nodex" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Nodex\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%V$\""
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Nodex"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\Nodex"
!macroend
