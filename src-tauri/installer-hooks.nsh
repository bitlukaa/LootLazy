!include LogicLib.nsh
!include FileFunc.nsh

; Delete only a caller-selected owned subtree. Never descend into junctions.
Function un.LootLazyRemoveTree
  Exch $0
  Push $1
  Push $2
  Push $3
  Push $4
  ; WebView caches can exceed the legacy Windows path length limit.
  StrCpy $4 $0 4
  ${If} $4 != "\\?\"
    StrCpy $4 $0 2
    ${If} $4 == "\\"
      StrCpy $0 $0 "" 2
      StrCpy $0 "\\?\UNC\$0"
    ${Else}
      StrCpy $0 "\\?\$0"
    ${EndIf}
  ${EndIf}
  System::Call 'kernel32::GetFileAttributesW(w r0)i.r1'
  StrCmp $1 -1 ll_tree_done
  IntOp $4 $1 & 0x10
  ${If} $4 <> 0
    IntOp $4 $1 & 0x400
    ${If} $4 = 0
      ClearErrors
      FindFirst $2 $3 "$0\*"
      IfErrors ll_tree_directory_done
      ll_tree_next:
        StrCmp $3 "" ll_tree_close
        StrCmp $3 "." ll_tree_advance
        StrCmp $3 ".." ll_tree_advance
        Push "$0\$3"
        Call un.LootLazyRemoveTree
        ll_tree_advance:
        ClearErrors
        FindNext $2 $3
        IfErrors ll_tree_close
        Goto ll_tree_next
      ll_tree_close:
      FindClose $2
    ${EndIf}
    ll_tree_directory_done:
    RMDir "$0"
  ${Else}
    IntOp $4 $1 & 0x400
    ${If} $4 = 0
      SetFileAttributes "$0" NORMAL
    ${EndIf}
    Delete "$0"
  ${EndIf}
  ll_tree_done:
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

!macro NSIS_HOOK_POSTUNINSTALL
  Push $R7
  Push $R8
  Push $R9
  ; /UPDATE reinstalls must preserve all runtime data and staged updates.
  ${If} $UpdateMode = 1
    Goto ll_cleanup_done
  ${EndIf}
  ${If} $INSTDIR == ""
    Goto ll_cleanup_invalid
  ${EndIf}
  GetFullPathName $R8 "$INSTDIR"
  ${GetRoot} "$R8" $R9
  ${If} $R8 == "$R9\"
    Goto ll_cleanup_invalid
  ${EndIf}
  System::Call 'kernel32::GetFileAttributesW(w "$R8")i.s'
  Pop $R9
  ${If} $R9 <> -1
    IntOp $R9 $R9 & 0x400
    ${If} $R9 <> 0
      Goto ll_cleanup_invalid
    ${EndIf}
  ${EndIf}
  ${If} $DeleteAppDataCheckboxState = 1
    DetailPrint "Removing application data..."
  ${EndIf}
  StrCpy $R7 0
  ll_cleanup_retry:
  Delete "$INSTDIR\LootLazy.exe.pending"
  Delete "$INSTDIR\LootLazy.exe.sig"
  Delete "$INSTDIR\LootLazy.ico"
  Delete "$INSTDIR\CHANGELOG.md"
  StrCpy $R9 0
  ${If} $DeleteAppDataCheckboxState = 1
    Push "$INSTDIR\data"
    Call un.LootLazyRemoveTree
    Push "$INSTDIR\games"
    Call un.LootLazyRemoveTree
    IfFileExists "$INSTDIR\data" 0 +2
      StrCpy $R9 1
    IfFileExists "$INSTDIR\games" 0 +2
      StrCpy $R9 1
  ${EndIf}
  IfFileExists "$INSTDIR\LootLazy.exe.pending" 0 +2
    StrCpy $R9 1
  IfFileExists "$INSTDIR\LootLazy.exe.sig" 0 +2
    StrCpy $R9 1
  IfFileExists "$INSTDIR\LootLazy.ico" 0 +2
    StrCpy $R9 1
  IfFileExists "$INSTDIR\CHANGELOG.md" 0 +2
    StrCpy $R9 1
  ${If} $R9 = 1
  ${AndIf} $R7 < 20
    IntOp $R7 $R7 + 1
    Sleep 250
    Goto ll_cleanup_retry
  ${EndIf}
  ; Remove an empty install folder, while retaining unrelated user files.
  RMDir "$INSTDIR"
  ${If} $R9 = 1
    SetErrorLevel 2
    MessageBox MB_OK|MB_ICONEXCLAMATION "Some LootLazy files could not be removed. Close programs using $INSTDIR and check folder access, then remove the remaining LootLazy files." /SD IDOK
  ${EndIf}
  Goto ll_cleanup_done
  ll_cleanup_invalid:
  SetErrorLevel 2
  MessageBox MB_OK|MB_ICONEXCLAMATION "LootLazy could not safely remove data from this installation path." /SD IDOK
  ll_cleanup_done:
  Pop $R9
  Pop $R8
  Pop $R7
!macroend
