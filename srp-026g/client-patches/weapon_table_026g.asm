; Patch 45: a weapon's upgrade window shows the weapon in the air above the
; lobby's workbench (build 2777, client_patch.md section 3.42). Patch 32 still
; decides what is shown (a kit's weapon, or a stash weapon in the chosen kit's
; hands from a scratch profile) and the lobby character still holds it; here
; the held weapon is drawn above the workbench on its own, turning about its
; middle, the camera flies there once it is, and a drag (the left button too)
; turns it. At WEAPON_TABLE_OFFSET of `.srpw`, after the stash code; its state
; (WEAPON_TABLE_STATE_SIZE bytes) at STATE, WEAPON_TABLE_STATE_OFFSET of
; `.srpd`; patch 32's mode at PREVIEW (WEAPON_PREVIEW_STATE_OFFSET).
;   nasm -f bin -DCAVE=0xA42A800 -DSTATE=0xA428F00 -DPREVIEW=0xA428E00 weapon_table_026g.asm -o weapon_table_026g.section.bin
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
;                camera's point is to go to BENCH (the height and the distance
;                by the setter), its yaw and pitch to the bench's view, the
;                weapon's turn to 0, and the camera flies at once (round 5:
;                no wait for the weapon); mode 0 with the table on (a close, or an
;                armour's window over a weapon's) turns it off: the point goes
;                back to the origin, the yaw and the pitch to what they were
;                before. Then the setter.
;   +5  tick     0x284020 `movss xmm0,[ebx+0C8h]` (the camera's tick, ebx =
;                camera+4, [esp+10h] the frame's time, at most 0.1 s): while
;                the table is on a drag turns the weapon (the yaw's move its
;                spin, the pitch's its tilt) instead of the camera; while the
;                camera flies, its point, yaw and pitch move a share RATE * dt
;                of the way to their targets, and once off and there it is
;                the stock camera again.
;   +10 cull     0x247576 `cmp byte [ebp-1],0 / jne 247592h` in player::mt_draw
;                (0x2474A0): the table's player takes the draw path even when
;                its own model was not seen (the camera looks away from it),
;                so that its held item is drawn.
;   +15 matrix   0x24778F `cmp byte [ebx+0F49Ch],0 / je 2477C2h`: once the
;                character makes no player (+D2C4h -1, +D2C8h 0), the table's
;                player's held item is drawn with the bench's transform
;                ([ebp-0E8h], the decal pose's buffer), its middle at BENCH,
;                and the weapon is there (READY); before, the decal pose's
;                transform (10 m under the floor, where the stock decal
;                editor puts it away): the camera flies meanwhile and the
;                old player's weapon is seen neither in the hand nor at the
;                bench (round 5). Once
;                there, a refresh's new player (an attach) does not take it
;                away: the current player's weapon stays at the bench until
;                the new player replaces it. The transform is also kept at
;                BENCH_MATRIX with a new STAMP (the interface for other
;                patches: patch_026g.py WEAPON_TABLE_MATRIX_OFFSET).
;   +20 left     0x283D81 `test byte [eax+0Ch],52h / setne al` in the camera's
;                on_mouse_move (eax = the mouse's state, esi = the camera): a
;                button of 52h as before; with the table on, the left button too
;                when its press is off the movie (the lobby menu's vtable+24h,
;                is_mouse_over_ui, false at the press's first move: not on a
;                control, not on an item being dragged), until it is let go.
; The table's player: mt_draw called by lobby_character's update (return
; address 0x284A83), whose character (the update's ebx, kept by mt_draw at
; [ebp-0F0h]) has that player (+D2B8h) and its upgrade window open (+D515h:
; only the lobby's own character), while the table is on. Anything else is
; the original's, byte for byte. The code finds its state with call/pop.
;
;   The table's key, the items_dict id of the held weapon, comes from the
;   record the player was built from (the character's own profile copy
;   +18h + 4634h * +D2B4h, its kit +D2B5h's slot 7 or 10, the record's +4):
;   every byte of it inside the character, each index checked. (Round 3 read
;   weapon+25Ch, which is not a pointer for a held weapon: 2 for the VSS in
;   the owner's crash of 2026-10-21 11:18:24.)
; The weapon's middle (the point that goes to BENCH): its skeleton model
; (weapon+1810h) has the render instance (+108h, skeleton_render_model_instance,
; vtable 0x9F0F24), whose model (+62B4h) holds the mesh's box (+108h min, +114h
; max, in the exported frame: rotY(pi) of the mesh's, as the instance's own
; box, 0x15A77F) and the bones' inverse bind matrices (+140h, 64 bytes each).
; mt_draw's [ebp-0Ch] holds the weapon's bones of the frame in model space
; ([ebp-18h] of them; the renderer skins with invbind * bone, 0x15A7EC). The
; middle is the box's centre, flipped, through bone 1's (`offset`, the body's:
; the idle pose moves it, the PM's 20 cm) inverse bind and pose. A default box
; (min x -1, max x 1) or anything missing: (0, 0, MIDDLE_Z) of the model.
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
%define BONES           -0x0C       ; mt_draw's frame: the held item's bone matrices
%define BONE_COUNT      -0x18       ; mt_draw's frame: how many
%define DECAL           0xF49C      ; player: drawn for the decal editor
%define HELD            0x128       ; player: the held item
%define MATRIX          -0xE8       ; mt_draw's frame: the decal pose's transform (64 bytes)
%define PLAYER          0xD2B8      ; lobby_character: its player
%define BUILDING        0xD2C4      ; lobby_character: the player's query, -1 when none
%define BUILD_AGAIN     0xD2C8      ; lobby_character: a setup_profile waits for it
%define UPGRADE_MODE    0xD515      ; lobby_character: 1 while an upgrade window is open
%define WEAPON_VTABLE   0xA04174    ; survarium::weapon
%define WEAPON_MODEL    0x1810      ; weapon: its skeleton model instance
%define COPIES          0x18        ; lobby_character: three player_profile copies of 4634h (the player's from)
%define COPY_SIZE       0x4634
%define SHOWN_COPY      0xD2B4      ; lobby_character: the current copy's index
%define SHOWN_KIT       0xD2B5      ; lobby_character: the kit it shows
%define KITS            0x57        ; player_profile: u8 kit count
%define KIT             0x58        ; player_profile: kit k at +58h + k * 704h, 23 slots {u16 index, u16 amount} at +40h
%define KIT_SIZE        0x704
%define WEAPON_1        (0x40 + 4 * 7)
%define WEAPON_2        (0x40 + 4 * 10)
%define ITEMS           0x1564      ; player_profile: u8 item count
%define ITEM_TABLE      0x1568      ; player_profile: records of B4h, +4 the u16 items_dict id
%define ITEM_SIZE       0xB4
%define MAX_ITEMS       0x45
%define MODEL_RENDER    0x108       ; skeleton model instance: its render instance
%define SKELETON_VTABLE 0x9F0F24    ; vostok::render::skeleton_render_model_instance
%define RENDER_MODEL    0x62B4      ; skeleton_render_model_instance: its skeleton_render_model
%define BOX_MIN         0x108       ; render_model: the box (min, then max at +114h)
%define BOX_MAX         0x114
%define INVERSE_BINDS   0x140       ; skeleton_render_model: the inverse bind matrices
%define MENU            0x88        ; lobby camera: the lobby menu
%define OVER_UI         0x24        ; lobby menu vtable: is_mouse_over_ui (al)
%define BUTTONS         0x0C        ; the mouse's state: the buttons held
%define LOBBY_BUTTONS   0x52        ; the camera's own drag buttons
%define LEFT_BUTTON     0x01

; mt_draw's sites: the call's return, and where the original goes
%define CULL_RET        0x24757B
%define DRAW            0x247592    ; the draw path
%define MATRIX_RET      0x247794
%define HAND            0x2477C2    ; `lea eax,[ebp-0A8h]`: the hand's transform
%define DRAW_ITEM       0x2477C8    ; eax = the item's transform

; The camera (lobby menu+10Ch), from the camera itself
%define SIDE_TARGET     0x98
%define HEIGHT_TARGET   0x9C
%define POINT_X         0xA0
%define HEIGHT          0xA4
%define POINT_Z         0xA8
%define YAW             0xB8
%define PITCH           0xBC
%define SIDE            0xAC
%define DISTANCE_TARGET 0xC0
%define DISTANCE        0xC4
%define SIDE_SPEED      0xE0        ; the camera's own steps' speeds (the setter's, the wheel's)
%define HEIGHT_SPEED    0xE4
%define DISTANCE_SPEED  0xE8
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
%define READY           0x28        ; 1: the window's weapon is drawn at the bench
%define EASE            0x2C        ; 1: a flight's camera steps ease with it (from `camera` until all are there)
%define EASED           0x30        ; float: how long they have (at most EASE_MAX s)
%define LEFT            0x34        ; the left button since it went down: 0 up, 1 the weapon's drag, 2 not
%define MIDDLE          0x38        ; float4: the weapon's middle, in its model's space (w 0)
%define STAMP           0x48        ; +1 at every bench transform written (the interface's frame stamp)
%define BENCH_MATRIX    0x50        ; float4x4: the bench transform the held item was last drawn with
%define BIND_MIDDLE     0x90        ; float4: the table's middle (bind space), before bone 1
%define CONST(label)    ((label) - STATE)

    jmp near camera
    jmp near tick
    jmp near cull
    jmp near matrix
    jmp near left

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
    xor eax, eax
    cmp dword [edi+TABLE], 0
    jne .again                      ; a weapon's window over a weapon's: the bench keeps its weapon
    mov [edi+READY], eax            ; until the new player's is there
.again:
    mov dword [edi+TABLE], 1
    mov dword [edi+FLY], 1
    mov dword [edi+EASE], 1
    mov [edi+EASED], eax
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
    mov dword [edi+EASE], 1
    mov [edi+EASED], eax
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
    sub esp, 64
    movups [esp], xmm1
    movups [esp+16], xmm2
    movups [esp+32], xmm3
    movups [esp+48], xmm4
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
    xor ecx, ecx                    ; ecx = 1 once a value is still away from its target (ch: a step)
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
    ; a flight: the camera's own three steps (the side angle, the height, the
    ; distance) ease as these do: their speeds RATE * |target - now| a
    ; second, each tick, so that all seven end together and slow down to
    ; the end (the setter's speeds are constant: those steps stopped dead
    ; mid-flight while the rest went on, and the rest snapped 1 mm at its
    ; end: the owner's "the camera jumps a bit at the end", 2026-10-23).
    ; They stop within 0.001 of their targets (0x283FA0, 0x284253): the
    ; flight is not over until they do, at most EASE_MAX seconds.
    cmp dword [edi+EASE], 0
    je .eased
    movss xmm0, [edi+EASED]
    addss xmm0, [esp+64+32+14h]
    movss [edi+EASED], xmm0
    movss xmm2, [edi+CONST(rate)]
    movups xmm3, [edi+CONST(abs_mask)]
    lea edx, [edi+CONST(steps)]
.speed:
    movzx eax, byte [edx]
    movzx ebp, byte [edx+1]
    movss xmm0, [esi+eax]
    subss xmm0, [esi+ebp]
    andps xmm0, xmm3
    comiss xmm0, [edi+CONST(step_close)]
    jb .stopped                     ; within 0.001: that step does not move
    movss xmm1, [edi+EASED]
    comiss xmm1, [edi+CONST(ease_max)]
    jae .stopped                    ; it cannot get there (a clamp): over anyway
    mov ch, 1
.stopped:
    mulss xmm0, xmm2
    movzx eax, byte [edx+2]
    movss [esi+eax], xmm0
    add edx, 3
    cmp byte [edx], 0
    jne .speed
    test ch, ch
    jnz .easing                     ; a step still eases: not over
    test cl, cl
    jnz .eased
    mov [edi+EASE], ecx             ; all seven there: the easing is over (0)
    jmp .eased
.easing:
    mov ecx, 1
.eased:
    movss xmm1, [esp+64+32+14h]     ; the share of the way: RATE * dt, at most 1
    movss xmm4, xmm1                ; and at least FLOOR * dt, so that each lands, exactly
    mulss xmm4, [edi+CONST(floor)]
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
    subss xmm0, [esi+eax]           ; the way left
    movaps xmm2, xmm0
    andps xmm2, [edi+CONST(abs_mask)]
    movaps xmm3, xmm2
    mulss xmm3, xmm1
    addss xmm3, xmm4                ; the step: share * |way| + FLOOR * dt
    comiss xmm3, xmm2
    jb .short
    mov ebp, [edi+ebp]              ; it gets there: the target itself (no snap later)
    mov [esi+eax], ebp
    jmp .stepped
.short:
    xorps xmm2, xmm2
    comiss xmm0, xmm2
    jae .forward
    subss xmm2, xmm3
    movaps xmm3, xmm2
.forward:
    addss xmm3, [esi+eax]
    movss [esi+eax], xmm3
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
    movups xmm4, [esp+48]
    add esp, 64
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
; ebx = the player, ebp = mt_draw's frame. The table's player, its
; character making no player: eax = the bench's transform and on at
; 0x2477C8, keeping every other register; else the original's way (the
; decal pose's at 0x247798, the hand's at 0x2477C2), every register kept.
; None of the three reads a flag before setting it.
matrix:
    call is_table
    jz .original
    push eax
    mov eax, [ebp+CHARACTER]
    cmp dword [eax+BUILDING], -1
    jne .building
    cmp byte [eax+BUILD_AGAIN], 0
    jne .building
    pop eax
    jmp .bench
.building:                          ; a player being made: the bench once shown keeps the
    call base_eax                   ; current player's weapon there until the new one is
    cmp dword [eax+READY], 0        ; (a refresh: a scope or a camouflage put on or off)
    pop eax
    jne .bench
    ret                             ; not shown yet: the decal pose's transform, 10 m under
.original:
    cmp byte [ebx+DECAL], 0
    jne .ret                        ; the 4 nops, then the decal pose's transform
    add dword [esp], HAND - MATRIX_RET
.ret:
    ret
.bench:
    push edi
    push eax
    sub esp, 64
    movups [esp], xmm0
    movups [esp+16], xmm1
    movups [esp+32], xmm2
    movups [esp+48], xmm3
    call base                       ; edi = the state
    mov dword [edi+READY], 1        ; the window's weapon is at the bench
    call middle
    lea eax, [ebp+MATRIX]
    ; rows (v * M): rotY(spin) * rotZ(tilt), then the middle of the model
    ; at BENCH
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
    xorps xmm0, xmm0
    movss [eax+0Ch], xmm0
    movss [eax+18h], xmm0
    movss [eax+1Ch], xmm0
    movss [eax+2Ch], xmm0
    movups [eax+30h], xmm0
    movups xmm0, [edi+MIDDLE]       ; row 3: BENCH - middle * the rows (w 1)
    push edx
    mov edx, eax
    call transform
    movups xmm1, [edi+CONST(bench_x)]
    subps xmm1, xmm0
    movups [eax+30h], xmm1
    lea edx, [edi+BENCH_MATRIX]     ; the interface: the transform drawn with, and a new stamp
    movups xmm0, [eax]
    movups [edx], xmm0
    movups xmm0, [eax+10h]
    movups [edx+10h], xmm0
    movups xmm0, [eax+20h]
    movups [edx+20h], xmm0
    movups [edx+30h], xmm1
    inc dword [edi+STAMP]
    pop edx
    movups xmm0, [esp]
    movups xmm1, [esp+16]
    movups xmm2, [esp+32]
    movups xmm3, [esp+48]
    add esp, 64+4                   ; eax: the transform
    pop edi
    add dword [esp], DRAW_ITEM - MATRIX_RET
    ret

; The weapon's middle in its model's space to MIDDLE (x, y, z, 0). edi = the
; state, ebx = the player, ebp = mt_draw's frame. Uses xmm0..xmm3; keeps
; every other register. A middle in the bind space (the table's, else the
; box's) goes through bone 1's inverse bind and its pose of this frame, so
; whatever the weapon's animation (3rd-view idle, preview) its middle is.
middle:
    pushad
    xorps xmm0, xmm0
    movups [edi+MIDDLE], xmm0
    mov eax, [edi+CONST(middle_z)]
    mov [edi+MIDDLE+8], eax         ; the fallback
    mov esi, [ebx+HELD]
    test esi, esi
    jz .done
    lea eax, [edi + WEAPON_VTABLE - STATE]
    cmp [esi], eax
    jne .done
    xor ebx, ebx                    ; 1: the table's middle at BIND_MIDDLE
    ; a weapon whose box does not serve: MIDDLES, by the items_dict id of the
    ; record the character's player holds (all inside the character, every
    ; index checked: the character's current copy, its kit's weapon 1, or 2
    ; without a 1, as patch 32's find_kit)
    mov eax, [ebp+CHARACTER]        ; is_table checked it
    movzx ecx, byte [eax+SHOWN_COPY]
    cmp ecx, 3
    jae .model
    imul ecx, ecx, COPY_SIZE
    lea ecx, [eax+ecx+COPIES]       ; the copy
    movzx edx, byte [eax+SHOWN_KIT]
    cmp edx, 3
    jae .model
    cmp dl, [ecx+KITS]
    jae .model
    imul edx, edx, KIT_SIZE
    movzx eax, word [ecx+edx+KIT+WEAPON_1]
    cmp ax, -1
    jne .held
    movzx eax, word [ecx+edx+KIT+WEAPON_2]
.held:
    cmp eax, MAX_ITEMS
    jae .model
    cmp al, [ecx+ITEMS]
    jae .model
    imul eax, eax, ITEM_SIZE
    movzx eax, word [ecx+eax+ITEM_TABLE+4]
    lea edx, [edi+CONST(middles)]
.find:
    movzx ecx, word [edx]
    test ecx, ecx
    jz .model
    add edx, 8
    cmp ecx, eax
    jne .find
    movsx eax, word [edx-6]         ; its x, y, z (mm, bind space)
    cvtsi2ss xmm0, eax
    movsx eax, word [edx-4]
    cvtsi2ss xmm1, eax
    movsx eax, word [edx-2]
    cvtsi2ss xmm2, eax
    unpcklps xmm0, xmm1
    movlhps xmm0, xmm2
    movss xmm1, [edi+CONST(millimetre)]
    shufps xmm1, xmm1, 0
    mulps xmm0, xmm1
    movups [edi+BIND_MIDDLE], xmm0
    inc ebx
.model:
    mov esi, [esi+WEAPON_MODEL]
    test esi, esi
    jz .done
    mov esi, [esi+MODEL_RENDER]
    test esi, esi
    jz .done
    lea eax, [edi + SKELETON_VTABLE - STATE]
    cmp [esi], eax
    jne .done
    mov esi, [esi+RENDER_MODEL]
    test esi, esi
    jz .done
    mov edx, [esi+INVERSE_BINDS]
    test edx, edx
    jz .done
    mov ecx, [ebp+BONES]
    test ecx, ecx
    jz .done
    cmp dword [ebp+BONE_COUNT], 2
    jb .done
    test ebx, ebx
    jz .own
    movups xmm0, [edi+BIND_MIDDLE]
    jmp .pose
.own:
    cmp dword [esi+BOX_MIN], 0xBF800000
    jne .box
    cmp dword [esi+BOX_MAX], 0x3F800000
    je .done                        ; the default box: the fallback
.box:
    xorps xmm0, xmm0                ; the box's centre, rotY(pi), its x 0: (0, y, -z); the box
                                    ; holds parts parked beside the weapon (the VSS's spare magazine)
    movss xmm1, [esi+BOX_MIN+4]
    addss xmm1, [esi+BOX_MAX+4]
    movss xmm2, [esi+BOX_MIN+8]
    addss xmm2, [esi+BOX_MAX+8]
    unpcklps xmm0, xmm1             ; (x, y, 0, 0)
    movlhps xmm0, xmm2              ; (x, y, z, 0)
    movups xmm1, [edi+CONST(flip_half)]
    mulps xmm0, xmm1
.pose:
    add edx, 64                     ; bone 1: its inverse bind, then its pose
    call transform
    lea edx, [ecx+64]
    call transform
    movups [edi+MIDDLE], xmm0
    xor eax, eax
    mov [edi+MIDDLE+12], eax
.done:
    popad
    ret

; xmm0 = (x, y, z, w) * the matrix at edx (rows: x * row 0 + y * row 1 + z *
; row 2 + row 3; w is not read). Uses xmm1..xmm3.
transform:
    movaps xmm1, xmm0
    shufps xmm1, xmm1, 0x00
    movups xmm3, [edx]
    mulps xmm1, xmm3
    movaps xmm2, xmm0
    shufps xmm2, xmm2, 0x55
    movups xmm3, [edx+16]
    mulps xmm2, xmm3
    addps xmm1, xmm2
    movaps xmm2, xmm0
    shufps xmm2, xmm2, 0xAA
    movups xmm3, [edx+32]
    mulps xmm2, xmm3
    addps xmm1, xmm2
    movups xmm3, [edx+48]
    addps xmm1, xmm3
    movaps xmm0, xmm1
    ret

; 0x283D81 `test byte [eax+0Ch],52h / setne al` -> call left + 2 nops. eax =
; the mouse's state, esi = the camera. al = the camera drags (what follows
; is `test al,al`); every other register kept.
left:
    test byte [eax+BUTTONS], LOBBY_BUTTONS
    jnz .yes
    push edi
    push edx
    push ecx
    mov dl, [eax+BUTTONS]
    call base                       ; edi = the state
    test dl, LEFT_BUTTON
    jz .up
    cmp dword [edi+TABLE], 0
    je .no
    mov eax, [edi+LEFT]
    cmp eax, 1
    je .drag
    cmp eax, 2
    je .no
    mov ecx, [esi+MENU]             ; the press's first move: off the movie?
    mov eax, [ecx]
    call [eax+OVER_UI]
    test al, al
    mov eax, 1
    jz .start
    mov al, 2
.start:
    mov [edi+LEFT], eax
    cmp eax, 1
    je .drag
    jmp .no
.up:
    mov dword [edi+LEFT], 0
.no:
    pop ecx
    pop edx
    pop edi
    xor al, al
    ret
.drag:
    pop ecx
    pop edx
    pop edi
.yes:
    mov al, 1
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
; the camera's own steps: target, now, speed; then 0
steps:            db SIDE_TARGET, SIDE, SIDE_SPEED, HEIGHT_TARGET, HEIGHT, HEIGHT_SPEED
                  db DISTANCE_TARGET, DISTANCE, DISTANCE_SPEED, 0
align 16, db 0
abs_mask:         dd 0x7FFFFFFF, 0, 0, 0   ; (aligned: an andps operand)
bench_x:          dd 4.25           ; BENCH: above the workbench (top y 0.91, x 3.75..4.88, z 1.70..4.60)
bench_y:          dd 1.30           ;   between its drill press (z < 2.40) and its vice (z > 3.50)
bench_z:          dd 3.05
                  dd 1.0            ;   (w, for row 3)
flip_half:        dd -0.5, 0.5, -0.5, 0.0   ; the box's centre, turned as the instance's box (rotY(pi))
view_yaw:         dd -1.5707964     ; the camera looks along +x, at the wall the bench stands at
view_pitch:       dd -0.25          ; from a little above
view_distance:    dd 1.2
middle_z:         dd 0.2            ; MIDDLE_Z: without a box, a rifle's middle (grip at the origin, along +z)
tilt_max:         dd 1.2
tilt_min:         dd -1.2
two_pi:           dd 6.2831855
rate:             dd 5.0            ; RATE: the share of the way a second
one:              dd 1.0
close:            dd 1.0e-12        ; there: within 1e-6 (squared; each lands on its target by FLOOR)
step_close:       dd 0.001          ; the camera's own steps stop within it (0x91A8F0)
ease_max:         dd 4.0            ; EASE_MAX: the longest a flight eases the camera's steps (s)
floor:            dd 0.01           ; FLOOR: the least speed of the point (m/s), the yaw and pitch (rad/s)
millimetre:       dd 0.001
; MIDDLES: the weapons whose box does not give their middle within 1 cm (a
; default box; the Magnum's spare rounds; a body off the axis, the RPD's):
; items_dict id, the middle of the gun (parts parked beside it left out) in
; its bind space (mm: the 3rd-view idle's middle through the inverse of bone
; 1's skinning in that pose), which the code takes through bone 1 as the
; box's, from the data (client_patch.md section 3.42)
middles:          dw 12, 5, -2, 240     ; toz_122 (weapons/toz_122)
                  dw 13, 10, -37, 88    ; ak_74u (weapons/ak74u)
                  dw 14, 15, -4, 270    ; rem_700 (weapons/rem_700)
                  dw 16, 1, -38, 275    ; toz_34 (weapons/toz34/toz_34)
                  dw 56, 1, 1, 96       ; magnum (weapons/magnum/magnum)
                  dw 77, -28, -39, 231  ; rpd (weapons/rpd/rpd)
                  dw 80, 10, -30, 167   ; akmn (weapons/akmn/akmn)
                  dw 90, 4, -8, 40      ; winchester_1894 (weapons/winchester_1894)
                  dw 107, 15, 11, 221   ; mosin_nagant (weapons/mosin_nagant/mosin_nagant)
                  dw 108, 0, -31, 276   ; izh_27_lancaster (weapons/izh_27_lancaster/izh_27_lancaster_)
                  dw 110, 10, -25, 309  ; mp_153 (weapons/mp153/mp153)
                  dw 125, 10, -17, 162  ; benelli_m4 (weapons/benelli_m4/benelli_m4)
                  dw 216, -5, -31, 21   ; vepr (weapons/vepr)
                  dw 220, 8, -28, 169   ; a_545 (weapons/a_545/a_545)
                  dw 303, 11, -16, 178  ; a9_91 (weapons/a9_91/a9_91)
                  dw 337, 0, -31, 276   ; izh_27_lancaster_premium (weapons/izh_27_lancaster/izh_27_lancaster_)
                  dw 2214, 11, -29, 174 ; valmet (weapons/valmet_rk62/valmet_rk62)
                  dw 2215, 10, -6, 141  ; soc_vepr (weapons/soc_vepr/soc_vepr)
                  dw 2227, 10, -17, 256 ; saiga_12 (weapons/saiga_12s/saiga_12s)
                  dw 2228, 10, -17, 256 ; saiga_12_premium (weapons/saiga_12s/saiga_12s_camo)
                  dw 2241, 10, -30, 167 ; akmn_premium (weapons/akmn/akmn_camo)
                  dw 2242, 10, -30, 167 ; akmn_premium_2 (weapons/akmn/akmn_camo_st)
                  dw 2243, 10, -37, 88  ; ak_74u_premium (weapons/aks74u_camo/ak74u_camo)
                  dw 2244, -5, -31, 21  ; vepr_premium (weapons/vepr_camo/vepr_camo)
                  dw 2249, 15, -4, 270  ; rem_700_premium (weapons/rem_700_camo_st)
                  dw 2251, 0, -31, 276  ; izh_27_lancaster_premium_2 (weapons/izh_27_lancaster_camo/izh_27_lancaster_camo_st)
                  dw 0
