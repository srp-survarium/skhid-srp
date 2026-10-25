; Patch 73 (build 2777): the kill marker and the hit marker show only what
; every game agrees on. The arena's blocks confirmed_markers.code and .state
; (link_026g.py links them; patch_026g.py's CONFIRMED_MARKERS_HOOKS call the
; three entries by their names, CONFIRMED_MARKERS_SITES rewrites two sites in
; place). client_patch.md section 3.58; the rules:
; ../docs/notes/confirmed-markers.md.
;
; A death and a hit make world events in the simulation (generate_killed_event
; 0x2DA170: type 0; base_player's hit 0x2D9F40: type 2, [+0] the attacker's
; place, [+1] the victim's, +4 the bullet, +0Ch the attacker's position), kept
; by time in the core's list (+12740h; push 0x2D0A60). A rollback drops every
; event after the world it loads (move 0x2D2E40 -> 0x2D3700) and the
; re-simulation makes them again; a world being loaded makes none (+18FE0h,
; push frees the node). game_world::tick plays the list twice a frame, from
; the time each player played last (0x1F7FCF, 0x1F7FF6): an event is
; "confirmed" when its time is at or before the time given, and the player
; then keeps its time; a type 2 event (+28h set) is played every frame too.
; - the hud's (0x1FB660, its handler 0x23B5F0: the kill feed's row) is given
;   the event horizon, now (+18FC4h) - 500 (+18FB4h): its rows are final;
; - the kill marker's (0x1FB6C0, its handler 0x233770 and the marker
;   0x23CF20) was given now - [ui+364h], which nothing writes (0): a kill the
;   game only predicted showed its marker at once and kept it when a
;   rollback took the kill back. CONFIRMED_MARKERS_SITES gives it -1, the
;   event horizon, as the hud's;
; - the hit marker was the shooter's own inflicted_damage_collector (player
;   +0F534h: the victim, the time, the damage summed for that victim, 0 to 1;
;   generate_hit_event 0x1F3C70 sums it), drawn by the player's draw
;   (0x247467 -> draw_damage_collector 0x238230) from the world of the
;   moment: a rollback put an older sum back, or another, and drew it again.
;   That draw is gone (CONFIRMED_MARKERS_SITES); each hit's event carries
;   the sum after it (+8, hit_event_push and hit_event_amount) and the
;   kill marker's player draws it when the event is confirmed (marker_event).
;
; - marker_event, jumped to at 0x233771 (the handler's type test,
;   `cmp dword [edi+18h],0 / push ebx / mov ebx,[esp+0Ch] / push esi /
;   jnz 0x2337FA`; edi the event): the same pushes, a kill to 0x23377D as
;   before; a confirmed hit of the local player on someone else drawn with
;   the event's victim, time and sum; then 0x2337FA as before;
; - hit_event_push, called at 0x2DA0B1 in place of push 0x2D0A60 (eax the
;   event, edx the core): the event kept for hit_event_amount unless a world
;   is being loaded, then the push;
; - hit_event_amount, jumped to at 0x1F3D7B (generate_hit_event's end,
;   `pop edi / pop esi / pop ebp / pop ebx / ret 1Ch`): for a bullet's hit
;   (ebp not 0; esi the attacker's collector) the event kept, if it is this
;   hit's (its victim bl, its time the first argument), gets the sum (+8,
;   the event's first unused field); then the five instructions.
; Nothing a match computes is read or written but the event's +8, which only
; marker_event reads.
BITS 32
%include "engine_026g.inc"
global marker_event, hit_event_push, hit_event_amount, hit_event
section .text

%define PUSH_WORLD_EVENT 0x2D0A60  ; eax the event, edx the core; frees the event while a world loads
%define LOADING 0x18FE0            ; the core's byte: deserialize is loading a world (push frees)
%define KILL_BACK 0x23377D         ; the handler's kill case, after the type test
%define OTHER_BACK 0x2337FA        ; the handler's end: a confirmed event's time kept
%define TYPE 0x18                  ; the event's type: 0 a kill, 2 a hit
%define HIT 2
%define ATTACKER 0                 ; a hit event's attacker's place (byte)
%define VICTIM 1                   ; its victim's place (byte)
%define SUM 8                      ; ours: the attacker's collected damage after the hit (float; 0 none)
%define TIME 0x24                  ; the event's time
%define CONFIRMED 0x14             ; the handler's second argument after its three pushes: the event is confirmed
%define OWNER_WORLD 4              ; the marker player's owner (ui+368h): +4 the game world
%define GAME 0x9C                  ; the game world's game
%define LOCAL_HOLDER 0x53AC        ; the game's local player holder: +4 the player or 0
%define PLACE 0x114                ; a player's place
%define UI 0x1FC                   ; the game world's game_world_ui
%define SHOWN 0x34C                ; draw_damage_collector's last record (its victim byte first)
%define NONE 0xFF                  ; no place
%define COLLECTED 8                ; the collector's sum
%define HIT_TIME 0x18              ; hit_event_amount's frame after its push: generate_hit_event's first argument

; The kill marker's event handler at 0x233771 (after its `push ecx`): edi the
; event; [esp+8] the owner, [esp+0Ch] confirmed.
marker_event:
    push ebx
    mov ebx, [esp+0Ch]             ; the owner, as the original
    push esi
    mov eax, [edi+TYPE]
    test eax, eax
    jz KILL_BACK                   ; a kill: the original's case
    cmp eax, HIT
    jne .back
    cmp byte [esp+CONFIRMED], 0
    je .back                       ; not confirmed: shown when it is, or never
    mov al, [edi+ATTACKER]
    cmp al, [edi+VICTIM]
    je .back                       ; the attacker hit himself
    mov ecx, [ebx+OWNER_WORLD]
    mov edx, [ecx+GAME]
    mov edx, [edx+LOCAL_HOLDER]
    mov edx, [edx+4]
    test edx, edx
    jz .back                       ; no local player
    cmp al, [edx+PLACE]
    jne .back                      ; someone else's hit
    lea edx, [ecx+UI]
    mov byte [edx+SHOWN], NONE     ; each confirmed hit shows (the draw skips a record like the last)
    sub esp, 12                    ; the record: the victim, the time, the sum
    movzx eax, byte [edi+VICTIM]
    mov [esp], eax
    mov eax, [edi+TIME]
    mov [esp+4], eax
    mov eax, [edi+SUM]
    mov [esp+8], eax
    mov eax, esp
    push edx
    push eax
    call engine_draw_damage_collector
    add esp, 8 + 12
.back:
    jmp OTHER_BACK

; base_player's hit at 0x2DA0B1: eax the hit's event, edx the core.
hit_event_push:
    cmp byte [edx+LOADING], 0
    jne .push                      ; freed by the push: nothing to keep
    mov [hit_event], eax
.push:
    jmp PUSH_WORLD_EVENT

; generate_hit_event's end at 0x1F3D7B: ebp the bullet or 0, esi the
; attacker's collector when ebp is not 0, bl the victim's place.
hit_event_amount:
    push eax
    mov eax, [hit_event]
    mov dword [hit_event], 0
    test eax, eax
    jz .done                       ; no event made
    test ebp, ebp
    jz .done                       ; no bullet: no sum
    cmp bl, [eax+VICTIM]
    jne .done                      ; not this hit's event
    push ecx
    mov ecx, [esp+4+HIT_TIME]
    cmp ecx, [eax+TIME]
    jne .other
    mov ecx, [esi+COLLECTED]
    mov [eax+SUM], ecx
.other:
    pop ecx
.done:
    pop eax
    pop edi
    pop esi
    pop ebp
    pop ebx
    ret 0x1C

section .bss
hit_event: resd 1                  ; the event the last hit pushed, until generate_hit_event's end
