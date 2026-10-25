; Patch 76: the death ragdoll (docs/notes/ragdoll.md; README patch 76). The
; arena's block ragdoll.code, with the Rust crate ragdoll_arena: four hooks,
; each a `call` over the game's instruction, which the entry does itself
; after the Rust function.
;
; The game's frames are aligned to 4 bytes only; Rust's i686 code assumes
; 16 at a call. Each entry aligns the stack before its call and keeps every
; register the game's code after the site reads.
bits 32
section .text

global rg_step, rg_cull, rg_apply, rg_eye
extern ragdoll_step, ragdoll_draws, ragdoll_apply, ragdoll_eye

%define CULL_SEEN   0xF4E2              ; player: its model not seen the last frame (0x247519)
%define MODEL       0xF290              ; player: its model
%define INPUT       0xF470              ; player: its input handler
%define ARRAY       -0x14               ; mt_draw's frame: the bones' matrices
%define COUNT       -0x1C               ; and their count

; game_world::tick 0x1F7C3C `cmp byte [esp+0Fh],0` (5 bytes, the join after
; the simulation's move, then `je 0x1F7C6C`) -> call rg_step. EBX = the game
; world. Every register kept; the compare redone for the je.
rg_step:
    pushad
    mov esi, esp
    and esp, -16
    sub esp, 12
    push ebx                            ; the game world
    call ragdoll_step
    mov esp, esi
    popad
    cmp byte [esp+0x13], 0              ; the original's [esp+0Fh], past our return address
    ret

; mt_draw 0x24757C `cmp byte [ebx+0F4E2h],0` (7 bytes; `je 0x247592` draws,
; else the next test may cull) -> call rg_cull + 2 nops. EBX = the player,
; EAX = its model's instance (live into 0x247592). Every register kept; ZF
; set for a corpse of ours (drawn), else the original's compare.
rg_cull:
    pushad
    mov esi, esp
    and esp, -16
    sub esp, 12
    push ebx                            ; the player
    call ragdoll_draws
    mov esp, esi
    test eax, eax
    popad                               ; (keeps the flags)
    jnz .draw
    cmp byte [ebx+CULL_SEEN], 0
    ret
.draw:
    cmp eax, eax                        ; ZF: the je draws
    ret

; mt_draw 0x247716 `mov ecx,[ebx+0F290h]` (6 bytes; the bones made, before
; the "Weapon" bone is read; the je target from 0x2476FF) -> call rg_apply +
; nop. EBX = the player, EBP = mt_draw's frame; EAX, ECX, EDX, ESI, EDI and
; the flags are dead there. The stack below ESP is free (ESP is the held
; item's matrices, which stay).
rg_apply:
    mov esi, esp
    and esp, -16
    sub esp, 4
    push dword [ebp+COUNT]              ; the count
    push dword [ebp+ARRAY]              ; the bones' matrices
    push ebx                            ; the player
    call ragdoll_apply
    mov esp, esi
    mov ecx, [ebx+MODEL]
    ret

; update_camera 0x1F3643 `mov ecx,[ebx+0F470h]` (6 bytes; both ways join
; here, EAX the eye transform given to the view's update at 0x1F364B) ->
; call rg_eye + nop. EBX = the player. EAX = ragdoll_eye's answer (the
; same, or the ragdoll's eye); EDX, ESI kept; ECX as the original.
rg_eye:
    push edx
    push esi
    mov esi, esp
    and esp, -16
    sub esp, 8
    push eax                            ; the transform given
    push ebx                            ; the player
    call ragdoll_eye
    mov esp, esi
    pop esi
    pop edx
    mov ecx, [ebx+INPUT]
    ret

; memmove(to, from, size) for the Rust crate (cdecl; returns `to`): the
; arena's C has memcpy and memset (research_native_026g.c), no memmove.
; Backward when `to` is above `from`, so an overlap copies right.
global memmove
memmove:
    push esi
    push edi
    mov edi, [esp+12]
    mov esi, [esp+16]
    mov ecx, [esp+20]
    mov eax, edi
    cmp edi, esi
    jbe .forward
    lea esi, [esi+ecx-1]
    lea edi, [edi+ecx-1]
    std
    rep movsb
    cld
    jmp .done
.forward:
    rep movsb
.done:
    pop edi
    pop esi
    ret
