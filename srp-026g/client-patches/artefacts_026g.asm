; Patch 54: the artefacts (build 2777): Spring's aim time and stopping power,
; Lifebone's heal, one press fires every artefact of its kind. Its numbers
; are in the artefacts' configs (release/content.tsv artefacts-1.json); this
; is what the stock code could not read or do.
;   nasm -f bin -DCAVE=0xA42C700 artefacts_026g.asm -o artefacts_026g.section.bin
; client_patch.md section 3.51; the rules: ../docs/notes/artefacts.md.
;
; A Spring (artefact_spring_core, type 6) keeps its config at +340h, filled
; by its loader 0x33ACF0 (esi = +340h, ebx = the config's `data`): five
; character modifier nodes {prev, next, index, value} and the active
; duration (+390h). The passive's are +340h (4 movement_speed) and +350h
; (5 max_carried_weight, 0 in retail); the active's +360h (4), +370h (3
; stamina_regeneration_speed), +380h (19 stamina_spending_speed). A node is
; linked into the holder's list of its index: holder+6B8h + kit (+115h) *
; 704h + index * 14h + 204h, {count, head, tail, ?, sorted flag +10h}; the
; getter 0x2D5AE0 sums the values of a list's nodes. The passive's nodes
; are linked at 0x311AD0 and unlinked at 0x311D10, the active's at 0x311BC0
; (eax = the artefact) and 0x311DA0 (stdcall, the artefact).
;
; The patch: the passive's +350h node becomes aim_time (49) with the value of
; `passive/aim_time_mod`; two more nodes after the object's own fields (both
; sizes, 0x25E7A8 and 0x2EEA74, grow to 3D8h) are the active's: +3B8h
; aim_time with `active/aim_time_mod`, +3C8h list 42 with
; `active/stopping_power_mod`, linked and unlinked with the active's three.
; List 42 (artefact_synergy, which no code of 0.26g reads and no config
; sets) is the stagger's: a hit's slow is times max(0, 1 + its sum).
BITS 32
ORG CAVE

%define CONFIG_CHILD 0xF6B10       ; eax = a config value, edi = a key: eax = its child
%define ACTIVE_KEY 0xA1C230        ; "active", the loader's own
%define PASSIVE_KEY 0xA1C1FC       ; "passive"
%define LOADER_BACK 0x33AE1A       ; `movss [esi+4Ch],xmm0`
%define LINK_BACK 0x311BCA         ; the active's link after its first instructions
%define UNLINK_BACK 0x311DA5       ; the active's unlink after `push ebx; mov ebx,[esp+8]`
%define UNLINK_NODE 0x2D5D00       ; eax = a node, esi = its list
%define GETTER 0x2D5AE0            ; eax = a kit's modifiers, esi = an index; (B, k): xmm0, `ret 8`
%define STAGGER_BACK 0x2DBD91      ; `comiss xmm2,xmm0`
%define LIFEBONE_BACK 0x3122B0     ; Lifebone's enable_active: its holder's modifiers, the timer
%define FIND_POOL 0x2E58B0         ; eax = the damage model's pools, ecx = &name: eax = the pool or 0
%define INCREASE_HEALTH 0x32A420   ; esi = a pool; (time, amount), `ret 8`
%define ACTION_BACK 0x313343       ; artefact_base::action: `pop edi; pop esi; ret 8`
%define AIM_TIME 0x31              ; the aim_time modifier's index (49)
%define STOPPING 0x2A              ; list 42: the stagger's multiplier
%define AIM_NODE 0x3B8             ; the active's aim_time node in a Spring
%define STOP_NODE 0x3C8            ; the active's stopping power node
%define FIRST_SLOT 13              ; the inventory's artefact slots, 13..18
%define LAST_SLOT 18

    jmp near loader                ; +0  from 0x33AE13
    jmp near link_active           ; +5  from 0x311BC0
    jmp near unlink_active         ; +10 from 0x311DA0
    jmp near lifebone_active       ; +15 from 0x3122AA
    jmp near fire_kind             ; +20 from 0x313339
    jmp near stagger               ; +25 from 0x2DBD89

; The loader at 0x33AE13 `mov dword [esi+48h],13h` (the active's third
; node): ebx = `data`, esi = the config; eax, edi and xmm0 are the next
; instructions' (the third node's value, the duration's lookup).
loader:
    mov dword [esi+0x48], 0x13
    pushad
    sub esp, 16
    movups [esp], xmm0
    mov edi, PASSIVE_KEY
    mov ebp, aim_key
    call value                     ; xmm0 = passive/aim_time_mod
    mov dword [esi+0x18], AIM_TIME
    movss [esi+0x1C], xmm0
    mov edi, ACTIVE_KEY
    call value                     ; xmm0 = active/aim_time_mod
    lea edx, [esi+AIM_NODE-0x340]
    mov ecx, AIM_TIME
    call node
    mov edi, ACTIVE_KEY
    mov ebp, stopping_key
    call value                     ; xmm0 = active/stopping_power_mod
    lea edx, [esi+STOP_NODE-0x340]
    mov ecx, STOPPING
    call node
    movups xmm0, [esp]
    add esp, 16
    popad
    jmp LOADER_BACK

; xmm0 = data/<edi>/<ebp> as a float (an int is converted, as the loader
; does). ebx = `data`; ebp and esi kept.
value:
    mov eax, ebx
    call CONFIG_CHILD
    mov edi, ebp
    call CONFIG_CHILD
    cmp word [eax+0x14], 2
    jne .int
    movss xmm0, [eax]
    ret
.int:
    cvtsi2ss xmm0, [eax]
    ret

; edx = a node: {0, 0, ecx, xmm0}.
node:
    xor eax, eax
    mov [edx], eax
    mov [edx+4], eax
    mov [edx+8], ecx
    movss [edx+0xC], xmm0
    ret

; eax = a Spring: ebp = its holder's kit's lists (+204h). Keeps the others
; but eax, ecx and edx.
lists:
    mov eax, [eax+0x250]           ; the inventory
    mov ecx, [eax+0x174]
    mov edx, [ecx]
    call [edx+0xC]                 ; the holder (a player)
    movzx ebp, byte [eax+0x115]
    imul ebp, ebp, 0x704
    add ebp, [eax+0x6B8]
    add ebp, 0x204
    ret

; edx = a node, ebp = the lists: the node goes last in the list of its
; index, as 0x311BC0 links its own.
link:
    mov ecx, [edx+8]
    imul ecx, ecx, 0x14
    add ecx, ebp
    inc dword [ecx]
    mov edi, [ecx+8]
    mov dword [edx+4], 0
    mov [edx], edi
    cmp dword [ecx+4], 0
    jne .tail
    mov [ecx+4], edx
    jmp .linked
.tail:
    mov [edi+4], edx
.linked:
    mov [ecx+8], edx
    test edi, edi
    jz .sorted
    movss xmm0, [edi+0xC]
    comiss xmm0, [edx+0xC]
    jbe .sorted
    mov byte [ecx+0x10], 0
.sorted:
    ret

; eax = a node, ebp = the lists: the node out of the list of its index.
unlink:
    mov esi, [eax+8]
    imul esi, esi, 0x14
    add esi, ebp
    jmp UNLINK_NODE

; 0x311BC0 `push ebx; push esi; mov esi,eax; mov eax,[esi+250h]`, eax = the
; Spring: its two nodes go in after the original's three.
link_active:
    pushad
    mov ebx, eax
    call lists
    lea edx, [ebx+AIM_NODE]
    call link
    lea edx, [ebx+STOP_NODE]
    call link
    popad
    push ebx
    push esi
    mov esi, eax
    mov eax, [esi+0x250]
    jmp LINK_BACK

; 0x311DA0 `push ebx; mov ebx,[esp+8]`, [esp+4] = the Spring: its two
; nodes out before the original's three.
unlink_active:
    pushad
    mov ebx, [esp+0x24]
    mov eax, ebx
    call lists
    lea eax, [ebx+AIM_NODE]
    call unlink
    lea eax, [ebx+STOP_NODE]
    call unlink
    popad
    push ebx
    mov ebx, [esp+8]
    jmp UNLINK_BACK

; The player's stagger 0x2DBCD0 at 0x2DBD89 `movss xmm2,[esi+31Ch]` (both
; ways there, the hits' loop or none): esi = the player, xmm0 and the local
; [esp+14h] = the sum of the recent hits' stopping power (the min after it
; reads the local), xmm1 0. The sum becomes M42(sum): times max(0, 1 + list
; 42), so an active Spring's -1 takes the slow away.
stagger:
    push esi
    sub esp, 8
    mov dword [esp+4], 0x3F800000  ; k = 1.0
    movss [esp], xmm0              ; B = the sum
    movzx eax, byte [esi+0x115]
    imul eax, eax, 0x704
    add eax, [esi+0x6B8]
    add eax, 0xF4                  ; the kit's modifiers
    mov esi, STOPPING
    call GETTER
    pop esi
    movss [esp+0x14], xmm0
    xorps xmm1, xmm1
    movss xmm2, [esi+0x31C]
    jmp STAGGER_BACK

; Lifebone's enable_active 0x312250 after its remove_affects, at 0x3122AA
; `mov ecx,[edi+250h]` (both ways there, the loop or none, have its four
; pushes): edi = the Lifebone, [esp+14h] = the time (later in the function
; that slot is scratch). Each pool of `heals` is filled to its maximum, as a
; medkit heals a pool (0x33D9C3): increase_health(time, Hmax - H).
lifebone_active:
    pushad
    mov eax, [edi+0x250]
    mov ecx, [eax+0x174]
    mov edx, [ecx]
    call [edx+8]                   ; the damage model's owner, as at 0x312267
    mov eax, [eax]
    lea ebp, [eax+0x108]           ; its pools
    mov ebx, heals
.heal:
    mov eax, ebp
    mov ecx, ebx                   ; &name
    call FIND_POOL
    test eax, eax
    jz .next
    mov esi, eax
    movss xmm0, [esi+0x9C]         ; Hmax
    subss xmm0, [esi+0xA0]         ; - H
    xorps xmm1, xmm1
    comiss xmm0, xmm1
    jbe .next
    sub esp, 4
    movss [esp], xmm0
    push dword [esp+0x38]          ; the time (pushad 20h + 14h, the amount's 4)
    call INCREASE_HEALTH
.next:
    add ebx, 4
    cmp ebx, heals_end
    jb .heal
    popad
    mov ecx, [edi+0x250]
    jmp LIFEBONE_BACK

; artefact_base::action 0x313300 has activated this one (state 3, pressed,
; no cooldown; enable_active, its effect, the time): at 0x313339
; `mov dword [esi+280h],4`, esi = the artefact, edi = the time. Every other
; artefact of the same class in the inventory's artefact slots is pressed
; too, through its own action (vtable +2Ch, as inventory::action presses
; an item of kind 1): those in their passive with no cooldown are activated;
; the others do nothing. This one is active by then, so their presses of it
; do nothing.
fire_kind:
    mov dword [esi+0x280], 4
    pushad
    mov ebx, [esi+0x250]           ; the inventory
    test ebx, ebx
    jz .done
    mov ebp, FIRST_SLOT
.slot:
    mov ecx, [ebx+0x10C+ebp*4]
    test ecx, ecx
    jz .next
    cmp ecx, esi
    je .next
    mov eax, [ecx]
    cmp eax, [esi]
    jne .next
    cmp dword [ecx+0x24C], 1
    jne .next
    push edi                       ; the time
    push 1                         ; pressed
    call [eax+0x2C]
.next:
    inc ebp
    cmp ebp, LAST_SLOT
    jbe .slot
.done:
    popad
    jmp ACTION_BACK

aim_key:
    db "aim_time_mod", 0
stopping_key:
    db "stopping_power_mod", 0
pain_name:
    db "pain", 0
body_name:
    db "body", 0
head_name:
    db "head", 0
; The pools Lifebone's active fills.
heals:
    dd pain_name, body_name, head_name
heals_end:
