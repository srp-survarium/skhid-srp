; Patch 34: the commit_suicide key (K) kills one's own player (build 2777;
; client_patch.md section 3.32): the entry of the Rust crate rust/suicide_key
; (what it does is said there), in the arena's block suicide_key.code.
;
; 0x242196 `call 0x242460` (spot, the input builder's last action; ebx = the
; handler, pushed with action 22h and &state) -> `call suicide_key`: the hook
; asks for action 1Fh and sets the input's bit 24 on its key going down; then
; the original call goes on with the same arguments and return address.
bits 32
section .text
%include "rust_hook_026g.inc"

%define ACTION_STATE 0x242460      ; (handler, action, &state) -> al, `ret 0Ch`

global suicide_key
suicide_key:
    RUST_HOOK suicide_key_hook
    jmp ACTION_STATE
