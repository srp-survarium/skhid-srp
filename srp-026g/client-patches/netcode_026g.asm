; Patch 79: the per-tick protocol's client (docs/notes/netcode_port_069.md
; phases 2 to 4; README patch 79). The arena's block netcode.code, with the
; Rust crate rust/netcode. Every entry into Rust is a RUST_HOOK
; (rust_hook_026g.inc: registers saved, stack aligned).
;
; Statically, whatever the server (a stock server never sends 0xF7, and the
; game sends 0xE2 only on a match of the per-tick protocol):
; - the tables know the two new ids: C->S 0xE2 (the client's update of a
;   tick: variable, plain, reliable, again every 50 ms) and S->C 0xF7 (the
;   server's: variable, unique so that a large one splits): the length
;   getter (netcode_length), the sent and received infos (netcode_send_info,
;   netcode_received_info), and the id ranges (bytes in patch_026g.py);
; - 0xD8's capability byte has bit 1, "speaks the per-tick protocol"
;   (netcode_connect).
; With a match of it (0xE4's trailer):
; - netcode_dispatch: 0xF7 to the Rust handler (the journal, -record_journal,
;   keeps the stock messages it makes, not the 0xF7), which hands the game's own
;   dispatch the stock messages of its changes; 0xE4 (the static info) to
;   the trailer's reader, which takes the trailer off before anything else
;   reads the message;
; - netcode_own_input, netcode_own_profile: the stock senders' own inputs
;   taken at their new_packet, the stock send skipped;
; - netcode_synchronize: after the stock synchronize, the updates due.
bits 32
section .text

%include "rust_hook_026g.inc"

global netcode_dispatch
global netcode_length
global netcode_send_info
global netcode_received_info
global netcode_connect
global netcode_own_input
global netcode_own_profile
global netcode_synchronize

; on_match_packet_received 0x27A300 at 0x27A322 (`add eax,0FFFFFF1Dh`,
; then `cmp eax,13h / ja / jmp [eax*4+27A738h]`): EAX the id (movzx), ESI
; the network client, EBX the reader.
netcode_dispatch:
    cmp eax, 0xF7
    je .update
    cmp eax, 0xE4
    je .static_info
.stock:
    add eax, 0xFFFFFF1D
    jmp 0x27A327
.update:
    RUST_HOOK netcode_update
    jmp 0x27A72C                    ; the dispatch's end, unrecorded (the reader consumed)
.static_info:
    RUST_HOOK netcode_static_info
    jmp .stock

; get_message_length 0x279670 at 0x279694 (`sub esi,0D8h`, then the jump
; table): DL the id. 0xE2 and 0xF7 are variable, at least a byte (0xF6's
; case, 0x2799C1).
netcode_length:
    cmp dl, 0xE2
    je .variable
    cmp dl, 0xF7
    je .variable
    sub esi, 0xD8
    jmp 0x27969A
.variable:
    jmp 0x2799C1

; get_sending_message_info 0x279A60 (ECX the id, EAX the info: u32
; lifetime, u16 resend, u8 flags): 0xE2 reliable, again every 50 ms, plain.
netcode_send_info:
    cmp ecx, 0xE2
    jne .stock
    mov dword [eax], 0xFFFFFFFF
    mov word [eax + 4], 50
    mov byte [eax + 6], 0x9F
    ret
.stock:
    add ecx, 0xFFFFFF28
    jmp 0x279A66

; get_received_message_info 0x279B10: 0xF7 unique (0x20), 500 ms, 50 ms.
netcode_received_info:
    cmp ecx, 0xF7
    jne .stock
    mov dword [eax], 500
    mov word [eax + 4], 50
    mov byte [eax + 6], 0x20
    ret
.stock:
    add ecx, 0xFFFFFF1D
    jmp 0x279B16

; send_connect 0x27A1B0's `call 0x379090` at 0x27A247: the packet in ESI,
; the capability byte at its buffer's data + EDI (written just before, by
; the late join's connect_capability).
netcode_connect:
    push ecx
    mov ecx, [esi + 0x2BC]
    mov ecx, [ecx + 4]
    or byte [ecx + edi], 0x02
    pop ecx
    jmp 0x379090

; The player-input sender 0x27E9F0 at 0x27EB21 (`mov eax,[edx+18h] / add
; ebx,0DCh`, then `push ebx / call eax`, new_packet). ZF from Rust: taken,
; the stock create, write and send skipped (on to 0x27EB4D).
netcode_own_input:
    RUST_HOOK netcode_own_input_hook
    jz .taken
    mov eax, [edx + 0x18]
    add ebx, 0xDC
    jmp 0x27EB2A
.taken:
    jmp 0x27EB4D

; The kit-choice sender 0x27EB90 at 0x27ECAC (`mov eax,[edx+18h] / push
; 0DEh`, then new_packet). Taken: on to 0x27ECD2.
netcode_own_profile:
    RUST_HOOK netcode_own_profile_hook
    jz .taken
    mov eax, [edx + 0x18]
    push 0xDE
    jmp 0x27ECB4
.taken:
    jmp 0x27ECD2

; send_match_messages 0x27CBF0's `call 0x27E6D0` (synchronize) at 0x27CD1F:
; [esp+4] the core, [esp+8] the time, [esp+0Ch] the sender, EAX the newest
; time less the horizon, EBX the network client; synchronize pops 0Ch and
; says in AL whether it sent. Then the updates due (Rust, AL set when one
; went).
netcode_synchronize:
    push dword [esp + 0x0C]
    push dword [esp + 0x0C]
    push dword [esp + 0x0C]
    call 0x27E6D0
    RUST_HOOK netcode_send
    ret 0x0C
