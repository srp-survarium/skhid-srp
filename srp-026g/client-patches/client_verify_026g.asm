; Patches 75, 77 and 78: the client check's entries (client_world_check.md;
; README patches 75..78), in the arena's block client_verify.code with the
; Rust crate rust/client_verify (what each hook does is said there). Each is
; a RUST_HOOK (registers saved, the stack aligned, the hook called, the
; registers and flags put back as it left them); ZF set: the game goes on as
; before; clear: its own other path.
bits 32
section .text
%include "rust_hook_026g.inc"

%define WORLD_LOAD 0x2D0B70        ; game_world_core::deserialize, `ret 10h`
%define WORLD_EPILOGUE 0x27C1E0    ; process_dynamic_match_info's epilogue (its frame in ebp)
%define DISPATCH_TABLE 0x27A738    ; on_match_packet_received's jump table, 0xE3..0xF6
%define DISPATCH_EPILOGUE 0x27A72C ; its epilogue (`lea esp,[ebp-70h]` ... `ret 8`)
%define ANOMALY_BACK 0x31B108      ; after the anomaly's index
%define ANOMALY_NO_STATE 0x31B0FB  ; its `xor ebp,ebp / jmp 31B108h` (id 0FFh)
%define ARTEFACT_BACK 0x2FE663     ; after the artefact's read: the game's null test
%define ARTEFACT_NONE 0x2FE67A     ; the no-artefact exit (ebp 0)

; Patch 75: the 0xE8 handler's `call 0x2D0B70` (0x27C0D9, 0x27C111; the
; core, the reader, 1, 8 pushed): read, the load with the same arguments
; and return address; refused (the reader consumed, the match left), out by
; the handler's epilogue, past the post-load steps.
global cv_world_load
cv_world_load:
    RUST_HOOK world_hook
    jnz WORLD_EPILOGUE
    jmp WORLD_LOAD

; Patch 75: the dispatch's `jmp [eax*4+27A738h]` (0x27A330): read, that jump
; (EAX as the site had it); refused, out by the dispatch's epilogue.
global cv_message
cv_message:
    RUST_HOOK message_hook
    jnz DISPATCH_EPILOGUE
    jmp [eax * 4 + DISPATCH_TABLE]

; Patch 77: `mov eax,[edi+68h] / movzx edx,cl / mov ebp,[eax+edx*4]`
; (0x31B0FF): the hook did them for an id in the vector.
global cv_anomaly
cv_anomaly:
    RUST_HOOK anomaly_hook
    jnz ANOMALY_NO_STATE
    jmp ANOMALY_BACK

; Patch 78: `lea eax,[edx+eax*4] / mov eax,[eax]` (0x2FE65E): the hook did
; them for an id in the table, or set ebp 0.
global cv_artefact
cv_artefact:
    RUST_HOOK artefact_hook
    jnz ARTEFACT_NONE
    jmp ARTEFACT_BACK
