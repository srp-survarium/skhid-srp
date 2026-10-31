; Patch 45: a weapon's upgrade window shows the weapon in the air above the
; lobby's workbench (build 2777, client_patch.md section 3.42). Patch 32 still
; decides what is shown (a kit's weapon, or a stash weapon in the chosen kit's
; hands from a scratch profile) and the lobby character still holds it; here
; the held weapon is drawn above the workbench on its own, turning about its
; middle, the camera flies there once it is, and a drag (the left button too)
; turns it. At WEAPON_TABLE_OFFSET of `.srpw`, after the stash code; its state
; (WEAPON_TABLE_STATE_SIZE bytes) at STATE, WEAPON_TABLE_STATE_OFFSET of
; `.srpd`; patch 32's mode at PREVIEW (WEAPON_PREVIEW_STATE_OFFSET).
;   nasm -f bin -DCAVE=0xA42A6B0 -DSTATE=0xA428F00 -DPREVIEW=0xA428E00 weapon_table_026g.asm -o weapon_table_026g.section.bin
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
;                patch 32's mode 1 (a weapon shown) turns the table on when
;                the lobby gives a spot (below; none: the stock camera, as an
;                armour's window): the camera's point is to go to SPOT (the height and the distance
;                by the setter), its yaw and pitch to the bench's view, the
;                weapon's turn to 0, and a request counted (REQUEST): the
;                camera flight (weapon_flight.rs, at lobby_camera.view; round
;                7) flies it from the next tick (round 5: no wait for the
;                weapon); mode 0 with the table on (a close, or an armour's
;                window over a weapon's) turns it off and counts a request:
;                the point is to go back to the origin, the yaw and the pitch
;                to what they were before. Then the setter.
;   +5  tick     0x284020 `movss xmm0,[ebx+0C8h]` (the camera's tick, ebx =
;                camera+4, [esp+10h] the frame's time, at most 0.1 s): while
;                the table is on a drag turns the weapon (the yaw's move its
;                spin, the pitch's its tilt) instead of the camera, and the
;                weapon drawn there is framed (the distance's target). The
;                camera's flight there and back is weapon_flight.rs's (round
;                7: until round 6 a glide here, RATE * dt of the way).
;   +10 cull     0x247576 `cmp byte [ebp-1],0 / jne 247592h` in player::mt_draw
;                (0x2474A0): the table's player takes the draw path even when
;                its own model was not seen (the camera looks away from it),
;                so that its held item is drawn.
;   +15 matrix   0x24778F `cmp byte [ebx+0F49Ch],0 / je 2477C2h`: once the
;                character makes no player (+D2C4h -1, +D2C8h 0), the table's
;                player's held item is drawn with the bench's transform
;                ([ebp-0E8h], the decal pose's buffer), its middle at SPOT,
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
;   +25 ray      0x284309 `call 2844D0h` in the camera's tick: no collision
;                ray while the camera is the table's (round 5c)
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
; The spot (round 6, 2026-10-29, docs/notes/lobby-scenes.md): each lobby
; scene says where the weapon floats, in its project's config, read when the
; table turns on: srp_lobby/weapon (a float3: the weapon's middle there) and
; srp_lobby/weapon_view (the yaw the camera looks at it from; the classic
; lobby's -pi/2, along +x at the wall the workbench stands at, by the content
; edit lobby-scenes-1). The bench's transform is the classic's turned by R =
; weapon_view + pi/2 about the vertical (rotY(spin) * rotZ(tilt) * rotY(R)), so
; the weapon lies across the view and tilts about its length in any lobby; R 0
; (the classic) leaves the classic's transform as it was, bit for bit.
;
; The weapon's middle (the point that goes to SPOT): its skeleton model
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

%define URO_FOV         0xA6FA28    ; uro_fov's value: the lobby camera's vertical field of view (degrees, patch 22)
%define OWNER           0x9C        ; lobby menu: its owner; +53B0h its viewport, +108h/+10Ch its width and height
%define VIEWPORT        0x53B0
%define RAY             0x2844D0    ; lobby camera: the collision ray from its point back to the eye; `ret 4`
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
%define ZOOM            0x94        ; the camera's zoom: patch 22's projection (fov_projection) takes
                                    ; 65 z + (uro_fov - 65) max(0, (z - 1/4) / (3/4)) degrees (vertical)
%define TURN_YAW        0xCC        ; a drag's moves of the frame
%define TURN_PITCH      0xD0

; The lobby's project and its config (lobby_menu::on_render_scenes_ready keeps the project at
; +114h, 0x220E01; the engine reads its camera/position as [[project+17Ch]+108h] -> camera ->
; position, 0x221975..0x2219E0). A binary config value: +0 its data (a table's children, a
; vector's floats, a number's bits), +14h its type.
%define PROJECT         0x114       ; lobby menu: its scene's project (0 none)
%define PROJECT_CONFIG  0x17C       ; the project: its binary_config
%define CONFIG_ROOT     0x108       ; binary_config: its root value
%define HAS_KEY         0x0F6C00    ; eax = table, esi = key: al = it is there
%define CHILD           0x0F6B10    ; eax = table, edi = key: eax = the child (no end check: has_key first)
%define VALUE_TYPE      0x14
%define T_INT           1
%define T_FLOAT         2
%define T_NAMED         3
%define T_FLOAT3        7

; The state, at STATE
%define TABLE           0x00        ; 1: the weapon is at the bench
%define FLIGHT          0x04        ; the camera flight's (weapon_flight.rs, its alone): 0 the stock camera,
                                    ; 1 moving (there or back), 2 resting at the bench
%define TARGET_X        0x08        ; the camera's targets: point x, point z, yaw, pitch
%define TARGET_Z        0x0C
%define TARGET_YAW      0x10
%define TARGET_PITCH    0x14
%define SAVED_YAW       0x18        ; the camera's yaw and pitch before the table
%define SAVED_PITCH     0x1C
%define SPIN            0x20        ; the weapon's turn about the vertical, by the drag's yaw
%define TILT            0x24        ; and about the bench's length, by its pitch (within TILT_MAX)
%define READY           0x28        ; 1: the window's weapon is drawn at the bench
%define REQUEST         0x2C        ; +1 at each request (the table on, again, or off): the flight's start
%define LENGTH          0x30        ; float: 0 not measured; - the window's weapon's length (its box's) to frame;
                                    ; framed: the wheel's farthest distance (> 0)
%define LEFT            0x34        ; the left button since it went down: 0 up, 1 the weapon's drag, 2 not
%define MIDDLE          0x38        ; float4: the weapon's middle, in its model's space (w 0)
%define STAMP           0x48        ; +1 at every bench transform written (the interface's frame stamp)
%define TARGET_ZOOM     0x4C        ; the camera's zoom the flight goes to (1 back at the stock camera)
%define BENCH_MATRIX    0x50        ; float4x4: the bench transform the held item was last drawn with
%define BIND_MIDDLE     0x90        ; float4: the table's middle (bind space), before bone 1
%define SPOT            0xA0        ; float4: the lobby's weapon spot (x, y, z, 1): the weapon's middle there
%define VIEW            0xB0        ; float: the yaw the camera looks at the spot from
%define TURN_COS        0xB4        ; cos and sin of the bench's turn R = VIEW + pi/2 (0 in the classic
%define TURN_SIN        0xB8        ;   lobby, whose camera looks along +x); SPOT..TURN_SIN hold while TABLE is 1
%define CONST(label)    ((label) - STATE)
%define FC(label)       ((label) - wide)    ; frame_view's constants, from ebx

    jmp near camera
    jmp near tick
    jmp near cull
    jmp near matrix
    jmp near left
    jmp near ray

; patch 32's start_cam and exit_cam: `jmp camera` where they jumped to the
; setter. Every register but xmm0 and xmm1 (the bench's while it is on) is
; the caller's; the setter follows.
camera:
    pushad
    mov esi, eax                    ; esi = the camera
    call base                       ; edi = the state
    lea ebx, [edi+CONST(wide)]      ; ebx: the constants (FC)
    cmp byte [edi + PREVIEW - STATE + MODE], 0
    je .off
    call spot                       ; the lobby's weapon spot: none, the stock camera (as an armour's)
    test eax, eax
    jz .off
    cmp dword [edi+FLIGHT], 0
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
    lea ecx, [eax+1]
    mov [edi+TABLE], ecx
    inc dword [edi+REQUEST]         ; the flight (weapon_flight.rs) flies there from the next tick
    mov [edi+SPIN], eax
    mov [edi+TILT], eax
    mov [edi+LENGTH], eax           ; the weapon's own length once it is drawn there
    ; the bench's view at the original's 65 degrees whatever uro_fov is (as a
    ; scope's, through patch 22's projection): the zoom z with 65 z + (u - 65)
    ; (z - 1/4) / (3/4) = 65 is (u + 130) / (4u - 65)
    fld dword [edi + URO_FOV - STATE]
    fld st0
    fmul dword [ebx+FC(four)]
    fsub dword [ebx+FC(original_fov)]
    fxch
    fadd dword [ebx+FC(zoom_add)]
    fdivrp st1, st0
    fstp dword [edi+TARGET_ZOOM]
    mov eax, [edi+SPOT]
    mov [edi+TARGET_X], eax
    mov eax, [edi+SPOT+8]
    mov [edi+TARGET_Z], eax
    mov eax, [ebx+FC(view_pitch)]
    mov [edi+TARGET_PITCH], eax
    fld dword [ebx+FC(two_pi)]   ; the bench's yaw, the nearest turn of it to the camera's
    fld dword [edi+VIEW]
    fsub dword [esi+YAW]
.turns:
    fprem1
    fnstsw ax
    test ah, 4
    jnz .turns
    fadd dword [esi+YAW]
    fstp dword [edi+TARGET_YAW]
    fstp st0
    movss xmm0, [ebx+FC(one)]    ; framed for a 1 m weapon until its own is known
    call frame_view                 ; xmm1 the distance, xmm2 the side angle
    movss [esp+32+4], xmm2          ; the setter's side angle: the weapon's place on the screen
    movss xmm0, [edi+SPOT+4]
    jmp .set
.off:
    cmp dword [edi+TABLE], 0
    je .set
    xor eax, eax
    mov [edi+TABLE], eax            ; the flight flies the camera back
    inc dword [edi+REQUEST]
    mov ecx, [ebx+FC(one)]
    mov [edi+TARGET_ZOOM], ecx      ; the player's field of view again
    mov [edi+TARGET_X], eax
    mov [edi+TARGET_Z], eax
    mov eax, [edi+SAVED_YAW]
    mov [edi+TARGET_YAW], eax
    mov eax, [edi+SAVED_PITCH]
    mov [edi+TARGET_PITCH], eax
.set:
    popad
    jmp CAMERA_SET

; The lobby's weapon spot (docs/notes/lobby-scenes.md): its project's
; srp_lobby/weapon (a float3) and srp_lobby/weapon_view (a float or an int,
; radians). esi = the camera, edi = the state. Found: SPOT, VIEW, TURN_COS and
; TURN_SIN written, eax 1; else eax 0 and the state as it was. Keeps every
; other register; the x87 stack as it was.
spot:
    push ebx
    push esi
    push ebp
    mov ebp, edi                    ; ebp = the state
    mov eax, [esi+MENU]
    test eax, eax
    jz .none
    mov eax, [eax+PROJECT]
    test eax, eax
    jz .none
    mov eax, [eax+PROJECT_CONFIG]
    test eax, eax
    jz .none
    mov eax, [eax+CONFIG_ROOT]
    test eax, eax
    jz .none
    lea edi, [ebp+CONST(key_srp_lobby)]
    call find
    test eax, eax
    jz .none
    cmp word [eax+VALUE_TYPE], T_NAMED
    jne .none
    mov ebx, eax                    ; srp_lobby
    lea edi, [ebp+CONST(key_weapon)]
    call find
    test eax, eax
    jz .none
    cmp word [eax+VALUE_TYPE], T_FLOAT3
    jne .none
    mov esi, [eax]                  ; its three floats
    test esi, esi
    jz .none
    mov eax, ebx
    lea edi, [ebp+CONST(key_weapon_view)]
    call find
    test eax, eax
    jz .none
    cmp word [eax+VALUE_TYPE], T_FLOAT
    je .float
    cmp word [eax+VALUE_TYPE], T_INT
    jne .none
    fild dword [eax]
    jmp .view
.float:
    fld dword [eax]
.view:
    fst dword [ebp+VIEW]
    fadd dword [ebp+CONST(half_pi)] ; R: the classic's -pi/2 gives 0 exactly (both are float32 pi/2)
    fsincos
    fstp dword [ebp+TURN_COS]
    fstp dword [ebp+TURN_SIN]
    mov eax, [esi]
    mov [ebp+SPOT], eax
    mov eax, [esi+4]
    mov [ebp+SPOT+4], eax
    mov eax, [esi+8]
    mov [ebp+SPOT+8], eax
    mov dword [ebp+SPOT+12], 0x3F800000     ; w 1, for the transform's row 3
    mov eax, 1
    jmp .end
.none:
    xor eax, eax
.end:
    mov edi, ebp
    pop ebp
    pop esi
    pop ebx
    ret

; eax = a named table, edi = a key (C string): eax = its child, 0 when it
; has none (has_key first: child has no end check). Keeps every other register.
find:
    push ecx
    push edx
    push esi
    push edi
    push eax
    mov esi, edi
    call HAS_KEY
    mov ecx, eax
    pop eax
    mov edi, [esp]
    test cl, cl
    jz .no
    call CHILD
    jmp .out
.no:
    xor eax, eax
.out:
    pop edi
    pop esi
    pop edx
    pop ecx
    ret

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
    cmp dword [edi+TABLE], 0
    je .out                         ; the table off: nothing of the camera's here
    lea esi, [ebx-4]                ; the camera
    lea ebp, [edi+CONST(wide)]      ; ebp: the constants (FC)
    cmp dword [edi+LENGTH], 0
    jge .framed
    movss xmm0, [edi+LENGTH]        ; the weapon drawn there (- its length): framed for it, gliding there
    call frame_view
    movss [esi+DISTANCE_TARGET], xmm1
    movss xmm0, xmm1                ; the wheel's farthest: max(FAR_SHARE d, d + FAR_REACH)
    mulss xmm0, [ebp+FC(far_share)]
    addss xmm1, [ebp+FC(flip_half)+4]
    maxss xmm1, xmm0
    movss [edi+LENGTH], xmm1        ; framed: the wheel's farthest (the flight's distance follows its target)
.framed:
    movss xmm0, [esi+TURN_YAW]      ; the drag turns the weapon, not the camera
    addss xmm0, [edi+SPIN]
    movss [edi+SPIN], xmm0
    movss xmm0, [esi+TURN_PITCH]
    addss xmm0, [edi+TILT]
    maxss xmm0, [ebp+FC(tilt_min)]
    minss xmm0, [ebp+FC(tilt_max)]
    movss [edi+TILT], xmm0
    xor eax, eax
    mov [esi+TURN_YAW], eax
    mov [esi+TURN_PITCH], eax
.out:
    movups xmm1, [esp]
    movups xmm2, [esp+16]
    movups xmm3, [esp+32]
    movups xmm4, [esp+48]
    add esp, 64
    popad
    movss xmm0, [ebx+0C8h]
    ret

; The bench's view of a weapon of length L = xmm0 (edi = the state, esi =
; the camera), at the original's field of view and the screen's aspect a
; (the viewport's, else 16:9): the camera looks at SPOT (the weapon's
; middle) and its side angle turns the view so that the middle is at FX of
; the width (and about the middle of the height) whatever the distance, so
; the wheel zooms into the weapon; and it is as near as keeps the ends at
; any spin m (FRAME_MARGIN, 0.02 of the width) in from the screen's left
; edge and from the upgrade window's left edge, and from the top and the
; bottom at any tilt.
;   The lobby movie lays itself out on 1920 x 1080, scaled by the width below
;   an aspect of 1.85 and by the height above (LobbyMenu.onStageResize); with
;   a' = a from 1.85 on, else 16:9, its width is 1080 a' units. The upgrade
;   window's left edge is PANEL_RIGHT (882) units from its right edge (the
;   inventory right-aligned, 112 in and 580 wide; the window 190 left of it:
;   UpgradeWindow's container.x): p = 882 / (1080 a') of the width from the
;   right. The middle's place: FRAME_SHIFT (365) units left of the screen's
;   middle, s = 365 / (1080 a'), FX = 1/2 - s: 0.310 at 16:9 and 16:10, 0.359
;   at 21:9, 0.405 at 32:9. FRAME_SHIFT is the one number to tune: the free
;   part between the Squad strip (88 units) and the window's edge has its
;   middle 397 units left, and the owner wanted it a little to the right
;   (2026-10-23).
;   T = tan(65 / 2) a; t = 2 s T, the tangent of the middle's angle off the
;   view's axis. In the view's frame the middle is at (-t, 1) * d with d the
;   distance along the axis; an end r = L / 2 from it stays right of m at any
;   spin while d >= L sqrt(1 / T^2 + (1 - 2m)^2) / (4 (1/2 - s - m)), left of
;   the window's edge less m while d >= L sqrt(1 / T^2 + (1 - 2m - 2p)^2) /
;   (4 (1/2 - p - m + s)), and in from the top and the bottom while d >=
;   TILT_ROOM L. The camera's distance is d sqrt(1 + t^2), at least
;   NEAR_VIEW: xmm1. The side angle sigma with the pitch P (VIEW_PITCH):
;   tan(angle) = cos P sin sigma / (cos^2 P cos sigma + sin^2 P) = t, so
;   sigma = atan(t cos P) + asin(t sin^2 P / (cos P sqrt(1 + t^2 cos^2 P))):
;   xmm2. Uses eax, xmm1, xmm2 and the x87 stack (left empty).
frame_view:
    push ebx
    lea ebx, [edi+CONST(wide)]      ; ebx: the constants (FC)
    sub esp, 16
    movss [esp], xmm0               ; L
    fld dword [ebx+FC(wide)]     ; a: the viewport's, else 16:9
    mov eax, [esi+MENU]
    test eax, eax
    jz .aspect
    mov eax, [eax+OWNER]
    test eax, eax
    jz .aspect
    mov eax, [eax+VIEWPORT]
    test eax, eax
    jz .aspect
    cmp dword [eax+0x10C], 0
    jle .aspect
    fstp st0
    fild dword [eax+0x108]
    fidiv dword [eax+0x10C]
.aspect:
    fld dword [ebx+FC(wide)]
    fld dword [ebx+FC(layout_switch)]
    fcomip st0, st2
    fcmovb st0, st1                 ; a', a
    fmul dword [ebx+FC(layout_height)]   ; 1080 a', a
    fld dword [ebx+FC(frame_shift)]
    fdiv st0, st1
    fstp dword [esp+4]              ; s; 1080 a', a
    fdivr dword [ebx+FC(panel_right)]
    fstp dword [esp+8]              ; p; a
    fmul dword [ebx+FC(tan_original)]    ; T
    fld1
    fdiv st0, st1
    fmul st0, st0
    fstp dword [esp+12]             ; 1 / T^2; T
    fld dword [esp+4]
    fadd st0, st0
    fmulp st1, st0                  ; t
    fld dword [esp+12]              ; the screen's left edge
    fadd dword [ebx+FC(spin_room)]
    fsqrt
    fld dword [esp+4]
    fmul dword [ebx+FC(four)]
    fsubr dword [ebx+FC(four_room)]
    fdivp st1, st0                  ; left, t
    fld dword [esp+8]               ; the upgrade window's edge
    fadd st0, st0
    fsubr dword [ebx+FC(spin_width)]
    fmul st0, st0
    fadd dword [esp+12]
    fsqrt
    fld dword [esp+4]
    fsub dword [esp+8]
    fmul dword [ebx+FC(four)]
    fadd dword [ebx+FC(four_room)]
    fdivp st1, st0                  ; right, left, t
    fcomi st0, st1
    fcmovb st0, st1
    fstp st1
    fld dword [ebx+FC(tilt_room)]        ; the height's
    fcomi st0, st1
    fcmovb st0, st1
    fstp st1                        ; d / L, t
    fmul dword [esp]
    fabs                            ; d, t (the tick gives - L)
    fld st1
    fmul st0, st0
    fld1
    faddp st1, st0
    fsqrt
    fmulp st1, st0                  ; the camera's distance, t
    fld dword [ebx+FC(near_view)]
    fcomi st0, st1
    fcmovb st0, st1
    fstp st1
    fstp dword [esp]                ; t
    fld st0
    fmul dword [ebx+FC(cos_pitch)]       ; t cos P, t
    fld st0
    fmul st0, st0
    fld1
    faddp st1, st0
    fsqrt
    fmul dword [ebx+FC(cos_pitch)]       ; cos P sqrt(1 + t^2 cos^2 P), t cos P, t
    fld st2
    fmul dword [ebx+FC(sin2_pitch)]
    fdivrp st1, st0                 ; x = t sin^2 P / that (below 0.03: asin x = x within 5e-6), t cos P, t
    fxch
    fld1
    fpatan                          ; atan(t cos P), x, t
    faddp st1, st0                  ; sigma, t
    fstp dword [esp+4]
    fstp st0
    movss xmm1, [esp]
    movss xmm2, [esp+4]
    add esp, 16
    pop ebx
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
    ; at SPOT
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
    push edx
    mov edx, eax
    cmp dword [edi+TURN_SIN], 0     ; the bench turned (not the classic's): the rows * rotY(R)
    jne .turn
    cmp dword [edi+TURN_COS], 0x3F800000
    je .turned
.turn:
    call turn
.turned:
    movups xmm0, [edi+MIDDLE]       ; row 3: SPOT - middle * the rows (w 1)
    call transform
    movups xmm1, [edi+SPOT]
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
    add edx, 6
    cmp ecx, eax
    jne .find
    movsx eax, byte [edx-4]         ; its x, y, z (mm, bind space)
    cvtsi2ss xmm0, eax
    movsx eax, byte [edx-3]
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
    cmp dword [edi+LENGTH], 0
    jne .measured
    mov eax, [ebp+CHARACTER]        ; while the character makes its player the weapon drawn is the
    cmp dword [eax+BUILDING], -1    ; old one (a window over a weapon's: the bench keeps it until
    jne .measured                   ; the new player comes): the window's weapon is measured then
    cmp byte [eax+BUILD_AGAIN], 0
    jne .measured
    movss xmm0, [esi+BOX_MIN+8]     ; - the window's weapon's length: its box along it, a default box 1 m
    subss xmm0, [esi+BOX_MAX+8]
    cmp dword [esi+BOX_MIN], 0xBF800000
    jne .length
    cmp dword [esi+BOX_MAX], 0x3F800000
    jne .length
    movss xmm0, [edi+CONST(minus_one)]
.length:
    movss [edi+LENGTH], xmm0
.measured:
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

; The rows at edx (0..2, 16 bytes each) times rotY(R), the bench's turn
; (rows (cos R, 0, sin R), (0, 1, 0), (-sin R, 0, cos R), as the rows above
; turn by the spin): (x, y, z) -> (x cos R - z sin R, y, x sin R + z cos R).
; edi = the state. Uses the x87 stack (left as it was).
turn:
    push ecx
    mov ecx, 3
.row:
    fld dword [edx]                 ; x
    fmul dword [edi+TURN_COS]
    fld dword [edx+8]               ; z
    fmul dword [edi+TURN_SIN]
    fsubp st1, st0                  ; x cos R - z sin R
    fld dword [edx]
    fmul dword [edi+TURN_SIN]
    fld dword [edx+8]
    fmul dword [edi+TURN_COS]
    faddp st1, st0                  ; x sin R + z cos R
    fstp dword [edx+8]
    fstp dword [edx]
    add edx, 16
    loop .row
    sub edx, 48
    pop ecx
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

; 0x284309 `call 2844D0h` -> call ray: the camera's tick casts a ray from its
; point back towards the eye through the lobby's collision scene (menu+3104h)
; and, at a hit, writes the hit's distance over the current one (+C4h), which
; the tick then clamps to the least (0.6). While the camera is the table's
; (FLIGHT not 0: flying there, there, flying back) no ray: the bench's view is made
; with nothing between its eye and the bench, and the window's player, once
; made, put something in the ray's way by the bench: the owner's log of
; 2026-10-23 (round 5c), the distance cut to 0.6 the frame that player came
; (the eye 0.8 m in one frame) and held at 1.09 of 1.2 after. Every register
; and flag the original's own way; `ret 4` as it.
ray:
    push eax
    call base_eax
    mov ecx, eax                    ; ecx = the state (ecx is the ray's to lose, as edx)
    mov eax, [esp]                  ; the camera
    cmp dword [ecx+FLIGHT], 0
    je .stock
    cmp dword [ecx+TABLE], 0        ; at the bench, framed: the wheel no farther than LENGTH, the
    je .none                        ; farthest (its least is patch 32's NEAR_MIN): the target (the
    cmp dword [ecx+LENGTH], 0       ; flight's distance follows it, gliding: round 7; no snap)
    jle .none
    movss xmm0, [ecx+LENGTH]
    movss xmm1, [eax+DISTANCE_TARGET]
    minss xmm1, xmm0
    movss [eax+DISTANCE_TARGET], xmm1
.none:
    pop eax
    ret 4
.stock:
    pop eax
    jmp RAY

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

align 16, db 0
flip_half:        dd -0.5, 0.5, -0.5, 0.0   ; the box's centre, turned as the instance's box (rotY(pi))
view_pitch:       dd -0.25          ; from a little above
tan_original:     dd 0.63707026     ; tan(65 / 2 degrees): the original's vertical field of view
four:             dd 4.0
original_fov:     dd 65.0
zoom_add:         dd 130.0
wide:             dd 1.7777778      ; 16:9: without a viewport, and the layout's below 1.85
layout_switch:    dd 1.85           ; the lobby movie scales by the height from this aspect on
layout_height:    dd 1080.0         ; its height in units
frame_shift:      dd 365.0          ; FRAME_SHIFT: the weapon's middle, units left of the screen's middle
panel_right:      dd 882.0          ; PANEL_RIGHT: the upgrade window's left edge, units from the right
spin_room:        dd 0.9216         ; (1 - 2m)^2, FRAME_MARGIN m 0.02 of the width
spin_width:       dd 0.96           ; 1 - 2m
four_room:        dd 1.92           ; 4 (1/2 - m)
cos_pitch:        dd 0.96891242     ; cos VIEW_PITCH
sin2_pitch:       dd 0.06120872     ; sin^2 VIEW_PITCH
far_share:        dd 1.25           ; FAR_SHARE: the wheel's farthest at least this of the framed distance,
                                    ; and FAR_REACH (flip_half's 0.5) m beyond it
minus_one:        dd -1.0
tilt_room:        dd 0.962          ; TILT_ROOM: (1/2) sqrt(1 + 1 / ((1 - 2m) tan(65/2))^2), and the pitch's
near_view:        dd 0.3            ; NEAR_VIEW: the nearest (patch 32's NEAR_MIN)
middle_z:         dd 0.2            ; MIDDLE_Z: without a box, a rifle's middle (grip at the origin, along +z)
tilt_max:         dd 1.2
tilt_min:         dd -1.2
two_pi:           dd 6.2831855
one:              dd 1.0
millimetre:       dd 0.001
half_pi:          dd 1.5707964      ; R = VIEW + pi/2: the bench's turn from the classic's view along +x
key_srp_lobby:    db "srp_lobby", 0
key_weapon:       db "weapon", 0
key_weapon_view:  db "weapon_view", 0
; MIDDLES: the weapons whose box does not give their middle within 1 cm (a
; default box; the Magnum's spare rounds; a body off the axis, the RPD's):
; items_dict id, the middle of the gun (parts parked beside it left out) in
; its bind space (mm: the 3rd-view idle's middle through the inverse of bone
; 1's skinning in that pose), which the code takes through bone 1 as the
; box's, from the data (client_patch.md section 3.42). 6 bytes each: the
; id a word, x and y a byte (within 4 cm of the axis), z a word.
%macro GUN 4
    dw %1
    db %2, %3
    dw %4
%endmacro
middles:          GUN 12, 5, -2, 240     ; toz_122 (weapons/toz_122)
                  GUN 13, 10, -37, 88    ; ak_74u (weapons/ak74u)
                  GUN 14, 15, -4, 270    ; rem_700 (weapons/rem_700)
                  GUN 16, 1, -38, 275    ; toz_34 (weapons/toz34/toz_34)
                  GUN 56, 1, 1, 96       ; magnum (weapons/magnum/magnum)
                  GUN 77, -28, -39, 231  ; rpd (weapons/rpd/rpd)
                  GUN 80, 10, -30, 167   ; akmn (weapons/akmn/akmn)
                  GUN 90, 4, -8, 40      ; winchester_1894 (weapons/winchester_1894)
                  GUN 107, 15, 11, 221   ; mosin_nagant (weapons/mosin_nagant/mosin_nagant)
                  GUN 108, 0, -31, 276   ; izh_27_lancaster (weapons/izh_27_lancaster/izh_27_lancaster_)
                  GUN 110, 10, -25, 309  ; mp_153 (weapons/mp153/mp153)
                  GUN 125, 10, -17, 162  ; benelli_m4 (weapons/benelli_m4/benelli_m4)
                  GUN 216, -5, -31, 21   ; vepr (weapons/vepr)
                  GUN 220, 8, -28, 169   ; a_545 (weapons/a_545/a_545)
                  GUN 303, 11, -16, 178  ; a9_91 (weapons/a9_91/a9_91)
                  GUN 337, 0, -31, 276   ; izh_27_lancaster_premium (weapons/izh_27_lancaster/izh_27_lancaster_)
                  GUN 2214, 11, -29, 174 ; valmet (weapons/valmet_rk62/valmet_rk62)
                  GUN 2215, 10, -6, 141  ; soc_vepr (weapons/soc_vepr/soc_vepr)
                  GUN 2227, 10, -17, 256 ; saiga_12 (weapons/saiga_12s/saiga_12s)
                  GUN 2228, 10, -17, 256 ; saiga_12_premium (weapons/saiga_12s/saiga_12s_camo)
                  GUN 2241, 10, -30, 167 ; akmn_premium (weapons/akmn/akmn_camo)
                  GUN 2242, 10, -30, 167 ; akmn_premium_2 (weapons/akmn/akmn_camo_st)
                  GUN 2243, 10, -37, 88  ; ak_74u_premium (weapons/aks74u_camo/ak74u_camo)
                  GUN 2244, -5, -31, 21  ; vepr_premium (weapons/vepr_camo/vepr_camo)
                  GUN 2249, 15, -4, 270  ; rem_700_premium (weapons/rem_700_camo_st)
                  GUN 2251, 0, -31, 276  ; izh_27_lancaster_premium_2 (weapons/izh_27_lancaster_camo/izh_27_lancaster_camo_st)
                  dw 0
