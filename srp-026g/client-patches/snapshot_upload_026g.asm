; Option B for the 0.26g client (build 2777): upload the world snapshot to srp.
; Port of ../srp-021d/client-patches/snapshot_upload.asm. Not run live yet.
; Every address and offset is 0.26g; the evidence is in
; docs/notes/client_patch.md (section 3, "Review of the injected
; code"). Assemble once per cave placement:
;
;   nasm -f bin -DCAVE=0xA425000 snapshot_upload_026g.asm -o snapshot_upload_026g.section.bin
;   nasm -f bin -DCAVE=0x325A60  snapshot_upload_026g.asm -o snapshot_upload_026g.hash-mismatch.bin
;
; The output is the cave, then the 6-byte ready hook, then the 7-byte request
; hook; patch_026g.py splits it in that order. The code has no absolute
; address operands (calls and jumps are relative, data goes through
; registers), so a rebased image would still run it.
;
; Differences from 0.21d:
; - No room after .text (0x5D bytes). The cave is a new PE section (default,
;   VA 0xA425000) or overwrites game_world_core::on_hash_mismatch (0x325A60).
; - The core is embedded: core = [nc+0x1F1C40] (pvp_match_core) + 0x1F8.
; - game_state_history_item::serialize is stdcall (item, writer*, mask).
; - buffer_writer::w is (writer*, src) on the stack, n in edi, callee pops 8.
; - The snapshot goes out as 0xE0 (0.26g's "leave", which has no payload).
; - Each chunk packet has its "may be joined" bit cleared, so a chunk is
;   always alone in its datagram (fixed ids carry no length when joined).
; - The upload is skipped when the packet pool could run dry.
;
; What was checked in the 0.26g disassembly, per line, is marked [n] and
; listed at the end of this file.

bits 32

%ifndef CAVE
%define CAVE 0xA425000          ; new section .srp (RVA 0xA415000); 0x325A60 = over on_hash_mismatch
%endif

SERIALIZE_ITEM  equ 0x324BE0    ; [1] game_state_history_item::serialize, stdcall(item, writer*, mask), ret 0Ch
BUFFER_WRITE    equ 0x07AC10    ; [2] buffer_writer::w: push src, push writer*, edi = n; ret 8
ALLOCA_PROBE    equ 0x424CA0    ; [3] _alloca_probe_16; eax = size
HAS_HORIZON     equ 0x2D1070    ; [4] game_world_core::is_there_event_horizon_item; eax = core; -> al
HORIZON_ITEM    equ 0x2D0B30    ; [5] game_world_core::event_horizon_history_item; eax = core; -> eax
READY_HOOK      equ 0x27AEA1    ; [6] network_client::on_match_ready: mov ebx,[ebx+1F1C48h], before the 0xDA send
REQUEST_HOOK    equ 0x27A505    ; [7] on_match_packet_received, 0xF6: cmp byte [g_debug_hash_mismatches],0
REQUEST_DONE    equ 0x27A5AB    ; [7] skips the rest of the message and returns

NC_PVP_CORE     equ 0x1F1C40    ; [8] network_client -> pvp_match_core
NC_MATCH_CLIENT equ 0x1F1C48    ; [9] network_client -> match_client (vtbl +0x18 new_packet, +0x1C enqueue)
NC_STATE        equ 0x1F1C64    ; 2 = static info received
PVP_CORE        equ 0x1F8       ; game_world_core embedded in pvp_match_core
CORE_NEWEST     equ 0x2FC0      ; [10] newest history item (tail)
CORE_MASKS      equ 0x12280     ; [11] players-mask history; head at +8, node {+4 next, +8 mask, +0xC time}
ITEM_TIME       equ 0xC0BB0
MC_PACKET_POOL  equ 0x578B0     ; [12] match_client -> packet pool; pool +0x30 packets in use, +0x34 capacity (256)
POOL_USED       equ 0x30
POOL_CAPACITY   equ 0x34
POOL_RESERVE    equ 16          ; packets left for inputs, hashes and the ready message
PACKET_FLAGS    equ 0x98        ; [13] 0x60 from the ctor: bit 5 single message, bit 6 may be joined
PACKET_WRITER   equ 0x2A8       ; [14] buffer_writer; its mutable_buffer is pkt+0x298 (capacity 0x1FC)

SNAPSHOT_MSG    equ 0xE0        ; C->S "leave", {inf, 500, 9F}; a real leave is empty
CHUNK           equ 480         ; [15] 3 header + 1 id + 8 chunk header + 480 = 492 <= 0x1FC, 16 left for ack bytes
BUFFER_SIZE     equ 0xFF00      ; the same stack buffer hash() serializes into (0x8000 in the stock exe; patch 60, WORLD_ROOM)

section cave start=0 vstart=CAVE align=1

; From READY_HOOK: ebx = network_client. Uploads the initial world that
; game_world_core::serialize (0x2D04F0, called at 0x27AD15) has just built,
; then sends 0xDA as before.
cave_ready:
    pushad
    mov     edi, ebx
    mov     eax, [edi + NC_PVP_CORE]
    test    eax, eax
    jz      .done
    mov     esi, [eax + PVP_CORE + CORE_NEWEST]
    test    esi, esi
    jz      .done
    call    upload
.done:
    popad
    mov     ebx, [ebx + NC_MATCH_CLIENT]    ; the instruction the hook replaced
    jmp     READY_HOOK + 6

; From REQUEST_HOOK: esi = network_client, ebx = reader. srp asked for a
; snapshot with 0xF6; upload the event-horizon item, the one 0xDF reports.
cave_request:
    pushad
    mov     edi, esi
    cmp     dword [edi + NC_STATE], 2
    jne     .done
    mov     eax, [edi + NC_PVP_CORE]
    test    eax, eax
    jz      .done
    add     eax, PVP_CORE
    call    HAS_HORIZON                     ; keeps esi, edi
    test    al, al
    jz      .done
    mov     eax, [edi + NC_PVP_CORE]
    add     eax, PVP_CORE
    call    HORIZON_ITEM                    ; keeps esi, edi
    test    eax, eax
    jz      .done
    mov     esi, eax
    call    upload
.done:
    popad
    jmp     REQUEST_DONE

; esi = history item, edi = network_client. Sends the item as SNAPSHOT_MSG
; chunks: u32 item_time, u16 offset, u16 total, then up to CHUNK bytes.
; Clobbers every register but ebp/esp (the callers pushad).
%define item     ebp - 0x04
%define client   ebp - 0x08
%define buffer   ebp - 0x0C
%define total    ebp - 0x10
%define offset   ebp - 0x14
%define packet   ebp - 0x18
%define header   ebp - 0x20         ; u32 time, u16 offset, u16 total
%define mbuffer  ebp - 0x30         ; mutable_buffer {allocator, data, capacity, size}
%define writer   ebp - 0x48         ; buffer_writer {0, ?, 0, 0, u8 0xFF, mutable_buffer*}
upload:
    push    ebp
    mov     ebp, esp
    sub     esp, 0x50
    mov     [item], esi
    mov     [client], edi
    mov     eax, BUFFER_SIZE
    call    ALLOCA_PROBE                    ; [3] only eax and esp change
    mov     [buffer], esp

    xor     eax, eax                        ; [16] the scratch writer, as hash() builds it
    mov     [mbuffer], eax
    mov     ecx, [buffer]
    mov     [mbuffer + 4], ecx
    mov     dword [mbuffer + 8], BUFFER_SIZE
    mov     [mbuffer + 0xC], eax
    mov     [writer], eax
    mov     [writer + 4], eax
    mov     [writer + 8], eax
    mov     [writer + 0xC], eax
    mov     byte [writer + 0x10], 0xFF
    lea     ecx, [mbuffer]
    mov     [writer + 0x14], ecx

    ; The players mask at the item's time, as hash() 0x325150 takes it (0 if none).
    mov     ecx, [esi + ITEM_TIME]
    mov     [header], ecx
    mov     eax, [edi + NC_PVP_CORE]
    mov     eax, [eax + PVP_CORE + CORE_MASKS + 8]
    xor     edx, edx
.find_mask:
    test    eax, eax
    jz      .serialize
    cmp     [eax + 0xC], ecx
    jbe     .found_mask
    mov     eax, [eax + 4]
    jmp     .find_mask
.found_mask:
    mov     edx, [eax + 8]
.serialize:
    push    edx
    lea     ecx, [writer]
    push    ecx
    push    dword [item]
    call    SERIALIZE_ITEM                  ; [1] pops its 3 arguments

    mov     eax, [mbuffer + 0xC]
    mov     [total], eax
    mov     [header + 6], ax
    and     dword [offset], 0

    ; [12] Each chunk takes a packet from the game side's pool until the
    ; server acks it, and an empty pool is a crash (new_packet does not
    ; check). Skip the upload unless every chunk fits with a reserve; srp
    ; asks again when nothing arrives.
    add     eax, CHUNK - 1
    xor     edx, edx
    mov     ecx, CHUNK
    div     ecx                             ; eax = chunks
    mov     ecx, [client]
    mov     ecx, [ecx + NC_MATCH_CLIENT]
    mov     ecx, [ecx + MC_PACKET_POOL]
    add     eax, [ecx + POOL_USED]
    add     eax, POOL_RESERVE
    cmp     eax, [ecx + POOL_CAPACITY]
    ja      .sent

.next_chunk:
    mov     eax, [offset]
    cmp     eax, [total]
    jae     .sent
    mov     [header + 4], ax

    mov     eax, [client]
    mov     ecx, [eax + NC_MATCH_CLIENT]
    mov     eax, [ecx]
    push    SNAPSHOT_MSG
    call    [eax + 0x18]                    ; [9] new_packet(id), thiscall, ret 4
    mov     [packet], eax
    and     byte [eax + PACKET_FLAGS], 0xBF ; [13] never joined with another message

    mov     edi, 8
    lea     ecx, [header]
    push    ecx
    add     eax, PACKET_WRITER
    push    eax
    call    BUFFER_WRITE                    ; [2] keeps edi; pops 8

    mov     edi, [total]
    sub     edi, [offset]
    cmp     edi, CHUNK
    jbe     .write_chunk
    mov     edi, CHUNK
.write_chunk:
    mov     ecx, [buffer]
    add     ecx, [offset]
    push    ecx
    mov     ecx, [packet]
    add     ecx, PACKET_WRITER
    push    ecx
    call    BUFFER_WRITE

    mov     eax, [client]
    mov     ecx, [eax + NC_MATCH_CLIENT]
    mov     eax, [ecx]
    push    dword [packet]
    call    [eax + 0x1C]                    ; [9] enqueue(pkt), thiscall, ret 4

    add     dword [offset], CHUNK
    jmp     .next_chunk
.sent:
    mov     esp, ebp
    pop     ebp
    ret

cave_end:

section ready_hook follows=cave vstart=READY_HOOK align=1
    jmp     cave_ready
    nop

section request_hook follows=ready_hook vstart=REQUEST_HOOK align=1
    jmp     cave_request
    nop
    nop

; What was read in the original 0.26g exe (dis.py 026g <VA> <LEN>):
;
; [1] 0x324BE0: `push ebx; mov ebx,[esp+0Ch]` (2nd argument = writer),
;     `push ebp; mov ebp,[esp+0Ch]` (1st = item); the mask is the 3rd
;     ([esp+1Ch] after five more pushes, 0x324CB1); ends `ret 0Ch`. hash()
;     calls it as push mask / push &writer / push item (0x3251D1..0x3251D7).
;     It sets writer+0x10 itself and saves ebx, ebp, esi, edi.
; [2] 0x7AC10: `push ebx; mov ebx,[esp+8]` = writer (last pushed);
;     mutable_buffer = [writer+0x14]; resize(old size + edi) (0x7AB30);
;     memcpy(data + old size, [esp+10h] = src, edi); `ret 8`. Saves ebx, esi;
;     edi is only read. Past the capacity with a null allocator it asserts
;     and dereferences null (0x7AB70), so a packet must stay <= 0x1FC bytes
;     and the world <= BUFFER_SIZE (hash() has the same limit every 50 ms;
;     patch 60 makes both 0xFF00, the most a u16 total holds in 480s).
; [3] 0x424CA0: aligns the size, tail-jumps to _chkstk 0x429F20; hash() uses
;     it the same way (0x325176..0x32518A: buffer = esp after the call).
; [4] 0x2D1070: reads [eax+2FBCh], [eax+18FDDh], item +0xC0BB0 / +0xC0BB9;
;     push/pop esi, edi; returns al. False for an item loaded from a
;     snapshot (+0xC0BB9), so a request right after a resync uploads nothing
;     until the horizon passes a simulated item.
; [5] 0x2D0B30: eax = core -> eax = item or 0; touches ecx, edx only.
;     try_send_all calls [4] then [5] with eax = [nc+1F1C40h]+1F8h
;     (0x27CD66..0x27CD85) before it sends 0xDF.
; [6] 0x27AE9A `mov byte [ebx+1F1C68h],0FFh` proves ebx = network_client at
;     the hook. The 6 replaced bytes hold no relocation. After the hook the
;     function loads esi, edx, eax afresh and pops edi; flags are not used.
;     Nothing branches into 0x27AEA2..0x27AEA6 (selftest_026g.py scans).
; [7] 0x27A300: `mov ebx,[ebp+0Ch]` = reader, `mov esi,ecx` = network_client,
;     edi = 0. The 0xF6 case (jump table 0x27A738 entry 19 = 0x27A4F4) skips
;     the payload when state != 2 and falls into 0x27A505 in every state, so
;     cave_request tests the state itself. 0x27A5AB sets the reader to its
;     end and jumps to the common exit 0x27A6FC (uses ebx, ebp only). The 7
;     replaced bytes hold one base relocation (0x27A507); patch_026g.py
;     turns it into a no-op entry.
; [8] 0x27AD1A `mov ecx,[eax+1F1C40h]`, 0x27CD66 (core = that + 0x1F8).
; [9] vtbl +0x18 = 0x279FE0 `lea eax,[ecx+578B0h]; jmp 37A0E0h` (ret 4,
;     returns the packet in eax, saves esi, edi); +0x1C = 0x27A2A0 (ret 4).
;     Both hook sites are followed by the client's own new_packet / enqueue
;     calls on the same object (0x27AEAE, 0x27B5E7), so the thread is right.
; [10] 0x2D04F0 appends the new item at [core+2FC0h].
; [11] hash() 0x325197..0x3251CE: list head [masks+8], node +0xC time,
;     +4 next, +8 mask; first node with time <= item time. hash() would read
;     address 8 on an empty list; this code passes mask 0 instead.
; [12] new_packet 0x37A0E0: `mov esi,[edi]` (edi = match_client+578B0h) is
;     the pool; 0x279D80 compares [pool+30h] with [pool+34h], pops the free
;     list (0x279E10) and increments [pool+30h]; 0x279D40 decrements it. An
;     empty free list asserts and returns 0 (0x279E8D..0x279EFC), and
;     0x37A0F6 then calls construct_packet with a null packet. The pool is
;     built at 0x378A77..0x378A90 with 0x2C000 bytes / 0x2C0 = 256 packets.
; [13] ctor 0x278D98 `mov byte [eax+98h],60h`. send_queued_packets joins
;     only when the first packet (0x3FCA86) and the candidate (0x3FCAD5)
;     both have bit 6, and only with -join_net_messages (0x3FCA57).
; [14] ctor 0x278D9F..0x278DD7: mutable_buffer at pkt+0x298 = {0, pkt+0x99,
;     0x1FC, 3}; writer +0x14 (pkt+0x2BC) points at it. The client's own
;     senders use `lea edi,[esi+2A8h]` (0x27CDD6).
; [15] send_queued_packets 0x3FC310: a packet over 0x1FC bytes asserts;
;     otherwise free = 0x1FC - size and min(wanted ack bytes, free) ack
;     bytes are appended (0x3FCA27..0x3FCA53); if some did not fit, one
;     extra keep-alive datagram carries them (0x3FCBCD..0x3FCBF4).
; [16] hash() 0x32518A..0x3251B0: mutable_buffer {0, esp, 0x8000, 0} (0xFF00 with patch 60) and
;     writer {+0 0, +4 unset, +8 0, +0xC 0, +0x10 0xFF, +0x14 &buffer}.
