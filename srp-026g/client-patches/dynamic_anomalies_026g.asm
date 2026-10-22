; Patch 42: dynamic anomalies (build 2777). The release's timed anomalies
; (0.28a2 to 0.46c2: timeout_anomaly_core and timeout_anomalies_rule_core)
; on 0.26g's generic_anomaly_core. The rules: ../docs/notes/dynamic_anomalies.md;
; client_patch.md section 3.38; checks: check_dynamic_anomalies_026g.py and
; selftest_026g.py check 42. The code is the section `.srpa` (patch_026g.py
; DYNAMIC_ANOMALIES_*), after .srpw (the page after its last, the upgrade preview's):
;   nasm -f bin -DCAVE=0xA42D000 dynamic_anomalies_026g.asm -o dynamic_anomalies_026g.section.bin
;
; A timed anomaly is a stock generic_anomaly_core with energy_enabled whose
; artefacts never respawn by themselves (artefacts_respawn_time_sec 100000
; or more; no stock core has more than 80). Its states are the release's
; timeline by energy, all of it the game's own code (select_state 0x31BA90,
; anomaly_state::initialize 0x31BC10 and execute 0x31BCB0: a state with
; active_time_sec ends with energy = energy_on_exit):
;   charging (top threshold, 20 s) -> damage (4 s) -> artefact (until taken)
;   -> after the pick-up (threshold = energy_af_container_use, 60 s) -> idle (0)
; Its energy says where it is: 0 idle; above 0 busy (it blocks a choice
; within 15 m); above energy_af_container_use active (it counts against 16).
; energy_maximum stays 0: every player action near a core adds its
; energy_on_* (0 here) and clamps to a maximum above 0 (0x33BB50), which
; would end an anomaly a player walks into.
;
; Two hooks, nothing else:
; - tick_hook: game_world_core::atomic_tick (0x2D1E20; [esp+4] the world,
;   [esp+8] the tick's length, edi now) first runs the release's anomalies
;   rule (0.28a2 timeout_anomalies_rule_core tick 0x5F7C00, choice 0x5F7EC0),
;   before any object of the tick: from the match's start + 30 s, a try at
;   the first tick at or past each try time, the next 20 to 40 s after that
;   tick; while fewer than 16 are active, one idle timed anomaly with a free
;   artefact of its own and no busy one within 15 m (places: the first
;   container's use box) starts charging now, its artefact due when its timed
;   states above the pick-up level have run (the release: charging + damage).
; - pickup_hook: generic_anomaly_core::on_artefact_container_use (0x33BCD0,
;   the owner's call when a player takes the artefact, use_execute 0x31C838):
;   a timed anomaly goes to its after-the-pick-up energy, no artefact due
;   (the release's 0x5F72C0: state 4, zones off). Any other core: the original.
;
; Lockstep: everything read is the world's (energies, states, artefacts,
; containers, the time rule, the anomalies' tick list in the project's order)
; or the static info's seed; the try times and draws are the seed's and the
; try's number. Nothing is kept outside the world, nothing new is
; serialized: a rollback, a resync and a late joiner choose alike, and the
; snapshot and its hash keep their format.
BITS 32
ORG CAVE

%define ATOMIC_TICK_ON     0x2D1E26   ; after `push ecx; mov eax,51EB851Fh`
%define PICKUP_ON          0x33BCDA   ; after `push ecx; push esi; mov esi,ecx; mov eax,[esi+15Ch]`
%define GENERIC_TICK       0x31B5B0   ; generic_anomaly_core::tick, its delegate's function
%define TIMELIMIT_VTABLE   0xA06E2C   ; timelimit_rule_core (constructor 0x317DB6)

; game_world_core
%define W_RULES            0x18824    ; rule*[] begin, end (+4): the mode config's order, the time rule first
%define W_ANOMALIES        0x188A4    ; tick list 6's first node: the project's generic_anomaly_cores, in its order
                                      ; (server_game_project::set_game_world_core 0x318B26)
%define W_STATIC_INFO      0x18FAC    ; the static info (world constructor 0x2D00BB)
%define INFO_SEED          0x577B0    ; its u32 id, the match's seed (0x2D01D7, static_info.md)
; timelimit_rule_core
%define RULE_START         0x12C      ; m_state_start_time_ms
%define RULE_STATE         0x148      ; 3: in process
; a tick list's node (the core's own at +194h): +0 object, +4 function, +0Ch next
%define NODE_OBJECT        0x00
%define NODE_FUNCTION      0x04
%define NODE_NEXT          0x0C
; generic_anomaly_core (load 0x33BD50, constructor 0x31ADF0)
%define RESPAWN_TIME       0x12C      ; f32 artefacts_respawn_time_sec
%define PENDING            0x138      ; u8: an artefact is due
%define PENDING_AT         0x13C      ; u32: when
%define CONTAINERS         0x140      ; artefact_container_core*[] begin, end (+4)
%define ENERGY_ENABLED     0x14C      ; u8
%define ENERGY             0x150      ; f32
%define AF_USE             0x15C      ; u32 energy_af_container_use
%define STATES             0x180      ; anomaly_state*[] begin, end (+4), highest threshold first
%define ARTEFACTS          0x1B0      ; artefact_base*[]
%define ARTEFACTS_COUNT    0x1B4
%define HOLDER             0x280      ; an artefact's: 0 while free (spawn_artefact 0x31B6C0)
; anomaly_state
%define THRESHOLD          0x08       ; u32 energy_threshold
%define ACTIVE_TIME        0x10       ; u32 active_time_sec
; the container's place: its first collision geometry (usable_object::load
; 0x2F81F0) -> ghost (collision_geometry::load 0x2F8F7C) -> bullet object
; (bt_ghost_object 0x3578F0) -> the world transform's origin (0x70C20)
%define GEOMETRIES         0x38
%define GEOMETRIES_COUNT   0x3C
%define GEOMETRY_GHOST     0x18
%define GHOST_OBJECT       0x18
%define OBJECT_ORIGIN      0x40

; the release's anomalies rule (0.28a2 to 0.31a2, every map)
%define DELAY              30000      ; spawn_anomalies_delay_s
%define INTERVAL           30000      ; try_spawn_anomaly_interval_s
%define VARIATION          10000      ; try_spawn_anomaly_variation_s
%define MAX_ACTIVE         16         ; max_active_anomalies_count
%define BLOCK_SQUARED      0x43610000 ; 225.0f: block_new_anomalies_spawn_radius 15, squared
%define TICK               50         ; the world's tick grid
%define TIMED_RESPAWN      0x47C35000 ; 100000.0f

%define LCG_MUL            0x08088405 ; the game's x = x*08088405h + 1
%define TRY_SALT           0x7A11ED42 ; the tries' stream from the seed
%define PICK_SALT          0x51C4E5A3 ; a try's choice from its draw

; --- entries: patch_026g.py jumps to these offsets --------------------------

tick_hook:                             ; +0
    pushad
    push dword [esp+0x28]              ; the tick's length
    push edi                           ; now
    push dword [esp+0x2C]              ; the world
    call anomalies_tick
    add esp, 12
    popad
    push ecx
    mov eax, 0x51EB851F
    jmp ATOMIC_TICK_ON

    times 0x40 - ($ - $$) db 0xCC

pickup_hook:                           ; +40h, ecx = the core, [esp+4] the container, [esp+8] now
    push edi
    mov edi, ecx
    call timed
    pop edi
    test al, al
    jz .stock
    cvtsi2ss xmm0, dword [ecx+AF_USE]
    movss [ecx+ENERGY], xmm0
    mov byte [ecx+PENDING], 0
    ret 8
.stock:
    push ecx
    push esi
    mov esi, ecx
    mov eax, [esi+AF_USE]
    jmp PICKUP_ON

    times 0x80 - ($ - $$) db 0xCC

; --- the rule ----------------------------------------------------------------

; anomalies_tick(world, now, delta), cdecl. Locals:
%define L_DRAW     0                    ; this try's draw
%define L_ACTIVE   4
%define L_COUNT    8                    ; candidates
%define L_SIZE     12
%define A_WORLD    L_SIZE + 0x14
%define A_NOW      L_SIZE + 0x18
%define A_DELTA    L_SIZE + 0x1C
anomalies_tick:
    push ebp
    push ebx
    push esi
    push edi
    sub esp, L_SIZE
    mov ebp, [esp+A_WORLD]
    ; the match's start: the time rule in process
    mov eax, [ebp+W_RULES]
    cmp eax, [ebp+W_RULES+4]
    je .out
    mov eax, [eax]
    test eax, eax
    jz .out
    cmp dword [eax], TIMELIMIT_VTABLE
    jne .out
    cmp dword [eax+RULE_STATE], 3
    jne .out
    mov ebx, [eax+RULE_START]
    add ebx, DELAY                     ; the first try
    mov eax, [ebp+W_STATIC_INFO]
    test eax, eax
    jz .out
    mov eax, [eax+INFO_SEED]
    xor eax, TRY_SALT
    mov ecx, 4096                      ; tries enough for 34 hours
.next_try:
    cmp ebx, [esp+A_NOW]
    ja .out                            ; the next try is later
    call lcg
    mov edx, [esp+A_NOW]
    sub edx, ebx
    cmp edx, [esp+A_DELTA]
    jb .try                            ; it falls in this tick
    ; the next one: from the tick that made this one, 20 to 40 s on
    mov esi, eax
    lea eax, [ebx+TICK-1]
    xor edx, edx
    mov edi, TICK
    div edi
    imul ebx, eax, TICK
    mov eax, esi
    mov edi, 2 * VARIATION
    mul edi
    lea ebx, [ebx+edx+INTERVAL-VARIATION]
    mov eax, esi
    dec ecx
    jnz .next_try
    jmp .out

.try:
    mov [esp+L_DRAW], eax
    ; the active ones (16 or more: nothing) and the candidates
    xor eax, eax
    mov [esp+L_ACTIVE], eax
    mov [esp+L_COUNT], eax
    mov ebx, [ebp+W_ANOMALIES]
.count:
    test ebx, ebx
    jz .counted
    mov edi, [ebx+NODE_OBJECT]
    call timed_node
    test al, al
    jz .count_next
    cvtsi2ss xmm1, dword [edi+AF_USE]
    movss xmm0, [edi+ENERGY]
    comiss xmm0, xmm1
    jbe .count_free
    inc dword [esp+L_ACTIVE]
.count_free:
    mov esi, edi
    call candidate
    test al, al
    jz .count_next
    inc dword [esp+L_COUNT]
.count_next:
    mov ebx, [ebx+NODE_NEXT]
    jmp .count
.counted:
    cmp dword [esp+L_ACTIVE], MAX_ACTIVE
    jae .out
    mov ecx, [esp+L_COUNT]
    test ecx, ecx
    jz .out
    mov eax, [esp+L_DRAW]
    xor eax, PICK_SALT
    call lcg
    mul ecx
    mov ecx, edx                       ; the chosen one's place among the candidates
    mov ebx, [ebp+W_ANOMALIES]
.find:                                 ; nothing changed since the count: it ends
    mov edi, [ebx+NODE_OBJECT]
    call timed_node
    test al, al
    jz .find_next
    mov esi, edi
    call candidate
    test al, al
    jz .find_next
    test ecx, ecx
    jz .chosen
    dec ecx
.find_next:
    mov ebx, [ebx+NODE_NEXT]
    jmp .find

.chosen:                               ; charging from now; its artefact when the timed states have run
    mov ebx, [esi+STATES]
    mov eax, [ebx]
    cvtsi2ss xmm0, dword [eax+THRESHOLD]
    movss [esi+ENERGY], xmm0
    xor ecx, ecx                       ; seconds
.timed_states:
    cmp ebx, [esi+STATES+4]
    je .timed_end
    mov eax, [ebx]
    mov edx, [eax+THRESHOLD]
    cmp edx, [esi+AF_USE]
    jbe .timed_end                     ; at or below the pick-up level
    add ecx, [eax+ACTIVE_TIME]
    add ebx, 4
    jmp .timed_states
.timed_end:
    imul ecx, ecx, 1000
    add ecx, [esp+A_NOW]
    mov [esi+PENDING_AT], ecx
    mov byte [esi+PENDING], 1
.out:
    add esp, L_SIZE
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

; eax = the next draw of eax
lcg:
    imul eax, eax, LCG_MUL
    inc eax
    ret

; ebx = a node of the anomalies' tick list, edi = its object; al = 1 for a
; timed anomaly. Uses eax.
timed_node:
    cmp dword [ebx+NODE_FUNCTION], GENERIC_TICK
    je timed
    xor eax, eax
    ret

; edi = a generic_anomaly_core; al = 1 when it is timed: energy_enabled, its
; artefacts never respawn by themselves, a state. Uses eax.
timed:
    xor eax, eax
    cmp byte [edi+ENERGY_ENABLED], 0
    je .out
    cmp dword [edi+RESPAWN_TIME], TIMED_RESPAWN
    jl .out                            ; a smaller (or negative) f32 is a smaller i32
    mov eax, [edi+STATES]
    cmp eax, [edi+STATES+4]
    mov eax, 0
    je .out
    inc eax
.out:
    ret

; edi = a core; eax = its place (3 floats) or 0 when it has none. Uses eax.
place:
    mov eax, [edi+CONTAINERS]
    cmp eax, [edi+CONTAINERS+4]
    je .none
    mov eax, [eax]
    test eax, eax
    jz .none
    cmp dword [eax+GEOMETRIES_COUNT], 0
    je .none
    mov eax, [eax+GEOMETRIES]
    test eax, eax
    jz .none
    mov eax, [eax]
    test eax, eax
    jz .none
    mov eax, [eax+GEOMETRY_GHOST]
    test eax, eax
    jz .none
    mov eax, [eax+GHOST_OBJECT]
    test eax, eax
    jz .none
    add eax, OBJECT_ORIGIN
    ret
.none:
    xor eax, eax
    ret

; esi = a timed core; al = 1 when it may be chosen: idle (energy 0), one of
; its own artefacts free (the release's 0x5F7EC0 asks the same), a place,
; and no busy timed anomaly closer than 15 m. ebp = the world. Keeps ebx,
; ecx, esi, edi; uses eax, edx, xmm0, xmm1.
candidate:
    push ebx
    push ecx
    push edi
    test dword [esi+ENERGY], 0x7FFFFFFF
    jnz .no
    mov ecx, [esi+ARTEFACTS_COUNT]
    mov edx, [esi+ARTEFACTS]
.artefact:
    test ecx, ecx
    jz .no                             ; none free: no artefact could land
    mov eax, [edx]
    test eax, eax
    jz .artefact_next
    cmp dword [eax+HOLDER], 0
    je .free
.artefact_next:
    add edx, 4
    dec ecx
    jmp .artefact
.free:
    mov edi, esi
    call place
    test eax, eax
    jz .no
    mov ecx, eax                       ; its place
    mov ebx, [ebp+W_ANOMALIES]
.other:
    test ebx, ebx
    jz .yes
    mov edi, [ebx+NODE_OBJECT]
    call timed_node
    test al, al
    jz .next
    xorps xmm1, xmm1
    movss xmm0, [edi+ENERGY]
    comiss xmm0, xmm1
    jbe .next                          ; idle
    call place
    test eax, eax
    jz .next
    movss xmm0, [eax]
    subss xmm0, [ecx]
    mulss xmm0, xmm0
    movss xmm1, [eax+4]
    subss xmm1, [ecx+4]
    mulss xmm1, xmm1
    addss xmm0, xmm1
    movss xmm1, [eax+8]
    subss xmm1, [ecx+8]
    mulss xmm1, xmm1
    addss xmm0, xmm1
    mov eax, BLOCK_SQUARED
    movd xmm1, eax
    comiss xmm0, xmm1
    jb .no                             ; closer than 15 m
.next:
    mov ebx, [ebx+NODE_NEXT]
    jmp .other
.yes:
    mov eax, 1
    jmp .end
.no:
    xor eax, eax
.end:
    pop edi
    pop ecx
    pop ebx
    ret

    dd CAVE                            ; the address it was assembled for (patch_026g.py checks it)
