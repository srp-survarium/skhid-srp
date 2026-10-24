; The Sponge (exe patch 59): the hooks of artefact type 12, in front of
; the C code of sponge_a_026g.c in the arena's block sponge.code
; (link_026g.py links it; patch_026g.py's SPONGE_HOOKS jump to the eight
; entries by their names). client_patch.md section 3.52.
;
;   sponge_construct  the artefact cook's construct 0x25E660 at its type
;                     switch (0x25E68A `cmp eax,5 / ja`): type 12 (eax 9) is ours
;   sponge_size       its size 0x25E770 at 0x25E79A (`cmp eax,5 / ja short`)
;   sponge_query      its query of the model 0x25E800 (the entry, `push ebp /
;                     mov ebp,esp / and esp,-8`): a sponge's is ours
;   sponge_predicate  the throw state's way in 0x2E4090 at 0x2E40C6 (the
;                     item's vtable +64h, `call 0x30A090`: a null deref for any
;                     item that is not a grenade set): a sponge not in the air
;   sponge_ready      the throw state's is_ready 0x33A650 at 0x33A677 (its
;                     grenade's exploded flag): a sponge's zone on
;   sponge_throw      the throw 0x33A770 at 0x33A7BB (`call 0x30A250`): ours
;   sponge_pull       the pull substate's pin 0x3103A0 at 0x3103C9 (`call
;                     0x30A120`): ours
;   sponge_zone_hit   the damage zone's hit loop 0x319AF0 at 0x319C8B (the
;                     receiver's vt+20h, instigator, inflictor and item 0): a
;                     zone a sponge ticks names its thrower, its inflictor and it
;   sponge_artefacts  the anomaly cook's look-up of a core's `artefacts`
;                     (0x2E9D40 at 0x2E9D53, `call 0xF6B10`): the table, or
;                     for a core with the owner's spots a copy with the
;                     Sponge's and the Rattle's units after its kinds
;                     (sp_artefacts, for the Rattle's patch too)
;   sponge_spawn      spawn_artefact 0x31B6C0 at 0x31B800, the container
;                     chosen: the artefact it may take (sp_spawn_pick), or
;                     none this time
;   sponge_links      the core's resolve_links 0x33C440 (its vtable slot
;                     0xA16A10, thiscall, `ret 1Ch`): the original, then the
;                     kinds of its picked containers kept (sp_links)
;
; The C functions and data it uses are symbols of the link: sp_vtable (+4 is
; the vtable as objects hold it, past its RTTI locator), sp_g (the state the
; zone's hit reads) and the functions below.
bits 32
global sponge_construct, sponge_size, sponge_query, sponge_predicate, sponge_ready, sponge_throw, sponge_pull
global sponge_zone_hit, sponge_artefacts, sponge_spawn, sponge_links
extern sp_construct, sp_size, sp_cook_query, sp_can_throw, sp_exploded, sp_throw, sp_pull_pin, sp_vtable, sp_g
extern sp_artefacts, sp_spawn_pick, sp_links
section .text

%define CONSTRUCT_DEFAULT 0x25E737 ; the switch's default: the base cook
%define CONSTRUCT_TABLE   0x25E693 ; jmp [eax*4+25E754h]
%define SIZE_DEFAULT      0x25E7D2
%define SIZE_TABLE        0x25E79F
%define QUERY_ON          0x25E806 ; after the prologue
%define PREDICATE_ON      0x2E40D2 ; test al,al
%define AS_SET_HAS        0x30A090 ; eax = a grenade set: al, a grenade ready
%define READY_ON          0x33A683 ; movzx eax,al
%define READY_YES         0x33A696
%define READY_NO          0x33A692
%define THROW_SET         0x30A250
%define THROW_ON          0x33A7D9
%define PULL_SET          0x30A120
%define PULL_ON           0x3103DC
%define ZONE_HIT_ON       0x319CAC
%define CONFIG_CHILD      0xF6B10  ; operator[]: eax the table, edi the key; eax the child (the call replaced)
%define SPAWN_ON          0x31B85F ; edx = the artefact (spawn_artefact's third draw done)
%define SPAWN_NONE        0x31B8C2 ; no artefact: the next delay
%define RESOLVE_LINKS     0x33C440 ; the slot's own function: thiscall (resolver, config by value), ret 1Ch
%define TYPE_SPONGE_LESS3 9        ; data/type 12 after the switch's `add eax,-3`
%define SP_VTABLE         sp_vtable + 4 ; as objects hold it
%define G_ZONE            sp_g + 0x0  ; struct sp_globals
%define G_INSTIGATOR      sp_g + 0x4
%define G_INFLICTOR       sp_g + 0x8
%define G_ITEM            sp_g + 0xC

sponge_construct:
    jmp near construct
sponge_size:
    jmp near size
sponge_query:
    jmp near query
sponge_predicate:
    jmp near predicate
sponge_ready:
    jmp near ready
sponge_throw:
    jmp near throw
sponge_pull:
    jmp near pull
sponge_zone_hit:
    jmp near zone_hit
sponge_artefacts:
    jmp near artefacts
sponge_spawn:
    jmp near spawn
sponge_links:
    jmp near links

; 0x25E68A, eax = data/type - 3; pushed ebx (= arg 2, &config), esi, edi;
; [esp+10h].. the memory, &config, &effect, the scene.
construct:
    cmp eax, TYPE_SPONGE_LESS3
    je .sponge
    cmp eax, 5
    ja CONSTRUCT_DEFAULT
    jmp CONSTRUCT_TABLE
.sponge:
    push dword [esp + 0x1C]           ; the scene
    push dword [esp + 0x1C]           ; &effect
    push ebx                          ; &config
    push dword [esp + 0x1C]           ; the memory
    call sp_construct                 ; stdcall: eax the object (0 for no memory)
    pop edi
    pop esi
    pop ebx
    ret 0x10

; 0x25E79A, the same; pushed ebx, esi, edi; `ret 4`.
size:
    cmp eax, TYPE_SPONGE_LESS3
    je .sponge
    cmp eax, 5
    ja SIZE_DEFAULT
    jmp SIZE_TABLE
.sponge:
    call sp_size
    pop edi
    pop esi
    pop ebx
    ret 4

; 0x25E800: thiscall (parent query, the artefact, &config), `ret 0Ch`.
query:
    push ecx
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    push dword [esp + 0x10]
    call sp_cook_query                ; thiscall: eax 1 when it was a sponge's
    pop ecx
    test eax, eax
    jz .stock
    ret 0xC
.stock:
    push ebp
    mov ebp, esp
    and esp, 0xFFFFFFF8
    jmp QUERY_ON

; 0x2E40C6, ecx = the item in the grenade slot; esi the weapon core (kept).
predicate:
    cmp dword [ecx], SP_VTABLE
    je .sponge
    mov eax, [ecx]
    mov edx, [eax + 0x64]
    call edx
    call AS_SET_HAS
    jmp PREDICATE_ON
.sponge:
    push ecx
    call sp_can_throw
    add esp, 4
    jmp PREDICATE_ON

; 0x33A677, esi = the throw state, al = its grenade's index (not FFh).
ready:
    mov ecx, [esi + 0x17C]
    cmp dword [ecx], SP_VTABLE
    je .sponge
    mov edx, [ecx + 0x268]
    jmp READY_ON
.sponge:
    push ecx
    call sp_exploded
    add esp, 4
    test al, al
    jnz READY_YES
    jmp READY_NO

; 0x33A7BB, esi = the throw state, edi = the player, [esp+10h] the hand's
; transform (0x309F00's, normalized by 0x727B0).
throw:
    mov ecx, [esi + 0x17C]
    add edi, 0x44C                    ; the aim's matrix
    cmp dword [ecx], SP_VTABLE
    je .sponge
    push edi
    lea eax, [esp + 0x14]
    push eax
    mov al, [esi + 0x180]
    push ecx
    call THROW_SET
    jmp THROW_ON
.sponge:
    push edi
    lea eax, [esp + 0x14]
    push eax
    push ecx
    call sp_throw                     ; cdecl (sponge, &hand, &aim)
    add esp, 12
    jmp THROW_ON

; 0x3103C9, esi = the pull substate, eax = the player's time.
pull:
    mov esi, [esi + 0x138]
    mov ecx, [esi + 0x17C]
    cmp dword [ecx], SP_VTABLE
    je .sponge
    push eax
    push ecx
    call PULL_SET
    jmp PULL_ON
.sponge:
    push eax                          ; the time: a tap's release already seen arms it
    push ecx
    call sp_pull_pin                  ; cdecl (sponge, time): al = the index, 0
    add esp, 8
    jmp PULL_ON

; 0x319C8B, edi = the zone, ebx = 0, eax = the receiver's vt+20h, ecx = the
; receiver, edx = the damage type, st0 = the power: the call
; (time, instigator, &part, type, hit, power, inflictor, item).
zone_hit:
    cmp edi, [G_ZONE]
    jne .stock_tail
    push dword [G_ITEM]
    push dword [G_INFLICTOR]
    jmp .floats
.stock_tail:
    push ebx
    push ebx
.floats:
    sub esp, 8
    fstp dword [esp + 4]
    fld dword [esp + 0x20]
    fstp dword [esp]
    push edx
    lea edx, [esp + 0x3C]
    push edx
    mov edx, [esp + 0xB4]
    cmp edi, [G_ZONE]
    jne .nobody
    push dword [G_INSTIGATOR]
    jmp .call
.nobody:
    push ebx
.call:
    push edx
    call eax
    jmp ZONE_HIT_ON

; 0x2E9D53, called: eax = the core's config, edi = "artefacts" (as operator[]
; 0xF6B10 takes them; it keeps ebx, esi, edi and ebp, and so does this);
; [ebp+8] the cook (query_for_artefacts' frame).
artefacts:
    push dword [ebp + 8]              ; the cook
    push eax                          ; the core's config
    call CONFIG_CHILD                 ; eax = its artefacts table
    push eax
    call sp_artefacts                 ; cdecl (table, core, cook): the table to read
    add esp, 12
    ret

; 0x31B800, jumped to: edi = the core, [ebp-8] the container chosen (its
; draw made), the free list's room allocated below; ebx, ecx, edx free.
spawn:
    push dword [ebp - 8]
    push edi
    call sp_spawn_pick                ; cdecl (core, container): the artefact or 0
    add esp, 8
    test eax, eax
    jz SPAWN_NONE
    mov edx, eax
    jmp SPAWN_ON

; the core's vtable slot 0xA16A10: thiscall, ecx = the core's +4, [esp+4]
; the link resolver, [esp+8] its config by value (24 bytes); ret 1Ch.
links:
    push ecx
    times 7 push dword [esp + 0x20]   ; the resolver and the config again
    call RESOLVE_LINKS
    pop ecx
    lea eax, [esp + 8]                ; the config
    push eax
    push ecx
    call sp_links                     ; cdecl (core+4, &config)
    add esp, 8
    ret 0x1C
