!macro NSIS_HOOK_POSTINSTALL
!if "${PRODUCTNAME}" == "PARLO"
  ; Preserve the installer identity while using the desired capitalization in Start.
  !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    Rename "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$SMPROGRAMS\Parlo-installing.lnk"
    Rename "$SMPROGRAMS\Parlo-installing.lnk" "$SMPROGRAMS\Parlo.lnk"
  ${EndIf}

  ; Migrate an older shortcut only when it belongs to this installation.
  !insertmacro IsShortcutTarget "$SMPROGRAMS\Callnotes.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    IfFileExists "$SMPROGRAMS\Parlo.lnk" 0 +3
      Delete "$SMPROGRAMS\Callnotes.lnk"
      Goto +2
    Rename "$SMPROGRAMS\Callnotes.lnk" "$SMPROGRAMS\Parlo.lnk"
  ${EndIf}
!endif
!macroend
