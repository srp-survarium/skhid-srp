; Patch 62 (build 2777): one key hides the whole interface and shows it again,
; in a match and in the lobby, for screenshots. The key is the game's own
; action toggle_replay_ui (19h, F11 by default; action table entry 0xA74CA8,
; kinds 5), which in the original only shows and hides the replay viewer's
; controls: in a match and in the lobby it did nothing. The arena's blocks
; hide_ui.code (link_026g.py links it; patch_026g.py's
; HIDE_UI_HOOKS call and jump to the three entries by their names).
; client_patch.md section 3.54; the rules: ../docs/notes/hide-ui-key.md.
;
; The match. The game world's on_keyboard_action (0x25CD00) asks the key
; binder for the key's action of kind 1 (0x25CD5F) and switches on actions
; 11h..19h (0x25CE1C, the table 0x25CF30): 19h goes to 0x25CED1, which takes
; the handler ([esp+10h]) and does nothing without the replay's movie (the
; handler's +18h, the hud's +14h; a match has none). There, without that
; movie, the hud movie ([[hud+8]+108h]+4) is asked root.interface_hidden and
; both it and the player icons' movie ([[hud+10h]+108h]+4) get
; root.hide_interface(not that): each hides its root, its whole interface
; (the hud, the crosshair, the minimap, the kits, the death screen and the
; markers over heads; the hud's screen effects are beside it and stay). The movies go on running (not the game world's
; show_ui, 0x1F95F0, whose hidden movies stand still and play what came
; meanwhile at once when shown): what happens while hidden plays out unseen.
; The kits before the start and the death screen too: the start, a death
; and a respawn keep the state (the owner, 2026-10-24; keys 1 to 3 still pick
; a kit, patch 58). A new match starts shown: the movies are the last
; match's, and the hud's initialize_match (0x234880) invokes the hud movie's
; root.reset (its call at 0x234AE8, ebp the hud), which shows the hud
; (HUD.reset); there the player icons' movie gets root.hide_interface(false)
; after it. The key is not taken (0x25CF0D, al 0), as in the
; original.
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
;
; The controls options. The options' list of keys is a table of the game's,
; 28h rows of 68h bytes at 0xA9DB28 (static data: the action, the label's
; localization key, the group (2 game, 0 lobby, 1 chat), the type, then two
; buffer_strings, the key being set and the key kept, each three pointers
; into its own 32-byte buffer), read by six functions of game_options
; (0x201150, 0x201248, 0x2013BF, 0x201A19, 0x201D05, 0x201F2B: the keys
; sent to the movie, a key set, a conflict, the movie's rows, the keys
; read from the binder, the keys applied with the console's bind/unbind),
; each counting 28h rows. A key set for an action without a row is kept
; nowhere (0x2013A0 finds no row) and never applied. The Rust Vostok controls
; module owns the extended writable table now. Startup copies the stock 40
; descriptors and appends F11 and mute-last-speaker from one list; PTT stays
; the existing stock row. The six readers and their counts are patched by
; HIDE_UI_TABLE_REFS/HIDE_UI_TABLE_COUNTS. No second table lives in this asm.
; All keys use the game's bind/unbind/save implementation.
BITS 32
%include "engine_026g.inc"
global hide_ui_match, hide_ui_lobby, hide_ui_reset
section .text

%define REPLAY_TOGGLE 0x25CEDB     ; the case 19h after its replay test: the replay's controls
%define KEY_OTHER 0x25CF0D         ; on_keyboard_action's end: the key not taken (al 0)
%define LOBBY_TAKEN 0x26CCAE       ; lobby_menu::on_keyboard_action after a key the movie typed: al 1
%define LOBBY_TO_MOVIE 0x26CDD2    ; its last part: the key to the movie (0x1FDD70), al 0
%define HANDLER 0x10               ; the case's [esp+10h]: the game world's input handler
%define REPLAY_MOVIE 0x18          ; the handler's +18h: the hud's +14h, the replay's controls
%define HUD 4                      ; the handler's +4: the hud (game_world_ui)
%define HUD_MOVIE 8                ; the hud's movie holders: the hud's +8, the player icons' +10h
%define ICONS_MOVIE 0x10
%define GFX_BOOL 2                 ; a GFx value's type (+4): a bool (+8)
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
    lea edi, [esi+HUD]
    call hud_hidden
    cmp eax, -1
    je KEY_OTHER                   ; no hud movie
    xor eax, 1
    push eax
    call hide_hud
    add esp, 4
    jmp KEY_OTHER

; The hud's movie (edi the hud, ecx the holder's place in it): eax the movie or 0.
movie_of:
    mov eax, [edi+ecx]
    test eax, eax
    jz .out
    mov eax, [eax+0x108]
    test eax, eax
    jz .out
    mov eax, [eax+4]
.out:
    ret

; edi the hud: eax 1 when its movie says its interface is hidden, 0 when not,
; -1 without the movie. ebx, esi, edi and ebp kept.
hud_hidden:
    mov ecx, HUD_MOVIE
    call movie_of
    test eax, eax
    jz .none
    sub esp, 24                    ; the answer, a GFx value (a bool: nothing to release)
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
    movzx eax, byte [esp+8]
    add esp, 24
    test eax, eax
    setnz al
    ret
.none:
    or eax, -1
    ret

; edi the hud, [esp+4] 1 to hide, 0 to show: root.hide_interface of the hud's
; movie and the player icons'. ebx, esi, edi and ebp kept.
hide_hud:
    push ebx
    mov ebx, [esp+8]
    mov ecx, HUD_MOVIE
    call .one
    mov ecx, ICONS_MOVIE
    call .one
    pop ebx
    ret
.one:
    call movie_of
    test eax, eax
    jz .skip
    sub esp, 24                    ; one GFx value: a bool
    xor edx, edx
    mov [esp], edx
    mov dword [esp+4], GFX_BOOL
    mov [esp+8], ebx
    mov [esp+12], edx
    mov [esp+16], edx
    mov [esp+20], edx
    mov edx, esp
    push 1
    push edx
    push 0
    push hide_interface
    push eax
    call engine_movie_invoke
    add esp, 20 + 24
.skip:
    ret

; initialize_match's Invoke of root.reset (0x234AE8): ecx the hud movie, the
; four arguments pushed (`ret 10h`), ebp the hud. The reset, then the hud and
; the icons shown (the reset shows the hud already).
hide_ui_reset:
    push edi
    mov edi, ebp
    push dword [esp+20]            ; the count
    push dword [esp+20]            ; the arguments
    push dword [esp+20]            ; the result
    push dword [esp+20]            ; the name
    push ecx
    call engine_movie_invoke
    add esp, 20
    push eax
    push 0
    call hide_hud
    add esp, 4
    pop eax
    pop edi
    ret 16

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
hide_interface:
    db "root.hide_interface", 0
