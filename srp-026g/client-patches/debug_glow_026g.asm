; A DEBUG build only (debug_glow_026g.py): never in patch_026g.py's release
; patch. Logs the artefacts' glow at every step of its chain into the game's
; own log (Survarium_<name>.log, channel "game", info), each line starting
; "srp-glow:", once per event: an activation, a presenter's activation or
; stop, a play, a stop of a particle system, an effect given back to its
; emitter, the artefact's calculator ending an effect, the local body's bones
; the particles read (40 a activation), and, from the start, the models added
; to the scene and their foreground and visibility parts (capped).
; The sections's own data follow the code (the section is RWX).
BITS 32
org BASE

%define LOG 0x111ED0            ; the game's logger (functor, log, 0x21E73B8, file, line, function, initiator, level, format, ...)
%define LOG_ON 0x111860         ; (level, initiator): al = on
%define LOG_FILTER 0x1F1D60C    ; a filter: when nonzero, LOG_ON decides
%define LOG_SINK 0x1F1D610      ; the log
%define LOG_CALLBACK 0x1F1D614  ; the functor's data
%define LOG_MANAGER 0x9DAF4C    ; the functor's manager
%define LOG_TAG 0x21E73B8
%define GAME 0x9FA4E4           ; "game"
%define FP_VT 0xA0B5E4          ; first_person_game_effects_presenter
%define TP_VT 0xA0B620          ; third_person_game_effects_presenter
%define FRAME 0x34              ; pushfd + pushad + the saved xmm0: the hooked site's [esp] is [esp+FRAME]

; The hooks' entries, 5 bytes apart (debug_glow_026g.py jumps here).
    jmp near stub_activate      ; +0   0x313435
    jmp near stub_calculator    ; +5   0x313640
    jmp near stub_present       ; +10  0x2AC400
    jmp near stub_play          ; +15  0x2AC290
    jmp near stub_stop          ; +20  0x22860
    jmp near stub_release       ; +25  0x29A320
    jmp near stub_remove        ; +30  0x229D0
    jmp near stub_world_stop    ; +35  0x35A8B0
    jmp near stub_world_remove  ; +40  0x35B110
    jmp near stub_reset         ; +45  0x35D120

; ENTER takes the section's lock (its values are shared by the game's and the
; render's threads), LEAVE gives it back.
%macro ENTER 0
    pushfd
    pushad
    sub esp, 16
    movups [esp], xmm0
%%spin:
    lock bts dword [glow_lock], 0
    jnc %%held
    pause
    jmp %%spin
%%held:
%endmacro
%macro LEAVE 0
    mov dword [glow_lock], 0
    movups xmm0, [esp]
    add esp, 16
    popad
    popfd
%endmacro
; vals[i] = a dword
%macro VAL 2
    mov eax, %2
    mov [vals + %1*4], eax
%endmacro
; vals[i] = a float at an address, x1000 (mm, or thousandths), as an int
%macro MM 2
    movss xmm0, %2
    mulss xmm0, [k1000]
    cvttss2si eax, xmm0
    mov [vals + %1*4], eax
%endmacro
; log(format, n values of vals)
%macro EMIT 2
    mov eax, %1
    mov ecx, %2
    call emit_log
%endmacro

; eax = the format, ecx = how many of vals. Keeps nothing but esp.
emit_log:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    mov esi, eax
    mov edi, ecx
    cmp dword [LOG_FILTER], 0
    je .go
    push 4
    push GAME
    call LOG_ON
    add esp, 8
    test al, al
    je .out
.go:
    mov eax, [LOG_CALLBACK]
    mov [functor + 8], eax
    xor edx, edx
    test eax, eax
    jz .nofunctor
    mov edx, LOG_MANAGER | 1
.nofunctor:
    mov [functor], edx
    mov dword [functor + 4], 0
    mov ecx, edi
    test ecx, ecx
    jz .pushed
.push:
    push dword [vals + ecx*4 - 4]
    dec ecx
    jnz .push
.pushed:
    push esi
    push 4
    push GAME
    push s_function
    push 1
    push s_file
    push LOG_TAG
    push dword [LOG_SINK]
    push functor
    call LOG
    lea esp, [ebp - 12]
.out:
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

; ---- EFFECT 0x313350 at 0x313435 `lea edi,[eax+358h]`, before game_effect_player::add:
; eax = the holder (a player), edi = the artefact + 268h.
stub_activate:
    ENTER
    mov ebx, [esp + 16 + 0x1C]          ; eax: the player
    mov edi, [esp + 16 + 0x00]          ; edi: the artefact + 268h
    sub edi, 0x268
    VAL 0, edi
    VAL 1, ebx
    xor ecx, ecx                        ; the presenter
    mov esi, [ebx + 0x390]
    test esi, esi
    jz .noholder
    mov ecx, [esi]
.noholder:
    xor edx, edx
    test ecx, ecx
    jz .novt
    mov edx, [ecx]
.novt:
    VAL 2, edx
    xor ebp, ebp
    cmp edx, FP_VT
    jne .tp
    mov ebp, [ecx + 0x478]
    mov [local_player], ebx
    mov eax, [ebx + 0xF290]
    mov [local_body], eax
    mov dword [bone_budget], 40
    mov dword [other_bone_budget], 4
    jmp .target
.tp:
    cmp edx, TP_VT
    jne .target
    mov ebp, [ecx + 0x10]
.target:
    VAL 3, ebp
    VAL 4, [ebx + 0xF290]
    MM 5, [ebx + 0x40C + 0x30]
    MM 6, [ebx + 0x40C + 0x34]
    MM 7, [ebx + 0x40C + 0x38]
    MM 8, [ebx + 0x47C]
    MM 9, [ebx + 0x480]
    MM 10, [ebx + 0x484]
    MM 11, [ebx + 0x46C]
    MM 12, [ebx + 0x470]
    MM 13, [ebx + 0x474]
    movzx eax, byte [ebx + 0xF4E1]
    mov [vals + 14*4], eax
    movzx eax, byte [ebx + 0xF4E0]
    mov [vals + 15*4], eax
    mov esi, [ebx + 0x128]              ; the weapon
    VAL 16, esi
    xor eax, eax
    mov [vals + 17*4], eax
    mov [vals + 18*4], eax
    mov [vals + 19*4], eax
    mov [vals + 20*4], eax
    mov [vals + 21*4], eax
    mov [vals + 22*4], eax
    test esi, esi
    jz .noweapon
    mov edi, [esi + 0x1808]
    test edi, edi
    jz .nohud
    VAL 17, [edi + 0x108]
    VAL 18, [edi + 0x10C]
.nohud:
    VAL 19, [esi + 0x1810]
    MM 20, [esi + 0x9BC + 0x30]
    MM 21, [esi + 0x9BC + 0x34]
    MM 22, [esi + 0x9BC + 0x38]
.noweapon:
    EMIT f_activate, 23
    LEAVE
    lea edi, [eax + 0x358]
    jmp 0x31343B

; ---- artefact_base::effect_calculator 0x313640 (ecx = the calculator, +8 the artefact):
; logs the call that ends an artefact's effect (its state no longer 4), once.
stub_calculator:
    ENTER
    mov esi, [ecx + 8]
    mov eax, [esi + 0x280]
    mov ebx, eax
    mov edi, calc_seen
    mov ecx, 32
.find:
    cmp [edi], esi
    je .found
    add edi, 4
    loop .find
    xor edi, edi                        ; not in the table
.found:
    cmp eax, 4
    jne .ended
    test edi, edi
    jnz .done                           ; active, already seen
    mov edi, calc_seen                  ; active: remember it
    mov ecx, 32
.free:
    cmp dword [edi], 0
    je .put
    add edi, 4
    loop .free
    jmp .done
.put:
    mov [edi], esi
    jmp .done
.ended:
    test edi, edi
    jz .done
    mov dword [edi], 0
    VAL 0, esi
    VAL 1, ebx
    VAL 2, [esp + FRAME]                ; who asked (the effect player)
    EMIT f_calculator, 3
.done:
    LEAVE
    mov ecx, [ecx + 8]
    cmp dword [ecx + 0x280], 4
    jmp 0x31364A

; ---- particle_game_effect_presenter's handler 0x2AC400 ([esp+4] the presenter,
; [esp+8] the effect, edx the event): its activations and stops.
stub_present:
    ENTER
    mov edx, [esp + 16 + 0x14]
    mov eax, [edx + 4]
    cmp eax, 1
    ja .done
    VAL 0, eax
    mov ebx, [esp + FRAME + 4]          ; the presenter
    mov esi, [esp + FRAME + 8]          ; the effect
    VAL 1, [ebx + 0xA8]
    VAL 2, [ebx + 8]
    VAL 3, esi
    VAL 4, [esi + 0x34]
    VAL 5, [esi + 0xC]
    mov ecx, s_no_player
    cmp dword [ebx + 8], 0
    je .decided
    mov ecx, s_skip
    mov eax, [esi + 0x34]
    cmp eax, 2
    je .goes
    cmp eax, [ebx + 0xA8]
    jne .decided
.goes:
    mov ecx, s_play
    cmp dword [vals], 0
    je .decided
    mov ecx, s_stop
.decided:
    mov [vals + 6*4], ecx
    EMIT f_present, 7
.done:
    LEAVE
    push ebp
    mov ebp, esp
    and esp, 0xFFFFFFF8
    sub esp, 0x3C
    jmp 0x2AC409

; ---- the presenter's play 0x2AC290 (eax = the player, [esp+4] the presenter, [esp+8] its entry)
stub_play:
    ENTER
    mov ebx, [esp + 16 + 0x1C]          ; the player
    mov esi, [esp + FRAME + 4]
    mov edi, [esp + FRAME + 8]
    VAL 0, ebx
    VAL 1, [ebx + 0xF290]
    VAL 2, [ebx + 0x498]
    VAL 3, [edi + 0xC]
    VAL 4, [esi + 0xA8]
    xor eax, eax
    cmp ebx, [local_player]
    sete al
    mov [vals + 5*4], eax
    EMIT f_play, 6
    LEAVE
    push esi
    lea edx, [eax + 0xF290]
    jmp 0x2AC297

; ---- scene_renderer::stop_particle_system 0x22860 ([esp+0Ch] &instance, [esp+10h] the finish time)
stub_stop:
    ENTER
    cmp dword [stop_budget], 0
    je .done
    dec dword [stop_budget]
    mov eax, [esp + FRAME + 0xC]
    VAL 0, [eax]
    MM 1, [esp + FRAME + 0x10]
    VAL 2, [esp + FRAME]
    EMIT f_stop, 3
.done:
    LEAVE
    sub esp, 0x4C
    push ebx
    push ebp
    jmp 0x22865

; ---- particle_game_effect_emitter's release of an effect 0x29A320 (ecx = the emitter, [esp+4] the effect)
stub_release:
    ENTER
    cmp dword [stop_budget], 0
    je .done
    dec dword [stop_budget]
    mov ebx, [esp + 16 + 0x18]          ; ecx: the emitter
    mov esi, [esp + FRAME + 4]
    VAL 0, ebx
    VAL 1, esi
    VAL 2, [esi + 0x30]
    VAL 3, [ebx + 0x13C]
    mov eax, [ebx + 0x130]
    sub eax, [ebx + 0x12C]
    sar eax, 2
    mov [vals + 4*4], eax
    VAL 5, [esp + FRAME]
    EMIT f_release, 6
.done:
    LEAVE
    push ebx
    push ebp
    mov ebp, [esp + 0xC]
    jmp 0x29A326

; ---- scene_renderer::remove_particle_system_instance 0x229D0 (capped)
stub_remove:
    ENTER
    cmp dword [remove_budget], 0
    je .done
    dec dword [remove_budget]
    mov eax, [esp + FRAME + 0xC]
    VAL 0, eax
    xor ecx, ecx
    test eax, eax
    jz .noderef
    mov ecx, [eax]
.noderef:
    mov [vals + 1*4], ecx
    VAL 2, [esp + FRAME]
    EMIT f_remove, 3
.done:
    LEAVE
    push ebp
    mov ebp, esp
    and esp, 0xFFFFFFF8
    sub esp, 0x44
    jmp 0x229D9

; ---- the render side: particle_world::stop 0x35A8B0 ([esp+4] the instance, [esp+8] the finish
; time: +2E8h playing 0, +301h stopping 1, +308h the time), its remove_particle_system_instance
; 0x35B110 (ecx the world, [esp+4] the instance), particle_system_instance_impl::reset 0x35D120
; ([esp+4] the instance), each capped
stub_world_stop:
    ENTER
    mov ecx, f_world_stop
    mov edx, 1
    call log_instance
    LEAVE
    mov eax, [esp + 4]
    xor ecx, ecx
    jmp 0x35A8B6
stub_world_remove:
    ENTER
    mov ecx, f_world_remove
    xor edx, edx
    call log_instance
    LEAVE
    push ecx
    mov eax, [ecx + 0x19C]
    jmp 0x35B117
stub_reset:
    ENTER
    mov ecx, f_reset
    xor edx, edx
    call log_instance
    LEAVE
    push ebp
    mov ebp, esp
    and esp, 0xFFFFFFF8
    jmp 0x35D126
log_instance:                            ; ecx = the format, edx = 1: a finish time at [site+8]
    cmp dword [render_budget], 0
    je .done
    dec dword [render_budget]
    mov [log_format], ecx
    mov eax, [esp + FRAME + 4 + 4]
    VAL 0, eax
    xor eax, eax
    mov [vals + 1*4], eax
    test edx, edx
    jz .notime
    MM 1, [esp + FRAME + 4 + 8]
.notime:
    VAL 2, [esp + FRAME + 4]
    mov eax, [log_format]
    mov ecx, 3
    call emit_log
.done:
    ret

align 4
k1000:          dd 1000.0
local_player:   dd 0
local_body:     dd 0
bone_budget:    dd 0
other_bone_budget: dd 0
remove_budget:  dd 300
render_budget:  dd 400
stop_budget:    dd 600
glow_lock:      dd 0
log_format:     dd 0
functor:        times 4 dd 0
vals:           times 32 dd 0
calc_seen:      times 32 dd 0

s_file:         db "debug_glow_026g.asm", 0
s_function:     db "srp-glow", 0
s_play:         db "play", 0
s_stop:         db "stop", 0
s_skip:         db "skip (view)", 0
s_no_player:    db "skip (no player)", 0
f_activate:     db "srp-glow: activate artefact=%p player=%p presenter_vt=%08x presenter_player=%p body=%p "
                db "body_pos_mm=%d,%d,%d eye_mm=%d,%d,%d look=%d,%d,%d head_visible=%d in_scene=%d "
                db "weapon=%p weapon_models=%p,%p weapon_model1810=%p weapon_pos_mm=%d,%d,%d", 0
f_calculator:   db "srp-glow: calculator artefact=%p state=%d -> the effect ends (caller %p)", 0
f_present:      db "srp-glow: present kind=%d presenter_view=%d presenter_player=%p effect=%p effect_view=%d emitter=%p -> %s", 0
f_play:         db "srp-glow: play player=%p body=%p bodyparts=%p instance=%p presenter_view=%d local=%d", 0
f_stop:         db "srp-glow: stop_particle_system instance=%p finish_ms=%d caller=%p", 0
f_release:      db "srp-glow: emitter_release emitter=%p effect=%p instance=%p view=%d pool=%d caller=%p", 0
f_remove:       db "srp-glow: remove_particle_system_instance arg=%p instance=%p caller=%p", 0
f_world_stop:   db "srp-glow: render particle_world::stop instance=%p finish_ms=%d caller=%p", 0
f_world_remove: db "srp-glow: render particle_world::remove_particle_system_instance instance=%p (-%d) caller=%p", 0
f_reset:        db "srp-glow: render particle_system_instance::reset instance=%p (-%d) caller=%p", 0
