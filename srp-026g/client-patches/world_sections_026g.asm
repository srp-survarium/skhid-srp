BITS 32
%include "engine_026g.inc"
section .text
global world_sections_construct

; The world's six sections in a history item grow on the heap
; (../docs/notes/client_patch.md section 3.62).
;
; game_state_history_item::construct (0x324510, ESI the item, EBX 0) gives
; each section an inline buffer and a mutable_buffer {+0 allocator 0,
; +4 data (the inline bytes), +8 capacity, +0xC size 0} after it: past the
; capacity, mutable_buffer::reallocate (0x7AB70) asserts ("mutable buffer
; overflow") with no allocator. Two shotguns' pellets in flight fill the
; bullets' 0x2400 bytes. Here, at the end of the constructor, each of the
; six gets the global multithreaded allocator (0xA8001C, the one the item's
; own lists use) and a block of the same capacity from it, through
; reallocate itself; its data, its capacity and its size are then the game's
; to grow as for any buffer with an allocator (resize doubles), and the
; destructor (0x324910) frees each block with that allocator. The inline
; bytes stay unused. What the sections hold, and so every world, is the
; same.

MT_ALLOCATOR    equ 0xA8001C        ; the global multithreaded allocator (vtable 0x9E6000)
RETURN          equ 0x3246C6        ; construct's `mov eax,esi / pop ebx / ret`

; At 0x3246C0, in place of `mov [esi+0C0BC8h],ebx`. ESI the item, EBX 0.
; EAX, ECX and EDX are free here (construct sets EAX next); the rest kept.
world_sections_construct:
    mov     [esi + 0xC0BC8], ebx    ; the instruction the hook took
    push    edi
    mov     edi, sections
.next:
    mov     eax, [edi]
    test    eax, eax
    jz      .done
    add     eax, esi                ; the section's mutable_buffer
    mov     dword [eax], MT_ALLOCATOR
    mov     dword [eax + 4], 0      ; no block yet: reallocate allocates one
    mov     dword [eax + 8], 0
    push    dword [edi + 4]         ; the capacity the inline buffer had
    push    eax
    call    engine_buffer_reallocate
    add     esp, 8
    add     edi, 8
    jmp     .next
.done:
    pop     edi
    jmp     RETURN

section .rodata
; {the mutable_buffer's offset in the item, its inline capacity}, in the
; body's order; the constructor's stores of each (0x324523..0x324686).
sections:
    dd 0xB6A90, 0x2400              ; bullets (0xB4690 + 2400h; 0x324537)
    dd 0xB76B8, 0x0C00              ; statistics (0xB6AB8 + C00h; 0x32457E)
    dd 0xBDF30, 0x2000              ; game objects (0xBBF30 + 2000h; 0x3246D6)
    dd 0xBBF08, 0x4000              ; physics (0xB7F08 + 4000h; 0x32460B)
    dd 0xB7EE0, 0x0800              ; rules (0xB76E0 + 800h; 0x3245C5)
    dd 0xC0B80, 0x0C00              ; effects (0xBFF80 + C00h; 0x324657)
    dd 0
