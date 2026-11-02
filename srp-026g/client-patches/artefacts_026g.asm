; Patch 54: the artefacts (build 2777): Spring's aim time and stopping power,
; Lifebone's heal, one press fires every artefact of its kind, Onyx fired
; together 1 s longer each and reducing a hit once (2026-10-25). Its numbers
; are in the artefacts' configs (release/content.tsv artefacts.tsv); this
; is what the stock code could not read or do.
; The arena's block artefacts.code (link_026g.py links it; patch_026g.py's
; ARTEFACTS_HOOKS jump to the six entries by their names).
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
global artefacts_loader, artefacts_link, artefacts_unlink, artefacts_lifebone, artefacts_fire
global artefacts_stagger, artefacts_onyx, artefacts_popup
section .text

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
%define ACTION_BACK 0x31330A       ; artefact_base::action after its state test: `jne 0x313344`
%define EFFECT 0x313350            ; artefact_base's activation effect (the artefact, the time), `ret 8`
%define ONYX_ENABLE 0x3128C0       ; Onyx's enable_active (vtable +80h): its active time +F14h, ms
%define ONYX_TIME_LEFT 0xF14       ; an Onyx's active time left, ms (its tick 0x3127E0 counts it down)
%define ONYX_EXTRA_MS 1000         ; each Onyx fired with others: 1 s more for all of them
%define ONYX_HIT_BACK 0x312C0A     ; Onyx's hit callback after its state test: `mov eax,[esp+8]`
%define POPUP_SHOW 0x25F9E8        ; Onyx's client update: `push 14h`, then set_parameterized_message
%define POPUP_SKIP 0x25F9F8        ; ... after it: the state seen, the function's end
%define AIM_TIME 0x31              ; the aim_time modifier's index (49)
%define STOPPING 0x2A              ; list 42: the stagger's multiplier
%define AIM_NODE 0x3B8             ; the active's aim_time node in a Spring
%define STOP_NODE 0x3C8            ; the active's stopping power node
%define FIRST_SLOT 13              ; the inventory's artefact slots, 13..18
%define LAST_SLOT 18

artefacts_loader:
    jmp near loader                ; +0  from 0x33AE13
artefacts_link:
    jmp near link_active           ; +5  from 0x311BC0
artefacts_unlink:
    jmp near unlink_active         ; +10 from 0x311DA0
artefacts_lifebone:
    jmp near lifebone_active       ; +15 from 0x3122AA
artefacts_fire:
    jmp near fire_kind             ; +20 from 0x313300
artefacts_stagger:
    jmp near stagger               ; +25 from 0x2DBD89
artefacts_onyx:
    jmp near onyx_hit              ; +30 from 0x312C03
artefacts_popup:
    jmp near onyx_popup            ; +35 from 0x25F9DD

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
; Spring: its two nodes go in, then the function links its own three.
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

; artefact_base::action 0x313300 (thiscall: pressed, the time; `ret 8`),
; its first ten bytes `push esi; mov esi,ecx; cmp dword [esi+280h],3`. A
; press (key-down) activates the one pressed and every other artefact of its
; class (vtable) and kind 1 in the inventory's artefact slots that is ready,
; each as action activates one, and returns: so a press of one cooling or
; active still fires the ready ones of its kind. A key-up goes on as the
; original (it does nothing). Onyx (its enable_active 0x3128C0): each Onyx
; fired adds ONYX_EXTRA_MS to every fired one's active time (+F14h), so n
; fired together are all active 3 s + (n - 1) s and end together.
fire_kind:
    cmp byte [esp+4], 0
    je .original
    pushad
    mov esi, ecx                   ; the one pressed
    mov edi, [esp+0x28]            ; the time (pushad 20h, the return, pressed)
    xor edx, edx                   ; how many are ready
    mov ecx, esi
    call ready
    adc edx, 0
    mov eax, [esi+0x250]           ; the inventory
    test eax, eax
    jz .counted
    mov ebp, FIRST_SLOT
.count:
    mov ecx, [eax+0x10C+ebp*4]
    call kin
    jnc .count_next
    call ready
    adc edx, 0
.count_next:
    inc ebp
    cmp ebp, LAST_SLOT
    jbe .count
.counted:
    xor ebx, ebx                   ; the time each fired one gets on top
    dec edx
    jle .extra
    mov eax, [esi]
    cmp dword [eax+0x80], ONYX_ENABLE
    jne .extra
    imul ebx, edx, ONYX_EXTRA_MS
.extra:
    mov ecx, esi
    call activate
    mov ebp, FIRST_SLOT
.slot:
    mov eax, [esi+0x250]
    test eax, eax
    jz .done
    mov ecx, [eax+0x10C+ebp*4]
    call kin
    jnc .next
    call activate
.next:
    inc ebp
    cmp ebp, LAST_SLOT
    jbe .slot
.done:
    popad
    ret 8
.original:
    push esi
    mov esi, ecx
    cmp dword [esi+0x280], 3
    jmp ACTION_BACK

; ecx = a slot's artefact (or 0), esi = the one pressed: CF = 1 when it is
; another of its class (vtable) and kind 1. Keeps every register.
kin:
    test ecx, ecx
    jz .no
    cmp ecx, esi
    je .no
    push eax
    mov eax, [ecx]
    cmp eax, [esi]
    pop eax
    jne .no
    cmp dword [ecx+0x24C], 1
    jne .no
    stc
    ret
.no:
    clc
    ret

; ecx = an artefact: CF = 1 when it is ready, in its passive (state 3) with
; no cooldown (+28Ch). Keeps every register.
ready:
    cmp dword [ecx+0x280], 3
    jne .no
    cmp dword [ecx+0x28C], 0
    jne .no
    stc
    ret
.no:
    clc
    ret

; ecx = an artefact, edi = the time, ebx = the time on top (ms): if it is
; ready, activated as action does (0x31331C..0x313339): its enable_active
; (vtable +80h), its effect, the time (+2A0h), state 4; then ebx more on its
; active time (+F14h, an Onyx's; ebx is 0 for any other kind). Keeps ebx,
; esi, edi and ebp.
activate:
    call ready
    jnc .no
    push esi
    mov esi, ecx
    mov eax, [esi]
    push edi
    call [eax+0x80]
    push edi
    push esi
    call EFFECT
    mov [esi+0x2A0], edi
    mov dword [esi+0x280], 4
    test ebx, ebx
    jz .own
    add [esi+ONYX_TIME_LEFT], ebx
.own:
    pop esi
.no:
    ret

; Onyx's hit callback 0x312BF0 (thiscall: the hit, &damage, &armour
; piercing; `ret 0Ch`) at 0x312C03 `cmp dword [ecx+280h],3`, after its hit
; type test: ecx = the Onyx. In its passive (state 3) it goes on as the
; original. Otherwise (active) only the first active Onyx of the slots
; (13..18, its class) takes its active's share of the hit; any other active
; Onyx leaves the hit as it is, so n active Onyx reduce it once. An Onyx in
; no slot goes on as the original. Keeps every register the original does.
onyx_hit:
    cmp dword [ecx+0x280], 3
    je .back                       ; its passive
    push esi
    push ebx
    mov eax, [ecx+0x250]           ; the inventory
    test eax, eax
    jz .apply
    mov edx, FIRST_SLOT
.slot:
    mov esi, [eax+0x10C+edx*4]
    test esi, esi
    jz .next
    cmp esi, ecx
    je .apply                      ; it is the first active one
    mov ebx, [esi]
    cmp ebx, [ecx]
    jne .next
    cmp dword [esi+0x280], 4
    je .skip                       ; an Onyx before it is active
.next:
    inc edx
    cmp edx, LAST_SLOT
    jbe .slot
.apply:
    pop ebx
    pop esi
    cmp dword [ecx+0x280], 3       ; the flags the original's jne reads
.back:
    jmp ONYX_HIT_BACK
.skip:
    pop ebx
    pop esi
    ret 0xC

; Onyx's client update 0x25F640 (ecx = ebx = the Onyx +F20h) when it sees
; the state turn 4 for the local player's Onyx, at 0x25F9DD `mov ecx,
; [ebx-0C30h] / push 0BB8h`: the activation hint (+2F0h) shown by
; set_parameterized_message 0x2375C0 (the text, 21h, 14h, 3000 ms). Now
; its time on screen is the active's time left (+F14h, ms), which the hud
; writes for the hint's %s in seconds; and of the Onyx one press fired
; (state 4, the same activation time +2A0h) only the first of the slots
; shows it, so a press shows it once. An Onyx in no slot shows its own.
; esi (the game) is kept; eax, ecx, edx and edi are free there.
onyx_popup:
    push esi
    lea edi, [ebx-0xF20]           ; the Onyx
    mov eax, [edi+0x250]           ; the inventory
    test eax, eax
    jz .show
    mov edx, FIRST_SLOT
.slot:
    mov ecx, [eax+0x10C+edx*4]
    test ecx, ecx
    jz .next
    cmp ecx, edi
    je .show                       ; it is the first of its press
    mov esi, [ecx]
    cmp esi, [edi]
    jne .next
    cmp dword [ecx+0x280], 4
    jne .next
    mov esi, [ecx+0x2A0]
    cmp esi, [edi+0x2A0]
    je .skip                       ; an Onyx before it, fired by the same press
.next:
    inc edx
    cmp edx, LAST_SLOT
    jbe .slot
.show:
    pop esi
    mov ecx, [ebx-0xC30]           ; the activation hint
    push dword [ebx-0xC]           ; its time on screen: the active's time left, ms
    jmp POPUP_SHOW
.skip:
    pop esi
    jmp POPUP_SKIP

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
