; The values the 0.26g launcher (build 2777) read from the installer's
; registry key, worked out from where the launcher itself is.
;
;   nasm -f bin -DCAVE=0x49A000 launcher_values_026g.asm -o launcher_values_026g.bin
;
; The launcher reads five strings of HKLM\Software\Vostok Games\Survarium
; through registry_key::get_string (0x14027): `file to execute`, `arguments`
; and `working folder` when PLAY is pressed, `file to execute` again to read
; the game's version out of it, `update server` before it starts the updater
; and `launcher` when it restarts itself. Only the game's installer wrote
; them. A game that was unpacked has none, and the launcher then shows
; "Error" and never enables PLAY.
;
; patch_launcher_026g.py makes the first seven bytes of get_string a jump to
; `hook` below. For those five names the hook fills the caller's buffer
; itself; every other name goes on to the original code and the registry.
;
;   file to execute   <the launcher's folder>\<exe_name>
;   working folder    <the launcher's folder>
;   launcher          the launcher's own path
;   arguments         <arguments>
;   update server     <server>
;
; exe_name, arguments and server are the three zero-filled slots at the end,
; ASCII with a zero at the end, filled in by patch_launcher_026g.py.
;
; registry_key::get_string, as its callers use it (launcher-026g.md, 2 and 6):
;   edi          the buffer, UTF-16
;   esi          its size in characters
;   [esp+4]      the open key (not used here)
;   [esp+8]      the value's name, UTF-16
;   returns al = 1 when the buffer holds the string, 0 when not; `ret 8`.
;
; The code never names an address of its own: it finds where it was loaded
; (call/pop) and adds that to the two image addresses it uses, so it also
; works when Windows or Wine loads the launcher at another base.
;
; `enter_key`, after it, makes Enter press PLAY: patch_launcher_026g.py
; makes the first instruction of the message loop's body a jump to it (see
; there).

bits 32
org CAVE

GET_STRING_BACK         equ 0x1402E     ; get_string after its first 7 bytes
IAT_GetModuleFileNameW  equ 0x3B0BC

EXE_NAME_SIZE           equ 32
ARGUMENTS_SIZE          equ 160
SERVER_SIZE             equ 64

%define NAME(first, third) (((first) << 8) | (third))

hook:
        push    ebx
        call    .here
.here:  pop     ebx
        sub     ebx, .here              ; ebx = where it is loaded - CAVE's image
        mov     eax, [esp+12]           ; the value's name
        movzx   ecx, word [eax]
        movzx   edx, word [eax+4]
        shl     ecx, 8
        or      ecx, edx                ; its first and third letters
        cmp     ecx, NAME('f', 'l')     ; "file to execute" ("first install version" is f, r)
        je      .exe
        cmp     ecx, NAME('w', 'r')     ; "working folder"
        je      .folder
        cmp     ecx, NAME('l', 'u')     ; "launcher"
        je      .self
        cmp     ecx, NAME('a', 'g')     ; "arguments"
        je      .arguments
        cmp     ecx, NAME('u', 'd')     ; "update server" ("UninstallString" has a capital U)
        je      .server

        ; Any other name: the instructions the jump replaced, then the rest
        ; of get_string. (Its `push ecx` only makes room for a local.)
        pop     ebx
        push    ebp
        mov     ebp, esp
        push    ecx
        lea     eax, [esi+esi]
        jmp     GET_STRING_BACK

.exe:   push    1
        jmp     .path
.folder:
        push    2
        jmp     .path
.self:  push    0
.path:  push    esi                     ; GetModuleFileNameW(NULL, buffer, size)
        push    edi
        push    0
        call    [ebx + IAT_GetModuleFileNameW]
        pop     edx                     ; 1, 2 or 0, pushed above
        test    eax, eax
        jz      .fail
        cmp     eax, esi                ; the size itself: the path was cut
        jae     .fail
        test    edx, edx
        jz      .ok                     ; "launcher": the whole path
        lea     ecx, [edi+eax*2]
.back:  sub     ecx, 2                  ; back to the last backslash
        cmp     ecx, edi
        jb      .fail
        cmp     word [ecx], 0x5C
        jne     .back
        cmp     edx, 2
        jne     .name
        mov     word [ecx], 0           ; "working folder": cut there
        jmp     .ok
.name:  lea     eax, [ecx+2]            ; "file to execute": the name goes after it
        mov     edx, eax
        sub     edx, edi
        shr     edx, 1                  ; characters before the name
        add     edx, EXE_NAME_SIZE
        cmp     edx, esi
        ja      .fail
        lea     edx, [ebx + exe_name]
        jmp     .copy

.arguments:
        lea     edx, [ebx + arguments]
        mov     ecx, ARGUMENTS_SIZE
        jmp     .whole
.server:
        lea     edx, [ebx + server]
        mov     ecx, SERVER_SIZE
.whole: cmp     esi, ecx                ; room for the whole slot
        jb      .fail
        mov     eax, edi

.copy:  movzx   ecx, byte [edx]         ; ASCII at edx to UTF-16 at eax, zero included
        mov     [eax], cx
        inc     edx
        add     eax, 2
        test    ecx, ecx
        jnz     .copy
.ok:    mov     al, 1
        jmp     .done
.fail:  xor     al, al
.done:  pop     ebx
        ret     8

; Enter presses PLAY. launcher::execute (0x14DB1) runs the window's message
; loop itself, with the launcher in edi and the MSG at [ebp-20h]: hwnd,
; message, wParam, lParam. Its body starts at 0x14EF0 with
; `cmp dword [ebp-1Ch], 113h` (WM_TIMER), which patch_launcher_026g.py makes
; a jump here. The key is looked at before TranslateMessage, so it works
; wherever the focus is in the window, the server's page included (the
; embedded browser would otherwise take it).
;
; PLAY is the image button at form+620h; while it is enabled its click
; callback (form+638h) is launch_survarium (0x1B893) with the form as its
; context (form+63Ch), and while it is disabled both are 0
; (launcher_form::set_play_button_active, 0x1B5E9). Enter calls that
; callback as a click does (0x1275B), so it does exactly what PLAY does and
; nothing while PLAY is off. A held Enter (lParam bit 30, the key was down
; already) presses it once. Every other message, and Enter while PLAY is
; off, goes on to the loop as before.

LOOP_TIMER_JNE          equ 0x14EF7     ; `jne` after the replaced `cmp`
LOOP_PROCESSED          equ 0x14F26     ; launcher::on_message_processed, then GetMessageW
LAUNCHER_FORM           equ 0x2DC4      ; launcher: the form shown
PLAY_CALLBACK           equ 0x638       ; form: PLAY's click callback, 0 while disabled
PLAY_CONTEXT            equ 0x63C       ; form: its argument
WM_KEYDOWN              equ 0x100
VK_RETURN               equ 0x0D

        align   16, db 0xCC
enter_key:
        cmp     dword [ebp-1Ch], WM_KEYDOWN
        jne     .other
        cmp     dword [ebp-18h], VK_RETURN
        jne     .other
        test    dword [ebp-14h], 1 << 30        ; a repeat of a held key
        jnz     .other
        mov     eax, [edi+LAUNCHER_FORM]
        test    eax, eax
        jz      .other
        mov     ecx, [eax+PLAY_CALLBACK]
        test    ecx, ecx
        jz      .other
        push    dword [eax+PLAY_CONTEXT]
        call    ecx
        pop     ecx
        jmp     LOOP_PROCESSED                  ; the key is not handed on

.other: cmp     dword [ebp-1Ch], 113h           ; the replaced instruction
        jmp     LOOP_TIMER_JNE

        align   16, db 0xCC
exe_name:
        times EXE_NAME_SIZE db 0
arguments:
        times ARGUMENTS_SIZE db 0
server:
        times SERVER_SIZE db 0

; refresh_status indexes four install format strings with pipe ids 6..9.
; State messages set the id to 2. A filename left by an earlier install
; then made the next install read a string id from unrelated stack locals.
install_status:
        mov     eax, [esi+0x51EC]
        cmp     eax, 6
        jb      .generic
        cmp     eax, 9
        ja      .generic
        jmp     0x16D0C
.generic:
        ; Bound and temporary destination are already on the stack. Use
        ; the actual status buffer, as the native generic path at 16CED.
        lea     eax, [esp+0x38]
        mov     [esp], eax
        push    dword 0xA9
        call    0x137A8
        jmp     0x16D4B

; PLAY when Windows refuses the game exe. on_play_button_clicked (0x17160)
; starts the game with CreateProcessW at 0x17216 and, when that fails,
; raises the assertion "Can't start the game" and ends the launcher. With
; error 4551 (ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION) the cause is Smart
; App Control: it blocks an unsigned exe that Windows has not seen often
; enough, which a new revision's exe is. patch_launcher_026g.py makes
; `test eax,eax / je 17235h / push dword [ebp-20h]` after the call a jump
; here and two nops. A started game goes back as before. For 4551 a box
; says what to do, in the launcher's language, and the launcher stays open
; with PLAY on (freeing the command line as the end of the function does,
; without its PostQuitMessage). Any other error goes to the assertion as
; before. esi is the launcher, ebx 0 (the function's own registers).

PLAY_STARTED            equ 0x17223     ; CreateProcessW worked: close the handles, quit
PLAY_ASSERT             equ 0x17235     ; the assertion "Can't start the game"
PLAY_RETURN             equ 0x172DC     ; free the command line, return
LAUNCHER_LANGUAGE       equ 0x1A64      ; launcher: 0 English .. 4 Polish
IAT_GetLastError        equ 0x3B218
IAT_MessageBoxW         equ 0x3B388
ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION equ 4551
MB_BOX                  equ 0x12030     ; MB_ICONWARNING | MB_TASKMODAL | MB_SETFOREGROUND
LANGUAGES               equ 5
BLOCKED_TEXT_SIZE       equ 512         ; bytes per language: 255 UTF-16 characters and a zero

        align   16, db 0xCC
play_started:
        test    eax, eax
        jz      .failed
        push    dword [ebp-20h]                 ; the replaced instructions
        jmp     PLAY_STARTED
.failed:
        call    .here
.here:  pop     edx
        sub     edx, .here                      ; edx = where it is loaded - CAVE's image
        push    edx
        call    [edx + IAT_GetLastError]
        pop     edx
        cmp     eax, ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION
        jne     PLAY_ASSERT
        mov     eax, [esi + LAUNCHER_LANGUAGE]
        cmp     eax, LANGUAGES
        jb      .language
        xor     eax, eax                        ; another one: English
.language:
        imul    eax, eax, BLOCKED_TEXT_SIZE
        push    MB_BOX
        lea     ecx, [edx + blocked_title]
        push    ecx
        lea     ecx, [edx + blocked_texts + eax]
        push    ecx
        push    0
        call    [edx + IAT_MessageBoxW]
        jmp     PLAY_RETURN

        align   2, db 0
blocked_title:
        dw      __utf16__('Skhid'), 0
; One zero-filled UTF-16 slot per language, filled in by patch_launcher_026g.py.
blocked_texts:
        times LANGUAGES * BLOCKED_TEXT_SIZE db 0
