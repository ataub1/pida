; PIDA – Ergänzung des NSIS-Installers (tauri.conf.json › bundle.windows.nsis.installerHooks; Tauri bindet die Datei
; am Anfang von installer.nsi ein, vor den Seiten). ADR 0004 Nachtrag 21, geprüft von tests/bundle_metadata.rs.
;
; Herausgeber „pida“ → „A-SIT“: Bis 0.2.0 stand kein bundle.publisher in tauri.conf.json; Tauri nahm „pida“ (zweiter
; Teil der Kennung at.pida) und merkte sich den Installationsordner unter Software\pida\PIDA (HKCU, Installation für
; den aktuellen Benutzer). Jetzt heißt der Schlüssel Software\A-SIT\PIDA. Ohne Übernahme riefe „Vor der Installation
; deinstallieren“ (Vorgabe beim Aktualisieren) das alte Deinstallationsprogramm mit leerem Ordner auf (`_?=`) – es
; bricht mit einer Fehlermeldung ab, die Aktualisierung bliebe auf der Seite „Bereits installiert“ stehen –, und ein
; eigener Installationsordner ginge verloren. Deshalb übernimmt der Installer beim Start der Oberfläche (vor dieser
; Seite) den alten Ordner, solange es den neuen Schlüssel nicht gibt. Den alten Schlüssel entfernt das alte
; Deinstallationsprogramm selbst. Ohne Oberfläche (/S) gibt es die Seite nicht; installiert wird dann wie bisher in den
; Standardordner.

!define MUI_CUSTOMFUNCTION_GUIINIT PidaAdoptLegacyPublisherKey

Function PidaAdoptLegacyPublisherKey
  Push $0
  ReadRegStr $0 SHCTX "Software\A-SIT\PIDA" ""
  StrCmp $0 "" 0 pida_adopt_done
  ReadRegStr $0 SHCTX "Software\pida\PIDA" ""
  StrCmp $0 "" pida_adopt_done 0
  WriteRegStr SHCTX "Software\A-SIT\PIDA" "" $0
  ; nur statt des Standardordners (ein Ordner per /D= bleibt)
  StrCmp $INSTDIR "$LOCALAPPDATA\PIDA" 0 pida_adopt_done
  StrCpy $INSTDIR $0
  pida_adopt_done:
  Pop $0
FunctionEnd

; Läuft PIDA noch? (Setup und Deinstallation, vor dem Kopieren bzw. Löschen; tauri-apps/tauri#16277)
;
; Seit Tauri 2.12 prüft die Vorlage über den Restart Manager, ob „$INSTDIR\pida.exe“ in Benutzung ist, und bricht ab,
; sobald RmShutdown nicht 0 liefert – auch wenn PIDA gerade beendet wurde (der Prozess ist noch auf dem Weg hinaus),
; Windows den Zugriff verweigert (anderes Konto, erhöhte Rechte, Virenscanner) oder gar kein PIDA mehr läuft und nur
; ein anderer Prozess die Datei kurz offen hält. Dann blieb die Deinstallation mit „Failed to kill PIDA“ stehen,
; obwohl PIDA geschlossen war. Diese Fassung ersetzt die Prüfung der Vorlage (utils.nsh ist vorher eingebunden):
;
; 1. Datei in Benutzung (Restart Manager, wie die Vorlage)? Sonst weiter.
; 2. Läuft ein Prozess „pida.exe“? Dann einmal fragen wie die Vorlage (außer still bzw. passiv), beenden über den
;    Restart Manager und – wie bis Tauri 2.11 – mit KillProcessCurrentUser.
; 3. Bis zu 10 s warten, bis die Datei frei ist (beendender Prozess, kurz geöffnete Datei).
; 4. Abbrechen nur, wenn danach noch ein PIDA-Prozess läuft; hält nur noch ein anderer Prozess die Datei, geht es
;    weiter (wie vor Tauri 2.12).

!macro PidaExeInUse executablePath result
  StrCpy ${result} 0
  !insertmacro RestartManager_StartSession $R0
  ${If} $R0 != ""
    !insertmacro RestartManager_RegisterFile $R0 "${executablePath}"
    ${If} $0 = 0
      System::Call 'RSTRTMGR::RmGetList(p R0, *i .r1, *i .r2, p 0, *i .r3) i .r0'
      ; ERROR_MORE_DATA: mindestens ein Prozess nutzt die Datei
      ${If} $0 = ${ERROR_MORE_DATA}
        StrCpy ${result} 1
      ${EndIf}
    ${EndIf}
    !insertmacro RestartManager_EndSession $R0
  ${EndIf}
!macroend

!macro PidaRmShutdown executablePath
  !insertmacro RestartManager_StartSession $R0
  ${If} $R0 != ""
    !insertmacro RestartManager_RegisterFile $R0 "${executablePath}"
    ${If} $0 = 0
      System::Call 'RSTRTMGR::RmShutdown(p R0, i ${RmForceShutdown}, p 0) i .r0'
    ${EndIf}
    !insertmacro RestartManager_EndSession $R0
  ${EndIf}
!macroend

!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executablePath productName
  !define UniqueID ${__LINE__}

  nsis_tauri_utils::StrReplace "$(appRunning)" "{{product_name}}" "${productName}"
  Pop $R1
  nsis_tauri_utils::StrReplace "$(appRunningOkKill)" "{{product_name}}" "${productName}"
  Pop $R2
  nsis_tauri_utils::StrReplace "$(failedToKillApp)" "{{product_name}}" "${productName}"
  Pop $R3

  ; $R4: Wartezeit in Schritten zu 250 ms, $R7: schon gefragt und beendet
  StrCpy $R4 0
  StrCpy $R7 0
  pida_check_${UniqueID}:
    !insertmacro PidaExeInUse "${executablePath}" $R5
    ${If} $R5 = 0
      Goto pida_done_${UniqueID}
    ${EndIf}
    ${If} $R7 = 0
      ; FindProcess: 0 = ein Prozess dieses Namens läuft
      nsis_tauri_utils::FindProcess "${MAINBINARYNAME}.exe"
      Pop $R6
      ${If} $R6 = 0
        StrCpy $R7 1
        IfSilent pida_kill_${UniqueID} 0
        ${IfThen} $PassiveMode != 1 ${|} MessageBox MB_OKCANCEL $R2 IDOK pida_kill_${UniqueID} IDCANCEL pida_cancel_${UniqueID} ${|}
        pida_kill_${UniqueID}:
          !insertmacro PidaRmShutdown "${executablePath}"
          nsis_tauri_utils::KillProcessCurrentUser "${MAINBINARYNAME}.exe"
          Pop $R6
      ${EndIf}
    ${EndIf}
    ${If} $R4 < 40
      IntOp $R4 $R4 + 1
      Sleep 250
      Goto pida_check_${UniqueID}
    ${EndIf}
    ; nach 10 s noch belegt: nur abbrechen, wenn PIDA selbst noch läuft
    nsis_tauri_utils::FindProcess "${MAINBINARYNAME}.exe"
    Pop $R6
    ${If} $R6 != 0
      Goto pida_done_${UniqueID}
    ${EndIf}
    IfSilent pida_silent_${UniqueID} pida_ui_${UniqueID}
    pida_silent_${UniqueID}:
      System::Call 'kernel32::AttachConsole(i -1)i.r0'
      ${If} $0 != 0
        System::Call 'kernel32::GetStdHandle(i -11)i.r0'
        System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)'
        FileWrite $0 "$R1$\n"
      ${EndIf}
      Abort
    pida_ui_${UniqueID}:
      Abort $R3
  pida_cancel_${UniqueID}:
    Abort $R1
  pida_done_${UniqueID}:

  !undef UniqueID
!macroend
