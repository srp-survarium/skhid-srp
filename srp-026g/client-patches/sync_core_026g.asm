; Patch 74: the sync core's render move (docs/notes/sync_core.md section 9;
; README patch 74). The arena's block sync_core.code, with the Rust crate
; sync_core_arena (sync_render_move).
;
; game_world::tick (0x1F77E0) moves the world to the game clock with
; `push eax(core) / mov eax,esi(time) / call 0x2D2E40` at 0x1F7C37;
; game_world_core::move takes the time in EAX and the core on the stack and
; pops 4 (`ret 4`), keeping EBX, ESI, EDI and EBP. sc_render_move is that
; call's target: the same shape, the decision and the stock move in
; sync_render_move (cdecl, which keeps the same registers).
;
; The game's frame here is aligned to 4 bytes only; Rust's i686 code assumes
; 16 at a call (the C patches build with -mincoming-stack-boundary=2 for the
; same reason). So the frame is aligned before the call: 16 at the call,
; esp+4 aligned at sync_render_move's entry.
bits 32
section .text

global sc_render_move
extern sync_render_move

sc_render_move:
    push ebp
    mov ebp, esp
    and esp, -16
    sub esp, 8
    push eax                    ; the target time
    push dword [ebp + 8]        ; the world core
    call sync_render_move
    leave
    ret 4
