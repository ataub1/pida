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
