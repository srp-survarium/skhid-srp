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

; ---------------------------------------------------------------------------
; The language: the game's own choice, `localization <name>` in
; <Documents>\survarium\user.cfg (the line the game's Settings write, exe
; patch 79, docs/notes/language-option.md), is the launcher's too. Its menu
; lists every language of the game (launcher_languages_026g.inc, generated
; from localization_026g.py's LANGUAGES) and writes that line; at its start
; it shows the line's language, or, without one, the language the game will
; start in (its Windows user locale's, else English). The registry's
; `language` value is no longer read or written.
;
; settings.language (settings+1Ch, the launcher's +1A64h) is the index in
; LANGUAGES: 0..LANGUAGE_COUNT-1. Every place the launcher reads it by its
; five original languages goes through one of these, by the jumps at
; `language_entries` (patch_launcher_026g.py LANGUAGE_HOOKS):
;
;   language_label    0x19663  the menu button's label (it was the 5-entry table 0x48280)
;   language_langid   0x150B3  the LANGID of the launcher's texts (0x3E098)
;   language_page     0x1A84B  get_language_code: the page's ?lang= and index-<code>.html
;   reload_language   0x1D852  settings::reload_language: from user.cfg, not the registry
;   save_language     0x1D8DA  settings::save_language: to user.cfg, not the registry
;   pick_language     0x19A08  the button's click: a menu of every language (it opened
;                              the five-row drop-down)
;
; The game finds its folder as SHGetFolderPathA(CSIDL_PERSONAL) + "\survarium"
; (client_patch.md, the user folder); so does this, with the same function.

IAT_CloseHandle         equ 0x3B098
IAT_ReadFile            equ 0x3B0EC
IAT_WriteFile           equ 0x3B0F8
IAT_GetProcessHeap      equ 0x3B118
IAT_HeapFree            equ 0x3B238
IAT_HeapAlloc           equ 0x3B23C
IAT_CreateFileA         equ 0x3B254
IAT_GetModuleHandleA    equ 0x3B258
IAT_GetProcAddress      equ 0x3B1D8
IAT_GetLastError        equ 0x3B218
IAT_DeleteFileA         equ 0x3B110
IAT_SHGetFolderPathA    equ 0x3B2BC
IAT_SHCreateDirectoryExA equ 0x3B2C0
IAT_GetWindowRect       equ 0x3B394
IAT_GetFileAttributesW  equ 0x3B1BC
IAT_CreateFileW         equ 0x3B12C

LANGUAGE_GLOBAL         equ 0x4819C     ; the index get_language_code reads
LANGUAGE_SELECTED       equ 0x19A20     ; language_menu::language_selected({menu, index})
SETTINGS_LANGUAGE       equ 0x1C        ; settings: the index
MENU_WINDOW             equ 0x4         ; language_menu: its window
MENU_SETTINGS           equ 0x154       ; language_menu: the settings

CFG_SIZE                equ 0x10000     ; a user.cfg up to this is read (the game's is ~3 KB)
PATH_SIZE               equ 320         ; MAX_PATH and the two names after it
CSIDL_PERSONAL          equ 5
TPM_FLAGS               equ 0x180       ; TPM_RETURNCMD | TPM_NONOTIFY
MF_CHECKED              equ 8

%macro DELTA 0                          ; ebx = where the code is loaded - CAVE's image
        call    %%here
%%here: pop     ebx
        sub     ebx, %%here
%endmacro

        align   16, db 0xCC
language_entries:                       ; patch_launcher_026g.py: 8 bytes each, in this order
        jmp     near language_label
        align   8, db 0xCC
        jmp     near language_langid
        align   8, db 0xCC
        jmp     near language_page
        align   8, db 0xCC
        jmp     near reload_language
        align   8, db 0xCC
        jmp     near save_language
        align   8, db 0xCC
        jmp     near pick_language
        align   8, db 0xCC
                                        ; then TORRENT_HOOKS, in this order ("Allow torrent", at the end)
        jmp     near panel_made
        align   8, db 0xCC
        jmp     near box_made
        align   8, db 0xCC
        jmp     near panel_restored
        align   8, db 0xCC
        jmp     near torrent_click
        align   8, db 0xCC

; eax = index -> eax = its label (UTF-16). Called in place of
; `mov eax, [eax*8+48280h]`; every other register kept.
language_label:
        push    ebx
        DELTA
        cmp     eax, LANGUAGE_COUNT
        jb      .ok
        xor     eax, eax
.ok:    lea     eax, [ebx + language_labels + eax*8]
        pop     ebx
        ret

; eax = index -> esi = the LANGID of the launcher's texts. Called in place of
; `movzx esi, word [eax*2+3E098h]`; every other register kept.
language_langid:
        push    ebx
        DELTA
        mov     esi, eax
        cmp     esi, LANGUAGE_COUNT
        jb      .ok
        xor     esi, esi
.ok:    movzx   esi, word [ebx + language_ui + esi*2]
        pop     ebx
        ret

; get_language_code: eax = the code of the launcher's page (UTF-16).
language_page:
        push    ebx
        DELTA
        mov     eax, [ebx + LANGUAGE_GLOBAL]
        cmp     eax, LANGUAGE_COUNT
        jb      .ok
        xor     eax, eax
.ok:    lea     eax, [ebx + language_pages + eax*8]
        pop     ebx
        ret

; edi = a PATH_SIZE buffer, dl = 1 to make the folder -> the path of user.cfg
; in it (ANSI), eax = 1; eax = 0 when there is no Documents folder.
config_path:
        push    esi
        movzx   esi, dl
        push    edi
        push    0
        push    0
        push    CSIDL_PERSONAL
        push    0
        call    [ebx + IAT_SHGetFolderPathA]
        test    eax, eax
        jnz     .fail
        mov     ecx, edi
.end:   cmp     byte [ecx], 0
        je      .folder
        inc     ecx
        jmp     .end
.folder:
        lea     edx, [ebx + folder_name]
        call    append
        test    esi, esi
        jz      .file
        push    ecx
        push    0
        push    edi
        push    0
        call    [ebx + IAT_SHCreateDirectoryExA]        ; or it is there already
        pop     ecx
.file:  lea     edx, [ebx + file_name]
        call    append
        mov     eax, 1
        pop     esi
        ret
.fail:  xor     eax, eax
        pop     esi
        ret

; The zero-ended text at edx to ecx, its zero included; ecx = that zero.
append: mov     al, [edx]
        mov     [ecx], al
        test    al, al
        jz      .done
        inc     edx
        inc     ecx
        jmp     append
.done:  ret

; edi = the path -> esi = a CFG_SIZE+128 buffer (0: no memory), ecx = the
; bytes read, edx = 0 when that is the whole file or there is no file, 1
; when the file could not be read whole (too big, locked) or there is no
; buffer. The buffer is the caller's to free.
load_config:
        call    [ebx + IAT_GetProcessHeap]
        push    CFG_SIZE + 128
        push    0
        push    eax
        call    [ebx + IAT_HeapAlloc]
        mov     esi, eax
        xor     ecx, ecx
        mov     edx, 1
        test    eax, eax
        jz      .done
        push    0
        push    0x80                    ; FILE_ATTRIBUTE_NORMAL
        push    3                       ; OPEN_EXISTING
        push    0
        push    3                       ; FILE_SHARE_READ | FILE_SHARE_WRITE
        push    0x80000000              ; GENERIC_READ
        push    edi
        call    [ebx + IAT_CreateFileA]
        cmp     eax, -1
        je      .no_file
        push    eax                     ; the handle
        push    0                       ; the bytes read
        mov     ecx, esp
        push    0
        push    ecx
        push    CFG_SIZE
        push    esi
        push    eax
        call    [ebx + IAT_ReadFile]
        pop     ecx
        pop     edx
        push    eax
        push    ecx
        push    edx
        call    [ebx + IAT_CloseHandle]
        pop     ecx
        pop     eax
        xor     edx, edx
        test    eax, eax
        jz      .partial
        cmp     ecx, CFG_SIZE
        jb      .done
.partial:
        mov     edx, 1
.done:  ret
.no_file:
        call    [ebx + IAT_GetLastError]
        xor     ecx, ecx
        xor     edx, edx
        cmp     eax, 2                  ; ERROR_FILE_NOT_FOUND
        je      .done
        cmp     eax, 3                  ; ERROR_PATH_NOT_FOUND
        je      .done
        inc     edx                     ; there, but not to be read
        ret

; esi = the file, ecx = its length, [esp+4] = a key (its length in a byte,
; then its letters: `setting_key`, `torrent_key`); `ret 4` -> eax = the
; start of the key's line or -1; then edx = the value's start, edi = its
; end, ebp = the line's end (its CR or LF, or the length). Offsets in the
; file.
find_setting:
        xor     eax, eax
.line:  cmp     eax, ecx
        jae     .none
        mov     edi, [esp + 4]
        movzx   edx, byte [edi]
        add     edx, eax                ; past the key
        cmp     edx, ecx
        jae     .next
        push    ecx
        push    esi
        lea     esi, [esi + eax]
        movzx   ecx, byte [edi]
        inc     edi
        repe    cmpsb
        pop     esi
        pop     ecx
        jne     .next
        movzx   edi, byte [esi + edx]
        cmp     edi, ' '
        je      .skip
        cmp     edi, 9
        jne     .next
.skip:  cmp     edx, ecx                ; the blanks before the value
        jae     .value
        cmp     byte [esi + edx], ' '
        je      .skip_one
        cmp     byte [esi + edx], 9
        jne     .value
.skip_one:
        inc     edx
        jmp     .skip
.value: mov     edi, edx
.word:  cmp     edi, ecx
        jae     .end
        movzx   ebp, byte [esi + edi]
        cmp     ebp, ' '
        jbe     .end                    ; a blank, CR, LF or a control
        inc     edi
        jmp     .word
.end:   mov     ebp, edi
.rest:  cmp     ebp, ecx
        jae     .found
        cmp     byte [esi + ebp], 13
        je      .found
        cmp     byte [esi + ebp], 10
        je      .found
        inc     ebp
        jmp     .rest
.found: ret     4
.next:  cmp     eax, ecx                ; past the next LF
        jae     .none
        cmp     byte [esi + eax], 10
        je      .after
        inc     eax
        jmp     .next
.after: inc     eax
        jmp     .line
.none:  mov     eax, -1
        ret     4

; edx = a name, ecx = its length -> eax = its index in LANGUAGES, or -1.
match_name:
        push    esi
        push    edi
        cmp     ecx, 15
        ja      .none
        xor     eax, eax
.each:  cmp     eax, LANGUAGE_COUNT
        jae     .none
        mov     edi, eax
        shl     edi, 4
        lea     edi, [ebx + language_names + edi]
        cmp     byte [edi + ecx], 0     ; as long as the name
        jne     .other
        push    ecx
        mov     esi, edx
        repe    cmpsb
        pop     ecx
        je      .done
.other: inc     eax
        jmp     .each
.none:  mov     eax, -1
.done:  pop     edi
        pop     esi
        ret

; Frees the buffer at esi (none when 0).
free_buffer:
        test    esi, esi
        jz      .done
        call    [ebx + IAT_GetProcessHeap]
        push    esi
        push    0
        push    eax
        call    [ebx + IAT_HeapFree]
.done:  ret

; esi = a module's name, edi = a function's name (ASCII) -> eax = its address or 0.
procedure:
        push    esi
        call    [ebx + IAT_GetModuleHandleA]
        test    eax, eax
        jz      .done
        push    edi
        push    eax
        call    [ebx + IAT_GetProcAddress]
.done:  ret

; settings::reload_language, eax = the settings: the index of user.cfg's
; language, else of the Windows user locale's, else English. Nothing written.
reload_language:
        pushad
        DELTA
        sub     esp, PATH_SIZE
        mov     edi, esp
        xor     edx, edx
        call    config_path
        test    eax, eax
        jz      .locale
        call    load_config
        test    esi, esi
        jz      .locale
        lea     eax, [ebx + setting_key]
        push    eax
        call    find_setting
        cmp     eax, -1
        je      .free
        mov     ecx, edi
        sub     ecx, edx                ; the value's length
        add     edx, esi                ; the value
        call    match_name
        push    eax
        call    free_buffer
        pop     eax
        cmp     eax, -1
        jne     .set
        jmp     .locale
.free:  call    free_buffer
.locale:
        lea     esi, [ebx + kernel32_name]
        lea     edi, [ebx + lcid_name]
        call    procedure
        test    eax, eax
        jz      .english
        call    eax                     ; GetUserDefaultLCID
        and     eax, 0x3FF              ; its primary language
        xor     ecx, ecx
.each:  cmp     ecx, LANGUAGE_COUNT
        jae     .english
        cmp     ax, [ebx + language_windows + ecx*2]
        je      .this
        inc     ecx
        jmp     .each
.this:  mov     eax, ecx
        jmp     .set
.english:
        xor     eax, eax
.set:   add     esp, PATH_SIZE
        mov     ecx, [esp + 28]         ; pushad's eax: the settings
        mov     [ecx + SETTINGS_LANGUAGE], eax
        mov     eax, ecx
        call    reload_torrent          ; and "Allow torrent", from the same file
        popad
        ret

; eax = the settings: "Allow torrent" (settings+0 current, +1 last saved:
; what start_updater's 4th argument and 1A say) is user.cfg's
; `enable_torrent_seeding on`, the game's own saved command (exe 0x1EE580);
; off without that line, with any other value, without a user.cfg. Run
; right after settings::reload (0x14200), whose registry `download_type` it
; replaces. Nothing written; every register kept.
reload_torrent:
        call    torrent_reset
        pushad
        DELTA
        sub     esp, PATH_SIZE
        push    0                       ; off
        lea     edi, [esp + 4]
        xor     edx, edx
        call    config_path
        test    eax, eax
        jz      .set
        call    load_config
        test    esi, esi
        jz      .set
        lea     eax, [ebx + torrent_key]
        push    eax
        call    find_setting
        cmp     eax, -1
        je      .free
        sub     edi, edx                ; the value's length
        cmp     edi, 2
        jne     .free
        cmp     word [esi + edx], 'on'
        jne     .free
        mov     dword [esp], 1
.free:  call    free_buffer
.set:   pop     eax
        add     esp, PATH_SIZE
        mov     ecx, [esp + 28]         ; pushad's eax: the settings
        mov     [ecx], al
        mov     [ecx + 1], al
        popad
        ret

; The one-time reset: "Allow torrent" is off on every install until the
; player ticks it, though the original game saved its default,
; `enable_torrent_seeding on`, in every player's user.cfg. Once an install
; (no RESET_NAME beside the launcher): the line is written `off`, then that
; file is made, and from then on the line is the player's alone. A user.cfg
; that cannot be written makes no file: the next start tries again. Every
; register kept.
RESET_CHARS             equ 300         ; the file's path, UTF-16 characters

torrent_reset:
        pushad
        DELTA
        sub     esp, 2*RESET_CHARS
        mov     edi, esp
        push    RESET_CHARS
        push    edi
        push    0
        call    [ebx + IAT_GetModuleFileNameW]
        test    eax, eax
        jz      .done
        cmp     eax, RESET_CHARS - RESET_NAME_CHARS - 1
        jae     .done                   ; no room for the name after the folder
        lea     ecx, [edi + eax*2]
.back:  sub     ecx, 2                  ; back to the last backslash
        cmp     ecx, edi
        jb      .done
        cmp     word [ecx], 0x5C
        jne     .back
        lea     edx, [ebx + reset_name]
.copy:  add     ecx, 2                  ; the name after it, its zero included
        mov     ax, [edx]
        mov     [ecx], ax
        add     edx, 2
        test    ax, ax
        jnz     .copy
        push    edi
        call    [ebx + IAT_GetFileAttributesW]
        cmp     eax, -1
        jne     .done                   ; there: done before
        call    [ebx + IAT_GetLastError]
        cmp     eax, 2                  ; ERROR_FILE_NOT_FOUND, nothing else
        jne     .done
        lea     eax, [ebx + off_text]
        push    eax
        lea     eax, [ebx + torrent_key]
        push    eax
        call    save_setting
        test    eax, eax
        jz      .done
        push    0
        push    0x80                    ; FILE_ATTRIBUTE_NORMAL
        push    1                       ; CREATE_NEW
        push    0
        push    0
        push    0x40000000              ; GENERIC_WRITE
        push    edi
        call    [ebx + IAT_CreateFileW]
        cmp     eax, -1
        je      .done
        push    eax
        call    [ebx + IAT_CloseHandle]
.done:  add     esp, 2*RESET_CHARS
        popad
        ret

; esi = the settings: user.cfg's `enable_torrent_seeding` line becomes
; settings+0's: `on` or `off`, as the game writes it. Every register kept.
save_torrent:
        pushad
        DELTA
        lea     eax, [ebx + off_text]
        cmp     byte [esi], 0
        je      .save
        lea     eax, [ebx + on_text]
.save:  push    eax
        lea     eax, [ebx + torrent_key]
        push    eax
        call    save_setting
        popad
        ret

; settings::save_language(settings), `ret 4`: user.cfg's `localization` line
; becomes the settings' language.
save_language:
        pushad
        DELTA
        mov     eax, [esp + 36]         ; the settings
        mov     eax, [eax + SETTINGS_LANGUAGE]
        cmp     eax, LANGUAGE_COUNT
        jae     .out
        shl     eax, 4
        lea     eax, [ebx + language_names + eax]
        push    eax
        lea     eax, [ebx + setting_key]
        push    eax
        call    save_setting
.out:   popad
        ret     4

; save_setting(key, value), `ret 8` -> eax = 1 when written, else 0:
; user.cfg's line of the key (as find_setting takes it) becomes
; `<key> <value>` (value: zero-ended ASCII),
; replaced where it is, added at the end when there is none, the file (and
; its folder) made when there is none. A file that cannot be read whole is
; left as it is. Written beside it and renamed over it, so it is never left
; half written.
SAVE_PATH               equ 0           ; locals, after `sub esp`: the path
SAVE_NEW                equ PATH_SIZE   ; the path of the new file
SAVE_VALUE              equ 2*PATH_SIZE ; the value
SAVE_OUT                equ 2*PATH_SIZE + 4   ; the new file's buffer
SAVE_KEY                equ 2*PATH_SIZE + 8   ; the key
SAVE_DONE               equ 2*PATH_SIZE + 12  ; 1 once written
SAVE_LOCALS             equ 2*PATH_SIZE + 16

save_setting:
        pushad
        DELTA
        sub     esp, SAVE_LOCALS
        mov     eax, [esp + SAVE_LOCALS + 36]
        mov     [esp + SAVE_KEY], eax
        mov     eax, [esp + SAVE_LOCALS + 40]
        mov     [esp + SAVE_VALUE], eax
        mov     dword [esp + SAVE_DONE], 0
        lea     edi, [esp + SAVE_PATH]
        mov     dl, 1
        call    config_path
        test    eax, eax
        jz      .done
        lea     edi, [esp + SAVE_PATH]
        call    load_config             ; esi buffer, ecx length, edx 0 whole
        test    edx, edx
        jnz     .free_in
        push    ecx
        call    [ebx + IAT_GetProcessHeap]
        push    CFG_SIZE + 128
        push    0
        push    eax
        call    [ebx + IAT_HeapAlloc]
        pop     ecx
        mov     [esp + SAVE_OUT], eax
        test    eax, eax
        jz      .free_in
        push    ecx
        push    dword [esp + 4 + SAVE_KEY]
        call    find_setting            ; eax line, edx value, edi value end, ebp line end
        pop     ecx
        push    ecx                     ; [esp] = the length, while the new text is made
        mov     edx, ebp                ; where the old text goes on
        mov     edi, [esp + 4 + SAVE_OUT]
        cmp     eax, -1
        jne     .replace
        ; No line: the file, a line end if its last line has none, the line.
        push    esi
        rep     movsb
        pop     esi
        mov     ecx, [esp]
        test    ecx, ecx
        jz      .add
        cmp     byte [esi + ecx - 1], 10
        je      .add
        mov     word [edi], 0x0A0D
        add     edi, 2
.add:   call    .line
        mov     word [edi], 0x0A0D
        add     edi, 2
        jmp     .write
.replace:
        ; Before the line, the line, the rest from the old line's end.
        push    esi
        mov     ecx, eax
        rep     movsb
        pop     esi
        call    .line
        push    esi
        mov     ecx, [esp + 4]          ; the length
        sub     ecx, edx
        add     esi, edx
        rep     movsb
        pop     esi
.write: pop     ecx                     ; the length (no longer needed)
        sub     edi, [esp + SAVE_OUT]   ; the new file's length
        mov     ebp, edi
        call    free_buffer             ; the old file's buffer
        ; <path>.new, written whole, then renamed over the file.
        lea     edi, [esp + SAVE_NEW]
        lea     edx, [esp + SAVE_PATH]
        mov     ecx, edi
        call    append
        lea     edx, [ebx + new_suffix]
        call    append
        push    0
        push    0x80
        push    2                       ; CREATE_ALWAYS
        push    0
        push    0
        push    0x40000000              ; GENERIC_WRITE
        push    edi
        call    [ebx + IAT_CreateFileA]
        cmp     eax, -1
        je      .free_out
        mov     esi, eax
        push    0
        mov     ecx, esp
        push    0
        push    ecx
        push    ebp
        push    dword [esp + 16 + SAVE_OUT]
        push    esi
        call    [ebx + IAT_WriteFile]
        pop     ecx                     ; the bytes written
        push    eax
        push    ecx
        push    esi
        call    [ebx + IAT_CloseHandle]
        pop     ecx
        pop     eax
        test    eax, eax
        jz      .drop
        cmp     ecx, ebp
        jne     .drop
        lea     esi, [ebx + kernel32_name]
        lea     edi, [ebx + move_name]
        call    procedure
        test    eax, eax
        jz      .drop
        push    1                       ; MOVEFILE_REPLACE_EXISTING
        lea     ecx, [esp + 4 + SAVE_PATH]
        push    ecx
        lea     ecx, [esp + 8 + SAVE_NEW]
        push    ecx
        call    eax                     ; MoveFileExA(new, path)
        test    eax, eax
        jz      .drop
        mov     dword [esp + SAVE_DONE], 1
        jmp     .free_out
.drop:  lea     ecx, [esp + SAVE_NEW]
        push    ecx
        call    [ebx + IAT_DeleteFileA]
.free_out:
        mov     esi, [esp + SAVE_OUT]
        call    free_buffer
        jmp     .done
.free_in:
        call    free_buffer
.done:  mov     eax, [esp + SAVE_DONE]
        add     esp, SAVE_LOCALS
        mov     [esp + 28], eax         ; popad's eax
        popad
        ret     8

; edi = where -> "<key> <value>" there, edi after it. The key and the value
; are at [esp + 4 (this call) + 4 (the length pushed) + SAVE_KEY, SAVE_VALUE].
.line:  push    esi
        mov     esi, [esp + 12 + SAVE_KEY]
        movzx   ecx, byte [esi]
        inc     esi
        rep     movsb
        mov     byte [edi], ' '
        inc     edi
        mov     esi, [esp + 12 + SAVE_VALUE]
.name:  lodsb
        test    al, al
        jz      .named
        stosb
        jmp     .name
.named: pop     esi
        ret

; The button's click (cdecl, the menu): a menu of every language by its own
; name, the current one checked, under the button; a pick is what a row of
; the drop-down did (language_selected: the settings saved, the window made
; again in that language).
PICK_CREATE             equ 0           ; locals: CreatePopupMenu
PICK_APPEND             equ 4           ; AppendMenuW
PICK_TRACK              equ 8           ; TrackPopupMenu
PICK_DESTROY            equ 12          ; DestroyMenu
PICK_MENU               equ 16          ; the menu made
PICK_RECT               equ 20          ; the button's rectangle: left, top, right, bottom
PICK_LOCALS             equ 36

pick_language:
        pushad
        DELTA
        mov     ebp, [esp + 36]         ; the language menu
        sub     esp, PICK_LOCALS
        lea     esi, [ebx + user32_name]
        lea     edi, [ebx + create_name]
        call    procedure
        mov     [esp + PICK_CREATE], eax
        lea     edi, [ebx + append_name]
        call    procedure
        mov     [esp + PICK_APPEND], eax
        lea     edi, [ebx + track_name]
        call    procedure
        mov     [esp + PICK_TRACK], eax
        lea     edi, [ebx + destroy_name]
        call    procedure
        mov     [esp + PICK_DESTROY], eax
        cmp     dword [esp + PICK_CREATE], 0
        je      .done
        cmp     dword [esp + PICK_APPEND], 0
        je      .done
        cmp     dword [esp + PICK_TRACK], 0
        je      .done
        cmp     dword [esp + PICK_DESTROY], 0
        je      .done
        call    [esp + PICK_CREATE]
        mov     [esp + PICK_MENU], eax
        test    eax, eax
        jz      .done
        mov     eax, [ebp + MENU_SETTINGS]
        mov     edx, [eax + SETTINGS_LANGUAGE]
        xor     esi, esi
.row:   push    edx                     ; kept over the call
        mov     eax, esi
        shl     eax, 5
        lea     eax, [ebx + language_natives + eax]
        push    eax                     ; its own name
        lea     eax, [esi + 1]
        push    eax                     ; the command: index + 1
        xor     eax, eax
        cmp     esi, edx
        jne     .unchecked
        mov     eax, MF_CHECKED
.unchecked:
        push    eax
        push    dword [esp + 16 + PICK_MENU]
        call    [esp + 20 + PICK_APPEND]
        pop     edx
        inc     esi
        cmp     esi, LANGUAGE_COUNT
        jb      .row
        lea     eax, [esp + PICK_RECT]
        push    eax
        push    dword [ebp + MENU_WINDOW]
        call    [ebx + IAT_GetWindowRect]
        push    0
        push    dword [ebp + MENU_WINDOW]
        push    0
        push    dword [esp + 12 + PICK_RECT + 12]       ; bottom
        push    dword [esp + 16 + PICK_RECT]            ; left
        push    TPM_FLAGS
        push    dword [esp + 24 + PICK_MENU]
        call    [esp + 28 + PICK_TRACK]
        mov     esi, eax                ; 0, or the index + 1
        push    dword [esp + PICK_MENU]
        call    [esp + 4 + PICK_DESTROY]
        test    esi, esi
        jz      .done
        dec     esi
        push    esi                     ; {menu, index}
        push    ebp
        push    esp
        call    LANGUAGE_SELECTED
        add     esp, 12
.done:  add     esp, PICK_LOCALS
        popad
        ret

; ---------------------------------------------------------------------------
; "Allow torrent" on the main window (patch 15). The settings window (the
; gear) stays out of sight (patch 8); its one setting that applies is a box
; in the bottom panel, left of the status line: YouTube's control (form+648h,
; out of sight since patch 8, moved there by patch_launcher_026g.py RECTS)
; with the settings window's own pictures (PNG 347 unticked, 345 under the
; pointer, 346 pressed, 344 ticked, loaded in the place of YouTube's three
; and Facebook's first), and its own text (string 0D3h, the launcher's
; language) in its font and colour (Calibri bold 18, white), where the
; settings window has it beside its box (0x1D1D4).
;
; The text is painted into the panel's picture (form+0ECh, JPG 301), as the
; status line's texts are: when the picture is made (`panel_made`) and every
; time launcher_form::set_status_texts (0x1B48D) copies it anew from its
; resource before it writes the status (`panel_restored`).
;
; The box's four pictures are made opaque once, over the panel's picture
; where the box is (`bake_box`, in `panel_made`): a transparent control is
; drawn over what its window last showed, and only Windows composes the
; launcher's windows (WS_EX_COMPOSITED) so that the panel is under it again;
; under Wine the check of a ticked box would stay when it is unticked.
;
; The setting is kept as the language is: in user.cfg, as the game's own
; saved command `enable_torrent_seeding on` / `off` (`reload_torrent` at the
; start, `save_torrent`), so it stays as the player left it until they
; change it. A click (the control calls its callback at the button's press,
; 0x1275B) toggles settings+0 (and +1), writes the line at once, fires the
; callback settings::save fires for a changed box (0x1D780: 1A to the
; updater while the pipe is connected, 0x172FA) and shows the pictures as
; settings_window::show_torrent_checkbox (0x1CE0E) does. The registry's
; `download_type` is neither read (`reload_torrent`) nor written.

TRANSPARENT_CONTROL     equ 0x12548     ; transparent_control::transparent_control
IMAGE_WINDOW            equ 0x12140     ; image_window::image_window
SET_PICTURE             equ 0x125CE     ; transparent_control: eax = it, ecx = its picture (and repaint)
COPY_PICTURE            equ 0x12BE0     ; (destination, source, &point), eax = the rectangle; cdecl
                                        ;   (AlphaBlend: the source's alpha over the destination)
CROP_PICTURE            equ 0x12051     ; (bitmap), eax = a rectangle of it -> a new bitmap of it; cdecl
FUNCTION0_CALL          equ 0x17522     ; boost::function0::operator(), eax = the function
SETTINGS_TORRENT_CHANGED equ 0x28       ; settings: the "download type changed" callback
LOAD_STRING             equ 0x137A8     ; load_string(id, buffer, size), cdecl
MAKE_FONT               equ 0x11CC7     ; [esi] = CreateFontW(height, weight, face); ret 0Ch
DRAW_TEXT               equ 0x137C3     ; into a picture: (text, bitmap, &font, colour, format,
                                        ;   left, right margin, top); cdecl
IAT_DeleteObject        equ 0x3B07C
CALIBRI                 equ 0x3F480     ; L"Calibri", the settings window's font
BOX_RECT                equ 0x458A4     ; the box's place in the panel: left, top (YouTube's rect)

FORM_PANEL_PICTURE      equ 0xEC        ; launcher_form: the bottom panel's picture (HBITMAP)
FORM_UNTICKED           equ 0x140       ; the box's pictures: YouTube's first, second
FORM_UNTICKED_HOVER     equ 0x14C
FORM_PRESSED            equ 0x158       ; YouTube's third
FORM_TICKED             equ 0x164       ; Facebook's first
FORM_BOX                equ 0x648       ; YouTube's control
FORM_SETTINGS           equ 0xB40       ; settings_window (form+6E8h) + 458h: the settings
CONTROL_HOVER           equ 0xC         ; transparent_control: the picture under the pointer

TORRENT_TEXT            equ 0xD3        ; "Allow torrent"
TEXT_CHARS              equ 64
TEXT_HEIGHT             equ 18          ; the settings window's text, where it is beside its box
TEXT_RIGHT              equ 25          ;   (0x1C38E, 0x3A96C): 25 pixels right of the box's left,
TEXT_DOWN               equ 1           ;   1 below its top
FONT_BOLD               equ 700
WHITE                   equ 0xFFFFFF
LABEL_FONT              equ 0           ; paint_label's locals
LABEL_TEXT              equ 4
LABEL_LOCALS            equ 4 + 2*TEXT_CHARS

; eax = the form: the box's text painted into the panel's picture. Every
; register kept.
paint_label:
        pushad
        mov     esi, eax                ; the form
        DELTA
        sub     esp, LABEL_LOCALS
        push    TEXT_CHARS
        lea     eax, [esp + 4 + LABEL_TEXT]
        push    eax
        push    TORRENT_TEXT
        call    LOAD_STRING
        add     esp, 12
        push    esi
        lea     esi, [esp + 4 + LABEL_FONT]
        lea     eax, [ebx + CALIBRI]
        push    eax
        push    FONT_BOLD
        push    TEXT_HEIGHT
        call    MAKE_FONT
        pop     esi
        mov     eax, [ebx + BOX_RECT + 4]
        add     eax, TEXT_DOWN
        push    eax                     ; top
        push    0                       ; right margin
        mov     eax, [ebx + BOX_RECT]
        add     eax, TEXT_RIGHT
        push    eax                     ; left
        push    0                       ; DT_LEFT | DT_TOP, as the settings window's
        push    WHITE
        lea     eax, [esp + 20 + LABEL_FONT]
        push    eax
        push    dword [esi + FORM_PANEL_PICTURE]
        lea     eax, [esp + 28 + LABEL_TEXT]
        push    eax
        call    DRAW_TEXT
        add     esp, 32
        push    dword [esp + LABEL_FONT]
        call    [ebx + IAT_DeleteObject]
        add     esp, LABEL_LOCALS
        popad
        ret

; eax = the form: each of the box's four pictures replaced by itself over
; the panel's picture where the box is (BOX_RECT): opaque, of the box's
; size. The PNG's bitmap is deleted; the form deletes the new one as it did
; the PNG's (0x1B328). Every register kept.
BAKE_RECT               equ 0           ; locals: 0, 0, the box's width, height
BAKE_POINT              equ 16          ; 0, 0
BAKE_LOCALS             equ 24

bake_box:
        pushad
        mov     esi, eax                ; the form
        DELTA
        sub     esp, BAKE_LOCALS
        xor     eax, eax
        mov     [esp + BAKE_RECT], eax
        mov     [esp + BAKE_RECT + 4], eax
        mov     [esp + BAKE_POINT], eax
        mov     [esp + BAKE_POINT + 4], eax
        mov     eax, [ebx + BOX_RECT + 8]
        sub     eax, [ebx + BOX_RECT]
        mov     [esp + BAKE_RECT + 8], eax
        mov     eax, [ebx + BOX_RECT + 12]
        sub     eax, [ebx + BOX_RECT + 4]
        mov     [esp + BAKE_RECT + 12], eax
        lea     edi, [ebx + box_pictures]
        mov     ebp, 4
.each:  push    dword [esi + FORM_PANEL_PICTURE]
        lea     eax, [ebx + BOX_RECT]
        call    CROP_PICTURE            ; the panel where the box is
        add     esp, 4
        test    eax, eax
        jz      .next                   ; none: the PNG's stays
        mov     edx, [edi]
        push    eax                     ; the new bitmap, kept
        lea     ecx, [esp + 4 + BAKE_POINT]
        push    ecx
        push    dword [esi + edx]       ; the PNG's
        push    eax
        lea     eax, [esp + 16 + BAKE_RECT]
        call    COPY_PICTURE            ; the PNG over it
        add     esp, 12
        mov     edx, [edi]
        push    dword [esi + edx]
        call    [ebx + IAT_DeleteObject]
        pop     eax
        mov     edx, [edi]
        mov     [esi + edx], eax
.next:  add     edi, 4
        dec     ebp
        jnz     .each
        add     esp, BAKE_LOCALS
        popad
        ret

box_pictures:   dd FORM_UNTICKED, FORM_UNTICKED_HOVER, FORM_PRESSED, FORM_TICKED

; In place of image_window::image_window for the bottom panel (0x1AD78):
; ebx = the form; edi, esi and the stack are image_window's. The box's
; pictures are loaded (0x1AAFC..) and not yet given to its control (0x1ADE2).
panel_made:
        mov     eax, ebx
        call    paint_label
        call    bake_box
        jmp     IMAGE_WINDOW

; In place of transparent_control::transparent_control for YouTube's
; control, the box (0x1AE1F): ebx = the form, ebp its constructor's frame
; ([ebp+0Ch] = the settings); edi, esi and the eight arguments on the stack
; are the control's. Ticked, its picture and the one under the pointer are
; the ticked one.
box_made:
        mov     eax, [ebp + 0Ch]
        cmp     byte [eax], 0
        je      .made
        mov     eax, [ebx + FORM_TICKED]
        mov     [esp + 4 + 12], eax     ; its picture
        mov     [esp + 4 + 16], eax     ; under the pointer
.made:  jmp     TRANSPARENT_CONTROL

; In place of the copy of the panel's picture from its resource in
; set_status_texts (0x1B4D9): the copy (its three arguments copied, eax the
; rectangle), then the box's text again. edi = the form.
panel_restored:
        push    dword [esp + 12]
        push    dword [esp + 12]
        push    dword [esp + 12]
        call    COPY_PICTURE
        add     esp, 12
        mov     eax, edi
        jmp     paint_label

; The box's click (cdecl, the form): YouTube's click handler (0x1BD03)
; jumps here.
torrent_click:
        push    ebx
        push    esi
        mov     ebx, [esp + 12]         ; the form
        mov     esi, [ebx + FORM_SETTINGS]
        cmp     byte [esi], 0
        sete    al
        mov     [esi], al
        mov     [esi + 1], al           ; saved
        call    save_torrent
        cmp     dword [esi + SETTINGS_TORRENT_CHANGED], 0
        je      .show
        lea     eax, [esi + SETTINGS_TORRENT_CHANGED]
        call    FUNCTION0_CALL          ; 1A while the updater is connected
.show:  mov     ecx, [ebx + FORM_UNTICKED]
        mov     edx, [ebx + FORM_UNTICKED_HOVER]
        cmp     byte [esi], 0
        je      .picture
        mov     ecx, [ebx + FORM_TICKED]
        mov     edx, ecx
.picture:
        lea     eax, [ebx + FORM_BOX]
        mov     [eax + CONTROL_HOVER], edx
        call    SET_PICTURE
        pop     esi
        pop     ebx
        ret

setting_key:    db 12, 'localization'
torrent_key:    db 22, 'enable_torrent_seeding'
on_text:        db 'on', 0
off_text:       db 'off', 0
        align   2, db 0
reset_name:     dw __utf16__('launcher_skhid.torrent_reset'), 0
RESET_NAME_CHARS equ ($ - reset_name) / 2
folder_name:    db '\survarium', 0
file_name:      db '\user.cfg', 0
new_suffix:     db '.new', 0
kernel32_name:  db 'kernel32.dll', 0
user32_name:    db 'user32.dll', 0
lcid_name:      db 'GetUserDefaultLCID', 0
move_name:      db 'MoveFileExA', 0
create_name:    db 'CreatePopupMenu', 0
append_name:    db 'AppendMenuW', 0
track_name:     db 'TrackPopupMenu', 0
destroy_name:   db 'DestroyMenu', 0

        align   2, db 0
%include "launcher_languages_026g.inc"
