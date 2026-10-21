; Patch 46: decals on a weapon (build 2777, client_patch.md section 3.43). At
; `.srpw`+1000h, the section's second page (the first holds patches 44, 32's
; stash and 45). PREVIEW is patch 32's state (WEAPON_PREVIEW_STATE_OFFSET of
; `.srpd`), TABLE patch 45's (WEAPON_TABLE_STATE_OFFSET).
;   nasm -f bin -DCAVE=0xA42A000 -DPREVIEW=0xA428E00 -DTABLE=0xA428F00 -DPRESS=0xA428FA0 weapon_decals_026g.asm -o weapon_decals_026g.section.bin
; (PRESS: this patch's 12 bytes, WEAPON_DECALS_STATE_OFFSET of `.srpd`)
;
; A decal item (category 26) on a weapon is an upgrade record in slot 0 or 1,
; as on armour; its modification appends {texture, <low_param>, <high_param>}
; to the host config's `data/decals` (the game puts the record's u64 there,
; 0x3584E0), which the weapon configs get from the data patch. The u64 is the
; decal editor's (0x263B60 packs it, 0x263D00 unpacks it into the decal's
; float4x4: rows right * s, up * s, forward * 1.5, the point - 0.75 forward):
; bits 0-9 yaw, 10-19 pitch, 20-26 roll, 27-30 size (/32), 31-41 x (+1.5,
; *682.67), 42-52 y (*1024), 53-63 z as x. On a character it is the decal's
; place on the bind pose; on a weapon it is a place in the weapon model's own
; space (its vertices' as the model's files hold them: grip at the origin,
; the muzzle towards +z) with y shifted by Y_SHIFT (the range is 0..2) and
; the size halved (SIZE): weapon_matrix.
;
; Entries (a jump table; patch_026g.py calls each):
;   +0  lookup  patch 44's bake_weapon (its frame): [ebp+CAMO] = the
;               camouflage's text or "", eax = 0 when there is neither a
;               camouflage nor a decal to paint
;   +5  fill    patch 44's bake_surface, edi = the parameters P: the
;               camouflage's layer (P+4E0h) when there is one, a layer
;               (P + i*270h) for each of the first two `data/decals`, and the
;               decals' part of the baked files' name
;   +A  pick    0x265999 `call 0x264960` in lobby_character's per-frame
;               decal preview (0x265980): for the weapon above the workbench
;               (patch 45) the mouse's ray meets the weapon's long axis, in
;               its model's space (the inverse of the transform patch 45
;               publishes, times bone 1's skinning), instead of the
;               character's collision model
;   +F  decode  0x2659B2 `call 0x263D00` there: the preview's box is the
;               weapon's decal at the weapon's transform at the bench
;   +14 bake    0x2641A0, the decal editor's bake (lobby_character, low,
;               high, keep; `ret 10h`): for the weapon, the click (1, 0) is
;               picked and accepted at once (46 05, the record, the movie's
;               exit_decal_edit); the server sends the account back and the
;               weapon is built again, which patch 44 bakes with the new
;               place. Its other calls (the start's, the cancel's) bake
;               nothing on a weapon.
;   +19 start   0x263938 `call 0x264CF0` at the end of the editor's start
;               (0x2636B0, esi = the character): on a weapon, the decal's
;               index in `data/decals` (+D52Ch) counts the records of slots 0
;               and 1 only (the stock counts every record but slot 3's, and a
;               weapon's camouflage is in slot 2)
;   +1E click   0x265C20, the character's mouse key handler: in a weapon's
;               edit the left button's release places the decal only after a
;               click (the cursor within 5 pixels of the press), not after a
;               drag, which turns the weapon (patch 45's left button)
; A weapon's decal edit: patch 45's table on, patch 32's weapon shown, the
; character's upgrade window open (+D515h) and the edit's host (+D520h)
; that window's item. Anything else: the original's code.
BITS 32
ORG CAVE

%ifndef PREVIEW
%error PREVIEW is required
%endif
%ifndef TABLE
%error TABLE is required
%endif
%ifndef PRESS
%error PRESS is required
%endif

; the game
has_key             equ 0x0F6C00    ; eax = table, esi = key: al = it is there
child               equ 0x0F6B10    ; eax = table, edi = key: eax = the child (no end check)
string_assign       equ 0x0127C0    ; eax = buffer_string, ecx = text
string_appendf      equ 0x0E45B0    ; eax = buffer_string, cdecl (format, ...)
DECODE              equ 0x263D00    ; cdecl (out, low, high): eax = out
ENCODE              equ 0x264A10    ; esi = lobby_character: edx:eax from +D530h, +D53Ch, +D518h, +D51Ch
PICK                equ 0x264960    ; stdcall (character, &point, &normal, mask, &id): al; `ret 14h`
RAY                 equ 0x26D130    ; esi = lobby menu, edi = &direction: [esp+4] = the origin; `ret 4`
ACCEPT              equ 0x263950    ; esi = character, stdcall (host, decal): the record, 46 05; `ret 8`
MOVIE_CALL          equ 0x511600    ; ecx = the movie, stdcall (name, 0, 0, 0)
FIND_ITEM           equ 0x277AA0    ; ecx = begin, edi = end, cdecl (&id): eax = the item or end
START_QUERY         equ 0x264CF0    ; esi = character: the host's config and the decal's textures
BAKE_BODY           equ 0x2641A6    ; after the bake's `push ebp / mov ebp,esp / and esp,-8`
EXIT_DECAL_EDIT     equ 0xA0BD7C    ; "root.exit_decal_edit"
KEY_CAMOUFLAGE      equ 0x9FD704
KEY_DECALS          equ 0x9F146C
EMPTY               equ 0x9DB708    ; ""
FORMAT_STRING       equ 0x9EDC00    ; "%s%s"
SUFFIX_NMAP         equ 0x9EF6B0    ; "_nmap"
SUFFIX_FMAP         equ 0x9EF6B8    ; "_fmap"
SUFFIX_SMAP         equ 0x9EF6C0    ; "_smap"
T_INT               equ 1
T_TABLE             equ 3
T_LIST              equ 4
T_STRING            equ 5

; camouflage_decal_cook_parameters (P)
LAYER_SIZE          equ 0x270       ; two decal layers, then the camouflage's
LAYER_DIFFUSE       equ 0x40        ; after the layer's float4x4: buffer_strings, 8Ch apart
LAYER_NORMAL        equ 0xCC
LAYER_FRESNEL       equ 0x158
LAYER_SMOOTHNESS    equ 0x1E4
P_CAMO              equ 2 * LAYER_SIZE
P_NAME              equ 0x7A4
MAX_DECALS          equ 2

; patch 44's frame (weapon_camo_026g.asm, ebp = esp after its pushad)
DATA                equ -8
CAMO                equ -0Ch

; lobby_character
MENU                equ 0xD2BC
EDITING             equ 0xD514      ; 1 while a decal is placed
UPGRADE_MODE        equ 0xD515      ; 1 while an upgrade window is open
EDIT_HOST           equ 0xD520      ; the edit's item and decal
EDIT_DECAL          equ 0xD524
DECAL_INDEX         equ 0xD52C      ; the decal's index in the host's `data/decals`
RECORD_INDEX        equ 0xD52D      ; its record's index in the host's records
POINT               equ 0xD530      ; the pick's point and normal
NORMAL              equ 0xD53C
VALUE               equ 0xD548      ; the u64 the edit gives
PICK_MASK           equ 0x20

; the lobby client's inventory (the host's records), as 0x2636B0 finds it
INVENTORY_OWNER     equ 0x3C        ; [[menu+9Ch]+53ACh] vtable: the lobby client
INVENTORY           equ 0x1EA978    ; begin, end: items of B4h
RECORDS             equ 0x60        ; item: begin, end of {id, dict id, u64, u8 slot} 18h each
RECORD_SIZE         equ 0x18
RECORD_SLOT         equ 0x10

; the mouse
LEFT_KEY            equ 0x151       ; the left button's code (0x3B9CA0)
KIND_DOWN           equ 0           ; the kinds: gone down, let go (2: still held)
KIND_UP             equ 1
MOUSE_X             equ 0x8C        ; lobby menu: the cursor, pixels (0x26D130)
MOUSE_Y             equ 0x90
CLICK_SLOP          equ 5           ; a press and a release this near: a click
CLICK_BODY          equ 0x265C26    ; after the handler's `push ebp / mov ebp,esp / and esp,-8`

; the held weapon (patch 45's reads, weapon_table_026g.asm)
PLAYER              equ 0xD2B8      ; lobby_character: its player
HELD                equ 0x128       ; player: the held item
WEAPON_VTABLE       equ 0xA04174    ; survarium::weapon
WEAPON_MODEL        equ 0x1810      ; weapon: its skeleton model instance
MODEL_RENDER        equ 0x108       ; skeleton model instance: its render instance
SKELETON_VTABLE     equ 0x9F0F24    ; vostok::render::skeleton_render_model_instance
SKINNING            equ 0x229C      ; skeleton_render_model_instance: the bones' skinning, transposed (0x15A801)

; patch 32's state, patch 45's and this one's
MODE                equ 0           ; patch 32: 1 while its window's weapon is shown
ITEM                equ 4           ;   the window's item
ON                  equ 0x00        ; patch 45: 1 while the weapon is at the bench
READY               equ 0x28        ;   1 once it is drawn there
BENCH_MATRIX        equ 0x50        ;   the transform it was last drawn with (rows, v * M)
PRESSED             equ 0           ; PRESS: 1 from the left button's press in a weapon's edit
PRESS_X             equ 4           ;   the cursor at the press
PRESS_Y             equ 8

    jmp near lookup
    jmp near fill
    jmp near pick
    jmp near decode
    jmp near bake
    jmp near start
    jmp near click

; ---------------------------------------------------------------- the bake

; patch 44's bake_weapon, ebp its frame, [ebp+DATA] the merged config's
; `data`: [ebp+CAMO] = the camouflage's text (non-empty) or "", eax = 0 when
; there is no camouflage and no decal. Keeps ebx and ebp.
lookup:
    push ebx
    xor ebx, ebx
    mov dword [ebp+CAMO], EMPTY
    mov eax, [ebp+DATA]
    mov esi, KEY_CAMOUFLAGE
    call has_key
    test al, al
    jz .decals
    mov eax, [ebp+DATA]
    mov edi, KEY_CAMOUFLAGE
    call child
    cmp word [eax+14h], T_STRING
    jne .decals
    mov eax, [eax]
    test eax, eax
    jz .decals
    cmp byte [eax], 0
    je .decals
    mov [ebp+CAMO], eax
    inc ebx
.decals:
    call decals
    or ebx, ecx
    mov eax, ebx
    pop ebx
    ret

; [ebp+DATA] = `data`: eax = its `decals` entries, ecx = how many to paint
; (at most MAX_DECALS; 0 without the table). Keeps ebx and ebp.
decals:
    mov eax, [ebp+DATA]
    mov esi, KEY_DECALS
    call has_key
    test al, al
    jz .none
    mov eax, [ebp+DATA]
    mov edi, KEY_DECALS
    call child
    cmp word [eax+14h], T_TABLE
    je .table
    cmp word [eax+14h], T_LIST
    jne .none
.table:
    movzx ecx, word [eax+16h]
    mov eax, [eax]
    cmp ecx, MAX_DECALS
    jbe .done
    mov ecx, MAX_DECALS
.done:
    ret
.none:
    xor eax, eax
    xor ecx, ecx
    ret

; patch 44's bake_surface, edi = P (its name begun), ebp = patch 44's frame.
; Every register kept.
fill:
    pushad
    mov esi, [ebp+CAMO]
    cmp byte [esi], 0
    je .decals
    lea eax, [edi+P_CAMO+LAYER_DIFFUSE]
    mov ecx, esi
    call string_assign
    push SUFFIX_NMAP
    push esi
    push FORMAT_STRING
    lea eax, [edi+P_CAMO+LAYER_NORMAL]
    call string_appendf
    add esp, 12
    push SUFFIX_SMAP
    push esi
    push FORMAT_STRING
    lea eax, [edi+P_CAMO+LAYER_SMOOTHNESS]
    call string_appendf
    add esp, 12
.decals:
    mov ebx, edi                    ; ebx = P
    call decals
    test ecx, ecx
    jz .done
    push ecx                        ; [esp+4] = the count, [esp] = the entries
    push eax
    xor edi, edi                    ; edi = the decal's index
.decal:
    imul esi, edi, 18h
    add esi, [esp]                  ; esi = the entry: a table {texture, low, high}
    cmp word [esi+14h], T_LIST
    je .entry
    cmp word [esi+14h], T_TABLE
    jne .next
.entry:
    movzx ecx, word [esi+16h]
    test ecx, ecx
    jz .next
    mov esi, [esi]                  ; its values
    cmp word [esi+14h], T_STRING
    jne .next
    mov eax, [esi]
    test eax, eax
    jz .next
    cmp byte [eax], 0
    je .next
    xor edx, edx                    ; the u64: [esp] low, [esp+4] high
    push edx
    push edx
    cmp ecx, 2
    jb .placed
    cmp word [esi+18h+14h], T_INT
    jne .high
    mov edx, [esi+18h]
    mov [esp], edx
.high:
    cmp ecx, 3
    jb .placed
    cmp word [esi+30h+14h], T_INT
    jne .placed
    mov edx, [esi+30h]
    mov [esp+4], edx
.placed:
    mov edx, [esp]
    or edx, [esp+4]
    jnz .layer
    mov edx, [defaults + edi*8]     ; never placed: the decal's own side of the weapon
    mov [esp], edx
    mov edx, [defaults + edi*8 + 4]
    mov [esp+4], edx
.layer:
    mov esi, [esi]                  ; esi = the texture's name
    imul eax, edi, LAYER_SIZE
    add eax, ebx                    ; eax = the layer
    push eax
    call weapon_matrix              ; (layer, low, high): the layer's float4x4
    pop eax
    push eax
    lea eax, [eax+LAYER_DIFFUSE]
    mov ecx, esi
    call string_assign
    mov eax, [esp]
    push SUFFIX_NMAP
    lea eax, [eax+LAYER_NORMAL]
    call suffixed
    mov eax, [esp]
    push SUFFIX_FMAP
    lea eax, [eax+LAYER_FRESNEL]
    call suffixed
    mov eax, [esp]
    push SUFFIX_SMAP
    lea eax, [eax+LAYER_SMOOTHNESS]
    call suffixed
    pop eax
    pop eax                         ; low
    pop edx                         ; high
    push eax
    push edx
    push esi
    push name_decal
    lea eax, [ebx+P_NAME]
    call string_appendf
    add esp, 16
.next:
    inc edi
    cmp edi, [esp+4]
    jb .decal
    add esp, 8
.done:
    popad
    ret

; eax = a buffer_string, esi = the texture, [esp+4] = a suffix: the two.
; Takes the suffix off the stack.
suffixed:
    push dword [esp+4]
    push esi
    push FORMAT_STRING
    call string_appendf
    add esp, 12
    ret 4

; cdecl (out, low, high): eax = out = the weapon's decal, the editor's
; float4x4 of the u64 with its point Y_SHIFT lower and its two sides SIZE
; times as long. Keeps ebx, esi, edi and ebp.
weapon_matrix:
    push dword [esp+0Ch]
    push dword [esp+0Ch]
    push dword [esp+0Ch]
    call DECODE
    add esp, 12
    movss xmm0, [eax+34h]
    subss xmm0, [y_shift]
    movss [eax+34h], xmm0
    movss xmm1, [size]
%assign row 0
%rep 2
%assign at 0
%rep 3
    movss xmm0, [eax+row*10h+at]
    mulss xmm0, xmm1
    movss [eax+row*10h+at], xmm0
%assign at at+4
%endrep
%assign row row+1
%endrep
    ret

; ---------------------------------------------------------------- the edit

; eax = a lobby_character: ZF clear when it places a decal of the weapon its
; open upgrade window shows above the workbench. Keeps every register.
is_weapon_edit:
    push ecx
    cmp dword [TABLE+ON], 0
    je .no
    cmp byte [PREVIEW+MODE], 0
    je .no
    cmp byte [eax+UPGRADE_MODE], 0
    je .no
    mov ecx, [eax+EDIT_HOST]
    cmp ecx, [PREVIEW+ITEM]
    jne .no
    test esp, esp
    pop ecx
    ret
.no:
    cmp ecx, ecx
    pop ecx
    ret

; xmm0 = (x, y, z, w) * the matrix at edx (rows; w is read: a point's w 1, a
; vector's 0). Uses xmm1..xmm3.
row_times:
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
    movaps xmm2, xmm0
    shufps xmm2, xmm2, 0xFF
    movups xmm3, [edx+48]
    mulps xmm2, xmm3
    addps xmm1, xmm2
    movaps xmm0, xmm1
    ret

; edi = out = the matrix at esi times the one at edx (rows, 4x4). Keeps
; every register; uses xmm0..xmm3. out may be esi's.
multiply:
    push ecx
    sub esp, 64
    xor ecx, ecx
.row:
    movups xmm0, [esi+ecx]
    call row_times
    movups [esp+ecx], xmm0
    add ecx, 16
    cmp ecx, 64
    jb .row
    xor ecx, ecx
.copy:
    movups xmm0, [esp+ecx]
    movups [edi+ecx], xmm0
    add ecx, 16
    cmp ecx, 64
    jb .copy
    add esp, 64
    pop ecx
    ret

; eax = out, edx = the lobby character: out = the transform from the weapon
; model's vertices (the bake's space) to the world as it is drawn at the
; bench: the skinning of bone 1 (`offset`, the body's; its render instance's
; matrix of the frame, kept transposed at +229Ch + 40h by 0x15A7FC: the
; inverse bind times the pose) times the transform patch 45 drew the weapon
; with (its state's +50h, valid while +0 and +28h are 1). ZF set when patch
; 45 has not drawn it there; the identity for the skinning when the held item
; is not a weapon with a skeleton instance. Keeps every register; uses
; xmm0..xmm3.
weapon_transform:
    pushad
    mov edi, eax
    cmp dword [TABLE+ON], 1
    jne .none
    cmp dword [TABLE+READY], 1
    jne .none
    sub esp, 64                     ; [esp] = bone 1's skinning, rows
    xorps xmm0, xmm0
    movups [esp], xmm0
    movups [esp+16], xmm0
    movups [esp+32], xmm0
    movups [esp+48], xmm0
    mov eax, 0x3F800000
    mov [esp], eax
    mov [esp+14h], eax
    mov [esp+28h], eax
    mov [esp+3Ch], eax
    mov eax, [edx+PLAYER]
    test eax, eax
    jz .skinned
    mov eax, [eax+HELD]
    test eax, eax
    jz .skinned
    cmp dword [eax], WEAPON_VTABLE
    jne .skinned
    mov eax, [eax+WEAPON_MODEL]
    test eax, eax
    jz .skinned
    mov eax, [eax+MODEL_RENDER]
    test eax, eax
    jz .skinned
    cmp dword [eax], SKELETON_VTABLE
    jne .skinned
    mov eax, [eax+SKINNING]
    test eax, eax
    jz .skinned
    add eax, 40h                    ; bone 1, transposed
%assign row 0
%rep 4
%assign col 0
%rep 4
    mov ecx, [eax+col*10h+row*4]
    mov [esp+row*10h+col*4], ecx
%assign col col+1
%endrep
%assign row row+1
%endrep
.skinned:
    mov esi, esp
    lea edx, [TABLE+BENCH_MATRIX]
    call multiply                   ; edi = the skinning * the bench's
    add esp, 64
    test esp, esp
    popad
    ret
.none:
    cmp eax, eax
    popad
    ret

; xmm0 = [%1] . [%2], three floats each; xmm1 is clobbered
%macro DOT 2
    movss xmm0, [%1]
    mulss xmm0, [%2]
    movss xmm1, [%1+4]
    mulss xmm1, [%2+4]
    addss xmm0, xmm1
    movss xmm1, [%1+8]
    mulss xmm1, [%2+8]
    addss xmm0, xmm1
%endmacro

; [%1] = [%2] x [%3] (three floats each); xmm0, xmm1 clobbered
%macro CROSS 3
    movss xmm0, [%2+4]
    mulss xmm0, [%3+8]
    movss xmm1, [%2+8]
    mulss xmm1, [%3+4]
    subss xmm0, xmm1
    movss [%1], xmm0
    movss xmm0, [%2+8]
    mulss xmm0, [%3]
    movss xmm1, [%2]
    mulss xmm1, [%3+8]
    subss xmm0, xmm1
    movss [%1+4], xmm0
    movss xmm0, [%2]
    mulss xmm0, [%3+4]
    movss xmm1, [%2+4]
    mulss xmm1, [%3]
    subss xmm0, xmm1
    movss [%1+8], xmm0
%endmacro

; the weapon pick's frame
W                   equ -40h        ; the weapon's transform (vertices to the world)
ORIGIN              equ -4Ch        ; the ray, in the world
DIRECTION           equ -58h
RELATIVE            equ -64h        ; the origin - the transform's point
LOCAL_ORIGIN        equ -70h        ; the ray in the weapon's space
LOCAL_DIRECTION     equ -7Ch
DUAL                equ -0ACh       ; the inverse's columns: row1 x row2, row2 x row0, row0 x row1, / det
PICK_FRAME          equ 0ACh

; 0x265999 `call 0x264960` in the preview (and the click's pick, below):
; stdcall (character, &point, &normal, mask, &id), al = 1 and the point and
; normal written when the ray hits. For the weapon above the bench: the
; mouse's ray (0x26D130) in the weapon model's space (the inverse of
; weapon_transform), the point of it nearest the model's z axis (its long
; axis through the grip), when that is in front of the eye, within REACH of
; the axis and between Z_MIN and Z_MAX along it: the point (y + Y_SHIFT) and
; the normal (the ray's direction, reversed: the decal looks along the ray).
; The decal's projection and occlusion find the surface. A weapon not yet
; drawn at the bench misses. Else the original.
pick:
    mov eax, [esp+4]
    call is_weapon_edit
    jnz weapon_pick
    jmp PICK
weapon_pick:
    push ebp
    mov ebp, esp
    sub esp, PICK_FRAME
    push ebx
    push esi
    push edi
    lea eax, [ebp+W]
    mov edx, [ebp+8]
    call weapon_transform
    jz .miss
    mov eax, [ebp+8]
    mov esi, [eax+MENU]
    lea edi, [ebp+DIRECTION]
    lea eax, [ebp+ORIGIN]
    push eax
    call RAY
%assign at 0
%rep 3
    movss xmm0, [ebp+ORIGIN+at]
    subss xmm0, [ebp+W+30h+at]
    movss [ebp+RELATIVE+at], xmm0
%assign at at+4
%endrep
    CROSS ebp+DUAL, ebp+W+10h, ebp+W+20h
    CROSS ebp+DUAL+0Ch, ebp+W+20h, ebp+W
    CROSS ebp+DUAL+18h, ebp+W, ebp+W+10h
    DOT ebp+W, ebp+DUAL             ; the determinant
    movss xmm2, xmm0
    andps xmm0, [abs_mask]
    comiss xmm0, [least_det]
    jb .miss                        ; no inverse
    movss xmm0, [one]
    divss xmm0, xmm2
    movss xmm2, xmm0
%assign at 0
%rep 9
    movss xmm0, [ebp+DUAL+at]
    mulss xmm0, xmm2
    movss [ebp+DUAL+at], xmm0
%assign at at+4
%endrep
%assign row 0
%rep 3
    DOT ebp+RELATIVE, ebp+DUAL+row*0Ch
    movss [ebp+LOCAL_ORIGIN+row*4], xmm0
    DOT ebp+DIRECTION, ebp+DUAL+row*0Ch
    movss [ebp+LOCAL_DIRECTION+row*4], xmm0
%assign row row+1
%endrep
    DOT ebp+LOCAL_DIRECTION, ebp+LOCAL_DIRECTION
    sqrtss xmm0, xmm0
    comiss xmm0, [least_det]
    jb .miss
    movss xmm2, [one]
    divss xmm2, xmm0
%assign at 0
%rep 3
    movss xmm0, [ebp+LOCAL_DIRECTION+at]
    mulss xmm0, xmm2
    movss [ebp+LOCAL_DIRECTION+at], xmm0
%assign at at+4
%endrep
    ; t = -(o.x d.x + o.y d.y) / (d.x^2 + d.y^2); along the axis: -(o . d)
    movss xmm2, [ebp+LOCAL_DIRECTION]
    mulss xmm2, xmm2
    movss xmm1, [ebp+LOCAL_DIRECTION+4]
    mulss xmm1, xmm1
    addss xmm2, xmm1
    movss xmm0, [ebp+LOCAL_ORIGIN]
    mulss xmm0, [ebp+LOCAL_DIRECTION]
    movss xmm1, [ebp+LOCAL_ORIGIN+4]
    mulss xmm1, [ebp+LOCAL_DIRECTION+4]
    addss xmm0, xmm1
    comiss xmm2, [least_across]
    jb .along
    divss xmm0, xmm2
    jmp .t
.along:
    movss xmm1, [ebp+LOCAL_ORIGIN+8]
    mulss xmm1, [ebp+LOCAL_DIRECTION+8]
    addss xmm0, xmm1
.t:
    xorps xmm3, xmm3
    subss xmm3, xmm0                ; xmm3 = t
    xorps xmm0, xmm0
    comiss xmm3, xmm0
    jb .miss                        ; behind the eye
%assign at 0
%rep 3
    movss xmm0, [ebp+LOCAL_DIRECTION+at]
    mulss xmm0, xmm3
    addss xmm0, [ebp+LOCAL_ORIGIN+at]
    movss [ebp+LOCAL_ORIGIN+at], xmm0   ; the point
%assign at at+4
%endrep
    movss xmm0, [ebp+LOCAL_ORIGIN]
    mulss xmm0, xmm0
    movss xmm1, [ebp+LOCAL_ORIGIN+4]
    mulss xmm1, xmm1
    addss xmm0, xmm1
    comiss xmm0, [reach_squared]
    ja .miss
    movss xmm0, [ebp+LOCAL_ORIGIN+8]
    comiss xmm0, [z_min]
    jb .miss
    comiss xmm0, [z_max]
    ja .miss
    mov eax, [ebp+0Ch]
    mov ecx, [ebp+LOCAL_ORIGIN]
    mov [eax], ecx
    movss xmm0, [ebp+LOCAL_ORIGIN+4]
    addss xmm0, [y_shift]
    movss [eax+4], xmm0
    mov ecx, [ebp+LOCAL_ORIGIN+8]
    mov [eax+8], ecx
    mov eax, [ebp+10h]
%assign at 0
%rep 3
    xorps xmm0, xmm0
    subss xmm0, [ebp+LOCAL_DIRECTION+at]
    movss [eax+at], xmm0
%assign at at+4
%endrep
    mov al, 1
    jmp .out
.miss:
    xor al, al
.out:
    pop edi
    pop esi
    pop ebx
    mov esp, ebp
    pop ebp
    ret 14h

; 0x2659B2 `call 0x263D00` in the preview (esi = the character), cdecl (out,
; low, high): the original's matrix, or for the weapon above the bench its
; decal (weapon_matrix) times weapon_transform (as the bench draws it; the
; preview picks nothing before the bench has).
decode:
    push dword [esp+0Ch]
    push dword [esp+0Ch]
    push dword [esp+0Ch]
    mov eax, esi
    call is_weapon_edit
    jnz .weapon
    call DECODE
    add esp, 12
    ret
.weapon:
    call weapon_matrix
    add esp, 12
    push ebp
    mov ebp, esp
    sub esp, 40h
    push esi
    push edi
    push edx
    mov edx, esi                    ; the character
    mov esi, eax                    ; the decal
    lea eax, [ebp-40h]
    call weapon_transform
    jz .done                        ; not drawn there yet (the preview picked nothing)
    mov edi, esi
    lea edx, [ebp-40h]
    call multiply                   ; the decal * the weapon's transform
.done:
    mov eax, esi
    pop edx
    pop edi
    pop esi
    mov esp, ebp
    pop ebp
    ret

; 0x2641A0 (the decal editor's bake: stdcall (character, low, high, keep),
; al; `ret 10h`) -> jmp bake + nop. For the weapon above the bench: the
; click's call (1, 0: "pick the place now") picks it, gives the edit its u64
; (+D548h) and accepts it as the bake's callback would (0x2648E0: 0x263950,
; then the movie's root.exit_decal_edit); a miss leaves the edit on (al 0,
; as the original). The start's and the cancel's calls bake nothing there.
bake:
    mov eax, [esp+4]
    call is_weapon_edit
    jnz .weapon
    push ebp
    mov ebp, esp
    and esp, -8
    jmp BAKE_BODY
.weapon:
    cmp dword [esp+8], 1
    jne .nothing
    cmp dword [esp+0Ch], 0
    jne .nothing
    push esi
    mov esi, [esp+8]
    push 0
    push PICK_MASK
    lea eax, [esi+NORMAL]
    push eax
    lea eax, [esi+POINT]
    push eax
    push esi
    call weapon_pick
    test al, al
    jz .done
    call ENCODE
    mov [esi+VALUE], eax
    mov [esi+VALUE+4], edx
    push dword [esi+EDIT_DECAL]
    push dword [esi+EDIT_HOST]
    call ACCEPT
    mov eax, [esi+MENU]
    mov ecx, [eax+690h]
    mov ecx, [ecx+108h]
    mov ecx, [ecx+4]
    push 0
    push 0
    push 0
    push EXIT_DECAL_EDIT
    call MOVIE_CALL
    mov al, 1
.done:
    pop esi
    ret 10h
.nothing:
    mov al, 1
    ret 10h

; 0x263938 `call 0x264CF0` (esi = the character) at the end of the editor's
; start: on a weapon, +D52Ch = the records of slots 0 and 1 before the
; edited one (+D52Dh), the decals' order in `data/decals`; a record of
; another slot is not a decal's: the edit is off again and nothing is
; queried. Then the original's call.
start:
    mov eax, esi
    call is_weapon_edit
    jz START_QUERY
    pushad
    mov eax, [esi+MENU]
    mov eax, [eax+9Ch]
    mov ecx, [eax+53ACh]
    mov eax, [ecx]
    call [eax+INVENTORY_OWNER]
    mov edi, [eax+INVENTORY+4]
    mov ecx, [eax+INVENTORY]
    push dword [esi+EDIT_HOST]
    push esp
    call FIND_ITEM
    add esp, 8
    cmp eax, edi
    je .query                       ; no such item: as the original
    mov edx, [eax+RECORDS]
    mov ecx, [eax+RECORDS+4]
    sub ecx, edx
    movzx edi, byte [esi+RECORD_INDEX]
    imul eax, edi, RECORD_SIZE
    cmp eax, ecx
    jae .not_decal
    cmp byte [edx+eax+RECORD_SLOT], 1
    ja .not_decal
    xor ebx, ebx                    ; the decals before it
.count:
    test edi, edi
    jz .counted
    cmp byte [edx+RECORD_SLOT], 1
    ja .skip
    inc ebx
.skip:
    add edx, RECORD_SIZE
    dec edi
    jmp .count
.counted:
    mov [esi+DECAL_INDEX], bl
.query:
    popad
    jmp START_QUERY
.not_decal:
    mov byte [esi+EDITING], 0
    popad
    ret

; 0x265C20, the character's mouse key handler (thiscall ecx = the character;
; the input world, the code, the kind; al; `ret 0Ch`) -> jmp click + nop.
; While a decal of the weapon above the bench is placed, the left button
; (151h) places it on its release (kind 1, the original's) only when the
; cursor moved CLICK_SLOP pixels or less since the press (kind 0): a drag
; turns the weapon (patch 45) and places nothing (al 0, the release passed
; on). Anything else: the original.
click:
    mov eax, ecx
    call is_weapon_edit
    jz .original
    cmp byte [ecx+EDITING], 0
    je .original
    cmp dword [esp+8], LEFT_KEY
    jne .original
    cmp dword [esp+0Ch], KIND_DOWN
    je .down
    cmp dword [esp+0Ch], KIND_UP
    jne .original
    push edx
    push ebx
    xor ebx, ebx
    xchg ebx, [PRESS+PRESSED]       ; a release ends the press
    test ebx, ebx
    jz .drag                        ; no press seen in the edit (the movie's "change"): no click
    mov eax, [ecx+MENU]
    mov edx, [eax+MOUSE_X]
    sub edx, [PRESS+PRESS_X]
    imul edx, edx
    mov ebx, [eax+MOUSE_Y]
    sub ebx, [PRESS+PRESS_Y]
    imul ebx, ebx
    add edx, ebx
    cmp edx, CLICK_SLOP * CLICK_SLOP
    pop ebx
    pop edx
    jbe .original
    xor al, al
    ret 0Ch
.drag:
    pop ebx
    pop edx
    xor al, al
    ret 0Ch
.down:
    push edx
    mov eax, [ecx+MENU]
    mov edx, [eax+MOUSE_X]
    mov [PRESS+PRESS_X], edx
    mov edx, [eax+MOUSE_Y]
    mov [PRESS+PRESS_Y], edx
    mov dword [PRESS+PRESSED], 1
    pop edx
.original:
    push ebp
    mov ebp, esp
    and esp, -8
    jmp CLICK_BODY

align 4, db 0
; a decal never placed (u64 0): the left side of the receiver for the first
; (x -, the side the bench shows), the right for the second, (0, 0.05, 0.12)
; of the model, size 0.15 (an eighth of a metre, halved), upright
defaults:           dd 0x20080000, 0x8A30CE00, 0x20080200, 0x8A30CE00
abs_mask:           dd 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF
y_shift:            dd 1.0          ; Y_SHIFT: a weapon's y (-1..1) in the u64's 0..2
size:               dd 0.5          ; SIZE: a weapon's decals are half a character's
one:                dd 1.0
least_det:          dd 1.0e-6
least_across:       dd 1.0e-4       ; a ray within 0.6 degrees of the axis is taken along it
reach_squared:      dd 0.0625       ; REACH: 0.25 m from the axis
z_min:              dd -0.6         ; Z_MIN, Z_MAX: a rifle's stock to beyond a long muzzle
z_max:              dd 1.4
name_decal:         db "_%s_%08x%08x", 0
