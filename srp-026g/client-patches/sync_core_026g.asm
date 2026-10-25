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
bits 32
section .text

global sc_render_move
extern sync_render_move

sc_render_move:
    push eax                    ; the target time
    push dword [esp + 8]        ; the world core
    call sync_render_move
    add esp, 8
    ret 4
