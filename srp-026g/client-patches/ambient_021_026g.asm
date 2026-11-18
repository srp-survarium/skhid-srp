BITS 32
section .text
extern ambient_021_insert
extern particle_021_original_intensity
global ambient_021_insert_hook
global particle_021_intensity_hook
; object_ambient_light::insert, after gamma conversion and intensity clamp.
; EBX object; original ESP+6Ch is linear RGB, +78h intensity. Preserve the
; machine, modify only the outgoing render parameters, replay the six bytes.
ambient_021_insert_hook:
    pushfd
    pushad
    mov ebp, esp
    and esp, -16
    sub esp, 512
    fxsave [esp]
    lea eax, [ebp+36+0x6c]
    push eax
    push ebx
    call ambient_021_insert
    add esp, 8
    fxrstor [esp]
    mov esp, ebp
    popad
    popfd
    mov al, [ebx+0x16d]
    jmp 0x290a45

; particle_engine::update_light: load the authored intensity. Skip the
; native x3 only for the actual render scene of an opted-in day-map world.
; Keep native MULSS itself for every other scene, including its FP behavior.
particle_021_intensity_hook:
    movss xmm0, [ebp+0x64]
    pushfd
    pushad
    mov ebp, esp
    and esp, -16
    sub esp, 512
    fxsave [esp]
    push dword [0x1f1d310] ; engine_world, native message_processor 0x18359
    push ebx
    call particle_021_original_intensity
    add esp, 8
    test eax, eax
    jz .native
    fxrstor [esp]
    mov esp, ebp
    popad
    popfd
    jmp 0x14f312
.native:
    fxrstor [esp]
    mov esp, ebp
    popad
    popfd
    mulss xmm0, [0xa2d73c]
    jmp 0x14f312
