; Patch 222: loose_oct_tree::unmove_all as one loop, the same bytes written
; (rust/patches/src/octree_unmove.toml's `code` hook).
;
; loose_oct_tree::unmove_all (0x396C0, slot +34h of the tree's vtable
; 0x9DE254; ECX the tree) clears the moved flag (+2Ch, a byte) of every
; object in the tree: renderer::render calls it on the scene's tree at the
; end of each frame (0x13DE6B), after the visibility stage read the flags
; (select_models' moved-only query, its predicate 0x150220). It goes through
; for_each_iterate (0x3A890): a recursion over every node (the root at tree
; +4, eight child pointers at node +0..1Ch) that works out each child's
; center and half size on the way (x87 and SSE, unused here) and calls the
; callback 0x396B0 (`mov byte [obj+2Ch], 0`) through a pointer for each
; object of a node (the list at node +24h, linked at object +1Ch), children
; first.
;
; The same nodes and objects here, by a loop with the nodes still to see on
; the stack (at most eight a level: less stack than the recursion's), each
; flag that is not 0 written 0 (one already 0 is left as it is: the same
; bytes, its cache line not dirtied). Only the order of the writes differs:
; each is a 0 into a different object. EBX, ESI, EDI, EBP kept; EAX, ECX,
; EDX and the flags changed, as a virtual thiscall may (the game's leaves ECX 0x396B0, EAX and EDX what the
; walk left); the x87 and XMM registers untouched (the game's leaves them as
; it found them, its x87 loads and stores in pairs).
; check_octree_unmove_026g.py runs both on the same trees.
bits 32
section .text

%macro push_child 1             ; node EAX's child %1, when there is one
    mov ecx, [eax + 4 * %1]
    test ecx, ecx
    jz %%none
    push ecx
%%none:
%endmacro

global octree_unmove_all
octree_unmove_all:              ; the jump from 0x396C0: ECX the tree
    mov eax, [ecx + 4]          ; the root
    test eax, eax
    jz .done
    push esi
    mov edx, esp                ; the stack's bottom: no node left
    push eax
.node:
    cmp esp, edx
    je .end
    pop eax
%assign child 0
%rep 8
    push_child child
%assign child child + 1
%endrep
    mov esi, [eax + 24h]        ; the node's objects
    test esi, esi
    jz .node
.object:
    cmp byte [esi + 2Ch], 0
    je .cleared
    mov byte [esi + 2Ch], 0
.cleared:
    mov esi, [esi + 1Ch]
    test esi, esi
    jnz .object
    jmp .node
.end:
    pop esi
.done:
    ret
