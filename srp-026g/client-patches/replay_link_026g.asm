; "Watch replay" from the lobby's History: the lobby's link callback starts
; the updater, which fetches the replay and saves it (it starts no game)
; (build 2777, patch revision 14, client_patch.md section 3.27). One entry,
; at the start; patch_026g.py checks the hook's target. At .srp+220h, after
; the upload code, the default server's stub and its address (0x21D at the
; most), before the patches' mount (.srp+400h).
;   nasm -f bin -DCAVE=0xA425220 replay_link_026g.asm -o replay_link_026g.section.bin
BITS 32
ORG CAVE

; The Flash callback video_tutorial_link_clicked (0x28CDA0, thiscall, one
; argument, `ret 4`, returns al = 1) gets the lobby client in eax through
; its owner's vtable+3Ch and then opened the client's link buffer (+1E6144h,
; filled as UTF-8 with a zero at its end by the server's 9E, 0x271426) with
; ShellExecuteA, whatever it held. The jump replaces `push 1 / push 0 /
; push 0` at 0x28CDB6; at that point [esp] is the handler's saved esi.
;
; Here the link is only read. Exactly `replay ` + 8 + ` ` + 32 + ` ` + 64
; lower-case hex digits + ` ` + 1 to 10 decimal digits, and its zero, starts
; <folder of this exe>\updater_skhid.exe with the link as its arguments and
; that folder as its working folder, the command line `updater_skhid.exe
; <link>` (CreateProcessW: no shell, no file
; association, no zone prompt). The program is always that fixed name; the
; link is never a program, a path or a URL. Anything else does nothing. The
; original's enable_element(videoTutorial) is not called.

PATH_UNITS  equ 0x208                   ; 520 UTF-16 units for a path
APP         equ 0                       ; the updater's path
FOLDER      equ APP + PATH_UNITS * 2    ; its folder, with its backslash
COMMAND     equ FOLDER + PATH_UNITS * 2 ; updater_skhid.exe <link>
COMMAND_UNITS equ PATH_UNITS + 0x100
STARTUP     equ COMMAND + COMMAND_UNITS * 2   ; STARTUPINFOW, 44h bytes
PROCESS     equ STARTUP + 0x44          ; PROCESS_INFORMATION, 10h bytes
FRAME       equ PROCESS + 0x10          ; under one page: no probe needed
NAME_UNITS  equ 18                      ; "updater_skhid.exe" and its zero
HEX_RUN     equ 0x80                    ; in the pattern: 80h + n = n hex digits

; The game's imports (kernel32), read through their IAT slots.
GetModuleHandleA    equ 0x87003c
CloseHandle         equ 0x870040
GetModuleFileNameW  equ 0x870074
GetProcAddress      equ 0x870084

replay_link:
    pushad
    call .pc
.pc:
    pop ebp                             ; addresses relative to the code
    lea esi, [eax+0x1e6144]             ; the link
    lea edi, [ebp+pattern-.pc]
.fixed:
    movzx eax, byte [edi]
    inc edi
    test al, al
    jz .size
    js .run
    cmp [esi], al                       ; a literal: a zero matches none
    jne .ignore
    inc esi
    jmp .fixed
.run:
    and eax, 0x7f
    xchg ecx, eax                       ; n hex digits
.hex:
    movzx edx, byte [esi]               ; a zero matches nothing: never read past it
    lea eax, [edx-0x30]                 ; 0..9
    cmp eax, 9
    jbe .hex_next
    sub edx, 0x61                       ; a..f
    cmp edx, 5
    ja .ignore
.hex_next:
    inc esi
    loop .hex
    jmp .fixed
.size:
    xor ecx, ecx
.digit:
    movzx edx, byte [esi+ecx]
    test edx, edx
    jz .digits_end
    sub edx, 0x30
    cmp edx, 9
    ja .ignore
    inc ecx
    cmp ecx, 10
    ja .ignore
    jmp .digit
.digits_end:
    test ecx, ecx
    jz .ignore

    mov esi, [esp+28]                   ; pushad's eax: the lobby client
    add esi, 0x1e6144                   ; the link again, checked
    sub esp, FRAME
    mov ebx, esp
    cld
    push PATH_UNITS
    push ebx
    push 0
    call [ebp+GetModuleFileNameW-.pc]   ; this exe's full path
    lea ecx, [eax-1]
    cmp ecx, PATH_UNITS - 1
    jae .free                           ; none, or cut short
    inc ecx
.back:
    dec ecx
    js .free                            ; no folder
    cmp word [ebx+ecx*2], 0x5c
    jne .back
    inc ecx                             ; the folder's units, its backslash too
    lea eax, [ecx+NAME_UNITS]
    cmp eax, PATH_UNITS
    ja .free
    push esi
    mov esi, ebx
    lea edi, [ebx+FOLDER]
    push ecx
    rep movsw
    mov word [edi], 0                   ; FOLDER = the folder
    pop ecx
    lea edi, [ebx+ecx*2]
    lea esi, [ebp+updater_name-.pc]
    mov ecx, NAME_UNITS
    rep movsw                           ; APP = the folder + updater_skhid.exe
    lea edi, [ebx+COMMAND]              ; the program's name, a space (the
    lea esi, [ebp+updater_name-.pc]     ; updater reads no first argument)
    mov cl, NAME_UNITS - 1
    rep movsw
    push 0x20
    pop eax
    stosw
    pop esi
.link:
    movzx eax, byte [esi]               ; checked ASCII: one unit each
    inc esi
    stosw
    test eax, eax
    jnz .link                           ; the zero too

    lea eax, [ebp+kernel32_name-.pc]
    push eax
    call [ebp+GetModuleHandleA-.pc]
    test eax, eax
    jz .free
    lea ecx, [ebp+create_process_name-.pc]
    push ecx
    push eax
    call [ebp+GetProcAddress-.pc]       ; CreateProcessW (the game imports the A form only)
    test eax, eax
    jz .free
    mov edx, eax
    lea edi, [ebx+STARTUP]
    xor eax, eax
    mov ecx, (0x44 + 0x10) / 4
    rep stosd
    mov byte [edi-0x54], 0x44           ; STARTUPINFOW's cb
    lea ecx, [ebx+PROCESS]
    push ecx
    lea ecx, [ebx+STARTUP]
    push ecx
    lea ecx, [ebx+FOLDER]
    push ecx                            ; working folder
    push eax                            ; this environment
    push eax                            ; no flags
    push eax                            ; no handle inherited
    push eax
    push eax
    lea ecx, [ebx+COMMAND]
    push ecx
    push ebx                            ; the program: APP, no search
    call edx
    test eax, eax
    jz .free
    push dword [ebx+PROCESS+4]          ; hThread
    call [ebp+CloseHandle-.pc]
    push dword [ebx+PROCESS]            ; hProcess
    call [ebp+CloseHandle-.pc]
.free:
    add esp, FRAME
.ignore:
    popad
    mov al, 1                           ; what the original returns
    pop esi
    ret 4

; The link's form: "replay ", 8 hex, " ", 32 hex, " ", 64 hex, " " (a byte
; HEX_RUN + n is n lower-case hex digits, any other a literal), then the
; zero; the size's digits and the link's zero follow it.
pattern:
    db "replay ", HEX_RUN + 8, " ", HEX_RUN + 32, " ", HEX_RUN + 64, " ", 0
updater_name:
    dw __utf16__("updater_skhid.exe"), 0
kernel32_name:
    db "kernel32.dll", 0
create_process_name:
    db "CreateProcessW", 0
