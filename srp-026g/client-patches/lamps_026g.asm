; Breakable lamps, build 2777. Every entry preserves the whole machine;
; the replayed instructions and continuation VAs are of the original exe.
BITS 32
section .text
extern bl_bind, bl_detach, bl_present, bl_ray, bl_serialize, bl_deserialize
extern bl_visual_insert, bl_visual_remove, bl_light_insert, bl_light_remove
global lamps_bind, lamps_remove, lamps_destroy, lamps_present, lamps_ray
global lamps_save, lamps_load, bl_visual_vtable, bl_light_vtable

%macro SAVE 0
    pushfd
    pushad
    mov ebp, esp
    and esp, -16
    sub esp, 512
    fxsave [esp]
%endmacro
%macro RESTORE 0
    fxrstor [esp]
    mov esp, ebp
    popad
    popfd
%endmacro

lamps_bind:                         ; insert(physics, core), ecx project
    SAVE
    mov eax, [ebp+40]               ; original [esp+4]: physics world
    push eax
    push dword [ebp+24]             ; original ecx: project
    call bl_bind
    add esp, 8
    RESTORE
    push ebp
    mov ebp, esp
    and esp, -8
    jmp 0x285756

lamps_remove:                       ; remove_static's epilogue, esi project
    SAVE
    push dword [ebp+4]
    call bl_detach
    add esp, 4
    RESTORE
    pop edi
    pop ebx
    ret

lamps_destroy:                      ; destructor fallback, before any member dies
    SAVE
    push dword [ebp+24]
    call bl_detach
    add esp, 4
    RESTORE
    push esi
    mov esi, ecx
    cmp dword [esi+0x1e8], 0
    jmp 0x28533a

lamps_present:                      ; after move and ragdolls; replay the conditional
    SAVE
    call bl_present
    RESTORE
    je .skip
    fld dword [esp+0x8c]
    jmp 0x1f7c4a
.skip:
    jmp 0x1f7c6c

lamps_ray:                          ; after ragdoll's disjoint ray hook
    SAVE
    lea eax, [ebp+36]
    push eax                       ; original check_collision frame
    push dword [ebp+28]             ; native ray_result
    push dword [ebp]                ; bullet (edi)
    call bl_ray
    add esp, 12
    RESTORE
    movq xmm0, [eax+8]
    movq [esp+0x40], xmm0
    jmp 0x32d63c

lamps_save:                         ; bullet_manager::serialize epilogue
    SAVE
    mov eax, [ebp+8]                ; native ebp
    push dword [eax+8]              ; writer
    push dword [ebp]                ; manager (edi)
    call bl_serialize
    add esp, 8
    RESTORE
    pop esi
    pop ebx
    mov esp, ebp
    pop ebp
    jmp 0x321c40

lamps_load:                         ; bullet_manager::deserialize epilogue
    SAVE
    push dword [ebp+16]             ; reader (ebx)
    push dword [ebp]                ; manager (edi)
    call bl_deserialize
    add esp, 8
    RESTORE
    pop edi
    pop esi
    pop ebp
    pop ebx
    ret 4

align 4
bl_visual_vtable:
    dd 0xa47c2c                    ; the original's complete-object locator
    dd bl_visual_insert, bl_visual_remove
bl_light_vtable:
    dd 0xa4db04
    dd 0x2916f0, 0xf1900, 0xf1f00, 0xf1f40
    dd 0xfda90, 0xfda90, 0xfda90, 0x291720
    dd bl_light_insert, bl_light_remove
