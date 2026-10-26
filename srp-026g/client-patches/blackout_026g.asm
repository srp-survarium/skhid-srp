; Patch 65 (build 2777): Blackout, a battery match with no interface but the
; players' markers (prototype). The arena's block blackout.code
; (link_026g.py links it with blackout_026g.c, the logic; its state, the
; options last read, in blackout.state); patch_026g.py's BLACKOUT_HOOKS jump
; to the entries by their names. The other parts of the patch are calls from
; patch 63's and patch 62's entries: td_options_entry calls bo_options (srp's
; block's tag 2), hide_ui_reset calls bo_match (hide_ui_026g.asm). The rules:
; ../docs/notes/blackout.md; client_patch.md section 3.58.
;
; - bo_tab_entry: game_world::on_keyboard_action's `call 0x2384C0` at
;   0x25CD34 (game_world_ui::show_player_list, esi the hud, [esp+4] the byte:
;   Tab down, as the game asks the keyboard at every key; bl the same byte,
;   which then sends every key to the function's end, 0x25CD3B, as long as
;   the Tab list is up): the call, then bo_tab keeps Tab's state and says
;   whether the key goes on as with Tab up (a Blackout match: no Tab list,
;   the kits' keys 1 to 3 of patch 58 at 0x25CD41 reach the kits Tab shows):
;   bl 0 then.
; - bo_input_entry: player_input_handler::process_first_person_mode
;   (0x241250, ebx the handler) at its epilogue 0x2421B0 (`pop edi / pop esi
;   / pop ebx / mov esp,ebp`; the early exit at 0x241284 and the last
;   actions' jumps come there too): bo_input filters the input's actions mask
;   (+33Ch, what the game applies and the relay carries), then the four
;   instructions and on at 0x2421B5 (`pop ebp / ret 8`).
BITS 32
global bo_tab_entry, bo_input_entry
extern bo_tab, bo_input
section .text

%define SHOW_PLAYER_LIST 0x2384C0  ; game_world_ui::show_player_list (esi the hud, [esp+4] the byte; ret 4)
%define INPUT_BACK 0x2421B5        ; process_first_person_mode's `pop ebp / ret 8`
%define INPUT_MASK 0x33C           ; the handler's actions mask

; At 0x25CD34: [esp+4] the byte (Tab down), esi the hud, bl the byte; the
; callee's `ret 4`.
bo_tab_entry:
    push dword [esp+4]
    call SHOW_PLAYER_LIST          ; as the game calls it (it pops its byte)
    pushad
    movzx eax, byte [esp+32+4]
    push eax
    call bo_tab
    add esp, 4
    test eax, eax
    jz .list
    mov byte [esp+16], 0           ; pushad's ebx: bl, the key goes on
.list:
    popad
    ret 4

; At 0x2421B0: ebx the input handler.
bo_input_entry:
    pushad
    lea eax, [ebx+INPUT_MASK]
    push eax
    call bo_input
    add esp, 4
    popad
    pop edi
    pop esi
    pop ebx
    mov esp, ebp
    jmp INPUT_BACK
