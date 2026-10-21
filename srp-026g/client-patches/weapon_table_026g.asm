; Patch 45: a weapon's upgrade window shows the weapon in the air above the
; lobby's workbench (build 2777, client_patch.md section 3.42). Patch 32 still
; decides what is shown (a kit's weapon, or a stash weapon in the chosen kit's
; hands from a scratch profile) and the lobby character still holds it; here
; the camera flies to the workbench, the held weapon is drawn there on its own
; and a drag turns it. At WEAPON_TABLE_OFFSET of `.srpw`, after the stash
; code; its state (40 bytes) at STATE, WEAPON_TABLE_STATE_OFFSET of `.srpd`;
; patch 32's mode at PREVIEW (WEAPON_PREVIEW_STATE_OFFSET).
;   nasm -f bin -DCAVE=0xA429800 -DSTATE=0xA428F00 -DPREVIEW=0xA428E00 weapon_table_026g.asm -o weapon_table_026g.section.bin
;
; The lobby camera (lobby menu+10Ch; ctor 0x283BC0, tick 0x283EF0 with ecx =
; camera+4, setter 0x2845C0) looks at the point (+A0h, +A4h, +A8h) from the
; distance +C4h, turned by the yaw +B8h and the pitch +BCh (rotY(yaw) *
; rotX(pitch): forward (-sin yaw, ., cos yaw), the eye = point - distance *
; forward); the side angle +ACh turns only the view. Only its ctor writes +A0h
; and +A8h (0): the stock camera always looks at the lobby character, who
; stands at the origin (0x21CCD2). A drag adds to +CCh (yaw) and +D0h (pitch)
; (on_mouse_move 0x283D30), which the tick adds to +B8h/+BCh (0x284020).
;
; Entries (a jump table, so that each stays where patch_026g.py calls it):
;   +0  camera   patch 32's start_cam and exit_cam jump here instead of the
;                setter (eax = the camera, ecx = where it keeps the distance,
;                xmm0 the height, xmm1 the distance, [esp+4] the side angle):
;                patch 32's mode 1 (a weapon shown) turns the table on: the
;                camera's point goes to BENCH (the height and the distance
;                by the setter), its yaw and pitch to the bench's view, the
;                weapon's turn to 0; mode 0 with the table on (a close, or
;                an armour's window over a weapon's) turns it off: the point
;                goes back to the origin, the yaw and the pitch to what they
;                were before. Then the setter.
;   +5  tick     0x284020 `movss xmm0,[ebx+0C8h]` (the camera's tick, ebx =
;                camera+4, [esp+10h] the frame's time, at most 0.1 s): while
;                the table is on, a drag turns the weapon (the yaw's move its
;                spin, the pitch's its tilt) instead of the camera; while the
;                camera flies, its point, yaw and pitch move a share RATE * dt
;                of the way to their targets, and once off and there it is
;                the stock camera again.
;   +10 cull     0x247576 `cmp byte [ebp-1],0 / jne 247592h` in player::mt_draw
;                (0x2474A0): the table's player takes the draw path even when
;                its own model was not seen (the camera looks away from it),
;                so that its held item is drawn.
;   +15 matrix   0x24778F `cmp byte [ebx+0F49Ch],0 / je 2477C2h`: the table's
;                player's held item is drawn with the bench's transform
;                ([ebp-0E8h], the decal pose's buffer) instead of its hand's.
; The table's player: mt_draw called by lobby_character's update (return
; address 0x284A83), whose character (the update's ebx, kept by mt_draw at
; [ebp-0F0h]) has that player (+D2B8h) and its upgrade window open (+D515h:
; only the lobby's own character), while the table is on. Anything else is
; the original's, byte for byte. The code finds its state with call/pop.
BITS 32
ORG CAVE

%ifndef STATE
%error STATE is required
%endif
%ifndef PREVIEW
%error PREVIEW is required
%endif

%define CAMERA_SET      0x2845C0    ; lobby camera: xmm0 height, xmm1 distance, [esp+4] side angle; `ret 4`
%define MODE            0           ; patch 32's state: 1 while its window's weapon is shown
%define LOBBY_DRAW      0x284A83    ; after lobby_character's update's `call 0x2474A0`
%define CHARACTER       -0xF0       ; mt_draw's frame: the update's ebx (mt_draw's first push after its frame)
%define LOCAL           -1          ; mt_draw's frame: the player is the world's own (draws whatever is seen)
%define DECAL           0xF49C      ; player: drawn for the decal editor
%define MATRIX          -0xE8       ; mt_draw's frame: the decal pose's transform (64 bytes)
%define PLAYER          0xD2B8      ; lobby_character: its player
%define UPGRADE_MODE    0xD515      ; lobby_character: 1 while an upgrade window is open

; mt_draw's sites: the call's return, and where the original goes
%define CULL_RET        0x24757B
%define DRAW            0x247592    ; the draw path
%define MATRIX_RET      0x247794
%define HAND            0x2477C2    ; `lea eax,[ebp-0A8h]`: the hand's transform
%define DRAW_ITEM       0x2477C8    ; eax = the item's transform

; The camera (lobby menu+10Ch), from the camera itself
%define POINT_X         0xA0
%define POINT_Z         0xA8
%define YAW             0xB8
%define PITCH           0xBC
%define TURN_YAW        0xCC        ; a drag's moves of the frame
%define TURN_PITCH      0xD0

; The state, at STATE
%define TABLE           0x00        ; 1: the weapon is at the bench
%define FLY             0x04        ; 1: the camera is the table's (going there, there, or coming back)
%define TARGET_X        0x08        ; the camera's targets: point x, point z, yaw, pitch
%define TARGET_Z        0x0C
%define TARGET_YAW      0x10
%define TARGET_PITCH    0x14
%define SAVED_YAW       0x18        ; the camera's yaw and pitch before the table
%define SAVED_PITCH     0x1C
%define SPIN            0x20        ; the weapon's turn about the vertical, by the drag's yaw
%define TILT            0x24        ; and about the bench's length, by its pitch (within TILT_MAX)
%define CONST(label)    ((label) - STATE)

    jmp near camera
    jmp near tick
    jmp near cull
    jmp near matrix

; patch 32's start_cam and exit_cam: `jmp camera` where they jumped to the
; setter. Every register but xmm0 and xmm1 (the bench's while it is on) is
; the caller's; the setter follows.
camera:
    pushad
    mov esi, eax                    ; esi = the camera
    call base                       ; edi = the state
    cmp byte [edi + PREVIEW - STATE + MODE], 0
    je .off
    cmp dword [edi+FLY], 0
    jne .on                         ; on, or still coming back: the yaw and pitch kept are the stock camera's
    mov eax, [esi+YAW]
    mov [edi+SAVED_YAW], eax
    mov eax, [esi+PITCH]
    mov [edi+SAVED_PITCH], eax
.on:
    mov dword [edi+TABLE], 1
    mov dword [edi+FLY], 1
    xor eax, eax
    mov [edi+SPIN], eax
    mov [edi+TILT], eax
    mov eax, [edi+CONST(bench_x)]
    mov [edi+TARGET_X], eax
    mov eax, [edi+CONST(bench_z)]
    mov [edi+TARGET_Z], eax
    mov eax, [edi+CONST(view_pitch)]
    mov [edi+TARGET_PITCH], eax
    fld dword [edi+CONST(two_pi)]   ; the bench's yaw, the nearest turn of it to the camera's
    fld dword [edi+CONST(view_yaw)]
    fsub dword [esi+YAW]
.turns:
    fprem1
    fnstsw ax
    test ah, 4
    jnz .turns
    fadd dword [esi+YAW]
    fstp dword [edi+TARGET_YAW]
    fstp st0
    movss xmm0, [edi+CONST(bench_y)]
    movss xmm1, [edi+CONST(view_distance)]
    jmp .set
.off:
    cmp dword [edi+TABLE], 0
    je .set
    xor eax, eax
    mov [edi+TABLE], eax            ; FLY stays: the camera flies back
    mov [edi+TARGET_X], eax
    mov [edi+TARGET_Z], eax
    mov eax, [edi+SAVED_YAW]
    mov [edi+TARGET_YAW], eax
    mov eax, [edi+SAVED_PITCH]
    mov [edi+TARGET_PITCH], eax
.set:
    popad
    jmp CAMERA_SET

; 0x284020 `movss xmm0,[ebx+0C8h]` -> call tick + 3 nops. ebx = camera+4,
; [esp+14h] the frame's time. Keeps every register but xmm0 (the original's
; load); what follows reads no flag before it sets them.
tick:
    pushad
    sub esp, 48
    movups [esp], xmm1
    movups [esp+16], xmm2
    movups [esp+32], xmm3
    call base
    cmp dword [edi+FLY], 0
    je .out
    lea esi, [ebx-4]                ; the camera
    cmp dword [edi+TABLE], 0
    je .fly
    movss xmm0, [esi+TURN_YAW]      ; the drag turns the weapon, not the camera
    addss xmm0, [edi+SPIN]
    movss [edi+SPIN], xmm0
    movss xmm0, [esi+TURN_PITCH]
    addss xmm0, [edi+TILT]
    maxss xmm0, [edi+CONST(tilt_min)]
    minss xmm0, [edi+CONST(tilt_max)]
    movss [edi+TILT], xmm0
    xor eax, eax
    mov [esi+TURN_YAW], eax
    mov [esi+TURN_PITCH], eax
.fly:
    xor ecx, ecx                    ; ecx = 1 once a value is still away from its target
    lea edx, [edi+CONST(fields)]
.away:
    movzx eax, byte [edx]
    movzx ebp, byte [edx+1]
    movss xmm0, [edi+ebp]
    subss xmm0, [esi+eax]
    mulss xmm0, xmm0
    comiss xmm0, [edi+CONST(close)]
    jb .near
    mov cl, 1
.near:
    add edx, 2
    cmp byte [edx], 0
    jne .away
    movss xmm1, [esp+48+32+14h]     ; the share of the way: RATE * dt, at most 1
    mulss xmm1, [edi+CONST(rate)]
    minss xmm1, [edi+CONST(one)]
    lea edx, [edi+CONST(fields)]
.step:
    movzx eax, byte [edx]
    movzx ebp, byte [edx+1]
    test ecx, ecx
    jnz .move
    mov ebp, [edi+ebp]              ; there: the target itself
    mov [esi+eax], ebp
    jmp .stepped
.move:
    movss xmm0, [edi+ebp]
    movss xmm2, [esi+eax]
    subss xmm0, xmm2
    mulss xmm0, xmm1
    addss xmm0, xmm2
    movss [esi+eax], xmm0
.stepped:
    add edx, 2
    cmp byte [edx], 0
    jne .step
    test ecx, ecx
    jnz .out
    cmp dword [edi+TABLE], 0
    jne .out
    mov [edi+FLY], ecx              ; back where it was: the stock camera
.out:
    movups xmm1, [esp]
    movups xmm2, [esp+16]
    movups xmm3, [esp+32]
    add esp, 48
    popad
    movss xmm0, [ebx+0C8h]
    ret

; 0x247576 `cmp byte [ebp-1],0 / jne 247592h` -> call cull + nop. Returns to
; 0x24757C (the original's next test) or to 0x247592 (the draw path); keeps
; every register. Neither place reads a flag before setting it.
cull:
    cmp byte [ebp+LOCAL], 0
    jne .draw
    call is_table
    jz .ret
.draw:
    add dword [esp], DRAW - CULL_RET
.ret:
    ret

; 0x24778F `cmp byte [ebx+0F49Ch],0 / je 2477C2h` -> call matrix + 4 nops.
; ebx = the player, ebp = mt_draw's frame. The table's player: eax = the
; bench's transform and on at 0x2477C8, keeping every other register; else
; the original's way (the decal pose's at 0x247798, the hand's at 0x2477C2),
; every register kept. None of the three reads a flag before setting it.
matrix:
    call is_table
    jnz .bench
    cmp byte [ebx+DECAL], 0
    jne .ret                        ; the 4 nops, then the decal pose's transform
    add dword [esp], HAND - MATRIX_RET
.ret:
    ret
.bench:
    push edi
    push eax
    call base                       ; edi = the state
    lea eax, [ebp+MATRIX]
    ; rows (v * M): rotY(spin) * rotZ(tilt), the centre CENTRE_Z of the
    ; model (along its length) at BENCH
    fld dword [edi+SPIN]
    fsincos                         ; st0 = cos a, st1 = sin a
    fld dword [edi+TILT]
    fsincos                         ; st0 = cos b, st1 = sin b, st2 = cos a, st3 = sin a
    fld st1
    fchs
    fstp dword [eax+10h]            ; row 1: (-sin b, cos b, 0)
    fst dword [eax+14h]
    fld st0
    fmul st0, st3
    fstp dword [eax+00h]            ; row 0: (cos a cos b, cos a sin b, sin a)
    fld st1
    fmul st0, st3
    fstp dword [eax+04h]
    fld st3
    fstp dword [eax+08h]
    fld st0
    fmul st0, st4
    fchs
    fstp dword [eax+20h]            ; row 2: (-sin a cos b, -sin a sin b, cos a)
    fld st1
    fmul st0, st4
    fchs
    fstp dword [eax+24h]
    fld st2
    fstp dword [eax+28h]
    fcompp
    fcompp
    fld dword [edi+CONST(centre_z)] ; row 3: BENCH - CENTRE_Z * row 2
    fmul dword [eax+20h]
    fsubr dword [edi+CONST(bench_x)]
    fstp dword [eax+30h]
    fld dword [edi+CONST(centre_z)]
    fmul dword [eax+24h]
    fsubr dword [edi+CONST(bench_y)]
    fstp dword [eax+34h]
    fld dword [edi+CONST(centre_z)]
    fmul dword [eax+28h]
    fsubr dword [edi+CONST(bench_z)]
    fstp dword [eax+38h]
    xor edi, edi
    mov [eax+0Ch], edi
    mov [eax+18h], edi
    mov [eax+1Ch], edi
    mov [eax+2Ch], edi
    mov dword [eax+3Ch], 0x3F800000 ; 1.0
    add esp, 4                      ; eax: the transform
    pop edi
    add dword [esp], DRAW_ITEM - MATRIX_RET
    ret

; ZF clear when ebx is the lobby character's player drawn by its update
; (ebp = mt_draw's frame) with the upgrade window open and the table on.
; Keeps every register.
is_table:
    push eax
    push ecx
    call base_eax
    cmp dword [eax+TABLE], 0
    je .no
    lea ecx, [eax + LOBBY_DRAW - STATE]
    cmp [ebp+4], ecx
    jne .no
    mov ecx, [ebp+CHARACTER]
    cmp [ecx+PLAYER], ebx
    jne .no
    cmp byte [ecx+UPGRADE_MODE], 0  ; ZF set: closed
    jmp .end
.no:
    cmp eax, eax
.end:
    pop ecx
    pop eax
    ret

; edi = the state where this image was loaded (eax is clobbered).
base:
    call base_eax
    mov edi, eax
    ret
; eax = the state. No flag changes.
base_eax:
    call .pc
.pc:
    pop eax
    lea eax, [eax + STATE - .pc]
    ret

; the camera's offsets and the state's targets the flight moves, then 0
fields:           db POINT_X, TARGET_X, POINT_Z, TARGET_Z, YAW, TARGET_YAW, PITCH, TARGET_PITCH, 0
align 4, db 0
bench_x:          dd 4.25           ; BENCH: above the workbench (top y 0.91, x 3.75..4.88, z 1.70..4.60)
bench_y:          dd 1.30           ;   between its drill press (z < 2.40) and its vice (z > 3.50)
bench_z:          dd 3.05
view_yaw:         dd -1.5707964     ; the camera looks along +x, at the wall the bench stands at
view_pitch:       dd -0.25          ; from a little above
view_distance:    dd 1.2
centre_z:         dd -0.18          ; CENTRE_Z: a rifle's middle (its grip at the origin, the muzzle at -z)
tilt_max:         dd 1.2
tilt_min:         dd -1.2
two_pi:           dd 6.2831855
rate:             dd 5.0            ; RATE: the share of the way a second
one:              dd 1.0
close:            dd 1.0e-6         ; there: within 0.001 (squared)
