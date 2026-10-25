; Patch 68 (build 2777): a weapon's attachments in at most three upgrade
; records, the weapon kit. The arena's block weapon_kit.code, with
; weapon_kit_026g.c (link_026g.py links them; patch_026g.py's
; WEAPON_KIT_HOOKS jump to the two entries by their names). README patch 68;
; client_patch.md section 3.56; the record's layout and the slots:
; ../docs/notes/weapon-kit.md and weapon_kit_026g.c.
;
; weapon_kit_query: item_config_cook::translate_query (0x2F1750) builds an
; item's modified config from the item record its query points at: 0x2F2060
; copies the query's {dict id, modifications*, upgrades*, dictionary} to
; [ebp+38h..44h] (the query's +8..+17h, the item record's own buffer_vectors),
; then the function reads the upgrades' buffer_vector at [ebp+40h] twice and
; nowhere else: its size at 0x2F1799 (the entries' count, [ebp+64h], and the
; alloca of their array) and each record's dict id and value in its loop at
; 0x2F18D0..0x2F195A (an entry each: the record's item's modification, the
; record's value as its params). `call 0x2F2060` at 0x2F1771 jumps here: the
; call, then, when the item's records hold a kit, a list of the records and
; one for each dict id the kit names is made on the stack below translate_query's
; frame (its epilogue takes esp from ebp, 0x2F2041, as after its own allocas)
; and [ebp+40h] points at it. Nothing is shared: two cooks at once each have
; their own. A list without a kit, or 0x2F2060's refusal (al 0: the query
; is another thread's), goes on as the original.
;
; weapon_kit_bridge: the lobby's item to Flash (0x288180, setup_inventory_data's
; upgrades: one object {id, param} a record, 0x2881E0..0x28834D). After its
; `param` (SetMember at 0x2882E1), `mov ecx,[esp+48h] / mov eax,[ecx]` at
; 0x2882E3 jumps here: a kit record's object gets `kit_lo` and `kit_hi`, the
; value's two halves as uint (the movie reads the dict ids the kit names,
; UpgradeWindow.as), then the two instructions and back at 0x2882E9 (the
; object's PushBack). Any other record's object is the original's.
;
; weapon_kit_attached_pair: setup_inventory_data (0x288520) maps each upgrade
; record's item id to its host ({id, host} pairs, 0x2885B7..0x2885DC: the
; push 0x28B0F0 at 0x2885C9, edi the pair, esi the pairs' vector, eax the
; host's records, edx the record's index x 3), from which each item's
; is_attached_to (0x288672 -> 0x287E30). Here the original's push, then
; weapon_kit_attached: a kit's named dict ids as pairs of their own, so the
; camouflage a scoped weapon's kit names shows worn on it.
BITS 32

global weapon_kit_query, weapon_kit_bridge, weapon_kit_attached_pair
extern weapon_kit_expand, weapon_kit_holds, weapon_kit_attached
section .text

%define GET_QUERY 0x2F2060          ; the query's {dict id, modifications*, upgrades*, dictionary} to [edi]: al
%define QUERY_BACK 0x2F1776         ; translate_query after that call
%define UPGRADES 0x40               ; translate_query's [ebp+40h]: the upgrades' buffer_vector*
%define DICTIONARY 0x44             ; [ebp+44h]: the items dictionary
%define LIST 0x90                   ; six records of 18h
%define LIST_SPACE 0xB0             ; the list, its buffer_vector, 16-aligned
%define BRIDGE_BACK 0x2882E9        ; the bridge's PushBack of the object
%define ITEM_UPGRADES 0x60          ; the item record (esi): the upgrades' buffer_vector
%define RECORD 0x18
%define RECORD_HIGH 0x0C
%define RECORD_SLOT 0x10
%define KIT_SLOT 3
%define KIT_MAGIC 0xA7
%define GFX_UINT 4
%define PAIR_PUSH 0x28B0F0          ; setup_inventory_data's pair push: edi the pair, esi the vector

; 0x2F1771: `call 0x2F2060` -> jmp here (esi the query's holder, edi
; ebp+38h, as for the call).
weapon_kit_query:
    call GET_QUERY
    test al, al
    jz .back
    pushad
    push dword [ebp+UPGRADES]
    call weapon_kit_holds           ; no kit: the original's way, its stack as it was
    add esp, 4
    test eax, eax
    popad
    jz .back
    sub esp, LIST_SPACE             ; translate_query's own stack from here on, as its allocas
    pushad
    lea ecx, [esp+32]               ; the list
    lea edx, [esp+32+LIST]          ; its buffer_vector
    push dword [ebp+DICTIONARY]
    push edx
    push ecx
    push dword [ebp+UPGRADES]
    call weapon_kit_expand
    add esp, 16
    test eax, eax
    jz .stock
    lea edx, [esp+32+LIST]
    mov [ebp+UPGRADES], edx
.stock:
    popad
.back:
    jmp QUERY_BACK

; 0x2882E3: `mov ecx,[esp+48h] / mov eax,[ecx]` -> jmp here + nop. esi the
; item record, byte [esp+80h] the record's index, [esp+30h] the object (a
; GFx value: +0 its interface, +4 its type, +8 its data).
weapon_kit_bridge:
    pushad
    lea ebp, [esp+32]               ; the bridge's esp
    movzx eax, byte [ebp+0x80]
    imul eax, eax, RECORD
    add eax, [esi+ITEM_UPGRADES]
    cmp byte [eax+RECORD_SLOT], KIT_SLOT
    jne .done
    mov edx, [eax+RECORD_HIGH]
    shr edx, 24
    cmp edx, KIT_MAGIC
    jne .done
    push dword [eax+8]
    push name_low
    call set_uint
    push dword [eax+RECORD_HIGH]
    push name_high
    call set_uint
.done:
    popad
    mov ecx, [esp+0x48]
    mov eax, [ecx]
    jmp BRIDGE_BACK

; ebp = the bridge's esp, eax = the record: the object's member [esp+4]
; (name) = [esp+8] (uint), as the bridge sets `id` (0x288240..0x288278).
; `ret 8`; eax kept.
set_uint:
    push eax
    sub esp, 16                     ; a GFx value: {interface 0, type uint, data, 0}
    mov dword [esp], 0
    mov dword [esp+4], GFX_UINT
    mov ecx, [esp+16+4+8]
    mov [esp+8], ecx
    mov dword [esp+12], 0
    mov ecx, [ebp+0x34]             ; the object is a display object: (type & 8Fh) == 0Ah
    and ecx, 0x8F
    xor edx, edx
    cmp cl, 0x0A
    sete dl
    mov ecx, [ebp+0x30]             ; its interface: SetMember(data, name, &value, display), thiscall
    mov eax, [ecx]
    push edx
    lea edx, [esp+4]
    push edx
    push dword [esp+16+4+4+8]       ; the name
    push dword [ebp+0x38]
    call [eax+0x14]
    add esp, 16
    pop eax
    ret 8

; 0x2885C9: `call 0x28B0F0` -> call here. ebp setup_inventory_data's frame:
; [ebp-14h] the inventory's vector (lc+1EA978h), [ebp-0Ch] its end, [ebp-18h]
; the host's id. Every register as 0x28B0F0 leaves it but eax and edx (the
; caller reloads both).
weapon_kit_attached_pair:
    pushad
    call PAIR_PUSH                  ; the original's pair
    mov ecx, [esp+28]               ; eax: the host's records
    mov edx, [esp+20]               ; edx: the record's index x 3
    lea ecx, [ecx+edx*8]
    mov eax, [ebp-0x14]
    push dword [ebp-0x0C]
    push dword [eax]
    push esi
    push dword [ebp-0x18]
    push ecx
    call weapon_kit_attached
    add esp, 20
    popad
    ret

section .rodata
name_low:   db "kit_lo", 0
name_high:  db "kit_hi", 0
