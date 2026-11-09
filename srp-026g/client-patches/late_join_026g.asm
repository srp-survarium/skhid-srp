; The late join's NASM that stayed (build 2777): the stubs where a typed hook
; does not fit, small and called where the game's code is in the middle of
; something: patch 11's on_match_ready jump, its histories' and capability
; byte's three instructions, patch 13's lists of players made again (a frame
; of the hud's function built and gone into) and its box of a model, patch
; 39's scoreboard rows, and patches 22 and 40's field of view (x87, every
; frame, every model). The late join's logic is Rust (rust/patches/src/
; late_join.rs; patches 14, 18, 19 and 20 too). The arena's block
; late_join.code (link_026g.py links it; patch_026g.py's LATE_JOIN_HOOKS
; jump or call to these entries by their names); before the Rust port it
; was the fixed first 4096 bytes of late_join_026g.section.bin.
; client_patch.md sections 3.11, 3.13, 3.22, 3.36 (patch 39) and 3.37 (40).
bits 32
global late_join_ready, late_join_history, late_join_capability
global late_join_rebuild_lists, late_join_lists, late_join_box
global late_join_tab_row, late_join_tab_shown
global late_join_fov_projection, late_join_fov_lod_scale, late_join_fov_screen_scale
extern late_join_on_ready       ; the Rust entry late_join.ready (rust_patches.md, `entry`)
section .text

; The state page `.srpd` (patch_026g.py's LATE_JOIN_STATE_VA, never moved):
; patch 39's dword (TAB_LIST_STATE_OFFSET), the places the scoreboard was
; last made with, and patch 40's k (FOV_LOD_STATE_OFFSET), a float, 0 until
; the first projection. patch_026g.py checks the two against these.
TAB_ROWS equ 0xA428E40
FOV_LOD_K equ 0xA428E50
; Patch 4's upload code's ready hook, at `.srp`'s start (SECTION_VA): where
; on_match_ready's jump went before the late join.
UPLOAD_READY equ 0xA425000

; on_match_ready's jump at 0x27AEA1, before the upload code's ready hook:
; EBX the network client. The Rust entry keeps every register.
late_join_ready:
    call late_join_on_ready
    jmp UPLOAD_READY

; 0x2D3906 in the world's constructor: both banks of twenty histories
; whatever the live roster (`add esi,edx` / `lea ecx,[ebp+0E3E4h]`).
late_join_history:
    mov esi, 20
    lea ecx, [ebp+0xe3e4]
    jmp 0x2d390e

; 0x27A23D, the 0xD8's debug byte (`mov [eax+edi],bl` / `mov eax,[esp+1Ch]`):
; the stock 00 or FF made 82, or 83 with -debug_hash_mismatch, the dialect
; the server tells the late join's games by (world receipts).
late_join_capability:
    and bl, 1
    or bl, 0x82
    mov [eax+edi], bl
    mov eax, [esp+0x1c]
    jmp 0x27a244

; The hud's lists of players: the scoreboard (root.list_set_players), the
; second list (root.set_players_list) and the enemies' markers
; (root.set_enemy_icon_ids). The game makes them once, in 0x234880 when the
; match is ready, for the players there are then (the loops end at the
; count, options+577B5h); nothing makes them again. stdcall(hud, options),
; ret 8: the late join's install calls it (vostok::late_join, a [[native]]
; of late_join.toml) and tab_shown below. rebuild_lists builds 0x234880's
; frame and goes into it where the lists are made (0x234E71); lists_hook
; takes it out again, through the function's own release of the two arrays
; and its own return, which comes back to late_join_rebuilt (the one return
; address lists_hook knows). Nothing else of the function runs: not its
; subscription, not root.reset, not the hud's items.
late_join_rebuild_lists:
    push dword [esp+8]
    push dword [esp+8]
    call rebuild_lists
late_join_rebuilt:
    ret 8

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

; 0x23572B in 0x234880, right after root.set_enemy_icon_ids, the last of
; the three lists: `mov ecx,[esp+98h]`. A run rebuild_lists began (its
; return address late_join_rebuilt) ends here.
late_join_lists:
    cmp dword [esp+0x5e4], late_join_rebuilt
    jne .start_of_match
    xor esi, esi                  ; as the function's end has it
    jmp 0x2363ca
.start_of_match:
    mov ecx, [esp+0x98]
    jmp 0x235732

; Patch 39 (../docs/notes/tab-list-left-places.md): the scoreboard has a row
; for a place that is in the match, and for no other. 0x23537A in
; 0x234880's first loop, where a place's row is put into the scoreboard's
; array (root.list_set_players): `lea eax,[esp+58h]` / `push eax` / `mov
; eax,[esp+138h]` / `push ebx` / `push eax` / `call edx`, the array's
; SetElement (its interface's vtable+34h) with the place (ebx) as the index.
; ecx is the array's interface ([esp+12Ch]), its data at [esp+134h], the
; row at [esp+58h], the hud at [esp+5E8h] (here 4 more each). The row goes
; in at the array's end (PushBack, vtable+3Ch, as the enemies' ids are put
; in at 0x234F2D) when the place is in the match: kd_stats_rule's `online`
; (on_player_entered_match 0x3239F0 sets it, on_player_left_match 0x323A00
; clears it, for the server's 0xE9 and 0xEA, in every game of the match at
; the same tick; a world brings it), which fill_match_hud_state 0x323980
; copies into the hud's state at +0C0h + 8 x place every tick. A list made:
; the mask kept in the state page is none (-1), so the next update of the
; shown list makes the lists again with the places of then (tab_shown),
; which keeps the mask after its own run.
late_join_tab_row:
    or dword [TAB_ROWS], -1
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

; Patch 39: 0x23B421 in game_world_ui::update_ui (0x23B200, edi = the hud),
; when the hud's state has changed, before the players' rows are refreshed
; while the list is shown: `cmp byte [edi+0BAh],0` (the `jz 23B477h` after
; the call stays and takes its flags from here). While the list is shown,
; the places in the match now (the hud's +0C0h + 8 x place, for the count
; the loop after this reads) are set against the mask the lists were last
; made with (TAB_ROWS); when they differ, the three lists are made again
; (late_join_rebuild_lists, as the late join's install has them made, with
; the options the hud's were made with, [hud+1Ch]) and the mask is kept. A
; list made by anything else leaves the mask at -1 (tab_row). The loop
; after this refreshes only the rows of places in the match
; (patch_026g.py's TAB_LIST_LOOP).
late_join_tab_shown:
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
    cmp [TAB_ROWS], edx
    je .same
    push edx
    push dword [edi+0x1c]
    push edi
    call late_join_rebuild_lists
    pop dword [TAB_ROWS]
.same:
    popad
    cmp byte [edi+0xba], 0
.hidden:
    ret

; Patch 13: 0x39E52 in loose_oct_tree::insert 0x39E30 (the scene's and the
; collision world's tree), right after the object gave its box: eax -> min
; xyz, max xyz, six floats; edi = the object, which keeps a copy at +4. A
; box with a number in it that is no number or not finite is outside the
; tree, and update_bounds 0x396F0 then doubles the tree's size without end
; (the size to reach works out as infinity): it takes a node each time
; until none is left and reads through a null one (0x399A8; seen live
; 2026-10-13 in a game that was given a newcomer, when a model in the scene
; was moved to such a place). Such a box is taken as a box of two metres at
; the origin; the next move of the object puts it where it is. Every
; insert of every model: kept in NASM.
late_join_box:
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
    mov eax, unit_box
    jmp .copy

; Patch 22: the camera's projection (0x242900, slot 0 of every camera's
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
; it passes, 65 z the original's at the same zoom, at FOV_LOD_K, for
; fov_lod_scale and fov_screen_scale below. k is exactly 1 at uro_fov 65 or
; less (A = 65 z + 0, the same two tangents) and through a full scope; 1.87
; unaimed at 100. The x87 stack is empty again before the call, as it was.
; Every camera, every frame, its exact bits pinned by check_fov_026g.py:
; kept in NASM (x87's fptan; the arena has no libm).
late_join_fov_projection:
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

; Patch 40: the high-FOV LOD compensation. The game measures a model by
; its size on the screen, through the projection: a static model's LOD
; (static_render_model_instance::select_lod 0x16EEC0, type 1, LOD 3 draws
; nothing) and every surface's screen factor (0x158D80: shader LOD, sun
; and spot shadow casters, rain occluders). At a wider angle a model is
; smaller on the screen, so it went down in LOD and out at a shorter
; distance than the original's 65 degrees. Both multiply by k
; (fov_projection above) after the original's own constant, so they see
; the model as the original at the same zoom would. Before the first
; projection the page is zero, and they keep the original's value. Every
; model, every frame: kept in NASM.
; select_lod's `mulss xmm0,[0x927A44]` (0.5, 0x16F2A4) calls here.
late_join_fov_lod_scale:
    mulss xmm0, [0x927A44]
    cmp dword [FOV_LOD_K], 0
    je .stock
    mulss xmm0, [FOV_LOD_K]
.stock:
    ret

; calculate_screen_factor's `fmul dword [0xA710D0]` (100, 0x1590AF) calls here.
late_join_fov_screen_scale:
    fmul dword [0xA710D0]
    cmp dword [FOV_LOD_K], 0
    je .stock
    fmul dword [FOV_LOD_K]
.stock:
    ret

section .rodata
align 4
unit_box:    dd -1.0, -1.0, -1.0, 1.0, 1.0, 1.0
fov_scope:   dd 0.25
fov_blend:   dd 1.3333334
fov_stock:   dd 65.0
fov_radians: dd 0.017453292
fov_half:    dd 0.008726646
