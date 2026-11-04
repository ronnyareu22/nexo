; Nexo: ganchos del instalador NSIS de Tauri (bundle.windows.nsis.installerHooks en tauri.conf.json).
;
; - Al instalar (no al actualizar): una regla del Firewall de Windows para que XDrop reciba, solo por TCP 47821,
;   solo para nexo.exe y solo desde Tailscale (100.64.0.0/10). La instalación es por usuario y sin administrador,
;   así que netsh se abre con permiso (un solo aviso de UAC: borra la regla vieja y crea la nueva en un paso).
;   Si se rechaza, sigue igual: Nexo funciona y Windows pregunta la primera vez que XDrop reciba.
; - Al desinstalar (no al actualizar): quita esa regla, también con permiso; si falla, no pasa nada.
;
; Actualizar desde la Mac: Nexo abre este mismo instalador con /S /UPDATE /R. La plantilla de Tauri ya hace lo demás:
; /S en silencio y cierra el Nexo abierto (Restart Manager) antes de copiar; /UPDATE no rehace los accesos directos
; ni quita el arranque; /R vuelve a abrir Nexo al terminar. La página final ya trae "Ejecutar Nexo" marcado.

!define NEXO_REGLA "Nexo XDrop"

!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode <> 1
    DetailPrint "Permitiendo que XDrop reciba por Tailscale (puede pedir permiso)…"
    ClearErrors
    ExecShellWait "runas" "$SYSDIR\cmd.exe" '/s /c "netsh advfirewall firewall delete rule name="${NEXO_REGLA}" >nul 2>&1 & netsh advfirewall firewall add rule name="${NEXO_REGLA}" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" protocol=TCP localport=47821 remoteip=100.64.0.0/10 profile=any >nul 2>&1"' SW_HIDE
    ${If} ${Errors}
      DetailPrint "Sin la regla del firewall: Windows preguntará la primera vez que XDrop reciba."
    ${EndIf}
    ClearErrors
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ClearErrors
    ExecShellWait "runas" "$SYSDIR\netsh.exe" 'advfirewall firewall delete rule name="${NEXO_REGLA}"' SW_HIDE
    ClearErrors
  ${EndIf}
!macroend
