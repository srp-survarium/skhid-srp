; The Rattle (exe patch 59): the hooks of artefact type 8 and of the sounds
; it quiets, in front of the C code of rattle_026g.c in the arena's block
; rattle.code (link_026g.py links it; patch_026g.py's RATTLE_HOOKS jump or
; call to the nine entries by their names). README section 59,
; client_patch.md section 3.53, docs/notes/rattle.md.
;
;   rattle_construct  the artefact cook's case for type 8, `call 0x25FA10`
;                     at 0x25E72C: Rattle's constructor, then the Rattle's
;                     vtable and numbers
;   rattle_steps      on_player_sound_event 0x2A6470 (its entry, `mov
;                     eax,ecx / mov edx,[eax+3Ch]`): the feet, the jump and
;                     the rustles played with the player's steps' quiet
;   rattle_landing    on_landing's step, `call 0x23FBE0` at 0x1F3BB6: the
;                     same for the landing
;   rattle_point      0x657C0's point proxy, `call 0x64F80` at 0x65874: the
;                     proxy marked with the steps' quiet while one is pending
;   rattle_shot       play_sound_callback's point proxy, `call 0x65670` at
;                     0x297F79: a fire state's proxy marked with its weapon
;                     user's shot quiet
;   rattle_amplitude  calculate_hdr_audio_frame, `movss [esi+4Ch],xmm0` at
;                     0x5CA37 (sound thread): the amplitude x the gain
;   rattle_loudness   the same, `movss [esi+40h],xmm0` at 0x5C8BA: the HDR
;                     loudness + 20 log10(gain)
;   rattle_hud        0x657C0's hud proxy, `call 0x65100` at 0x65803: as
;                     rattle_point (the carrier's own steps in first person)
;   rattle_shot_hud   play_sound_callback's hud proxy, `call 0x65100` at
;                     0x297F5E: as rattle_shot (his own shots in first person)
;
; The carrier hears himself as the others hear him: his own game plays his
; steps and shots as hud proxies (0x65100), marked as the point proxies
; are; the sound thread's two stores are every propagator's, hud or point.
; The quiet is the u16 at proxy+3Ah, the y of the proxy's ext parameters
; (zeroed by the constructors 0x5A6B0 and 0x5A840, read by no code): gain
; 1 - q/65535.
; The stubs call only the game functions their sites called (the replaced
; calls: 0x25FA10, 0x23FBE0, 0x64F80, 0x65670, 0x65100; 0x23FBE0 takes xmm0,
; 0x65100 eax the scene) and the
; C code (rt_construct, rt_steps, rt_shot); rt_g is the C code's state (the
; steps' pending quiet, the block rattle.state).
bits 32
global rattle_construct, rattle_steps, rattle_landing, rattle_point, rattle_shot, rattle_amplitude
global rattle_loudness, rattle_hud, rattle_shot_hud
extern rt_construct, rt_steps, rt_shot, rt_g
section .text

%define RATTLE_CONSTRUCT 0x25FA10  ; eax = &effect, ecx = the config's root; stdcall (memory, scene)
%define STEPS_ON         0x2A6475  ; after the entry's two instructions
%define LANDING_STEP     0x23FBE0  ; cl, edx, xmm0 and four stack args; ret 10h
%define NEW_POINT        0x64F80   ; sound_scene::new_point_sound_instance_proxy, ret 10h
%define EMIT_POINT       0x65670   ; sound_emitter::emit_point_sound: eax, ecx, two stack args; ret 8
%define NEW_HUD          0x65100   ; sound_scene::new_hud_sound_instance_proxy: eax the scene, four stack args; ret 10h
%define AMPLITUDE_ON     0x5CA3C
%define LOUDNESS_ON      0x5C8BF
%define G_PENDING        rt_g + 0x0
%define PROXY_QUIET      0x3A
%define PROPAGATOR_PROXY 0x8       ; new_sound_propagator::m_proxy

rattle_construct:
    jmp near construct
rattle_steps:
    jmp near steps
rattle_landing:
    jmp near landing
rattle_point:
    jmp near point
rattle_shot:
    jmp near shot
rattle_amplitude:
    jmp near amplitude
rattle_loudness:
    jmp near loudness
rattle_hud:
    jmp near hud
rattle_shot_hud:
    jmp near shot_hud

; 0x25E72C, called: eax = &effect, ecx = the root; [esp+4] the memory,
; [esp+8] the scene (stdcall, ret 8 as the constructor).
construct:
    push ecx                          ; the root
    push dword [esp + 0xC]            ; the scene
    push dword [esp + 0xC]            ; the memory
    call RATTLE_CONSTRUCT             ; eax the object, 0 for no memory
    pop ecx
    push ecx
    push eax
    call rt_construct                 ; stdcall (object, root): eax the object
    ret 8

; 0x2A6470, its entry: ecx = player_equipment_sound_effect, [esp+4] the
; event (thiscall, ret 4; eax 0).
steps:
    push ecx
    push dword [ecx + 0x3C]           ; the player
    call rt_steps                     ; stdcall (player): the pending quiet
    pop ecx
    push dword [esp + 4]
    call .original
    mov dword [G_PENDING], 0
    ret 4
.original:
    mov eax, ecx
    mov edx, [eax + 0x3C]
    jmp STEPS_ON

; 0x1F3BB6, called: edi = the player, cl = not first view, edx = the time,
; xmm0 = the speed, four stack args (ret 10h).
landing:
    push ecx
    push edx
    sub esp, 4
    movss [esp], xmm0
    push edi
    call rt_steps
    movss xmm0, [esp]
    add esp, 4
    pop edx
    pop ecx
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    call LANDING_STEP
    mov dword [G_PENDING], 0
    ret 0x10

; 0x65874, called with 0x657C0's four stack args (ret 10h) and its edi and
; ebx; eax = &the new proxy's pointer.
point:
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    call NEW_POINT
    jmp mark

; 0x65803, called with 0x657C0's four stack args (ret 10h), eax = the scene;
; eax = &the new proxy's pointer.
hud:
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    call NEW_HUD

; the new proxy (eax = &its pointer) marked with the pending quiet
mark:
    movzx ecx, word [G_PENDING]
    test ecx, ecx
    jz .done
    mov edx, [eax]
    test edx, edx
    jz .done
    mov [edx + PROXY_QUIET], cx
.done:
    ret 0x10

; 0x297F79, called: eax = the scene, ecx = the sound, two stack args (ret
; 8); play_sound_callback's effect at [esp+2Ch] of the site, [esp+30h] here.
shot:
    push dword [esp + 8]
    push dword [esp + 8]
    call EMIT_POINT                   ; eax = &the proxy's pointer
    push eax
    push eax
    push dword [esp + 0x38]           ; the effect
    call rt_shot                      ; stdcall (effect, &proxy)
    pop eax
    ret 8

; 0x297F5E, called: eax = the scene, four stack args (ret 10h); the effect at
; [esp+34h] of the site, [esp+38h] here.
shot_hud:
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    call NEW_HUD                      ; eax = &the proxy's pointer
    push eax
    push eax
    push dword [esp + 0x40]           ; the effect
    call rt_shot                      ; stdcall (effect, &proxy)
    pop eax
    ret 0x10

; 0x5CA37, esi = the propagator, xmm0 = its amplitude (eax free).
amplitude:
    mov eax, [esi + PROPAGATOR_PROXY]
    movzx eax, word [eax + PROXY_QUIET]
    test eax, eax
    jz .store
    neg eax
    add eax, 0xFFFF
    push eax
    fild dword [esp]
    fmul dword [one_over_quiet]       ; the gain
    fstp dword [esp]
    mulss xmm0, [esp]
    add esp, 4
.store:
    movss [esi + 0x4C], xmm0
    jmp AMPLITUDE_ON

; 0x5C8BA, esi = the propagator, xmm0 = its HDR loudness in dB (eax free).
loudness:
    mov eax, [esi + PROPAGATOR_PROXY]
    movzx eax, word [eax + PROXY_QUIET]
    test eax, eax
    jz .store
    cmp eax, 0xFFFF
    jae .silent
    neg eax
    add eax, 0xFFFF
    push eax
    fild dword [esp]
    fmul dword [one_over_quiet]       ; g
    fldlg2
    fxch
    fyl2x                             ; log10(2) x log2(g)
    fmul dword [twenty]
    fstp dword [esp]
    addss xmm0, [esp]
    add esp, 4
    jmp .store
.silent:
    addss xmm0, [silent_db]
.store:
    movss [esi + 0x40], xmm0
    jmp LOUDNESS_ON

section .rodata
align 4
one_over_quiet: dd 0x37800080         ; 1 / 65535
twenty:         dd 20.0
silent_db:      dd -100.0
