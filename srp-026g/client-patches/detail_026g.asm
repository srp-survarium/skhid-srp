; Distance detail: bounded native r_distance_detail command (Normal/High/Maximum).
; The stock integer item handles Apply, Cancel, saving and preset membership.
; A native integer option participates in the stock graphics preset system.
BITS 32
%include "engine_026g.inc"
global detail_options, detail_label, detail_model, detail_shader, detail_texture, detail_grass
global detail_command_vtable, detail_register, detail_execute, detail_state, detail_previous
global detail_model_screen, detail_model_distance, detail_grass_screen, detail_grass_metric, detail_shader_threshold
global detail_grass_model, detail_grass_distance, detail_commit
global detail_preset_compare, detail_preset_address, detail_default_next
section .text

; Both graphics-tab constructor exits, after all original items were stored.
; EBX owns the tab; its array and count have been extended to 20 items.
detail_options:
    push detail_allocation
    push 0x20
    push dword [0x1f1d9c4]
    call engine_malloc
    add esp, 12
    test eax, eax
    jz .store
    mov edi, eax
    push 3
    push detail_values
    push detail_command_name
    push edi
    push 19
    push ebx
    call engine_options_int
    add esp, 24
    mov eax, edi
.store:
    mov edx, [ebx]
    mov [edx+19*4], eax
    mov eax, ebx
    pop edi
    pop esi
    pop ebx
    ret 8

 ; Replace the old streaming-bool registration with a native render_cc_u32.
; The original static-init guard still owns registration. One command owns
; detail and derives the renderer's streaming byte on every execution,
; including user.cfg and console execution, not just menu Apply.
detail_register:
    pushad
    mov eax, [0x2260498]
    mov ecx, [eax]
    mov [detail_command+4], ecx
    mov dword [eax], detail_command
    push 2                        ; user.cfg command category
    push 1                        ; serializable
    push 2                        ; maximum enum value
    push 0                        ; minimum enum value
    push detail_state
    push detail_command_name
    push detail_command+0x10
    call engine_cc_u32
    add esp, 28
    mov dword [detail_command+0x10], detail_command_vtable+4
    mov eax, [0x2260498]
    mov byte [eax+0x132], 1        ; default Normal uses streaming
    popad
    mov esi, 1                    ; original registration's outgoing value
    jmp 0x3ce2f7

detail_execute:
    push dword [esp+4]
    push ecx
    call engine_render_u32_execute
    add esp, 8
    mov eax, [0x2260498]
    cmp dword [detail_state], 2
    setne byte [eax+0x132]
    ret 4

 ; end_render_options_changing has consumed the changed-command list.
; Join its previous=current checkpoint; preserve the pending save-flag ZF.
detail_commit:
    push eax
    mov eax, [detail_state]
    mov [detail_previous], eax
    pop eax
    mov ecx, 0x4b
    jmp 0x3cf3da

; The game's first ten entries are populated from its graphics preset data.
; Keep those exact entries and append detail to the same native loops.
detail_preset_compare:
    cmp cl, 10
    je .detail
    movzx eax, cl
    add eax, edi
    lea eax, [eax*8+0x1f1b188]
    jmp .compare
.detail:
    mov eax, detail_preset_normal
    cmp edi, 30                    ; High row index 3 * original stride 10
    jne .ultra
    mov eax, detail_preset_high
    jmp .compare
.ultra:
    cmp edi, 40                    ; Ultra row index 4 * original stride 10
    jne .compare
    mov eax, detail_preset_maximum
.compare:
    cmp [eax], ebp
    jmp 0x261363

detail_preset_address:
    cmp ecx, 10
    je .detail
    lea eax, [eax+eax*4]
    lea edi, [ecx+eax*2]
    lea edi, [edi*8+0x1f1b188]
    jmp 0x2620e7
.detail:
    mov edi, detail_preset_normal
    cmp eax, 3
    jne .ultra
    mov edi, detail_preset_high
    jmp .done
.ultra:
    cmp eax, 4
    jne .done
    mov edi, detail_preset_maximum
.done:
    jmp 0x2620e7

detail_default_next:
    add ebp, 8
    sub dword [esp+0x10], 1
    pushfd
    cmp dword [esp+0x14], 1
    jne .done
    mov ebp, detail_preset_maximum
.done:
    popfd
    jmp 0x1ff5be

; fill_settings_data: its twentieth label buffer is initialized by the
; enlarged native array constructor. Preserve the preceding comparison flags.
detail_label:
    pushfd
    pushad
    push detail_label_key
    lea eax, [esi+19*0x4c]
    push eax
    call engine_string_assign
    add esp, 8
    popad
    popfd
    lea eax, [esi+0x558]
    jmp 0x2009bf

; select_lod has already handled a missing LOD0 (AL=AA). Full detail
; follows its own LOD0 return. Otherwise the original LOD1 availability test.
detail_model:
    push edx
    mov edx, [0x2260498]
    cmp dword [detail_state], 2
    pop edx
    je 0x16eefd
    cmp byte [ecx+1], 0
    jne 0x16ef08
    jmp 0x16eefd

; get_shader_lod_index: ECX already holds the renderer's active options.
detail_shader:
    cmp dword [detail_state], 2
    je .full
    mov eax, [ecx+0xe8]
    jmp 0x158d2c
.full:
    xor eax, eax
    ret 4

; The draw path selects its own model LOD; the culling patch's cached LOD
; does not control that selection. Preserve missing LOD0 and native refcount
; cleanup by using the selector's existing LOD0 return.
detail_grass_model:
    push ecx
    mov ecx, [0x2260498]
    cmp dword [detail_state], 2
    pop ecx
    je 0x1cc521
    cmp byte [edi+1], 0
    jne 0x1cc54d
    jmp 0x1cc521

; After the frustum query, the stock code rejects patch centers over 100 m.
; Maximum keeps those candidates. Normal retains the original comparison.
; Occlusion and frustum culling, and shadow-pass limits, are untouched.
detail_grass_distance:
    pushfd
    push ecx
    mov ecx, [0x2260498]
    cmp dword [detail_state], 2
    pop ecx
    je .full
    popfd
    pushfd
    cmp dword [detail_state], 1
    jne .normal
    popfd
    comiss xmm0, [detail_grass_limit]
    ja 0x16064d
    jmp 0x1605d0
.normal:
    popfd
    ja 0x16064d
    jmp 0x1605d0
.full:
    popfd
    jmp 0x1605d0

; process_culling already set the grass patch's LOD to zero. Keep native
; culling and the available-LOD clamp; skip only distance-based selection.
detail_grass:
    cmp dword [detail_state], 2
    je 0x160615
    movss xmm2, [ecx+0x50]
    movss xmm1, [ecx+0x54]
    cmp dword [detail_state], 1
    jne .done
    mulss xmm2, [detail_high_scale]
    mulss xmm1, [detail_high_scale]
.done:
    jmp 0x1605ea

; The native non-streamed texture request asks for the complete mip chain
; but normally drops the top levels according to texture quality. Full
; detail keeps them. Explicit non-streamed assets in adaptive mode are intact.
detail_texture:
    pushfd
    mov edx, [0x2260498]
    cmp dword [detail_state], 2
    je .full
    mov edx, [ecx+0x88194]
    jmp .done
.full:
    xor edx, edx
.done:
    popfd
    jmp 0x3d9c49

 ; Scale only local selection metrics, never shared model descriptors.
detail_model_screen:
    cmp dword [detail_state], 1
    jne .done
    mulss xmm0, [detail_high_scale]
.done:
    movss xmm1, [eax+8]
    jmp 0x16f2b1

detail_model_distance:
    movss xmm0, [esp+0x10]
    cmp dword [detail_state], 1
    jne .done
    mulss xmm0, [detail_high_inverse]
.done:
    jmp 0x16f2f1

detail_grass_screen:
    cmp dword [detail_state], 1
    jne .done
    mulss xmm0, [detail_high_scale]
.done:
    movss xmm1, [ebx+8]
    jmp 0x1cc814

detail_grass_metric:
    cmp dword [detail_state], 1
    jne .done
    fmul dword [detail_high_inverse]
.done:
    fst dword [esp+0x10]
    fld dword [ebx]
    jmp 0x1cc922

detail_shader_threshold:
    mov eax, [esp+4]
    cmp dword [detail_state], 1
    jne .done
    mulss xmm0, [detail_high_inverse]
.done:
    comiss xmm0, [eax+0x58]
    jmp 0x158d66

section .rodata
align 4
; Native render_cc_u32 command methods; execute synchronizes the derived byte.
detail_command_vtable:
    dd 0xa589c4
    dd 0x3cf4a0, detail_execute, 0xece00, 0xece20, 0xece40, 0x75af0, 0xec6e0
detail_values: dd detail_normal_key, detail_high_key, detail_maximum_key
detail_preset_normal: dd 19, 0
detail_preset_high: dd 19, 1
detail_preset_maximum: dd 19, 2
detail_high_scale: dd 1.5
detail_high_inverse: dd 0.6666666666666667
detail_grass_limit: dd 22500.0
detail_command_name: db "r_distance_detail", 0
detail_normal_key: db "st_distance_detail_normal", 0
detail_high_key: db "st_distance_detail_high", 0
detail_maximum_key: db "st_distance_detail_maximum", 0
detail_allocation: db "distance detail option", 0
detail_label_key: db "st_distance_detail_option", 0

section .data
align 4
; render_cc header + cc_u32 + previous-value pointer, same native layout.
detail_command:
    dd 0xa2864c, 0, 0, 4           ; no shader define; reload-textures change kind
    times 0x50 db 0
    dd detail_previous
detail_state: dd 0
detail_previous: dd 0
