; Patch 66 (build 2777): voice chat in a match, team push-to-talk through
; skhid_voice.dll (srp-026g/client-voice) and srp's voice relay. The arena's
; blocks voice.code and voice.state (link_026g.py links them; patch_026g.py's
; VOICE_HOOKS and VOICE_SLOTS name the entries below). The logic is
; voice_026g.c; this file is the glue at the game's sites, which hands each
; one to it as a plain cdecl call with every register kept.
; client_patch.md section 3.55; the rules: ../docs/notes/voice-chat.md.
;
; - voice_register: game::game's call of game_world::game_world (0x1E90A3,
;   `ret 8`, ebx the game): the world made, then the voice's console
;   variables registered (vc_register), before user.cfg runs and before the
;   options look their variables up;
; - voice_connect: match_client::connect (0x27A1B0, thiscall: the host, the
;   port, the session, the time, a functor; `ret 14h`) at its entry: the
;   voice started (vc_connect), then the function as it was. A replay's
;   match client has another connect (0x122350) and never gets here;
; - voice_finish: game_world::on_finish_match (0x1F96F0, eax the world) and
;   voice_deactivate: game_world::on_deactivate (0x1F9520, ecx the world) at
;   their entries: the voice stopped (vc_stop), then the function;
; - voice_tick: the game world's vtable slot 2 (0x9FC4FC: game_world::tick
;   0x1F77E0, ecx the world, `ret 0Ch`), called every frame of a world: the
;   voice's frame (vc_frame), then the tick;
; - voice_key: the game world's on_keyboard_action's call of
;   show_player_list (0x25CD34, esi the hud, the shown byte pushed; `ret 4`),
;   which every key of every kind passes (the Tab test, before patch 58's
;   kind test): the key to vc_key (the game, the key, the kind, the input
;   world: [edi-15Ch], [ebp+0Ch], [ebp+10h], [ebp+8]), then the call;
; - voice_menu: activate_main_menu's call of game_options::activate
;   (0x1EDBDC, eax the options, the state under the menu pushed; `ret 4`):
;   the menu shown, then its voice panel filled (vc_menu, which tells a
;   match from the lobby by the state);
; - voice_menu_call: game_options' Scaleform callback, slot 0x9FD47C
;   (0x1FF5D0, thiscall: ?, the name, the arguments, the count; `ret 10h`):
;   a call of the voice panel (srp_voice_*) is vc_menu_call's, anything
;   else the function's.
BITS 32
extern vc_register, vc_connect, vc_stop, vc_frame, vc_key, vc_menu, vc_menu_call
global voice_register, voice_connect, voice_finish, voice_deactivate, voice_tick
global voice_key, voice_menu, voice_menu_call, voice_cc_float
section .text

%define GAME_WORLD_CONSTRUCTOR 0x1F67B0  ; game_world::game_world, `ret 8`
%define CONNECT_BODY 0x27A1B5            ; match_client::connect after `push ebx / push ebp / push esi / mov esi,ecx`
%define FINISH_BODY 0x1F96F9             ; on_finish_match after `push edi / mov edi,eax / mov eax,[edi+9Ch]`
%define DEACTIVATE_BODY 0x1F9525         ; on_deactivate after `push ebx / push esi / push edi / mov edi,ecx`
%define TICK 0x1F77E0                    ; game_world::tick
%define SHOW_PLAYER_LIST 0x2384C0        ; game_world_ui::show_player_list, `ret 4`
%define OPTIONS_ACTIVATE 0x200F50        ; game_options::activate, `ret 4`
%define OPTIONS_CALLBACK 0x1FF5D0        ; game_options' Scaleform callback, `ret 10h`
%define CC_FLOAT 0x1DD40                 ; cc_value<float>::cc_value: eax the object, min in xmm0; `ret 14h`
%define CC_FLOAT_VTABLE 0x9E7C6C         ; cc_float, as the hud's (0x233AD7)
%define GAME 0x15C                       ; the input handler's -15Ch: the game ([edi-15Ch], 0x25CD4B)

voice_register:
    push dword [esp+8]
    push dword [esp+8]
    call GAME_WORLD_CONSTRUCTOR
    pushad
    call vc_register
    popad
    ret 8

voice_connect:
    pushad
    push dword [esp+44]            ; the session (entry +0Ch)
    push dword [esp+44]            ; the port (entry +8)
    push dword [esp+44]            ; the host (entry +4)
    call vc_connect
    add esp, 12
    popad
    push ebx
    push ebp
    push esi
    mov esi, ecx
    jmp CONNECT_BODY

voice_finish:
    pushad
    call vc_stop
    popad
    push edi
    mov edi, eax
    mov eax, [edi+0x9C]
    jmp FINISH_BODY

voice_deactivate:
    pushad
    call vc_stop
    popad
    push ebx
    push esi
    push edi
    mov edi, ecx
    jmp DEACTIVATE_BODY

voice_tick:
    pushad
    push ecx                       ; the game world
    call vc_frame
    add esp, 4
    popad
    jmp TICK

voice_key:
    pushad
    push dword [ebp+8]             ; the input world
    push dword [ebp+0x10]          ; the kind: 1 down, 2 held, 3 up
    push dword [ebp+0xC]           ; the key (DIK)
    push dword [edi-GAME]          ; the game
    call vc_key
    add esp, 16
    popad
    jmp SHOW_PLAYER_LIST

voice_menu:
    push eax                       ; the options
    push dword [esp+8]             ; the state under the menu
    call OPTIONS_ACTIVATE
    pop ecx
    pushad
    push dword [esp+36]            ; the state (entry +4)
    push ecx
    call vc_menu
    add esp, 8
    popad
    ret 4

voice_menu_call:
    pushad
    push dword [esp+48]            ; the count (entry +10h)
    push dword [esp+48]            ; the arguments (entry +0Ch)
    push dword [esp+48]            ; the name (entry +8)
    push ecx                       ; the handler
    call vc_menu_call
    add esp, 16
    test eax, eax
    popad
    jnz .taken
    jmp OPTIONS_CALLBACK
.taken:
    ret 0x10

; void voice_cc_float(void *object, const char *name, float *value, float max,
; unsigned saved, unsigned kind): cc_value<float>'s constructor with min 0
; (its min is in xmm0: no library row), the object then a cc_float.
voice_cc_float:
    push ebp
    push ebx
    push esi
    push edi
    mov ebx, [esp+20]              ; the object
    push dword [esp+40]            ; the kind
    push dword [esp+40]            ; saved
    push dword [esp+40]            ; max
    push dword [esp+40]            ; the value
    push dword [esp+40]            ; the name
    mov eax, ebx
    xorps xmm0, xmm0               ; min 0
    call CC_FLOAT                  ; pops its five
    mov eax, CC_FLOAT_VTABLE
    mov [ebx], eax
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

; The action table's entry 37h is renamed to this (patch_026g.py's
; voice_writes): `bind mute_last_speaker <key>` in user.cfg, the options'
; 42nd controls row (hide_ui_026g.asm). In a data block: the game reads it.
section .data
global mute_action_name
mute_action_name:
    db "mute_last_speaker", 0
