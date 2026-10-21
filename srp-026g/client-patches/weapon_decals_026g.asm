; Patch 46: decals on a weapon (build 2777, client_patch.md section 3.43). At
; `.srpw`+1000h, the section's second page (the first holds patches 44, 32's
; stash and 45). PREVIEW is patch 32's state (WEAPON_PREVIEW_STATE_OFFSET of
; `.srpd`), TABLE patch 45's (WEAPON_TABLE_STATE_OFFSET).
;   nasm -f bin -DCAVE=0xA42A000 -DPREVIEW=0xA428E00 -DTABLE=0xA428F00 weapon_decals_026g.asm -o weapon_decals_026g.section.bin
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
;               (patch 45) the mouse's ray meets the weapon's long axis
;               instead of the character's collision model
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

; patch 32's state and patch 45's
MODE                equ 0           ; patch 32: 1 while its window's weapon is shown
ITEM                equ 4           ;   the window's item
ON                  equ 0x00        ; patch 45: 1 while the weapon is at the bench
SPIN                equ 0x20        ;   its turns
TILT                equ 0x24

    jmp near lookup
    jmp near fill
    jmp near pick
    jmp near decode
    jmp near bake
    jmp near start

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

; eax = out: the weapon's transform at the bench, as patch 45's matrix makes
; it (rows, v * M): rotY(spin) * rotZ(tilt), the point CENTRE_Z along the
; model's z at BENCH. Keeps every register but the x87 stack's use.
bench_matrix:
    fld dword [TABLE+SPIN]
    fsincos                         ; st0 = cos a, st1 = sin a
    fld dword [TABLE+TILT]
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
    fld dword [centre_z]            ; row 3: BENCH - CENTRE_Z * row 2
    fmul dword [eax+20h]
    fsubr dword [bench_x]
    fstp dword [eax+30h]
    fld dword [centre_z]
    fmul dword [eax+24h]
    fsubr dword [bench_y]
    fstp dword [eax+34h]
    fld dword [centre_z]
    fmul dword [eax+28h]
    fsubr dword [bench_z]
    fstp dword [eax+38h]
    push ecx
    xor ecx, ecx
    mov [eax+0Ch], ecx
    mov [eax+18h], ecx
    mov [eax+1Ch], ecx
    mov [eax+2Ch], ecx
    mov dword [eax+3Ch], 0x3F800000
    pop ecx
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

; the weapon pick's frame
W                   equ -40h        ; the weapon's transform
ORIGIN              equ -4Ch        ; the ray, in the world
DIRECTION           equ -58h
RELATIVE            equ -64h        ; the origin - the transform's point
LOCAL_ORIGIN        equ -70h        ; the ray in the weapon's space
LOCAL_DIRECTION     equ -7Ch
PICK_FRAME          equ 7Ch

; 0x265999 `call 0x264960` in the preview (and the click's pick, below):
; stdcall (character, &point, &normal, mask, &id), al = 1 and the point and
; normal written when the ray hits. For the weapon above the bench: the
; mouse's ray (0x26D130) in the weapon's space (the inverse of the bench's
; transform), the point of it nearest the model's z axis (its long axis
; through the grip), when that is in front of the eye, within REACH of the
; axis and between Z_MIN and Z_MAX along it: the point (y + Y_SHIFT) and the
; normal (the ray's direction, reversed: the decal looks along the ray).
; The decal's projection and occlusion find the surface. Else the original.
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
    mov eax, [ebp+8]
    mov esi, [eax+MENU]
    lea edi, [ebp+DIRECTION]
    lea eax, [ebp+ORIGIN]
    push eax
    call RAY
    lea eax, [ebp+W]
    call bench_matrix
%assign at 0
%rep 3
    movss xmm0, [ebp+ORIGIN+at]
    subss xmm0, [ebp+W+30h+at]
    movss [ebp+RELATIVE+at], xmm0
%assign at at+4
%endrep
%assign row 0
%rep 3
    DOT ebp+RELATIVE, ebp+W+row*10h
    movss [ebp+LOCAL_ORIGIN+row*4], xmm0
    DOT ebp+DIRECTION, ebp+W+row*10h
    movss [ebp+LOCAL_DIRECTION+row*4], xmm0
%assign row row+1
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
; decal (weapon_matrix) at the weapon's transform there: out * W.
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
    sub esp, 80h
    push esi
    push edi
    mov esi, eax                    ; the decal
    lea eax, [ebp-40h]
    call bench_matrix
    lea edi, [ebp-80h]              ; the product, row by row
%assign row 0
%rep 4
%assign col 0
%rep 3
    movss xmm0, [esi+row*10h]
    mulss xmm0, [ebp-40h+col*4]
    movss xmm1, [esi+row*10h+4]
    mulss xmm1, [ebp-40h+10h+col*4]
    addss xmm0, xmm1
    movss xmm1, [esi+row*10h+8]
    mulss xmm1, [ebp-40h+20h+col*4]
    addss xmm0, xmm1
%if row == 3
    addss xmm0, [ebp-40h+30h+col*4]
%endif
    movss [edi+row*10h+col*4], xmm0
%assign col col+1
%endrep
%assign row row+1
%endrep
%assign row 0
%rep 4
%assign col 0
%rep 3
    mov eax, [edi+row*10h+col*4]
    mov [esi+row*10h+col*4], eax
%assign col col+1
%endrep
%assign row row+1
%endrep
    mov eax, esi
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

align 4, db 0
; a decal never placed (u64 0): the left side of the receiver for the first
; (x -, the side the bench shows), the right for the second, (0, 0.05, 0.12)
; of the model, size 0.15 (an eighth of a metre, halved), upright
defaults:           dd 0x20080000, 0x8A30CE00, 0x20080200, 0x8A30CE00
y_shift:            dd 1.0          ; Y_SHIFT: a weapon's y (-1..1) in the u64's 0..2
size:               dd 0.5          ; SIZE: a weapon's decals are half a character's
least_across:       dd 1.0e-4       ; a ray within 0.6 degrees of the axis is taken along it
reach_squared:      dd 0.0625       ; REACH: 0.25 m from the axis
z_min:              dd -0.6         ; Z_MIN, Z_MAX: a rifle's stock to beyond a long muzzle
z_max:              dd 1.4
bench_x:            dd 4.25         ; patch 45's BENCH and CENTRE_Z (weapon_table_026g.asm)
bench_y:            dd 1.30
bench_z:            dd 3.05
centre_z:           dd -0.18
name_decal:         db "_%s_%08x%08x", 0
