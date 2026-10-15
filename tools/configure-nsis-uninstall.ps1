function Set-LootLazyUninstallDefaults {
 param([Parameter(Mandatory=$true)][string]$Path)
 $text=[IO.File]::ReadAllText($Path).Replace("`r`n","`n")
 $marker='; LootLazy: delete application data by default'
 if($text.Contains($marker)){
  if(!$text.Contains('SendMessage $DeleteAppDataCheckbox ${BM_SETCHECK} 1 0')){throw 'Incomplete uninstall defaults patch'}
  return
 }
 $font='SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1'
 if([regex]::Matches($text,[regex]::Escape($font)).Count -ne 1 -or [regex]::Matches($text,'(?m)^Function un\.onInit$').Count -ne 1){throw 'NSIS template changed; review uninstall defaults before publishing'}
 $text=$text.Replace($font,$font+"`n"+'  SendMessage $DeleteAppDataCheckbox ${BM_SETCHECK} 1 0')
 $text=$text.Replace('Function un.onInit','Function un.onInit'+"`n  $marker`n"+'  StrCpy $DeleteAppDataCheckboxState 1')
 [IO.File]::WriteAllText($Path,$text)
}
