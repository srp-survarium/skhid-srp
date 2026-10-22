; The Sponge (exe patch 55 path A): the hooks of artefact type 12, in front
; of the C code of sponge_a_026g.c at +0xC000 of `.srpx` (sponge_a_026g.py
; builds it; patch_026g.py puts it there). client_patch.md, patch 55 path A.
;
;   +00  construct  the artefact cook's construct 0x25E660 at its type switch
;                   (0x25E68A `cmp eax,5 / ja`): type 12 (eax 9) is ours
;   +05  size       its size 0x25E770 at 0x25E79A (`cmp eax,5 / ja short`)
;   +0A  query      its query of the model 0x25E800 (the entry, `push ebp /
;                   mov ebp,esp / and esp,-8`): a sponge's is ours
;   +0F  predicate  the throw state's way in 0x2E4090 at 0x2E40C6 (the item's
;                   vtable +64h, `call 0x30A090`: a null deref for any item
;                   that is not a grenade set): a sponge not in the air
;   +14  ready      the throw state's is_ready 0x33A650 at 0x33A677 (its
;                   grenade's exploded flag): a sponge's zone on
;   +19  throw      the throw 0x33A770 at 0x33A7BB (`call 0x30A250`): ours
;   +1E  pull       the pull substate's pin 0x3103A0 at 0x3103C9 (`call
;                   0x30A120`): ours
;   +23  zone_hit   the damage zone's hit loop 0x319AF0 at 0x319C8B (the
;                   receiver's vt+20h, instigator, inflictor and item 0): a
;                   zone a sponge ticks names its thrower, its inflictor and it
;
; -D: CAVE (this code's VA), STATE (the state page), SP_VTABLE (the sponge's
; vtable as objects hold it), and the C functions: SP_CONSTRUCT, SP_SIZE,
; SP_COOK_QUERY, SP_CAN_THROW, SP_EXPLODED, SP_THROW, SP_PULL_PIN.
bits 32
org CAVE

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
%define TYPE_SPONGE_LESS3 9        ; data/type 12 after the switch's `add eax,-3`
%define G_ZONE            STATE + 0x0
%define G_INSTIGATOR      STATE + 0x4
%define G_INFLICTOR       STATE + 0x8
%define G_ITEM            STATE + 0xC

entries:
    jmp near construct
    jmp near size
    jmp near query
    jmp near predicate
    jmp near ready
    jmp near throw
    jmp near pull
    jmp near zone_hit

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
    call SP_CONSTRUCT                 ; stdcall: eax the object (0 for no memory)
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
    call SP_SIZE
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
    call SP_COOK_QUERY                ; thiscall: eax 1 when it was a sponge's
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
    call SP_CAN_THROW
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
    call SP_EXPLODED
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
    call SP_THROW                     ; cdecl (sponge, &hand, &aim)
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
    push ecx
    call SP_PULL_PIN                  ; al = the index, 0
    add esp, 4
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

    times 0x200 - ($ - $$) db 0xCC
