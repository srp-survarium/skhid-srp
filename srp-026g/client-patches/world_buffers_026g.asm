; Patch 60 (build 2777): a saved world's game objects take 0x4000 bytes, as in
; 0.28, not 0x2000. The arena's block world_buffers.code (link_026g.py links
; it; patch_026g.py's WORLD_BUFFERS_HOOKS call the two entries by their
; names). client_patch.md section 3.54.
;
; Every world the game keeps (game_state_history_item, 0xC0BD0 bytes: the
; rollback history's twenty, the 0.26g debug option's one on the stack at
; 0x2D2241) holds at +0BBF30h its game objects' saved state: two streams, each
; 0x2000 bytes in place, then its mutable_buffer {allocator, data, capacity,
; size} and its buffer_writer:
;
;   +0000h  bytes of the first stream (the hashed and sent one)   0x2000
;   +2000h  mutable_buffer {0, +0000h, 2000h, size}
;   +2010h  buffer_writer (+2024h: the mutable_buffer)
;   +2028h  bytes of the second stream (kept, never sent)          0x2000
;   +4028h  mutable_buffer {0, +2028h, 2000h, size}
;   +4038h  buffer_writer (+404Ch: the mutable_buffer)
;
; made by 0x3246D0 (eax = item+0BBF30h; called once, at 0x32463A in the
; item's constructor 0x324510) and taken down by 0x324750 (edi = the same;
; called once, at 0x3249BA in the destructor 0x324910). The allocator is 0:
; a stream that would grow past its capacity asserts (mutable_buffer's
; resize 0x7AB30 -> 0x7AB70), and a world whose objects save more than 0x2000
; bytes stops every game. 0.28 has room for 0x4000 in place; a 0.26g world
; cannot grow (each field after +0BBF30h moves, and 0xC0BD0 is in the code),
; so the streams' bytes move out of it instead:
;
; - construct (at 0x32463A, for `call 0x3246D0`): the game's own 0x3246D0,
;   then a block of 0x4000 bytes from the game's malloc (0xF7650, the
;   allocator at [1F1D3F4h] behind a lock: the worlds are made on the logic
;   thread) for each stream: data = the block, capacity = 0x4000. The
;   allocator stays 0 (no growth past 0x4000, as before past 0x2000). A
;   malloc that fails leaves that stream as the game made it (0x2000 in
;   place).
; - destroy (at 0x3249BA, for `call 0x324750`): the game's own 0x324750 (it
;   frees through the allocator only, which is 0), then each stream whose
;   data is not its own bytes in place is given back (free 0xF76B0) and set
;   to them again ({+0000h, 2000h} and {+2028h, 2000h}).
;
; Nothing else reads the streams' bytes in place: every reader and writer
; goes through the mutable_buffer's data (the world's serialize 0x2D2570 by
; the writers +2010h and +4038h; its deserialize 0x2D0B70 by data and size,
; 0x2D2B6E..0x2D2B98; the item's serialize 0x324210, deserialize 0x324F35..
; 0x324FA0, clear 0x324290 and the late join's shift of the saved objects).
; A world is never copied: each is made by 0x324510 and taken down by
; 0x324910 (the history's pool 0x2D3F90 / 0x2D0739 / 0x2D4000, the rollback's
; remake 0x3252A7, the stack's one 0x2D2446 / 0x2D254D), so each has its
; own two blocks.
BITS 32
%include "engine_026g.inc"
global world_buffers_construct, world_buffers_destroy
section .text

%define STREAMS_CONSTRUCT 0x3246D0  ; eax = item+0BBF30h: the two streams in place; `ret`
%define STREAMS_DESTROY 0x324750    ; edi = item+0BBF30h: the writers' lists, an allocator's data; `ret`
%define FIRST 0x0000                ; the first stream's bytes in place
%define SECOND 0x2028               ; the second's
%define BUFFER 0x2000               ; a stream's mutable_buffer, after its bytes
%define DATA 4                      ; mutable_buffer +4: data, +8: capacity
%define CAPACITY 8
%define IN_PLACE 0x2000             ; the room each stream has in place
%define ROOM 0x4000                 ; 0.28's

; At 0x32463A, for `call 0x3246D0`: eax = item+0BBF30h. Every register but
; ecx and edx is kept (eax too, as 0x3246D0 keeps it).
world_buffers_construct:
    call STREAMS_CONSTRUCT
    push eax
    push ebx
    lea ebx, [eax + FIRST]
    call give_room
    lea ebx, [eax + SECOND]
    call give_room
    pop ebx
    pop eax
    ret

; ebx = a stream: its bytes from malloc, ROOM of them. Keeps eax, ebx.
give_room:
    push eax
    push ROOM
    call engine_crt_malloc
    add esp, 4
    test eax, eax
    jz .kept                        ; the stream stays as the game made it
    mov [ebx + BUFFER + DATA], eax
    mov dword [ebx + BUFFER + CAPACITY], ROOM
.kept:
    pop eax
    ret

; At 0x3249BA, for `call 0x324750`: edi = item+0BBF30h. Every register is
; as 0x324750 leaves it.
world_buffers_destroy:
    call STREAMS_DESTROY
    pushad
    lea ebx, [edi + FIRST]
    call take_room
    lea ebx, [edi + SECOND]
    call take_room
    popad
    ret

; ebx = a stream: data that is not its bytes in place goes back to the
; allocator, and the stream is its bytes in place again.
take_room:
    mov eax, [ebx + BUFFER + DATA]
    cmp eax, ebx
    je .done
    test eax, eax
    jz .done
    push eax
    call engine_crt_free
    add esp, 4
    mov [ebx + BUFFER + DATA], ebx
    mov dword [ebx + BUFFER + CAPACITY], IN_PLACE
.done:
    ret
