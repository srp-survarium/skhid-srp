; Patch 65 (build 2777): Blackout, a battery match with no interface and a
; team's band on every character (prototype). The arena's block blackout.code
; (link_026g.py links it with blackout_026g.c, the logic; its state, the
; options last read, in blackout.state); patch_026g.py's BLACKOUT_HOOKS jump
; to the entry by its name. The other two parts of the patch are calls from
; patch 63's and patch 62's entries: td_options_entry calls bo_options (srp's
; block's tag 2), hide_ui_reset calls bo_match (hide_ui_026g.asm). The rules:
; ../docs/notes/blackout.md; client_patch.md section 3.58.
;
; - bo_visual_entry: profile_skin_visual_cook::on_configs_loaded (0x204560) at
;   0x2056F0, after the parts and the head (`mov edx,[esp+58h] / mov
;   esi,[ebp+0Ch]`, both paths join there: 0x205653's jump and the head's
;   end), the function's own esp: [esp+1Ch] the cook data (the visual),
;   [esp+348h] its name's {begin, cursor, end}, [ebp+10h] the profile.
;   bo_visual adds the team's band; then the two instructions and on at
;   0x2056F7.
; - bo_tab_entry: game_world::on_keyboard_action's `call 0x2384C0` at
;   0x25CD34 (game_world_ui::show_player_list, esi the hud, [esp+4] the byte:
;   Tab down, as the game asks the keyboard at every key): bo_tab keeps it,
;   then the call.
; - bo_input_entry: player_input_handler::process_first_person_mode
;   (0x241250, ebx the handler) at its epilogue 0x2421B0 (`pop edi / pop esi
;   / pop ebx / mov esp,ebp`; the early exit at 0x241284 and the last
;   actions' jumps come there too): bo_input filters the input's actions mask
;   (+33Ch, what the game applies and the relay carries), then the four
;   instructions and on at 0x2421B5 (`pop ebp / ret 8`).
BITS 32
global bo_visual_entry, bo_tab_entry, bo_input_entry
extern bo_visual, bo_tab, bo_input
section .text

%define VISUAL_BACK 0x2056F7       ; on_configs_loaded after the two instructions
%define SHOW_PLAYER_LIST 0x2384C0  ; game_world_ui::show_player_list (esi the hud, [esp+4] the byte; ret 4)
%define INPUT_BACK 0x2421B5        ; process_first_person_mode's `pop ebp / ret 8`
%define INPUT_MASK 0x33C           ; the handler's actions mask

bo_visual_entry:
    pushad
    lea eax, [esp+32+0x348]
    push eax                       ; the name
    push dword [ebp+0x10]          ; the profile
    push dword [esp+32+8+0x1C]     ; the visual
    call bo_visual
    add esp, 12
    popad
    mov edx, [esp+0x58]
    mov esi, [ebp+0x0C]
    jmp VISUAL_BACK

; At 0x25CD34: [esp+4] the byte (Tab down), esi the hud; the callee's `ret 4`.
bo_tab_entry:
    pushad
    movzx eax, byte [esp+32+4]
    push eax
    call bo_tab
    add esp, 4
    popad
    jmp SHOW_PLAYER_LIST

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
