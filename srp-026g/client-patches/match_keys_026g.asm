; Patch 58 (build 2777): in a match, the keys 1, 2 and 3 pick kit 1, 2 and 3
; while the kits are shown, and an option hides the other players' chat lines.
; The arena's blocks match_keys.code and match_keys.state (link_026g.py links
; them; patch_026g.py's MATCH_KEYS_HOOKS call and jump to three entries by
; their names, MATCH_KEYS_POLL_SLOT points the fourth's slot at it).
; client_patch.md section 3.53; the rules: ../docs/notes/match-hud-options.md.
;
; The input tick. Every game::tick runs the input world's tick (0x3B7BD0,
; at 0x1ECB67) before the game world's (0x1ECBE1), which turns a dead local
; player's hud to the kits. The input world's tick polls its devices (the
; keyboard's execute 0x3B8D20: DirectInput's buffered data since the last
; poll, and the keys down after it), then calls every input handler's slot
; +14h (the input world; `ret 4`), then has the devices hand out what they
; polled (the keyboard's process 0x3B9AF0: each buffered event in order, a
; key going down as kind 1 and one going up as kind 2, then every key down
; as kind 3, held), then calls every handler's slot +18h. The game world's
; handler (vtable 0x9FC588) has both slots `ret 4` (0x75AF0).
;
; The kits' screen and the keys (the fix of 2026-10-25: a key 3 pressed in
; play to heal, the death in the same frames, picked kit 3). A key's kind 1
; is an edge: a key held makes none. But the keys handed out in a tick are
; those polled at its start, so the first tick after the hud showed the kits
; hands out every key that went down in the frame before them, in play.
; And a player still pressing 3 to heal in the moment after the death, before
; seeing it, presses on the kits: the owner (2026-10-25) wants the keys
; ignored for KIT_KEYS_DELAY (300 ms) after the kits appear too.
; kit_keys, set at slot +14h of the game world's handler (match_keys_poll,
; after the poll, before the keys are handed out) from the hud's cursor
; (the kits, below) and GetTickCount, is one of three states; OPENING
; carries the time of the poll that first saw the kits (kit_keys_since,
; meaningful in OPENING alone). The transitions, total over the poll's one
; event (the kits shown at it or not, at the time now):
;   CLOSED  -- shown -->                          OPENING(now)
;   OPENING(t) -- shown, now - t < KIT_KEYS_DELAY --> OPENING(t)
;   OPENING(t) -- shown, now - t >= KIT_KEYS_DELAY --> OPEN
;   OPEN    -- shown -->                          OPEN
;   any     -- hidden -->                         CLOSED
; A key going down picks a kit only in OPEN: OPEN comes at a later poll than
; the one that saw the kits come, so every key OPEN hands out went down on
; them, at least KIT_KEYS_DELAY (less a frame) after they came. A key held or
; pressed before then picks nothing until it is let go and pressed again:
; held, it makes no kind 1.
;
; The keys. A key reaches the game world's on_keyboard_action (0x25CD00,
; thiscall: ecx = its input handler, game_world+1F8h; the input world, the
; DirectInput key, the kind 1 down / 2 up / 3 held; `ret 0Ch`) before the
; player's handler (priority 7 before 10). After its Tab test (a held Tab
; returns at 0x25CD3B) it asks `cmp dword [ebp+10h],1 / jnz near 0x25CF0D`
; at 0x25CD41 (edi = the handler, ebp its frame): the key's kind. The hud is
; the handler +4; its +2DCh (the handler's +2E0h) is set while the hud shows
; the cursor (show_cursor 0x23A930), which it does with the kits (0x2383B0:
; the death screen, and the kits before the start). A key 1, 2 or 3 going
; down in OPEN, the cursor still shown, then calls the hud movie's
; `root.select_kit_key(0, 1 or 2)`, which picks that kit as a click on it
; would when the kits are shown and the chat takes no typing (HUD.as). The
; function then goes on as it did, at 0x25CD4B (a key down) or 0x25CF0D (any
; other): the key is not taken, and goes on to the player's handler as before
; (a dead player sends no input).
;
; The chat. The gameplay options' option 4, "g_private_messages_in_game"
; (0xA064FC; `_no_in_batle` of main_menu.swf, labelled
; st_private_messages_in_game_option), names no console variable, so the
; option is dead (options_item_base 0x260E90 finds no command). The hud's
; constructor call (0x1F6845, in the game world's constructor, before the
; configs are run) registers a cc_bool of that name on hide_chat, kept in
; user.cfg, as the hud's own show_quests_progress is (0x2339C9): the option is
; live, its checkbox enabled by the movie and labelled "Hide chat in match"
; (its text in client-patches/localization/*.tsv). A received chat line goes through
; chat_handler::add_message (0x208AD0, stdcall: the handler, the channel, the
; body, the name, the tag; `ret 14h`) from process_incoming_text_message's
; (0x26B6F0) call at 0x26B916; with the option on, in a match (the handler's +0Dh, set_mode's
; flag) a line of a player channel the hud shows (3 clan, 5 match, 6 and 7
; the teams, 8 squad, 9) is dropped there. The server's lines (0, 2), the
; general and private channels (1, 4, which the hud does not show in a match:
; add_message gives them to the lobby's movie) and the player's own lines
; (the local echo, another call) are shown as before.
BITS 32
%include "engine_026g.inc"
global match_keys_poll, match_keys_keyboard, match_keys_hud, match_keys_chat
global chat_variable, hide_chat          ; the option's variable and its byte (check_match_keys_026g.py reads them)
global kit_keys, kit_keys_since          ; the kits' screen's state (check_match_keys_026g.py sets and reads it)
section .text

%define KEY_DOWN_BACK 0x25CD4B     ; on_keyboard_action's key down, after the site's `jnz`
%define KEY_OTHER 0x25CF0D         ; its `jnz` target: the key not taken (al 0)
%define HUD_CONSTRUCTOR 0x233810   ; game_world_ui::game_world_ui: eax = the game world, the hud; `ret 4`
%define ADD_MESSAGE 0x208AD0       ; chat_handler::add_message, `ret 14h`
%define OPTION_NAME 0xA064FC       ; "g_private_messages_in_game"
%define CC_BOOL_VTABLE 0x9E7C4C    ; cc_bool, as the hud's show_quests_progress (0x2339E8)
%define KEY_DOWN 1
%define DIK_1 2                    ; DIK_1..DIK_3: the keys 1, 2 and 3 above the letters
%define DIK_3 4
%define CURSOR 0x2E0               ; the handler's +2E0h: the hud's +2DCh, the cursor shown
%define HUD_MOVIE 0x0C             ; the handler's +0Ch: the hud's +8, its movie's holder
%define KEY 0x0C                   ; the frame's [ebp+0Ch]: the key
%define KIND 0x10                  ; [ebp+10h]: the kind
%define GFX_UINT 4                 ; a GFx value's type (+4): unsigned int
%define IN_MATCH 0x0D              ; the chat handler's +0Dh: set_mode's flag, its movie the hud
%define CLAN 3                     ; the channels the hud shows: 3, 5..9
%define MATCH 5
%define LAST_CHANNEL 9
%define CLOSED 0                   ; kit_keys: the kits not shown at the last poll
%define OPENING 1                  ; shown, for less than KIT_KEYS_DELAY since kit_keys_since
%define OPEN 2                     ; shown, for KIT_KEYS_DELAY or more
%define KIT_KEYS_DELAY 300         ; ms the kit keys pick nothing after the kits appear
%define GET_TICK_COUNT 0x870090    ; GetTickCount's import (stdcall, ms, wraps)

; Slot +14h of the game world's handler (0x9FC59C; thiscall: ecx = the
; handler, the input world; `ret 4`): a poll taken, its keys not yet handed
; out. kit_keys' transition.
match_keys_poll:
    cmp byte [ecx+CURSOR], 0
    je .closed
    cmp byte [kit_keys], OPENING
    je .opening
    ja .done                       ; OPEN stays
    call [GET_TICK_COUNT]          ; CLOSED -> OPENING(now)
    mov [kit_keys_since], eax
    mov byte [kit_keys], OPENING
    ret 4
.opening:
    call [GET_TICK_COUNT]
    sub eax, [kit_keys_since]      ; unsigned: over GetTickCount's wrap too
    cmp eax, KIT_KEYS_DELAY
    jb .done                       ; OPENING(t) stays
    mov byte [kit_keys], OPEN
.done:
    ret 4
.closed:
    mov byte [kit_keys], CLOSED
    mov dword [kit_keys_since], 0
    ret 4

; The game world's on_keyboard_action at 0x25CD41, its kind's test (edi = the
; handler, ebp the function's frame).
match_keys_keyboard:
    cmp dword [ebp+KIND], KEY_DOWN
    jne KEY_OTHER
    pushad
    mov eax, [ebp+KEY]
    sub eax, DIK_1
    cmp eax, DIK_3 - DIK_1
    ja .back                       ; not 1, 2 or 3
    cmp byte [kit_keys], OPEN
    jne .back                      ; the kits not up for KIT_KEYS_DELAY yet
    cmp byte [edi+CURSOR], 0
    je .back                       ; no kits shown
    mov edx, [edi+HUD_MOVIE]
    test edx, edx
    jz .back
    mov edx, [edx+0x108]
    test edx, edx
    jz .back
    mov edx, [edx+4]               ; the movie: [[[hud+8]+108h]+4], as the hud invokes it
    test edx, edx
    jz .back
    sub esp, 24                    ; one GFx value: no interface, an unsigned int, the kit
    mov dword [esp], 0
    mov dword [esp+4], GFX_UINT
    mov [esp+8], eax
    mov dword [esp+12], 0
    mov dword [esp+16], 0
    mov dword [esp+20], 0
    mov eax, esp
    push 1                         ; one argument
    push eax
    push 0                         ; no result
    push select_kit_key
    push edx
    call engine_movie_invoke
    add esp, 20 + 24
.back:
    popad
    jmp KEY_DOWN_BACK

; The game world's constructor's call of the hud's (0x1F6845): the hud made,
; then the option's variable registered, once.
match_keys_hud:
    push dword [esp+4]             ; the hud; eax the game world, as the site gives it
    call HUD_CONSTRUCTOR
    pushad
    cmp byte [registered], 0
    jne .done
    mov byte [registered], 1
    push 2                         ; the kind the hud's saved variables have
    push 1                         ; saved to user.cfg
    push hide_chat
    push OPTION_NAME
    push chat_variable
    call engine_cc_bool
    add esp, 20
    mov eax, CC_BOOL_VTABLE
    mov [chat_variable], eax       ; a cc_bool (not `mov dword [m],imm`: the self-test reads 8 bytes of ndisasm's line)
.done:
    popad
    ret 4

; process_incoming_text_message's call of add_message (0x26B916): a player's line dropped
; with the option on, in a match, on a channel the hud shows.
match_keys_chat:
    cmp byte [hide_chat], 0
    je .show
    mov eax, [esp+4]               ; the chat handler
    cmp byte [eax+IN_MATCH], 0
    je .show
    mov eax, [esp+8]               ; the channel
    cmp eax, CLAN
    je .hide
    cmp eax, MATCH
    jb .show
    cmp eax, LAST_CHANNEL
    ja .show
.hide:
    ret 0x14
.show:
    jmp ADD_MESSAGE

section .rodata
select_kit_key:
    db "root.select_kit_key", 0

section .bss
alignb 4
chat_variable:
    resb 0x48                      ; the cc_bool (cc_value<bool>, 0x1DDB0)
hide_chat:
    resb 1                         ; its value: 1 hides the lines
registered:
    resb 1
kit_keys:
    resb 1                         ; CLOSED, OPENING or OPEN (match_keys_poll)
alignb 4
kit_keys_since:
    resd 1                         ; OPENING's: GetTickCount at the poll that first saw the kits
