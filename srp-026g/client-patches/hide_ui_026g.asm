; Patch 62 (build 2777): one key hides the whole interface and shows it again,
; in a match and in the lobby, for screenshots. The key is the game's own
; action toggle_replay_ui (19h, F11 by default; action table entry 0xA74CA8,
; kinds 5), which in the original only shows and hides the replay viewer's
; controls: in a match and in the lobby it did nothing. The arena's blocks
; hide_ui.code and hide_ui.state (link_026g.py links them; patch_026g.py's
; HIDE_UI_HOOKS call and jump to the three entries by their names).
; client_patch.md section 3.54; the rules: ../docs/notes/hide-ui-key.md.
;
; The match. The game world's on_keyboard_action (0x25CD00) asks the key
; binder for the key's action of kind 1 (0x25CD5F) and switches on actions
; 11h..19h (0x25CE1C, the table 0x25CF30): 19h goes to 0x25CED1, which takes
; the handler ([esp+10h]) and does nothing without the replay's movie (the
; handler's +18h, the hud's +14h; a match has none). There, without that
; movie, the game world's own show_ui (0x1F95F0, its state +0A0h; the
; activation shows the ui with it, the deactivation hides it) hides or shows
; the hud movie and the player icons' movie: the hud, the crosshair, the
; minimap and the markers over heads. Hidden only by this key, and not while
; the hud shows the cursor (the kits: the death screen, before the start).
; The key is not taken (0x25CF0D, al 0), as in the original.
;
; The kits. show_profiles_selector (0x2383B0) shows the kits with
; show_cursor(1) (its call at 0x23845B, edi the hud): before it, a game world
; this key hid gets its ui back, so the death screen and its kits are never
; hidden.
;
; The lobby. lobby_menu::on_keyboard_action (0x26CB80) asks the key binder for
; the lobby's actions (kind 2) at 0x26CCD2 (eax the binder, edi 2, esi the
; key); the F keys never reach the lobby movie (its key map, 0x1FDF20, has
; none). When the key has no lobby action, the binder is asked again for kind
; 1; toggle_replay_ui invokes the lobby movie's root.toggle_interface (which
; hides it, or shows it, LobbyMenu.as), and the key is taken: the function
; returns at 0x26CCAE (al 1), as after a key the movie typed, and the movie
; never gets it. While the movie says its interface is hidden
; (root.interface_hidden), any other key goes to the movie with no action of
; the game (a window's key, F6's Play at 0x26CDA2): the function goes on at
; 0x26CDD2, which hands the key to the movie and returns 0; the movie shows the
; interface and takes the key (its press and its release). Any other key, the
; interface shown, goes on as before.
BITS 32
%include "engine_026g.inc"
global hide_ui_match, hide_ui_kits, hide_ui_lobby
global hidden_world                ; the game world this key hid, or 0 (check_hide_ui_026g.py reads it)
section .text

%define REPLAY_TOGGLE 0x25CEDB     ; the case 19h after its replay test: the replay's controls
%define KEY_OTHER 0x25CF0D         ; on_keyboard_action's end: the key not taken (al 0)
%define SHOW_CURSOR 0x23A930       ; game_world_ui::show_cursor, edi the hud, `ret 4`
%define LOBBY_TAKEN 0x26CCAE       ; lobby_menu::on_keyboard_action after a key the movie typed: al 1
%define LOBBY_TO_MOVIE 0x26CDD2    ; its last part: the key to the movie (0x1FDD70), al 0
%define HANDLER 0x10               ; the case's [esp+10h]: the game world's input handler
%define REPLAY_MOVIE 0x18          ; the handler's +18h: the hud's +14h, the replay's controls
%define WORLD 0x1C                 ; the handler's +1Ch: the hud's +18h, the game world
%define CURSOR 0x2E0               ; the handler's +2E0h: the hud's +2DCh, the cursor shown
%define HUD_WORLD 0x18             ; the hud's +18h: the game world
%define UI_SHOWN 0xA0              ; the game world's +0A0h: show_ui's state
%define LOBBY_MOVIE 0x690          ; the lobby menu's movie holder: [[+690h]+108h]+4
%define NO_ACTION 0x49             ; the binder's answer for a key without an action
%define TOGGLE_UI 0x19             ; toggle_replay_ui
%define MATCH_KINDS 1              ; the kind the game world asks (0x25CD5F)

; The case of action 19h in on_keyboard_action, at 0x25CED1 (`mov esi,[esp+10h] /
; cmp dword [esi+18h],0 / jz 0x25CF0D`).
hide_ui_match:
    mov esi, [esp+HANDLER]
    cmp dword [esi+REPLAY_MOVIE], 0
    jne REPLAY_TOGGLE              ; a replay: its controls, as before
    mov ecx, [esi+WORLD]
    test ecx, ecx
    jz KEY_OTHER
    cmp byte [ecx+UI_SHOWN], 0
    je .show
    cmp byte [esi+CURSOR], 0
    jne KEY_OTHER                  ; the kits are shown: not hidden
    mov [hidden_world], ecx
    push 0
    push ecx
    call engine_show_ui
    add esp, 8
    jmp KEY_OTHER
.show:
    cmp [hidden_world], ecx
    jne KEY_OTHER                  ; hidden, but not by this key: left as it is
    mov dword [hidden_world], 0
    push 1
    push ecx
    call engine_show_ui
    add esp, 8
    jmp KEY_OTHER

; show_profiles_selector's call of show_cursor (0x23845B): [esp+4] shown, edi
; the hud.
hide_ui_kits:
    cmp byte [esp+4], 0
    je .cursor
    mov eax, [edi+HUD_WORLD]
    test eax, eax
    jz .cursor
    cmp [hidden_world], eax
    jne .cursor                    ; not hidden by this key
    mov dword [hidden_world], 0
    cmp byte [eax+UI_SHOWN], 0
    jne .cursor
    push 1
    push eax
    call engine_show_ui
    add esp, 8
.cursor:
    jmp SHOW_CURSOR

; lobby_menu::on_keyboard_action's call of get_binded_action (0x26CCD2): eax
; the binder, edi 2 (the lobby's kind), esi the key, [esp+4] the toggle's
; place; ebx the lobby menu; `ret 4`.
hide_ui_lobby:
    push eax                       ; the binder
    push dword [esp+8]             ; the toggle's place
    push esi
    push edi
    push eax
    call engine_binded_action      ; the lobby's own actions first
    add esp, 16
    pop ecx
    push eax                       ; the answer
    cmp eax, NO_ACTION
    jne .hidden
    sub esp, 4                     ; this answer's toggle: a place of ours, the function's left as it was
    mov edx, esp
    push edx
    push esi
    push MATCH_KINDS
    push ecx
    call engine_binded_action
    add esp, 20
    cmp eax, TOGGLE_UI
    jne .hidden
    call lobby_movie
    test eax, eax
    jz .hidden
    push 0                         ; no argument
    push 0
    push 0                         ; no result
    push toggle_interface
    push eax
    call engine_movie_invoke
    add esp, 20
    add esp, 4
    mov dword [esp], LOBBY_TAKEN   ; the key taken: the function returns 1, the movie never gets it
    ret 4
.hidden:                           ; the interface hidden: no action of the game (a window, F6's Play)
    call lobby_movie
    test eax, eax
    jz .answer
    sub esp, 24                    ; the answer, a GFx value (+4 its type, +8 a bool: nothing to release)
    xor edx, edx
    mov [esp], edx
    mov [esp+4], edx
    mov [esp+8], edx
    mov [esp+12], edx
    mov [esp+16], edx
    mov [esp+20], edx
    mov edx, esp
    push 0
    push 0
    push edx
    push interface_hidden
    push eax
    call engine_movie_invoke
    add esp, 20
    mov dl, [esp+8]
    add esp, 24
    test dl, dl
    jz .answer
    add esp, 4
    mov dword [esp], LOBBY_TO_MOVIE ; the movie gets the key, as a typed one: it shows the interface
    ret 4
.answer:
    pop eax
    ret 4

; The lobby menu's movie (ebx the lobby menu): [[+690h]+108h]+4, or 0.
lobby_movie:
    mov eax, [ebx+LOBBY_MOVIE]
    test eax, eax
    jz .out
    mov eax, [eax+0x108]
    test eax, eax
    jz .out
    mov eax, [eax+4]
.out:
    ret

section .rodata
toggle_interface:
    db "root.toggle_interface", 0
interface_hidden:
    db "root.interface_hidden", 0

section .bss
alignb 4
hidden_world:
    resd 1
