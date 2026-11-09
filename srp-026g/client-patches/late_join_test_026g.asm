; Test sessions only (late_join_026g.py --test-session): such an exe passes
; the login by and connects straight to its -server=host:port as the
; session TEST_SESSION, for the match server's late_join_native example.
; Assembled at CAVE, the room in `.srp` the late join's code had before its
; Rust port (patch_026g.py's LATE_JOIN_VA, int3 in every other build).
bits 32
%ifndef CAVE
%error CAVE is required
%endif
%ifndef TEST_SESSION
%error TEST_SESSION is required
%endif
org CAVE

; 0x1EC0E1's call: EDI = the game, EAX = the network client; stack (host,
; port, spectator), ret 12. The game's own connect (0x1ED6E0), then for a
; player the match connection as TEST_SESSION (0x21BEB0).
direct_match:
    push ebp
    mov ebp, esp
    pushad
    push dword [ebp+16]
    push dword [ebp+12]
    push dword [ebp+8]
    call 0x1ed6e0
    cmp dword [ebp+16], 0
    jne .done
    mov ecx, [edi+0x53ac]
    test ecx, ecx
    jz .done
    mov dword [ecx+0x1ee7ec], TEST_SESSION
    push TEST_SESSION
    push 1
    push dword [ebp+12]
    push dword [ebp+8]
    call 0x21beb0
.done:
    popad
    leave
    ret 12
