; Patch 83 (build 2777): a DirectInput step that fails at the start stops the
; game with a message instead of a crash. The arena's block
; input_devices.code with input_devices_026g.c (link_026g.py links them;
; patch_026g.py's INPUT_DEVICES_HOOKS call the entries by their names).
; client_patch.md section 3.59.
;
; The input world's create_devices (0x3B82F0) calls DirectInput8Create and
; then the keyboard's (0x3B8BC0) and the mouse's (0x3B94F0) constructors,
; which call IDirectInput8::CreateDevice. The game (MASTER_GOLD: the 0.10b
; source's R_ASSERTs compiled out) checks none of the three results and
; reads through the interface or the device: a NULL is an access violation.
; Each entry hands the step's raw result to input_devices_check, which
; returns when the step succeeded and otherwise shows the message and ends
; the process. Then the entry does what the site's own bytes did.
;
; - input_dinput_created, create_devices' `cmp byte [esp+1Ch],0` (0x3B8319,
;   right after DirectInput8Create; eax its HRESULT, ebx &world+1Ch (the
;   interface it wrote), ebp the window). Only on the DirectInput path (the
;   byte, create_devices' second argument, 0: `je 0x3B83B3`); the other path
;   makes no DirectInput device. Ends with the same compare, for the je.
; - input_keyboard_created, the keyboard's `mov eax,[edi] / mov ecx,[eax] /
;   mov edx,[ecx+2Ch]` (0x3B8C0A, right after CreateDevice; eax its
;   HRESULT, edi &keyboard+914h (the device), ebx the window).
; - input_mouse_created, the mouse's same three (0x3B9522; eax the HRESULT,
;   edi &mouse+28h, ebp the window).
;
; input_devices_check is cdecl: it keeps ebx, esi, edi and ebp. eax, ecx
; and edx are dead at the DirectInput site (both ways load them before a
; read) and loaded again by the keyboard's and mouse's three moves.
BITS 32
global input_dinput_created, input_keyboard_created, input_mouse_created
extern input_devices_check
section .text

%define STEP_DIRECT_INPUT 0        ; input_devices_026g.c's enum input_step
%define STEP_KEYBOARD 1
%define STEP_MOUSE 2

input_dinput_created:
    cmp byte [esp+0x20], 0         ; the site's [esp+1Ch], past our return address
    jne .other                     ; not the DirectInput path
    push ebp                       ; the window
    push dword [ebx]               ; the interface
    push eax                       ; the HRESULT
    push STEP_DIRECT_INPUT
    call input_devices_check
    add esp, 16
.other:
    cmp byte [esp+0x20], 0
    ret

input_keyboard_created:
    push ebx                       ; the window
    push dword [edi]               ; the device
    push eax                       ; the HRESULT
    push STEP_KEYBOARD
    call input_devices_check
    add esp, 16
    mov eax, [edi]
    mov ecx, [eax]
    mov edx, [ecx+0x2c]            ; SetDataFormat
    ret

input_mouse_created:
    push ebp                       ; the window
    push dword [edi]               ; the device
    push eax                       ; the HRESULT
    push STEP_MOUSE
    call input_devices_check
    add esp, 16
    mov eax, [edi]
    mov ecx, [eax]
    mov edx, [ecx+0x2c]            ; SetDataFormat
    ret
