; Patch 63 (build 2777): team damage turned back on its dealer past a
; threshold, as 0.28a to 0.31a2 had it. The arena's block team_damage.code
; (link_026g.py links it with team_damage_026g.c, the logic); patch_026g.py's
; TEAM_DAMAGE_HOOKS jump and call to the six entries by their names.
; client_patch.md section 3.54; the rules: ../docs/notes/team-damage.md.
;
; - td_hit: base_player::hit_impl's entry (0x2D9B90; stdcall: the player, the
;   hit; `ret 8`). td_reflect says whether the hit goes to its dealer
;   instead (it then calls hit_impl for the dealer itself); if not, the
;   function's own first three instructions and on at 0x2D9B96.
; - td_lost: hit_impl's `subss xmm0,[eax+0A0h]` at 0x2D9EA9 (ebx the
;   player, edi the hit, eax the pain pool; xmm0 the pain before, after it
;   the pain the hit took, which the player's on_hit is then given; x87's
;   st0 holds a value: the C code is SSE only). td_count counts it; back at
;   0x2D9EB1 with every register and xmm0 as the instruction left them.
; - td_options: match_options::deserialize's epilogue at 0x1F0F30 (ebx the
;   options, esi the reader): td_options reads the srp block, then the
;   function's own five pops and `ret 8`.
; - td_kd: kd_stats_rule_cook's create, its `call 0x323790` (the rule's
;   constructor: esi the rule, the query; `ret 4`, eax the rule) at
;   0x2CCE4A: the constructor, then td_kd_made.
; - td_save: game_world_core::serialize at 0x2D2716, after the effects'
;   three writes (ebp the core, edi their writer, esi the history item):
;   td_save, then `mov byte [esi+0C0BBAh],0` and on at 0x2D271D.
; - td_load: game_world_core::deserialize at 0x2D2C75, after the effects'
;   three reads (ebp the core, the reader at [esp+3Ch]): td_load, then
;   `mov eax,[ebp+19034h]` and on at 0x2D2C7B.
BITS 32
%include "engine_026g.inc"
global td_hit, td_lost, td_options_entry, td_kd, td_save_entry, td_load_entry
extern td_reflect, td_count, td_options, td_kd_made, td_save, td_load
section .text

%define HIT_IMPL_BODY 0x2D9B96     ; hit_impl after `push ebp / mov ebp,esp / sub esp,6Ch`
%define LOST_BACK 0x2D9EB1         ; after the `subss`
%define KD_CONSTRUCTOR 0x323790
%define SAVE_BACK 0x2D271D
%define LOAD_BACK 0x2D2C7B
%define PAIN 0xA0                  ; the pain pool's value
%define HISTORY_COMPLETED 0xC0BBA  ; the history item's "a rule answers completed" byte
%define CORE_ENTITIES 0x19034      ; [ebp+19034h], the next read's

; base_player::hit_impl's entry: [esp+4] the player, [esp+8] the hit.
td_hit:
    pushad
    push dword [esp+32+8]          ; the hit
    push dword [esp+32+4+4]        ; the player
    call td_reflect
    add esp, 8
    test eax, eax
    popad                          ; (no flags changed)
    jnz .turned
    push ebp
    mov ebp, esp
    sub esp, 0x6C
    jmp HIT_IMPL_BODY
.turned:
    ret 8

; hit_impl at 0x2D9EA9.
td_lost:
    subss xmm0, [eax+PAIN]
    pushad
    sub esp, 16
    movdqu [esp], xmm0
    push dword [esp]               ; the pain lost (a float)
    push edi                       ; the hit
    push ebx                       ; the player
    call td_count
    add esp, 12
    movdqu xmm0, [esp]
    add esp, 16
    popad
    jmp LOST_BACK

; match_options::deserialize's end, at 0x1F0F30.
td_options_entry:
    pushad
    push esi                       ; the reader
    push ebx                       ; the options
    call td_options
    add esp, 8
    popad
    pop edi
    pop esi
    pop ebp
    pop ebx
    pop ecx
    ret 8

; kd_stats_rule_cook's create, at 0x2CCE4A: esi the rule, [esp+4] the query.
td_kd:
    push dword [esp+4]             ; the query
    call KD_CONSTRUCTOR            ; eax = esi = the rule
    pushad
    push dword [esp+32+4]          ; the query
    push eax                       ; the rule
    call td_kd_made
    add esp, 8
    popad
    ret 4

; game_world_core::serialize, at 0x2D2716.
td_save_entry:
    pushad
    push edi                       ; the effects' writer
    push ebp                       ; the core
    call td_save
    add esp, 8
    popad
    mov byte [esi+HISTORY_COMPLETED], 0
    jmp SAVE_BACK

; game_world_core::deserialize, at 0x2D2C75: the reader at [esp+3Ch].
td_load_entry:
    pushad
    lea eax, [esp+32+0x3C]
    push eax                       ; the reader
    push ebp                       ; the core
    call td_load
    add esp, 8
    popad
    mov eax, [ebp+CORE_ENTITIES]
    jmp LOAD_BACK
