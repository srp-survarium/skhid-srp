; Patch 32's find_kit, grown (client_patch.md section 3.30): a weapon that no
; kit holds as its weapon 1 (or weapon 2 with no weapon 1), as one only in
; the stash or a weapon 2 behind a weapon 1, is shown in the hands of the kit
; chosen in the lobby. Display only: the lobby character is built from a
; scratch copy of the account's profile in which that kit's weapon 1 is the
; window's weapon; the account's profile is never written. At STASH_OFFSET of
; `.srpw`, after the weapon camouflage's code; its state (two dwords) at +0Ch
; of patch 32's.
;   nasm -f bin -DCAVE=0xA429360 -DSTATE=0xA428E00 weapon_stash_026g.asm -o weapon_stash_026g.section.bin
;
; weapon_preview_026g.asm calls find_kit_or_show where it called its find_kit
; (start_cam, setup_hook): eax = a profile (or 0), ecx = an item id. Out: edx
; = the kit that shows it, or -1; eax = the profile to show it from (the
; scratch copy when it was made, else as given). Every other register, the
; flags' DF and the stack are kept (xmm0 is not: 0x284D10 and 0x270390
; clobber it, which both callers allow).
;
; The game's functions (client_patch.md section 3.30, read 2026-10-21):
;   item_by_id   0x1F0090  eax = items_dictionary, [esp+4] = u32 dict id;
;                          `ret 4`; eax = the dictionary item or 0
;   find_item    0x277AA0  ecx = begin, edi = end, [esp+4] = &item id (the
;                          caller pops); eax = the inventory item or end
;   item_copy    0x270390  eax = source, edi = a constructed destination
;                          (inventory_item_descr copy-assign: its two
;                          buffer_vectors point into the record itself)
;   profile_copy 0x284D10  esi = destination, edi = source (player_profile
;                          copy-assign, every item record included)
;   profile_ctor 0x2F73F0  ecx = this
;   setup_profile's early return compares the kit's id and revision with the
;   character's copy: the scratch's revision of the kit is a new one for
;   every scratch, so the character is rebuilt from it, and from the
;   account's profile again once the window closes.
BITS 32
ORG CAVE

%ifndef STATE
%error STATE is required
%endif

%define ITEM_BY_ID      0x1F0090
%define FIND_ITEM       0x277AA0
%define ITEM_COPY       0x270390
%define PROFILE_COPY    0x284D10
%define PROFILE_CTOR    0x2F73F0
%define ALLOCATE        0x0E4F90    ; esi = allocator, eax = size, [esp+4] = where: eax; `ret 4`
%define ALLOCATION_NAME 0x420789    ; ecx = ALLOCATION_WHERE: eax
%define GAME_ALLOCATOR  0x1F1D9C4
%define ALLOCATION_WHERE 0xAC1220

%define KITS            0x57        ; profile: u8 kit count
%define KIT             0x58        ; profile: kit k at +58h + k * 704h
%define KIT_SIZE        0x704
%define WEAPON_1        (0x40 + 4 * 7)
%define WEAPON_2        (0x40 + 4 * 10)
%define ITEMS           0x1564      ; profile: u8 item count
%define ITEM_TABLE      0x1568      ; profile: records of B4h, +0 the item id, +4 the dict id
%define ITEM_SIZE       0xB4
%define MAX_ITEMS       0x45
%define REVISIONS       0x45FC      ; profile: u32 revision of kit k at +45FCh + 4k
%define CHOSEN_KIT      0x4632      ; profile: the kit chosen in the lobby
%define PROFILE_SIZE    0x4634
%define INVENTORY       0x4634      ; the account's profile (lc+1E6344h): the inventory vector after it
%define INVENTORY_ALLOC 0x463C      ;   {begin, end, allocator, storage end}: allocator = [GAME_ALLOCATOR]
%define CLIENT_PLAYERS  (0x1E56C0 - 0x1E6344) ; from the account's profile: lc+1E56C0h
%define DICTIONARY      0x53A4      ; [lc+1E56C0h]+53A4h: the items dictionary
%define CATEGORY        0x228       ; dictionary item: u8 item_category

; The state, at STATE (patch 32's mode, item and least are at +0..+0Bh):
%define SCRATCH         0x0C        ; the scratch profile (made once), or 0
%define SERIAL          0x10        ; the last scratch revision

find_kit_or_show:
    call find_kit
    test edx, edx
    jns .ret                        ; a kit holds it: patch 32's own case
    test eax, eax
    jz .ret
    pushad                          ; [esp+28] = eax, [esp+20] = edx
    mov ebp, eax                    ; ebp = the profile
    mov ebx, ecx                    ; ebx = the item id
    ; only the account's profile has the inventory behind it (setup_profile is
    ; also given a character's own copy and a squad mate's temporary one)
    mov eax, [GAME_ALLOCATOR]
    cmp [ebp+INVENTORY_ALLOC], eax
    jne .none
    push ebx
    mov ecx, [ebp+INVENTORY]
    mov edi, [ebp+INVENTORY+4]
    push esp                        ; &the item id
    call FIND_ITEM
    add esp, 8
    cmp eax, edi
    je .none                        ; not in the inventory
    mov esi, eax                    ; esi = the inventory item
    ; a weapon (item_category 8, 13..17, as the movie's isWeaponCategory)
    mov eax, [ebp+CLIENT_PLAYERS]
    mov eax, [eax+DICTIONARY]
    movzx ecx, word [esi+4]
    push ecx
    call ITEM_BY_ID
    test eax, eax
    jz .none
    movzx eax, byte [eax+CATEGORY]
    cmp eax, 8
    je .weapon
    sub eax, 13
    cmp eax, 4
    ja .none
.weapon:
    call state
    xchg edi, eax                   ; edi = the state
    mov eax, [edi+SCRATCH]
    test eax, eax
    jnz .copy
    push edi
    push esi
    mov esi, [GAME_ALLOCATOR]
    mov ecx, ALLOCATION_WHERE
    call ALLOCATION_NAME
    push eax
    mov eax, PROFILE_SIZE
    call ALLOCATE
    pop esi
    pop edi
    test eax, eax
    jz .none
    mov ecx, eax
    call PROFILE_CTOR
    mov [edi+SCRATCH], eax
.copy:
    push edi                        ; the state
    push esi                        ; the inventory item
    mov esi, eax                    ; the scratch
    mov edi, ebp                    ; the account's profile
    call PROFILE_COPY
    pop eax                         ; the inventory item
    ; its record in the scratch's table, or a new one after the last
    movzx ecx, byte [esi+ITEMS]
    xor edx, edx
.find:
    cmp edx, ecx
    jae .append
    imul edi, edx, ITEM_SIZE
    cmp [esi+edi+ITEM_TABLE], ebx
    je .slot
    inc edx
    jmp .find
.append:
    cmp ecx, MAX_ITEMS
    jae .full
    imul edi, edx, ITEM_SIZE
    lea edi, [esi+edi+ITEM_TABLE]
    push edx
    call ITEM_COPY
    pop edx
    inc byte [esi+ITEMS]
.slot:
    pop edi                         ; the state
    movzx ecx, byte [ebp+CHOSEN_KIT]
    imul eax, ecx, KIT_SIZE
    mov [esi+eax+KIT+WEAPON_1], dx  ; weapon 1 = that record (its amount as the kit's)
    mov eax, [edi+SERIAL]
    inc eax
    or eax, 0x80000000              ; never a revision srp sends (counters from 1)
    mov [edi+SERIAL], eax
    mov [esi+ecx*4+REVISIONS], eax
    mov [esp+20], ecx               ; edx: the chosen kit
    mov [esp+28], esi               ; eax: the scratch
    jmp .none
.full:
    pop edi
.none:
    popad
.ret:
    ret

; Patch 32's find_kit (weapon_preview_026g.asm): eax = a profile (or 0), ecx
; = an item id: edx = the first kit (below the profile's count) whose held
; weapon (slot 7, or slot 10 when 7 is empty) is that item, or -1. Keeps
; every other register.
find_kit:
    push ebx
    push esi
    push edi
    or edx, -1
    test eax, eax
    jz .done
    movzx ebx, byte [eax+ITEMS]
    lea esi, [eax+KIT+WEAPON_1]
.kit:
    inc edx
    cmp dl, [eax+KITS]
    jae .none
    movzx edi, word [esi]
    cmp di, -1
    jne .held
    movzx edi, word [esi+WEAPON_2-WEAPON_1]
.held:
    add esi, KIT_SIZE
    cmp edi, ebx
    jae .kit
    imul edi, edi, ITEM_SIZE
    cmp [eax+edi+ITEM_TABLE], ecx
    jne .kit
    jmp .done
.none:
    or edx, -1
.done:
    pop edi
    pop esi
    pop ebx
    ret

; eax = the state where this image was loaded. No flag changes.
state:
    call .pc
.pc:
    pop eax
    lea eax, [eax + STATE - .pc]
    ret
