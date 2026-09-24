; Optional TTS information only: no download or consent during installation.
!define GLAGOL_ICON_SOURCE "${__FILEDIR__}\..\icons\icon.ico"
Page custom GlagolComponentsPage

Function GlagolComponentsPage
  Call SkipIfPassive
  IfSilent 0 +2
    Abort
  ${If} $LANGUAGE == 1049
    !insertmacro MUI_HEADER_TEXT "Glagol" "Диктовка и озвучка EN/RU"
  ${Else}
    !insertmacro MUI_HEADER_TEXT "Glagol" "English and Russian dictation and speech"
  ${EndIf}
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${If} $LANGUAGE == 1049
    ${NSD_CreateLabel} 0 0 100% 135u "Glagol поддерживает английские и русские интерфейс, диктовку и озвучку. Их языки выбираются независимо.$\r$\n$\r$\nОбе модели озвучки Silero, RU и EN, — необязательные компоненты под CC BY-NC-SA 4.0 для некоммерческого использования.$\r$\n$\r$\nМодели и движки скачиваются отдельно по вашему выбору. Silero RU/EN используют общий runtime. Для диктовки Silero не нужна.$\r$\n$\r$\nКод Glagol — MIT. Диктовка доступна организациям; условия выбранной модели или провайдера действуют отдельно."
  ${Else}
    ${NSD_CreateLabel} 0 0 100% 135u "Glagol supports English and Russian interface, dictation and synthesis. Choose each language independently.$\r$\n$\r$\nBoth Silero TTS models, RU and EN, are optional components under CC BY-NC-SA 4.0 for noncommercial use.$\r$\n$\r$\nModels and runtimes download separately by choice. Silero RU/EN share one runtime. Dictation does not require Silero.$\r$\n$\r$\nGlagol code is MIT licensed. Dictation supports organization use; each selected model or provider has its own terms."
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
