; Optional TTS information only: no download or consent during installation.
!define GLAGOL_ICON_SOURCE "${__FILEDIR__}\..\icons\icon.ico"
Page custom GlagolComponentsPage

Function GlagolComponentsPage
  Call SkipIfPassive
  IfSilent 0 +2
    Abort
  !insertmacro MUI_HEADER_TEXT "Glagol" "Dictation / Диктовка · TTS / Озвучка"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${If} $LANGUAGE == 1049
    ${NSD_CreateLabel} 0 0 100% 135u "Глагол можно использовать для диктовки, в том числе в организации и через собственный офисный сервер.$\r$\n$\r$\nОзвучка Silero TTS v5.5 — дополнительный компонент под лицензией CC BY-NC-SA 4.0 для некоммерческого использования.$\r$\n$\r$\nМодель и движок скачиваются отдельно, только по вашему выбору в настройках. Для диктовки Silero не нужна.$\r$\n$\r$\nСам Глагол распространяется под MIT. Условия выбранного провайдера диктовки действуют отдельно."
  ${Else}
    ${NSD_CreateLabel} 0 0 100% 135u "Glagol supports dictation, including organization use with your own office server.$\r$\n$\r$\nSilero TTS v5.5 is an optional component under CC BY-NC-SA 4.0 for noncommercial use.$\r$\n$\r$\nThe model and runtime download separately only when you choose them in Settings. Dictation does not require Silero.$\r$\n$\r$\nGlagol itself is MIT licensed. Your chosen dictation provider's terms apply separately."
  ${EndIf}
  Pop $0
  nsDialogs::Show
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File /oname=glagol-${VERSION}.ico "${GLAGOL_ICON_SOURCE}"
  ; A versioned explicit icon bypasses stale Windows shortcut icon cache.
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\glagol-${VERSION}.ico"
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
    CreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\glagol-${VERSION}.ico"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${EndIf}
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\glagol-${VERSION}.ico"
!macroend
