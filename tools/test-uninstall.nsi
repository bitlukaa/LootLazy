Unicode true
!include LogicLib.nsh
!include FileFunc.nsh
!include WinMessages.nsh
!include "${HOOK_PATH}"
Name "LootLazy cleanup fixture"
OutFile "${OUTPUT_PATH}"
RequestExecutionLevel user
SilentInstall silent
SilentUnInstall silent
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
Var UpdateMode
Function un.ConfirmShow
  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1
FunctionEnd
Function un.onInit
  ${GetParameters} $0
  ${GetOptions} $0 "/DELETE=" $1
  ${IfNot} ${Errors}
    StrCpy $DeleteAppDataCheckboxState $1
  ${EndIf}
  ${GetOptions} $0 "/UPDATE=" $UpdateMode
FunctionEnd
Section
  WriteUninstaller "$EXEDIR\probe-uninstall.exe"
SectionEnd
Section Uninstall
  Delete "$INSTDIR\LootLazy.exe"
  Delete "$INSTDIR\README.md"
  !insertmacro NSIS_HOOK_POSTUNINSTALL
SectionEnd
