; Patch 42, milestone 1: the dynamic anomalies' choice (build 2777). Not in
; the exe yet: milestone 2 calls it from the world's tick and gives it its
; place in the exe. The rules: ../docs/notes/dynamic_anomalies.md;
; client_patch.md section 3.38; the check: check_dynamic_anomalies_026g.py.
;   nasm -f bin -DCAVE=<address> dynamic_anomalies_026g.asm -o <file>
;
; The release's timeout_anomalies_rule_core (0.28a2 tick 0x5F7C00, choice
; 0x5F7EC0) on 0.26g's generic_anomaly_core: a timed core is a generic core
; with energy_enabled and an energy_maximum above 0 (no stock core has one)
; whose states are the release's timeline by energy (charging, damage,
; artefact, after the pick-up, idle). Its energy says where it is:
;   0                       idle: it may be chosen
;   above 0                 busy: it blocks a choice within 15 m
;   above energy_maximum    active: it counts against the 16
; The game's own code does the rest: the states' timers and energy_on_exit
; (anomaly_state::initialize 0x31BC10, execute 0x31BCB0), the artefact
; (spawn_artefact 0x31B6C0), and the pick-up's drop to energy_maximum
; (on_artefact_container_use 0x33BCD0).
;
; try(ctx), cdecl, one argument; eax = the core chosen, or 0. Keeps ebx,
; esi, edi, ebp; uses xmm0 and xmm1. Nothing it reads or keeps is outside
; the world: the try times and the draws come from the static seed and the
; try's number, so a rollback, a resync and a late joiner choose alike.
;   ctx+0   u32 now (ms, the match clock)
;   ctx+4   u32 delta: this tick's length; a try falls in (now-delta, now]
;   ctx+8   u32 start: the match's start (ms), 0xFFFFFFFF before it
;   ctx+12  u32 seed: the static info's id (the cores' seed)
;   ctx+16  u32 count of entries
;   ctx+20  entries: 16 bytes each, in the world's tick order:
;           +0 generic_anomaly_core*, +4 f32 x, +8 f32 y, +12 f32 z
;           (the place: its artefact container's position)
BITS 32
ORG CAVE

; generic_anomaly_core (load 0x33BD50, constructor 0x31ADF0)
%define PENDING          0x138     ; u8: an artefact is due
%define PENDING_AT       0x13C     ; u32: when
%define ENERGY_ENABLED   0x14C     ; u8
%define ENERGY           0x150     ; f32
%define ENERGY_MAXIMUM   0x154     ; f32
%define STATES_BEGIN     0x180     ; anomaly_state*[], highest threshold first
%define STATES_END       0x184
%define ARTEFACTS        0x1B0     ; artefact_base*[] (spawn_artefact 0x31B6C0)
%define ARTEFACTS_COUNT  0x1B4
%define HOLDER           0x280     ; an artefact's: 0 when free (spawn_artefact's test)
%define THRESHOLD        0x08      ; anomaly_state: u32 energy_threshold

; the release's anomalies rule (0.28a2 to 0.31a2, every map)
%define DELAY            30000     ; spawn_anomalies_delay_s
%define INTERVAL         30000     ; try_spawn_anomaly_interval_s
%define VARIATION        10000     ; try_spawn_anomaly_variation_s
%define MAX_ACTIVE       16        ; max_active_anomalies_count
%define BLOCK_SQUARED    0x43610000 ; 225.0f: block_new_anomalies_spawn_radius 15, squared
%define ARTEFACT_AFTER   24000     ; charging_state_timeout 20 + damage_state_timeout 4

%define LCG_MUL          0x08088405 ; the game's x = x*08088405h + 1
%define TRY_MIX          0x9E3779B9 ; the try's number into the seed

%define C_NOW    0
%define C_DELTA  4
%define C_START  8
%define C_SEED   12
%define C_COUNT  16
%define C_TABLE  20

try:
    push ebp
    push ebx
    push esi
    push edi
    sub esp, 8                     ; [esp] the draw, [esp+4] a loop's count
    mov ebp, [esp+0x1C]            ; ctx
    xor eax, eax
    mov ecx, [ebp+C_START]
    cmp ecx, -1
    je .done                       ; the match has not started
    add ecx, DELAY - VARIATION     ; the first try's earliest time
    mov edx, [ebp+C_NOW]
    sub edx, ecx
    jb .done                       ; before it
    ; Try k is at start + DELAY + k*INTERVAL + [-VARIATION, VARIATION): only
    ; k = (now - (start + DELAY - VARIATION)) / INTERVAL can be in this tick
    ; (2*VARIATION + delta < INTERVAL).
    mov eax, edx
    xor edx, edx
    mov ecx, INTERVAL
    div ecx
    mov esi, eax                   ; k
    imul eax, eax, TRY_MIX
    xor eax, [ebp+C_SEED]
    call lcg
    mov [esp], eax
    mov ecx, 2 * VARIATION
    mul ecx                        ; edx = [0, 2*VARIATION)
    imul eax, esi, INTERVAL
    add eax, edx
    add eax, [ebp+C_START]
    add eax, DELAY - VARIATION     ; the try's time
    mov edx, [ebp+C_NOW]
    sub edx, eax
    jb .none                       ; later than now
    cmp edx, [ebp+C_DELTA]
    jae .none                      ; at or before the last tick

    ; the active ones: 16 or more, nothing
    xor ebx, ebx
    mov esi, [ebp+C_TABLE]
    mov eax, [ebp+C_COUNT]
    mov [esp+4], eax
.count:
    cmp dword [esp+4], 0
    je .counted
    mov edi, [esi]
    call timed
    test al, al
    jz .count_next
    movss xmm0, [edi+ENERGY]
    comiss xmm0, [edi+ENERGY_MAXIMUM]
    jbe .count_next
    inc ebx
.count_next:
    add esi, 16
    dec dword [esp+4]
    jmp .count
.counted:
    cmp ebx, MAX_ACTIVE
    jae .none

    ; the candidates, counted
    xor ebx, ebx
    mov esi, [ebp+C_TABLE]
    mov eax, [ebp+C_COUNT]
    mov [esp+4], eax
.first:
    cmp dword [esp+4], 0
    je .drawn
    call candidate
    test al, al
    jz .first_next
    inc ebx
.first_next:
    add esi, 16
    dec dword [esp+4]
    jmp .first
.drawn:
    test ebx, ebx
    jz .none
    mov eax, [esp]
    call lcg
    mul ebx
    mov ebx, edx                   ; the chosen one's place among the candidates
    mov esi, [ebp+C_TABLE]
.second:                           ; nothing changed since the count: it ends
    call candidate
    test al, al
    jz .second_next
    test ebx, ebx
    jz .chosen
    dec ebx
.second_next:
    add esi, 16
    jmp .second

.chosen:                           ; charging from now; its artefact in 24 s
    mov edi, [esi]
    mov eax, [edi+STATES_BEGIN]
    mov eax, [eax]
    cvtsi2ss xmm0, dword [eax+THRESHOLD]
    movss [edi+ENERGY], xmm0
    mov byte [edi+PENDING], 1
    mov eax, [ebp+C_NOW]
    add eax, ARTEFACT_AFTER
    mov [edi+PENDING_AT], eax
    mov eax, edi
    jmp .done
.none:
    xor eax, eax
.done:
    add esp, 8
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

; edi = a core; al = 1 for a timed core (energy_enabled, energy_maximum
; above 0, a state). Uses eax.
timed:
    xor eax, eax
    cmp byte [edi+ENERGY_ENABLED], 0
    je .out
    cmp dword [edi+ENERGY_MAXIMUM], 0
    jle .out                       ; a positive f32 is a positive i32
    mov eax, [edi+STATES_BEGIN]
    cmp eax, [edi+STATES_END]
    mov eax, 0
    je .out
    inc eax
.out:
    ret

; esi = an entry; al = 1 when its core may be chosen: timed, idle (energy
; 0), one of its artefacts free (the release's 0x5F7EC0 asks the same), and
; no busy timed core closer than 15 m. Uses eax, ecx, edx, edi, xmm0, xmm1.
candidate:
    mov edi, [esi]
    call timed
    test al, al
    jz .no
    test dword [edi+ENERGY], 0x7FFFFFFF
    jnz .no
    mov ecx, [edi+ARTEFACTS_COUNT]
    mov edx, [edi+ARTEFACTS]
.artefact:
    test ecx, ecx
    jz .no                         ; none free: no artefact could land
    mov eax, [edx]
    cmp dword [eax+HOLDER], 0
    je .free
    add edx, 4
    dec ecx
    jmp .artefact
.free:
    mov ecx, [ebp+C_COUNT]
    mov edx, [ebp+C_TABLE]
.other:
    test ecx, ecx
    jz .yes
    mov edi, [edx]
    call timed
    test al, al
    jz .next
    xorps xmm1, xmm1
    movss xmm0, [edi+ENERGY]
    comiss xmm0, xmm1
    jbe .next                      ; idle
    movss xmm0, [edx+4]
    subss xmm0, [esi+4]
    mulss xmm0, xmm0
    movss xmm1, [edx+8]
    subss xmm1, [esi+8]
    mulss xmm1, xmm1
    addss xmm0, xmm1
    movss xmm1, [edx+12]
    subss xmm1, [esi+12]
    mulss xmm1, xmm1
    addss xmm0, xmm1
    mov eax, BLOCK_SQUARED
    movd xmm1, eax
    comiss xmm0, xmm1
    jb .no                         ; closer than 15 m
.next:
    add edx, 16
    dec ecx
    jmp .other
.yes:
    mov eax, 1
    ret
.no:
    xor eax, eax
    ret
