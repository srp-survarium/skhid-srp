; Patch 64 (build 2777): Blackout, a battery match with no interface and a
; team's band on every character (prototype). The arena's block blackout.code
; (link_026g.py links it with blackout_026g.c, the logic; its state, the
; options last read, in blackout.state); patch_026g.py's BLACKOUT_HOOKS jump
; to the entry by its name. The other two parts of the patch are calls from
; patch 63's and patch 62's entries: td_options_entry calls bo_options (srp's
; block's tag 2), hide_ui_reset calls bo_match (hide_ui_026g.asm). The rules:
; ../docs/notes/blackout.md; client_patch.md section 3.57.
;
; - bo_visual_entry: profile_skin_visual_cook::on_configs_loaded (0x204560) at
;   0x2056F0, after the parts and the head (`mov edx,[esp+58h] / mov
;   esi,[ebp+0Ch]`, both paths join there: 0x205653's jump and the head's
;   end), the function's own esp: [esp+1Ch] the cook data (the visual),
;   [esp+348h] its name's {begin, cursor, end}, [ebp+10h] the profile.
;   bo_visual adds the team's band; then the two instructions and on at
;   0x2056F7.
BITS 32
global bo_visual_entry
extern bo_visual
section .text

%define VISUAL_BACK 0x2056F7       ; on_configs_loaded after the two instructions

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
