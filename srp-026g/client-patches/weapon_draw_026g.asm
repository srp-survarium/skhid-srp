; Patch 32, the draw: a weapon's upgrade window draws the lobby's player as
; the lobby does, in its idle pose with the weapon in its hands (build 2777,
; client_patch.md section 3.30). One entry; patch_026g.py checks the hook's
; target. At .srp+D20h, after the forward effect's scrolling (which ends at
; +D1Ch), before the shell_32.dll check (+E00h). It reads the mode of
; weapon_preview_026g.asm's state (STATE, 0xE00 of `.srpd`).
;   nasm -f bin -DCAVE=0xA425D20 -DSTATE=0xA428E00 weapon_draw_026g.asm -o weapon_draw_026g.section.bin
;
; The lobby character's update (0x2849F0) gives its player the upgrade
; window's mode at every frame (`mov [ecx+0F49Ch],al` at 0x284A33, al =
; character+D515h). With it set the player is drawn for the decal editor:
; player::mt_draw (0x2474A0) puts the skeleton in its bind pose (0x2476F8,
; the T-pose) and the held item 10 m under it (0x24778F), and
; player::render_model (0x1F3070; its test of +F49Ch at 0x1F30E2) takes
; another transform. While a weapon's
; window shows that weapon's kit (weapon_preview_026g.asm's mode 1) the
; player gets 0 instead, and is drawn as in the lobby. Else the original.
BITS 32
ORG CAVE

%ifndef STATE
%error STATE is required
%endif

%define MODE            0           ; the state's mode: 1 while a weapon's window shows the weapon
%define DECAL_POSE      0xF49C      ; player: drawn for the decal editor

; 0x284A33 `mov [ecx+0F49Ch],al` -> call player_mode + nop. ecx = the
; character's player, al = the window's mode. Keeps every register; no
; instruction after the site reads the flags before it sets them.
player_mode:
    push eax
    call .pc
.pc:
    pop eax
    cmp byte [eax + STATE + MODE - .pc], 0
    pop eax
    jne .weapon
    mov [ecx+DECAL_POSE], al
    ret
.weapon:
    mov byte [ecx+DECAL_POSE], 0
    ret
