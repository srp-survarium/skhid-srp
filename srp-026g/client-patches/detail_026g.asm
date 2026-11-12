; Distance-based detail, native r_use_texture_streaming owns the setting.
; Its render-options command already handles Apply, Cancel, saving and reload.
; The renderer's active options, never the menu's pending checkbox, decide LOD.
BITS 32
%include "engine_026g.inc"
global detail_options, detail_label, detail_model, detail_shader, detail_texture, detail_grass
section .text

; Both graphics-tab constructor exits, after all original items were stored.
; EBX owns the tab; its array and count have been extended to 20 items.
detail_options:
    push detail_allocation
    push 0x1c
    push dword [0x1f1d9c4]
    call engine_malloc
    add esp, 12
    test eax, eax
    jz .store
    mov edi, eax
    push 0                         ; native boolean option
    push 19                        ; appended item, existing ids unchanged
    push ebx
    push 0xa2844c                  ; r_use_texture_streaming
    push edi
    call engine_options_item
    add esp, 20
    mov dword [edi], 0xa06744      ; native options_item_bool vtable
    mov eax, edi
.store:
    mov edx, [ebx]
    mov [edx+19*4], eax
    mov eax, ebx
    pop edi
    pop esi
    pop ebx
    ret 8

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
    cmp byte [edx+0x132], 0
    pop edx
    je 0x16eefd
    cmp byte [ecx+1], 0
    jne 0x16ef08
    jmp 0x16eefd

; get_shader_lod_index: ECX already holds the renderer's active options.
detail_shader:
    cmp byte [ecx+0x132], 0
    je .full
    mov eax, [ecx+0xe8]
    jmp 0x158d2c
.full:
    xor eax, eax
    ret 4

; process_culling already set the grass patch's LOD to zero. Keep native
; culling and the available-LOD clamp; skip only distance-based selection.
detail_grass:
    cmp byte [ecx+0x132], 0
    je 0x160615
    movss xmm2, [ecx+0x50]
    jmp 0x1605e5

; The native non-streamed texture request asks for the complete mip chain
; but normally drops the top levels according to texture quality. Full
; detail keeps them. Explicit non-streamed assets in adaptive mode are intact.
detail_texture:
    pushfd
    mov edx, [0x2260498]
    cmp byte [edx+0x132], 0
    je .full
    mov edx, [ecx+0x88194]
    jmp .done
.full:
    xor edx, edx
.done:
    popfd
    jmp 0x3d9c49

section .rodata
detail_allocation: db "distance detail option", 0
detail_label_key: db "st_distance_detail_option", 0
