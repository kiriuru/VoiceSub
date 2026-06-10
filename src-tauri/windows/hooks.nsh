; VoiceSub NSIS hooks — runtime folders are created by the app, never by the installer.

; Keep user settings across upgrades and uninstall (user-data/, logs/).



!macro NSIS_HOOK_PREINSTALL

  ; Ensure target exists before file copy (RequestExecutionLevel user, no UAC).

  CreateDirectory "$INSTDIR"

!macroend



!macro NSIS_HOOK_POSTINSTALL

  ; Runtime dirs: app also creates these on first start; pre-create + ACL for writability.

  CreateDirectory "$INSTDIR\user-data"

  CreateDirectory "$INSTDIR\logs"

  ClearErrors

  ; Users (S-1-5-32-545): modify on runtime folders — SamLogic/MS pattern for shared install dir.

  ExecWait '"$SYSDIR\cmd.exe" /c icacls "$INSTDIR\user-data" /grant *S-1-5-32-545:(OI)(CI)M /C' $0

  ClearErrors

  ExecWait '"$SYSDIR\cmd.exe" /c icacls "$INSTDIR\logs" /grant *S-1-5-32-545:(OI)(CI)M /C' $0

  ClearErrors

  !insertmacro VoiceSubVerifyWebView2Runtime

!macroend



!macro VoiceSubVerifyWebView2Runtime

  StrCpy $R9 ""

  ${If} ${RunningX64}

    ReadRegStr $R9 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"

  ${EndIf}

  ${If} $R9 == ""

    ReadRegStr $R9 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"

  ${EndIf}

  ${If} $R9 == ""

    ReadRegStr $R9 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"

  ${EndIf}

  ${If} $R9 == ""

    MessageBox MB_ICONEXCLAMATION|MB_OK "$(voicesubWebView2Missing)"

  ${EndIf}

!macroend



!macro NSIS_HOOK_PREUNINSTALL

  ; Prevent recursive removal of persisted runtime data if the uninstaller

  ; would otherwise clear the install directory during upgrade/reinstall.

!macroend



!macro NSIS_HOOK_POSTUNINSTALL

  ; Intentionally empty: do not delete $INSTDIR\user-data or $INSTDIR\logs.

!macroend


