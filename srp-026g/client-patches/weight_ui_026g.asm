BITS 32
section .text
extern weight_ui_load, weight_ui_invoke
global weight_ui_config, weight_ui_display
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
; 0x22110F: player config resource in ESI; lobby in EBX.
weight_ui_config:
    SAVE
    mov eax, [esi+0x108]
    push eax
    push ebx
    call weight_ui_load
    add esp, 8
    RESTORE
    mov eax, [esi+0x108]
    jmp 0x221115
; 0x2222EB: replaces GFx::Movie::Invoke, ECX movie, ESI lobby.
; Original stack: return, name, result (null), args, count (2).
weight_ui_display:
    SAVE
    push dword [ebp+48]
    push dword [ebp+24]
    push dword [ebp+4]
    call weight_ui_invoke
    add esp, 12
    mov [ebp+28], eax
    RESTORE
    ret 16
