; Research (exe patch 53): the four hooks of the mode, then the stubs the
; game calls of the rule and its cook, in front of the Rust crate
; research_arena (its logic: crates/research-rule) in the arena's block
; research.code (link_026g.py links it; patch_026g.py's RESEARCH_HOOKS call or
; jump to the four entries by their names). The game's frames are aligned
; to 4 only and Rust's i686 code assumes 16 at a call: every way into the
; crate is a stub here that aligns the stack first.
;
;   +0  register  game's constructor 0x1E9B4C (`mov eax,ebx / call 0x1EC170`,
;                 after its own client cooks, ebx = the game): `call` here;
;                 research_register_cook(game) makes and registers the rule's
;                 cook, then the two instructions are done here
;   +5  hud       game_world_ui::update_ui 0x23B3DE (`cmp dword [edi+20h],4 /
;                 jne 0x23B3FC`, after the teams' points went to the movie,
;                 edi = the ui): `call` here; research_update_hud(ui) every
;                 frame, then the compare and its branch
;   +A  welcome   update_ui 0x23B351 (`cmp eax,4 / jne 0x23B36A`, the match's
;                 first message by mode): `jmp` here; mode 5's is
;                 st_start_match_welcome_message_5 (0.27d's key)
; +100h           the slots: the rule's vtable (rn_rule_vtable), its tick
;                 delegate and the cook's vtable (rn_cook_vtable) point at
;                 the rn_* stubs, each a thiscall of the game's that calls the
;                 crate's research_* (cdecl, ecx first) on an aligned stack
;                 and pops what the game's own slot in its place pops
;
bits 32
global research_register, research_hud, research_welcome, research_icons
extern research_update_markers
extern research_register_cook, research_update_hud      ; the Rust functions
section .text

%define GAME_COOKS_DONE 0x1EC170   ; what the hooked call called
%define HUD_NOT_TDM     0x23B3FC   ; update_ui: past set_last_players
%define WELCOME_SHOW    0x23B35B   ; update_ui: push 1770h ... call 0x2375C0
%define WELCOME_NONE    0x23B36A   ; update_ui: no message for this mode
%define WELCOME_TDM     0xA03AAC   ; st_start_match_welcome_message_3

entries:
research_register:
    jmp near register
research_hud:
    jmp near hud
research_welcome:
    jmp near welcome
research_icons:
    jmp near icons

register:
    pushad
    mov ebp, esp                    ; popad gives the game's back
    and esp, -16
    sub esp, 12
    push ebx
    call research_register_cook
    mov esp, ebp
    popad
    mov eax, ebx
    add dword [esp], 2              ; return past the replaced call
    jmp GAME_COOKS_DONE

hud:
    pushad
    mov ebp, esp
    and esp, -16
    sub esp, 12
    push edi
    call research_update_hud
    mov esp, ebp
    popad
    cmp dword [edi + 0x20], 4
    je .tdm
    mov dword [esp], HUD_NOT_TDM
.tdm:
    ret

welcome:
    cmp eax, 4
    jne .research
    mov ecx, WELCOME_TDM
    jmp WELCOME_SHOW
.research:
    cmp eax, 5
    jne WELCOME_NONE
    mov ecx, welcome_key
    jmp WELCOME_SHOW

; Keep the original update_player_icons call and its ret 4 ABI. Rust is
; entered on a 16-byte aligned stack; flags and all game registers survive.
icons:
    push dword [esp + 4]
    call 0x23AA10
    pushfd
    pushad
    mov ebp, esp
    and esp, -16
    sub esp, 12
    push dword [ebp + 0x28]
    call research_update_markers
    mov esp, ebp
    popad
    popfd
    ret 4

welcome_key:
    db "st_start_match_welcome_message_5", 0

    times 0x100 - ($ - $$) db 0xCC

; A slot the game calls: %1, a thiscall of %3 dwords on the stack, which it
; pops (`ret 4 x %3`); calls %2(ecx, the dwords) (cdecl) with the stack
; aligned to 16 at the call. EAX is %2's. %4, when given, is done first.
%macro SLOT 3-4
global %1
extern %2
%1:
%if %0 > 3
    %4
%endif
    push ebp
    mov ebp, esp
    and esp, -16
%assign pad (-4 * (%3 + 1)) & 15
%if pad
    sub esp, pad
%endif
%assign i %3
%rep %3
    push dword [ebp + 4 + 4 * i]
%assign i i - 1
%endrep
    push ecx
    call %2
    leave
%if %3
    ret 4 * %3
%else
    ret
%endif
%endmacro

; the tick delegate (timelimit's 0x317F10: dt, time, step)
SLOT rn_tick,           research_rule_tick, 3
; the rule's slots (tdm_rule_core's in the same places)
SLOT rn_on_event,       research_rule_on_event, 3          ; 7: type, data, time
SLOT rn_entered,        research_rule_entered, 2           ; 8: place, time
SLOT rn_left,           research_rule_left, 2              ; 9: place, time
; 11: the world core; the tick delegate's function (the rule's +114h) is
; rn_tick, written here (patch_026g.py's CALLBACKS finds it by this `mov`)
SLOT rn_set_core,       research_rule_set_core, 1, {mov dword [ecx + 0x114], rn_tick}
SLOT rn_set_blackboard, research_rule_set_blackboard, 1    ; 12: the blackboard
SLOT rn_serialize,      research_rule_serialize, 1         ; 15: the writer
SLOT rn_deserialize,    research_rule_deserialize, 2       ; 16: the reader, a dword
SLOT rn_fill_hud,       research_rule_fill_hud, 2          ; 17: the hud state, a dword
SLOT rn_completed,      research_rule_completed, 1         ; 18: the time
; the cook's (tdm_rule_core_cook's 9 and 10)
SLOT rn_cook_create,    research_cook_create_rule, 1       ; the query
SLOT rn_cook_size,      research_cook_rule_size, 0
