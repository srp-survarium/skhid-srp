; Experimental late join, build 2777. Requires the growing-roster dialect.
bits 32
%ifndef CAVE
%error CAVE is required
%endif
; Patch 39's one dword in the state page (patch_026g.py's
; TAB_LIST_STATE_OFFSET): the places the scoreboard was last made with.
%define TAB_ROWS 0xe40
org CAVE
; Patch 40's k, a float (0 until the first projection): FOV_LOD_STATE_OFFSET
; of the state page (patch_026g.py), after the scoreboard's TAB_ROWS.
FOV_LOD_K equ STATE_VA + 0xE50

; eax=core, [esp+4]=slot, ret 4. Used for initial and appended slots.
; Separate ID ranges keep world IDs independent of the initial roster size.
bind_slot:
    jmp bind_slot_reserved

times 96-($-$$) db 0xcc
; Construct both banks of twenty histories; preserve the live roster count.
history_capacity:
    mov esi, 20
    lea ecx, [ebp+0xe3e4]
    jmp 0x2d390e

times 128-($-$$) db 0xcc
; D8's stock debug flag is 00 or FF. Normalize it before advertising the
; experimental dialect, so debug clients send 81 and ordinary clients 80.
connect_capability:
    and bl, 1
    or bl, 0x80
    mov [eax+edi], bl
    mov eax, [esp+0x1c]
    jmp 0x27a244

times 192-($-$$) db 0xcc
; cdecl(bytes, length, uninitialized_profile_storage, dictionary) -> bool.
; Validation runs first: a malformed descriptor never touches the output.
; Construct in its final location: items have pointers into their own storage.
; Called by the background-loader handler and final metadata installation.
decode_profile:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    sub esp, 12
    push dword [ebp+20]
    push dword [ebp+12]
    push dword [ebp+8]
    call C_CODE
    add esp, 12
    test eax, eax
    jz .done
    mov ecx, [ebp+16]
    call 0x2f7c50
    mov eax, [ebp+8]
    mov [esp], eax
    mov [esp+4], eax
    mov eax, [ebp+12]
    mov [esp+8], eax
    mov esi, esp
    mov edi, [ebp+16]
    push dword [ebp+20]
    call 0x1f1070
    mov eax, 1
.done:
    add esp, 12
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

times 320-($-$$) db 0xcc
; cdecl(context, game_world, spatial_tree, dictionary) -> query id.
; Requests only usable kits. The resource engine copies the stack variants;
; their profile pointer refers to the runtime's retained context storage.
query_players:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    sub esp, 0x120
    xor eax, eax
    mov edi, esp
    mov ecx, 0x48
    rep stosd
    mov esi, [ebp+8]
    call .pc
.pc:
    pop ebx
    sub ebx, .pc                    ; executable relocation delta
    call 0x205ba0                   ; native user-data type id
    mov [ebp-0x11c], eax
.kit:
    mov ecx, [ebp-0x118]
    bt [esi+44], ecx                ; lj_context.kit_mask
    jnc .next
    mov edx, [ebp-0x114]           ; compact request index
    lea eax, [ebx+0xa09f48]         ; gameplay/players/default.player
    mov [ebp+edx*8-0x40], eax
    mov dword [ebp+edx*8-0x3c], 0x54
    imul edi, edx, 48
    lea edi, [ebp+edi-0xe0]
    mov [ebp+edx*4-0xf0], edi
    lea eax, [ebx+0xa09f90]         ; native variant vtable
    mov [edi], eax
    lea eax, [esi+60]              ; immutable profile
    mov [edi+8], eax
    mov eax, [esi+24]              ; place
    mov [edi+12], al
    mov eax, [ebp+12]
    mov [edi+16], eax              ; game world
    mov eax, [ebp+16]
    mov [edi+20], eax              ; spatial tree
    mov eax, [ebp+20]
    mov [edi+24], eax              ; dictionary
    mov [edi+28], cl               ; kit, spectator byte remains zero
    mov [edi+40], edi
    mov eax, [ebp-0x11c]
    mov [edi+44], eax
    inc dword [ebp-0x114]
.next:
    inc dword [ebp-0x118]
    cmp dword [ebp-0x118], 3
    jb .kit
    push dword 0
    push esi
    push dword 0
    lea eax, [ebx+query_completed]
    push eax
    lea edi, [ebp-0x110]
    call 0x27d280                  ; native boost member callback, ret 16
    push dword 1                  ; normal asynchronous query
    push dword 0                  ; no parent query
    lea eax, [ebp-0xf0]
    push eax
    push dword [ebx+0x1f1d9c4]
    lea eax, [ebp-0x110]
    push eax
    lea eax, [ebp-0x40]
    mov ecx, [ebp-0x114]
    call 0xe5b60
    add esp, 20
    add esp, 0x120
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

times 576-($-$$) db 0xcc
; The camera's projection (patch 22; 0x242900, slot 0 of every camera's
; vtable) jumps here at its first instruction. ecx = the camera, [esp+4] =
; the matrix to fill, [esp+8] = the viewport's size; `ret 8`, eax = the
; matrix, as the original. The original passes the matrix builder 0xEEE30
; far (+8Ch), near (+88h), the aspect and xmm0 = z * 65 degrees, z being the
; zoom (+90h): 1 not aimed, 1/aim_zoom_factor with iron sights and a scope's
; fov_factor (0.25) aimed, slid by the aim's progress (0x1F3480). Here the
; angle in degrees is
;   65 z + (uro_fov - 65) * max(0, (z - 0.25) / 0.75):
; uro_fov unaimed, nearly it with iron sights, the original's 65 * 0.25
; through a 4x scope whatever the setting, and between them a slide that
; follows the aim. Only xmm0 of the XMM registers is written, as before.
; Patch 40: it also keeps k = max(1, tan(A/2) / tan(65 z/2)), A the angle
; it passes, 65 z the original's at the same zoom, at FOV_LOD_K of the
; state page, for fov_lod_scale and fov_screen_scale below. k is exactly 1
; at uro_fov 65 or less (A = 65 z + 0, the same two tangents) and through
; a full scope; 1.87 unaimed at 100. The x87 stack is empty again before
; the call, as it was.
fov_projection:
    mov eax, [esp+8]
    push esi
    mov esi, [esp+8]
    push dword [ecx+0x8c]
    push dword [ecx+0x88]
    push eax
    fld dword [ecx+0x90]
    fld st0
    fsub dword [fov_scope]
    fldz
    fcomi st0, st1
    fcmovb st0, st1
    fstp st1
    fmul dword [fov_blend]
    fld dword [0xA6FA28]
    fsub dword [fov_stock]
    fmulp st1, st0
    fxch st1
    fmul dword [fov_stock]
    fld st0
    faddp st2, st0
    fld st1
    fmul dword [fov_radians]
    fstp dword [esp]
    fmul dword [fov_half]
    fptan
    fstp st0
    fxch st1
    fmul dword [fov_half]
    fptan
    fstp st0
    fdivrp st1, st0
    fld1
    fxch st1
    fcomi st0, st1
    fcmovbe st0, st1
    fstp st1
    fstp dword [FOV_LOD_K]
    movss xmm0, [esp]
    fld dword [eax]
    fdiv dword [eax+4]
    fstp dword [esp]
    call 0xEEE30
    add esp, 12
    mov eax, esi
    pop esi
    ret 8
fov_scope:   dd 0.25
fov_blend:   dd 1.3333334
fov_stock:   dd 65.0
fov_radians: dd 0.017453292
fov_half:    dd 0.008726646

times 768-($-$$) db 0xcc
; Native boost member callback: ecx=context, [esp+4]=queries_result.
; queries_result owns its resources through the return from this callback.
query_completed:
    push ebp
    mov ebp, esp
    push esi
    push edi
    sub esp, 12
    mov esi, ecx
    mov edi, [ebp+8]
    add edi, 0x12c
    xor eax, eax
    mov [esp], eax
    mov [esp+4], eax
    mov [esp+8], eax
.kit:
    bt [esi+44], eax
    jnc .next
    mov edx, [edi]
    mov [esp+eax*4], edx
    add edi, 0x2e0
.next:
    inc eax
    cmp eax, 3
    jb .kit
    mov eax, esp
    push eax
    push esi
    call LJ_COMPLETE
    add esp, 8
    add esp, 12
    pop edi
    pop esi
    pop ebp
    ret 4

times 896-($-$$) db 0xcc
; cdecl(size): the game's allocator and debug label, ret 4 in the native ABI.
allocate_context:
    push esi
    call .pc
.pc:
    pop edx
    sub edx, .pc
    mov esi, [edx+0x1f1d9c4]
    lea eax, [edx+context_label]
    push eax
    mov eax, [esp+12]
    call 0xe4f90
    pop esi
    ret

times 960-($-$$) db 0xcc
; cdecl(pointer). Native free also clears the supplied local pointer.
free_context:
    call .pc
.pc:
    pop ecx
    sub ecx, .pc
    mov ecx, [ecx+0x1f1d9c4]
    mov eax, [esp+4]
    lea edx, [esp+4]
    push edx
    call 0x1539d0
    add esp, 4
    ret

times 1024-($-$$) db 0xcc
retain_world:
    mov eax, [esp+4]
    lock inc dword [eax+0xd0]
    ret

times 1056-($-$$) db 0xcc
release_world:
    lea ecx, [esp+4]
    jmp 0x11540

context_label: db 'srp late-join profile', 0

times 1120-($-$$) db 0xcc
install_context:
    push ebp
    mov ebp, esp
    call .pc
.pc:
    pop edx
    sub edx, .pc
    lea eax, [edx+0x321790]
    push eax
    lea eax, [edx+player_ready]
    push eax
    lea eax, [edx+bind_animations]
    push eax
    lea eax, [edx+bind_physics]
    push eax
    lea eax, [edx+bind_core]
    push eax
    mov eax, esp
    push eax
    push dword [ebp+8]
    call LJ_INSTALL
    add esp, 28
    test eax, eax
    jz .done
    ; The newcomer has its place: the lists of players made at the match's
    ; start (the scoreboard among them) are made again, with it in them.
    push eax
    mov ecx, [ebp+8]
    mov edx, [ecx+8]              ; context.client
    mov eax, [edx+0x1f1c48]
    add eax, 0xc                  ; the options (the profiles and their count), as
    push eax                      ; the client's vtable+34h gives them (0x219990)
    mov edx, [edx+0x14]           ; game; its game_world is at +18, the hud at +1fc of that
    add edx, 0x214
    push edx                      ; both as 0x1F8542..0x1F855D pass them
    call rebuild_lists
.rebuild_return:
    pop eax
.done:
    pop ebp
    ret
bind_core:
    mov eax, [esp+4]
    push dword [esp+8]
    call bind_slot
    ret
bind_physics:
    mov eax, [esp+4]
    push dword [esp+8]
    call 0x3218a0
    ret
bind_animations:
    push edi
    mov edi, [esp+8]
    push dword [esp+12]
    call register_animations
    pop edi
    ret
player_ready:
    push esi
    mov esi, [esp+8]
    call 0x1f29d0
    pop esi
    ret

times 1312-($-$$) db 0xcc
query_context:
    mov ecx, [esp+4]
    mov edx, [ecx+8]              ; context.client
    mov edx, [edx+0x14]           ; game, its game_world is embedded at +18
    push dword [edx+0x53a4]
    mov eax, [ecx+12]             ; context.world = pvp_match_core
    push dword [eax+0x130]        ; pvp.spatial_tree (constructor 0x2d567b)
    add edx, 0x18
    push edx
    push ecx
    call query_players
    add esp, 16
    ret

times 1408-($-$$) db 0xcc
; cdecl(client, seed, serial, place, status). A full packet pool means retry
; on the server's next Prepare; allocating from an exhausted pool is fatal.
send_prepared:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    sub esp, 24
    mov ebx, [ebp+8]
    cmp dword [ebx+0x1f1c64], 2
    jne .done
    mov ecx, [ebx+0x1f1c48]
    test ecx, ecx
    jz .done
    mov eax, [ebp+12]
    cmp [ecx+0x577bc], eax
    jne .done
    mov edx, [ecx+0x578b0]
    mov eax, [edx+0x30]
    add eax, 17
    cmp eax, [edx+0x34]
    ja .done
    mov dword [ebp-36], 0xffffffff
    mov dword [ebp-32], 0x0000ffff
    mov eax, [ebp+12]
    mov [ebp-28], eax
    mov eax, [ebp+16]
    mov [ebp-24], eax
    mov eax, [ebp+20]
    mov [ebp-20], al
    mov eax, [ebp+24]
    mov [ebp-19], al
    mov word [ebp-18], 1
    mov eax, [ecx]
    push 0xe0
    call [eax+0x18]
    mov esi, eax
    and byte [eax+0x98], 0xbf
    lea edx, [ebp-36]
    push edx
    add eax, 0x2a8
    push eax
    mov edi, 20
    call 0x7ac10
    mov ecx, [ebx+0x1f1c48]
    mov eax, [ecx]
    push esi
    call [eax+0x1c]
.done:
    add esp, 24
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

times 1664-($-$$) db 0xcc
; on_match_ready, before the existing initial-snapshot upload hook. ebx=NC.
ready_hook:
    pushad
    call initialize_api
    mov esi, eax
    mov edi, [ebx+0x1f1c40]
    test edi, edi
    jz .done
    cmp [esi+40], ebx
    jne .activate
    cmp [esi+44], edi
    je .done
.activate:
    mov edx, [edi+0x124]
    movzx eax, byte [edx+0x577b5]
    push eax
    push dword [edx+0x577b0]
    mov eax, [ebx+0x14]
    push dword [eax+0x53a4]
    push edi
    push ebx
    push esi
    call LJ_ACTIVATE
    add esp, 24
.done:
    popad
    jmp UPLOAD_READY

times 1760-($-$$) db 0xcc
; Patch 40: the high-FOV LOD compensation. The game measures a model by
; its size on the screen, through the projection: a static model's LOD
; (static_render_model_instance::select_lod 0x16EEC0, type 1, LOD 3 draws
; nothing) and every surface's screen factor (0x158D80: shader LOD, sun
; and spot shadow casters, rain occluders). At a wider angle a model is
; smaller on the screen, so it went down in LOD and out at a shorter
; distance than the original's 65 degrees. Both multiply by k
; (fov_projection above) after the original's own constant, so they see
; the model as the original at the same zoom would. Before the first
; projection the page is zero, and they keep the original's value.
; select_lod's `mulss xmm0,[0x927A44]` (0.5, 0x16F2A4) calls here.
fov_lod_scale:
    mulss xmm0, [0x927A44]
    cmp dword [FOV_LOD_K], 0
    je .stock
    mulss xmm0, [FOV_LOD_K]
.stock:
    ret

times 1792-($-$$) db 0xcc
; calculate_screen_factor's `fmul dword [0xA710D0]` (100, 0x1590AF) calls here.
fov_screen_scale:
    fmul dword [0xA710D0]
    cmp dword [FOV_LOD_K], 0
    je .stock
    fmul dword [FOV_LOD_K]
.stock:
    ret

times 1856-($-$$) db 0xcc
; E4 case of on_match_packet_received: esi=NC, ebx=reader.
prepare_hook:
    cmp dword [esi+0x1f1c64], 1
    je .initial
    cmp dword [esi+0x1f1c64], 2
    jne .end
    pushad
    call get_state
    cmp [eax+40], esi
    jne .done
    mov ecx, [ebx+4]
    cmp ecx, [ebx]
    jb .done
    mov edx, [ebx]
    add edx, [ebx+8]
    jc .done
    sub edx, ecx
    jc .done
    push edx
    push ecx
    push eax
    call LJ_PREPARE
    add esp, 12
.done:
    popad
.end:
    jmp 0x27a5ab
.initial:
    pushad
    call get_state
    push eax
    call LJ_DEACTIVATE
    add esp, 4
    popad
    jmp 0x27a385

times 2048-($-$$) db 0xcc
; Replace the call to player::~player in its scalar deleting destructor.
; Save the profile before destruction and notify only after native cleanup.
player_destroyed_hook:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    mov ebx, ecx
    mov esi, [ecx+0x6b8]
    call 0x1f2100
    push eax
    call get_state
    push esi
    push ebx
    push eax
    call LJ_DESTROYED
    add esp, 12
    pop eax
    pop esi
    pop ebx
    pop ebp
    ret

times 2176-($-$$) db 0xcc
initialize_api:
    push ebx
    call .pc
.pc:
    pop ebx
    sub ebx, .pc
    call get_state
    cmp dword [eax+8], 0
    jne .done
%assign field 8
%macro api 1
    lea edx, [ebx+%1]
    mov [eax+field], edx
%assign field field+4
%endmacro
    api allocate_context
    api free_context
    api decode_profile
    api retain_world
    api release_world
    api query_context
    api install_context
    api send_prepared
.done:
    pop ebx
    ret

times 2304-($-$$) db 0xcc
get_state:
    call .pc
.pc:
    pop eax
    add eax, STATE_VA-.pc
    ret

times 2336-($-$$) db 0xcc
; on_match_disconnected, immediately before releasing NC's PVP reference.
disconnect_hook:
    pushad
    call get_state
    cmp [eax+40], edi
    jne .done
    push eax
    call LJ_DEACTIVATE
    add esp, 4
.done:
    popad
    mov ecx, [edi+0x1f1c40]
    jmp 0x27b51d

times 2432-($-$$) db 0xcc
; network_client destructor: retire before destroying match_client or PVP.
client_destroyed_hook:
    pushad
    call get_state
    cmp [eax+40], ecx
    jne .done
    push eax
    call LJ_DEACTIVATE
    add esp, 4
.done:
    popad
    push ebp
    mov ebp, esp
    and esp, -8
    jmp 0x219e66

times 2480-($-$$) db 0xcc
; The scoreboard has a row for a place that is in the match, and for no
; other (patch 39; ../docs/notes/tab-list-left-places.md): a place whose
; player left, by its own word or by a link the server gave up on, is not
; shown. 0x23537A in 0x234880's first loop, where a place's row is put
; into the scoreboard's array (root.list_set_players): `lea eax,[esp+58h]`
; / `push eax` / `mov eax,[esp+138h]` / `push ebx` / `push eax` / `call
; edx`, the array's SetElement (its interface's vtable+34h) with the place
; (ebx) as the index. ecx is the array's interface ([esp+12Ch]), its data
; at [esp+134h], the row at [esp+58h], the hud at [esp+5E8h] (here 4 more
; each). The row goes in at the array's end (PushBack, vtable+3Ch, as the
; enemies' ids are put in at 0x234F2D) when the place is in the match:
; kd_stats_rule's `online` (on_player_entered_match 0x3239F0 sets it,
; on_player_left_match 0x323A00 clears it, for the server's 0xE9 and 0xEA,
; in every game of the match at the same tick; a world brings it), which
; fill_match_hud_state 0x323980 copies into the hud's state at +0C0h +
; 8 x place every tick. A list made: the mask kept in the state page is
; none (-1), so the next update of the shown list makes the lists again
; with the places of then (tab_shown), which keeps the mask after its own
; run.
tab_row:
    call get_state
    or dword [eax+TAB_ROWS], -1
    mov eax, [esp+0x5ec]          ; the hud
    cmp byte [eax+ebx*8+0xc0], 0
    je .not_in_match
    lea eax, [esp+0x5c]           ; the row
    push eax
    push dword [esp+0x13c]        ; the array's data
    mov edx, [ecx]
    call [edx+0x3c]               ; PushBack
.not_in_match:
    ret

%ifdef TEST_SESSION
times 2560-($-$$) db 0xcc
; Test-only startup: EDI=game, EAX=NC; stack=(host,port,spectator), ret 12.
direct_match:
    push ebp
    mov ebp, esp
    pushad
    push dword [ebp+16]
    push dword [ebp+12]
    push dword [ebp+8]
    call 0x1ed6e0
    cmp dword [ebp+16], 0
    jne .done
    mov ecx, [edi+0x53ac]
    test ecx, ecx
    jz .done
    mov dword [ecx+0x1ee7ec], TEST_SESSION
    push TEST_SESSION
    push 1
    push dword [ebp+12]
    push dword [ebp+8]
    call 0x21beb0
.done:
    popad
    leave
    ret 12
%endif

times 2816-($-$$) db 0xcc
; The slot's ids, its weapons' places, and where its saved objects go.
;
; The world saves and loads its game objects in the order of one list
; (core+18914h the first, +18918h the last; a node is vtable, +4 the one
; before, +8 the next; 0x2D2699 saves, 0x2D2B74 loads): that order is the
; layout of the world's "game objects" bytes, so every game of a match must
; hold the same one. The game appends: a slot's grenades and traps when the
; slot is bound (0x303820, 0x3067A0: one node for each of a stack), then
; the map's objects (0x285750). A slot bound later, when the match takes a
; player in, came after the map's objects, while that player's own game,
; which starts with the slot, had it before them: the two never agreed.
; So a slot's nodes always go in front of everything the list holds, in the
; order the game binds them: the list is the same whenever a slot was
; bound, the newest slot first.
bind_slot_reserved:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    sub esp, 20
    mov ebx, eax
    mov esi, [ebp+8]
    movzx eax, byte [esi+0x2c]
    inc eax
    shl eax, 16
    mov [ebp-16], eax
    mov eax, [ebx+0x18fb0]
    mov [ebp-12], eax
    lea eax, [ebp-16]
    mov [ebx+0x18fb0], eax
    ; Six weapon IDs per player: three kits, two weapons each.
    mov eax, [ebx+0x18b4c]
    mov edx, [ebx+0x18b50]
    lea ecx, [eax+120*4]
    cmp edx, ecx
    cmovb edx, ecx
    mov [ebp-20], edx
    movzx edx, byte [esi+0x2c]
    imul edx, 24
    add eax, edx
    mov [ebx+0x18b50], eax
    ; The list is put aside: the game binds the slot into an empty one.
    mov eax, [ebx+0x18914]
    mov [ebp-24], eax
    mov eax, [ebx+0x18918]
    mov [ebp-28], eax
    xor eax, eax
    mov [ebx+0x18914], eax
    mov [ebx+0x18918], eax
    push esi
    mov eax, ebx
    call 0x321820
    mov eax, [ebp-24]             ; the first of what was put aside
    mov ecx, [ebx+0x18918]        ; the slot's last node
    test ecx, ecx
    jnz .has_nodes
    mov [ebx+0x18914], eax        ; the slot has none: the list as it was
    jmp .last
.has_nodes:
    test eax, eax
    jz .listed                    ; nothing was put aside
    mov [ecx+8], eax              ; ... and what was put aside follows it
    mov [eax+4], ecx
.last:
    mov eax, [ebp-28]
    mov [ebx+0x18918], eax
.listed:
    mov eax, [ebp-20]
    mov [ebx+0x18b50], eax
    mov eax, [ebp-12]
    mov [ebx+0x18fb0], eax
    add esp, 20
    pop esi
    pop ebx
    pop ebp
    ret 4

times 3072-($-$$) db 0xcc
; Native ABI: EDI=player, stack=(animation table), ret 4.
register_animations:
    push dword [esp+4]
    call 0x2d8100
    push ebx
    call .pc
.pc:
    pop ebx
    sub ebx, .pc-CAVE
    lea eax, [ebx+animation_less-CAVE]
    push eax
    lea eax, [ebx+animation_append-CAVE]
    push eax
    push dword [esp+16]
    call LJ_ANIMATIONS
    add esp, 12
    pop ebx
    ret 4

animation_less:
    push edi
    mov edi, [esp+12]
    push dword [esp+8]
    call 0x2ef060
    movzx eax, al
    pop edi
    ret

animation_append:
    push esi
    mov esi, [esp+8]
    lea eax, [esp+12]
    push eax
    call 0x2ef410
    pop esi
    ret

times 3168-($-$$) db 0xcc
; 0x316E57 in the respawn rule's spawn 0x316DA0: `mov edx,[esi]` / `mov
; edx,[edx+1Ch]`, the load of the call that initialises the new kit's
; player (vtable+1Ch). Here the new kit is the current one (0x316DE3); esi =
; its player; [esp+10h] is the kit of the player that was current before and
; [esp+18h] the new one's (0x316E02, 0x316E06; the game's own test of a
; change at 0x316E0A), +24h here behind the call and pushad.
;
; A world holds the current kit's player alone. A game that loaded one during
; a match (a newcomer, a game that was resynced or came back) has the other
; kits' players as they were built; the games that played them have them as
; their last life left them. Before a spawn the respawn tick takes the
; place's current player out of the world (0x3169A0, vtable+24h with 1:
; 0x2DA790): the weapon in hand is put away and the inventory goes back to
; the profile (0x2FD2D0), each weapon's magazine and chambered round back
; into its ammunition stack (0x2E18E0). A weapon keeps through that, and the
; initialize does not set again: the time it was last ticked (+5A4h; only
; the weapon in hand is ticked, and its activation, 0x2E2A30, which the
; initialize runs for the weapon it puts in hand, sets it to the spawn's time
; first), the fire mode (+5E6h, changed by the player's input alone,
; 0x2E0CE1) and the ammunition kind (+5B1h, +5B2h). Seen in saved worlds of
; the players' server: +5A4h of a kit's weapon in slot 10, -1 in a game that
; loaded a world, the time of its last use in the others.
; At every spawn each weapon of the player gets -1 at +5A4h; at a spawn into
; another kit than the one before, also the fire mode, the ammunition kind
; and, as a safety (LIKELY already so after the removal above), +5A8h, the
; magazine, the aim (+5E8h), +5ECh, the chamber (+5F1h), +5F5h and the
; recoil (+5FCh..+60Ch) their constructor's values (0x2DF0D0), before the
; initialize: the weapon put in hand is then activated as a new one is. A
; respawn into the same kit keeps what the player chose: that kit's player
; is in every world, so every game has it alike.
kit_state_hook:
    pushad
    call .pc
.pc:
    pop ebx
    add ebx, 0x2e30f0-.pc         ; weapon_core's bind, where the image is
    mov ecx, [esi+0x10c]          ; the player's inventory
    test ecx, ecx
    jz .done
    mov al, [esp+0x34]            ; the kit before (the spawn's [esp+10h])
    cmp al, [esp+0x3c]            ; the new kit (the spawn's [esp+18h])
    setne al
    xor edi, edi                  ; slot 0..22, the inventory's +10Ch
.slot:
    mov edx, [ecx+edi*4+0x10c]
    test edx, edx
    jz .next
    mov ebp, [edx]
    cmp [ebp+0x70], ebx
    jne .next                     ; not a weapon (the item is weapon+10h)
    add edx, 0x594                ; weapon+5A4h
    or dword [edx], -1            ; +5A4h m_last_tick_time_in_ms
    test al, al
    jz .next
    or dword [edx+0x04], -1       ; +5A8h m_ammo_switch_time_in_ms
    xor ebp, ebp
    mov [edx+0x0d], bp            ; +5B1h m_current_ammo_id, +5B2h m_next_ammo_id
    mov [edx+0x41], bp            ; +5E5h m_ammo_in_magazine, +5E6h m_fire_queue_type
    mov byte [edx+0x44], 0        ; +5E8h m_aimed
    mov dword [edx+0x48], 6       ; +5ECh
    mov byte [edx+0x4d], 0        ; +5F1h m_is_round_chambered
    mov byte [edx+0x51], 0        ; +5F5h m_need_to_auto_reload
    mov [edx+0x58], ebp           ; +5FCh..+608h, 0.0
    mov [edx+0x5c], ebp
    mov [edx+0x60], ebp
    mov [edx+0x64], ebp
    or dword [edx+0x68], -1       ; +60Ch
.next:
    inc edi
    cmp edi, 23
    jb .slot
.done:
    popad
    mov edx, [esi]                ; what the call replaces
    mov edx, [edx+0x1c]
    ret

times 3328-($-$$) db 0xcc
; The hud's lists of players: the scoreboard (root.list_set_players), the
; second list (root.set_players_list) and the enemies' markers
; (root.set_enemy_icon_ids). The game makes them once, in 0x234880 when the
; match is ready, for the players there are then (the loops end at the
; count, options+577B5h); nothing makes them again, so the players of a
; match did not see a newcomer in their scoreboard.
; Stack=(hud, options), ret 8, as 0x234880 itself. This builds that
; function's frame and goes into it where the lists are made (0x234E71);
; lists_hook takes it out again, through the function's own release of
; the two arrays and its own return. Nothing else of the function runs:
; not its subscription, not root.reset, not the hud's items.
rebuild_lists:
    mov eax, [esp+4]
    mov ecx, [esp+8]
    ; Only the hud this match's start made, with its two movies.
    cmp [eax+0x1c], ecx
    jne .not_ours
    cmp dword [eax+8], 0
    je .not_ours
    cmp dword [eax+0x10], 0
    je .not_ours
    sub esp, 0x5d4
    push ebx
    push ebp
    push esi
    push edi
    mov ebp, [esp+0x5e8]          ; the hud, as 0x234888 has it
    lea ebx, [ebp+8]              ; as 0x234AC8
    mov [esp+0x98], ebx           ; as 0x234AE1
    xor eax, eax
    mov [esp+0xdc], eax           ; two values the function's end releases
    mov [esp+0x160], eax          ; when they hold something: they hold nothing
    jmp 0x234e71
.not_ours:
    ret 8

times 3456-($-$$) db 0xcc
; 0x23572B in 0x234880, right after root.set_enemy_icon_ids, the last of
; the three lists: `mov ecx,[esp+98h]`. A run that rebuild_lists began (its
; return address is the one in install_context) ends here.
lists_hook:
    call .pc
.pc:
    pop ecx
    add ecx, install_context.rebuild_return-.pc
    cmp [esp+0x5e4], ecx
    je .rebuilt
    ; ... or the one in tab_shown (patch 39).
    add ecx, tab_shown.rebuilt-install_context.rebuild_return
    cmp [esp+0x5e4], ecx
    jne .start_of_match
.rebuilt:
    xor esi, esi                  ; as the function's end has it
    jmp 0x2363ca
.start_of_match:
    mov ecx, [esp+0x98]
    jmp 0x235732

times 3520-($-$$) db 0xcc
; 0x39E52 in loose_oct_tree::insert 0x39E30 (the scene's and the collision
; world's tree), right after the object gave its box: eax -> min xyz, max
; xyz, six floats; edi = the object, which keeps a copy at +4. A box with a
; number in it that is no number or not finite is outside the tree, and
; update_bounds 0x396F0 then doubles the tree's size without end (the size
; to reach works out as infinity): it takes a node each time until none is
; left and reads through a null one (0x399A8; seen live 2026-10-13 in a
; game that was given a newcomer, when a model in the scene was moved to
; such a place). Such a box is taken as a box of two metres at the origin;
; the next move of the object puts it where it is.
box_hook:
    xor ecx, ecx
.next:
    mov edx, [eax+ecx*4]
    and edx, 0x7f800000
    cmp edx, 0x7f800000
    je .not_a_box
    inc ecx
    cmp ecx, 6
    jb .next
.copy:
    movq xmm0, [eax]
    movq [edi+4], xmm0
    jmp 0x39e5b
.not_a_box:
    call .pc
.pc:
    pop eax
    add eax, .box-.pc
    jmp .copy
align 4, db 0xcc
.box: dd -1.0, -1.0, -1.0, 1.0, 1.0, 1.0

times 3600-($-$$) db 0xcc
; Patch 39 (tab_row above): 0x23B421 in game_world_ui::update_ui (0x23B200,
; edi = the hud), when the hud's state has changed, before the players'
; rows are refreshed while the list is shown: `cmp byte [edi+0BAh],0` (the
; `jz 23B477h` after the call stays and takes its flags from here). While
; the list is shown, the places in the match now (the hud's +0C0h + 8 x
; place, for the count the loop after this reads) are set against the
; mask the lists were last made with (TAB_ROWS of the state page); when
; they differ, the three lists are made again (rebuild_lists, as the late
; join's install has them made, with the options the hud's were made
; with, [hud+1Ch]) and the mask is kept. A list made by anything else
; leaves the mask at -1 (tab_row). The loop after this refreshes only the
; rows of places in the match (patch_026g.py's TAB_LIST_LOOP).
tab_shown:
    cmp byte [edi+0xba], 0
    je .hidden
    pushad
    mov eax, [edi+0x18]
    mov eax, [eax+0x4d4c]
    mov eax, [eax+0x124]          ; the options, as 0x23B42C..0x23B435 read them
    movzx ecx, byte [eax+0x577b5] ; the places
    cmp ecx, 20
    jbe .places
    mov ecx, 20                   ; the hud's state has twenty
.places:
    xor edx, edx
.place:
    dec ecx
    js .counted
    cmp byte [edi+ecx*8+0xc0], 0
    je .place
    bts edx, ecx
    jmp .place
.counted:
    call get_state
    lea esi, [eax+TAB_ROWS]
    cmp [esi], edx
    je .same
    push edx
    push dword [edi+0x1c]
    push edi
    call rebuild_lists
.rebuilt:
    pop dword [esi]
.same:
    popad
    cmp byte [edi+0xba], 0
.hidden:
    ret

times 3712-($-$$) db 0xcc
; 0x316EA0 in the respawn rule's spawn 0x316DA0 (slot +5Ch of every respawn
; rule; the rule's tick 0x316820 calls it for every spawn, a match's first
; too, in every game and again in every replay of the tick after a
; rollback): `mov edx,[esi]` / `mov eax,[edx+20h]`, the load of the call
; that puts the player into the world (vtable+20h). Here the new kit is the
; current one (0x316DE3) and its player is initialised (vtable+1Ch: the
; inventory set up from the profile, a weapon in hand; no weapon object is
; made anew); esi = that player, ebp = the spawn's time.
;
; A weapon's random state, weapon_core+5BCh (boost's minstd_rand: x * 48271
; mod 2^31-1; drawn from by the spread of its shots alone, 0x2E1D50 and
; 0x2E2120), is set to 1 by the constructor and read from a world by
; deserialize, and by nothing else. A world holds the current kit's player
; alone, so a game that loaded one during a match (a newcomer, a game that
; was resynced) had the other kits' weapons at 1 where the games that played
; them had them drawn from: a respawn into such a kit put the games out of
; step at once, and so did a rollback over a respawn into another kit. Each
; weapon of the player's inventory (an item whose vtable+70h is weapon_core's
; bind 0x2E30F0: weapon_core's and the client's weapon's vtables alone have
; it; the inventory holds the item, weapon+10h) gets a state every game works
; out alike from its place (+114h), kit (+115h), slot and the spawn's time:
;   h = ((place << 16 | kit << 8 | slot) * 0x85EBCA6B) ^ (time * 0x9E3779B1)
;   state = (h ^ h >> 15) mod (2^31 - 2) + 1, in 1..2^31-2
; (0 would stay 0 for ever, 2^31-1 would become 0).
weapon_random_hook:
    pushad
    call .pc
.pc:
    pop ebx
    add ebx, 0x2e30f0-.pc         ; weapon_core's bind, where the image is
    mov ecx, [esi+0x10c]          ; the player's inventory
    test ecx, ecx
    jz .done
    xor edi, edi                  ; slot 0..22, the inventory's +10Ch
.slot:
    mov edx, [ecx+edi*4+0x10c]
    test edx, edx
    jz .next
    mov eax, [edx]
    cmp [eax+0x70], ebx
    jne .next                     ; not a weapon
    movzx eax, byte [esi+0x114]   ; place
    shl eax, 8
    mov al, [esi+0x115]           ; kit
    shl eax, 8
    or eax, edi                   ; slot
    imul eax, eax, -0x7a143595    ; 0x85EBCA6B
    push edx
    imul edx, ebp, -0x61c8864f    ; the time * 0x9E3779B1
    xor eax, edx
    mov edx, eax
    shr edx, 15
    xor eax, edx
    push ecx
    xor edx, edx
    mov ecx, 0x7ffffffe
    div ecx
    pop ecx
    inc edx
    pop eax
    mov [eax+0x5ac], edx          ; weapon+5BCh
.next:
    inc edi
    cmp edi, 23
    jb .slot
.done:
    popad
    mov edx, [esi]                ; what the call replaces
    mov eax, [edx+0x20]
    ret

times 3840-($-$$) db 0xcc
; A game's first world of a match (game_world_core::deserialize 0x2D0B70,
; the block 0x2D0FE3..0x2D105C while the world is still frozen; a newcomer's,
; a game coming back, and every game's at a match's start) gives each place
; of the world an input and a kit choice at the world's time, two calls at
; 0x2D1050 and 0x2D1055 with edi -> {core, the world's time, place}. The
; game's own (0x2D4AB0, 0x2D4B50) put the entry at the newest end of the
; place's list even when the list held newer ones (inputs and kit choices
; relayed between the server's 0xE5 and the world), where every other
; writer keeps it in time order (the network's 0x1FB42A / 0x1FBF00 insert
; through 0x2D4850); and the kit choice they put was the kit in use
; (player+115h), not the one the world has chosen for the next spawn
; (place+0Dh, read from the world at 0x2D2A37). Each tick 0x321790 takes
; the place's next kit from the newest entry at or before the tick, from
; the newest end: a choice of the world, or one relayed for later, was
; replaced by the kit in use, and the newcomer respawned that player in
; another kit than the other games did. Here the same entries, the kit
; choice being the world's, go where their time puts them; an entry of the
; same time is given the value, as the game's own did.
;
; first_load_input replaces `call 0x2d4ab0`: the current player's input
; (+32Ch, +330h) into the place's input list (core+2FCCh, 480h a place).
first_load_input:
    push ebx
    push ebp
    push esi
    movzx eax, byte [edi+8]
    mov ecx, [edi]
    lea esi, [eax+eax*8]
    shl esi, 7
    add esi, [ecx+0x2fcc]
    lea eax, [eax+eax*2]
    shl eax, 4
    add eax, [ecx+0x18458]
    movzx ecx, byte [eax+0xc]
    mov eax, [eax+ecx*4]
    mov ebx, [eax+0x32c]
    mov ebp, [eax+0x330]
    call entry_at_time
    mov [eax+8], ebx
    mov [eax+0xc], ebp
    pop esi
    pop ebp
    pop ebx
    ret

times 3920-($-$$) db 0xcc
; first_load_kit replaces `call 0x2d4b50`: the kit the world has chosen for
; the place's next spawn (place+0Dh) into its kit choices (core+89D8h).
first_load_kit:
    push ebx
    push esi
    movzx eax, byte [edi+8]
    mov ecx, [edi]
    lea esi, [eax+eax*8]
    shl esi, 7
    add esi, [ecx+0x89d8]
    lea eax, [eax+eax*2]
    shl eax, 4
    add eax, [ecx+0x18458]
    mov bl, [eax+0xd]
    call entry_at_time
    mov [eax+8], bl
    pop esi
    pop ebx
    ret

; esi = a place's list (+4 the oldest, +8 the newest; a node: +0 the next
; newer, +4 the next older, +20h its time), [edi+4] = the time. Returns in
; eax the node of that time: the one there is, or a new one (the game's
; 0x1FCB60, which may first give the oldest node back to the pool when
; the pool is full) put before the oldest node that is newer (0x2D4850; at
; the newest end when there is none, as the game's own did). Keeps ebx,
; ebp, esi, edi.
entry_at_time:
    mov ecx, [edi+4]
    mov eax, [esi+8]
.same:
    test eax, eax
    jz .new
    cmp [eax+0x20], ecx
    je .found
    jb .new                       ; older: none of that time
    mov eax, [eax+4]
    jmp .same
.new:
    mov eax, esi
    call 0x1fcb60
    mov ecx, [edi+4]
    mov [eax+0x20], ecx
    mov edx, [esi+8]
.older:
    test edx, edx
    jz .oldest
    cmp [edx+0x20], ecx
    jb .after
    mov edx, [edx+4]
    jmp .older
.after:
    mov edx, [edx]                ; before the next newer one, or at the end
    jmp .link
.oldest:
    mov edx, [esi+4]              ; before the oldest (none: the list is empty)
.link:
    push esi
    call 0x2d4850
.found:
    ret

times 4096-($-$$) db 0xcc
