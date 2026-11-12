; Distance-based detail, native r_use_texture_streaming owns the setting.
; Its render-options command already handles Apply, Cancel, saving and reload.
; A native integer option participates in the stock graphics preset system.
BITS 32
%include "engine_026g.inc"
global detail_options, detail_label, detail_model, detail_shader, detail_texture, detail_grass
global detail_option_vtable, detail_initialize, detail_apply
global detail_grass_model, detail_grass_distance
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
    push 2
    push detail_values
    push 0xa2844c                  ; existing r_use_texture_streaming command
    push edi
    push 19
    push ebx
    call engine_options_int
    add esp, 24
    mov dword [edi], detail_option_vtable + 4
    mov eax, edi
.store:
    mov edx, [ebx]
    mov [edx+19*4], eax
    mov eax, ebx
    pop edi
    pop esi
    pop ebx
    ret 8

; Native integer lifecycle; only storage conversion differs. Normal (0)
; maps to streaming on, Maximum (1) to off. No conversion in ActionScript.
detail_initialize:
    mov eax, [ecx+8]
    test eax, eax
    jz .normal
    mov eax, [eax+0x40]
    cmp byte [eax], 0
    sete al
    jmp .store
.normal:
    xor eax, eax
.store:
    mov [ecx+0x1d], al
    mov [ecx+0x1e], al
    ret

detail_apply:
    push esi
    push edi
    movzx edx, byte [ecx+0x1d]
    mov [ecx+0x1e], dl
    xor edx, 1
    ; A temporary native bool receiver uses the existing command's Apply,
    ; including renderer reload tracking. It cannot escape this call.
    sub esp, 0x1c
    mov esi, ecx
    mov edi, esp
    mov ecx, 7
    rep movsd
    mov [esp+0x18], edx
    push esp
    call engine_options_bool_apply
    add esp, 0x20
    pop edi
    pop esi
    ret

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

; The draw path selects its own model LOD; the culling patch's cached LOD
; does not control that selection. Preserve missing LOD0 and native refcount
; cleanup by using the selector's existing LOD0 return.
detail_grass_model:
    push ecx
    mov ecx, [0x2260498]
    cmp byte [ecx+0x132], 0
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
    cmp byte [ecx+0x132], 0
    pop ecx
    je .full
    popfd
    ja 0x16064d
    jmp 0x1605d0
.full:
    popfd
    jmp 0x1605d0

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
align 4
; RTTI locator followed by the six standard options_item_int slots.
detail_option_vtable:
    dd 0xa4c770
    dd 0x261310, detail_initialize, 0x261150, 0x261260, detail_apply, 0x261300
detail_values: dd detail_normal_key, detail_maximum_key
detail_preset_normal: dd 19, 0
detail_preset_maximum: dd 19, 1
detail_normal_key: db "st_distance_detail_normal", 0
detail_maximum_key: db "st_distance_detail_maximum", 0
detail_allocation: db "distance detail option", 0
detail_label_key: db "st_distance_detail_option", 0
