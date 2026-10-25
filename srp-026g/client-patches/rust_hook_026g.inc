; The hook ABI: how the game enters a Rust patch (docs/notes/rust_patches.md;
; the Rust side: patch-core's hook module). `%include "rust_hook_026g.inc"`
; in a patch's NASM entry file (link_026g.py assembles with -I of this
; folder).
;
; The game's frames are aligned to 4 only, and rustc's i686 code assumes 16
; at a call (it keeps 16-byte locals with movaps): every way from the game
; into Rust is one of these macros, which align the stack first.
;
; RUST_HOOK target
;   In the middle of the game's code (a replaced call, a jump to us): saves
;   the flags and every register, calls target(&regs) (cdecl; Regs: EDI, ESI,
;   EBP, ESP, EBX, EDX, ECX, EAX, EFLAGS, as pushad and pushfd leave them)
;   with the stack aligned to 16 and DF clear, and puts the registers and
;   the flags back from what target left in Regs (ESP is not taken from
;   it). The entry goes on after it: the replaced instructions, a jmp back.
;
; RUST_THISCALL name, target, n
;   `name`: a function the game calls with ECX and n dwords on the stack,
;   which it pops (`ret 4 x n`): a vtable slot, a delegate. Calls
;   target(ecx, the n dwords) (cdecl); EAX (EDX) target's.
;
; RUST_CDECL name, target, n
;   `name`: a function the game calls cdecl with n dwords on the stack.
;   Calls target(the n dwords); EAX (EDX) target's.
;
; EBX, ESI, EDI and EBP are kept by all three (Rust's extern "C" keeps them).

%ifndef RUST_HOOK_026G_INC
%define RUST_HOOK_026G_INC

%macro RUST_HOOK 1
extern %1
    pushfd
    pushad
    cld
    mov ebp, esp                    ; the Regs
    and esp, -16
    sub esp, 12
    push ebp
    call %1
    mov esp, ebp
    popad
    popfd
%endmacro

; the n dwords of the caller's frame (above the return address), pushed
; last first, after the padding that leaves the call aligned to 16; ebp the
; frame (`push ebp / mov ebp, esp` done); %2 the dwords pushed after them
%macro RUST_PUSH_ARGUMENTS 2
    and esp, -16
%assign rust_pad (-4 * (%1 + %2)) & 15
%if rust_pad
    sub esp, rust_pad
%endif
%assign rust_i %1
%rep %1
    push dword [ebp + 4 + 4 * rust_i]
%assign rust_i rust_i - 1
%endrep
%endmacro

%macro RUST_THISCALL 3
global %1
extern %2
%1:
    push ebp
    mov ebp, esp
    RUST_PUSH_ARGUMENTS %3, 1
    push ecx
    call %2
    leave
%if %3
    ret 4 * %3
%else
    ret
%endif
%endmacro

%macro RUST_CDECL 3
global %1
extern %2
%1:
    push ebp
    mov ebp, esp
    RUST_PUSH_ARGUMENTS %3, 0
    call %2
    leave
    ret
%endmacro

%endif
