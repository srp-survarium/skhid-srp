; Patch 32 grown (client_patch.md section 3.30): an upgrade window's item the
; lobby character does not show is shown on it, display only, from a scratch
; copy of the account's profile; the account's profile is never written.
;   - a weapon no kit holds as its held weapon (only in the stash, or a
;     weapon 2 behind a weapon 1): the chosen kit's weapon 1 (find_kit_or_show,
;     which patch 32's two finds call: weapon_preview_026g.asm -DFIND);
;   - armour the chosen kit does not wear: worn in the first armour slot the
;     game lets it into (items_dictionary::can_move_item_to_slot, slots 0..6),
;     in the decal pose the window gives the character (armour_*: patch 32's
;     start, exit and setup_profile hooks come here first, then go on to it).
; At STASH_OFFSET of `.srpw`, after the weapon camouflage's code; its state
; at +0Ch of patch 32's.
;   nasm -f bin -DCAVE=0xA429360 -DSTATE=0xA428E00 -DPREVIEW=0xA427E50 weapon_stash_026g.asm -o weapon_stash_026g.section.bin
;
; The game's functions (client_patch.md section 3.30, read 2026-10-21):
;   item_by_id   0x1F0090  eax = items_dictionary, [esp+4] = u32 dict id;
;                          `ret 4`; eax = the dictionary item or 0
;   can_move     0x2CA910  stdcall (dictionary, item category, kit slot):
;                          al; `ret 0Ch`
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
%ifndef PREVIEW
%error PREVIEW (weapon_preview_026g.asm's address) is required
%endif

%define START_CAM       (PREVIEW + 0x00)    ; patch 32's entries
%define EXIT_CAM        (PREVIEW + 0x6D)
%define SETUP_HOOK      (PREVIEW + 0xAD)
%define SETUP_PROFILE   0x284B10    ; eax = a profile, [esp+4] the character, [esp+8] the kit; `ret 8`

%define ITEM_BY_ID      0x1F0090
%define CAN_MOVE        0x2CA910
%define FIND_ITEM       0x277AA0
%define ITEM_COPY       0x270390
%define PROFILE_COPY    0x284D10
%define PROFILE_CTOR    0x2F73F0
%define ALLOCATE        0x0E4F90    ; esi = allocator, eax = size, [esp+4] = where: eax; `ret 4`
%define ALLOCATION_NAME 0x420789    ; ecx = ALLOCATION_WHERE: eax
%define GAME_ALLOCATOR  0x1F1D9C4
%define ALLOCATION_WHERE 0xAC1220

%define MENU            0xD2BC      ; lobby_character: the lobby menu
%define UPGRADE_MODE    0xD515      ; lobby_character: 1 while an upgrade window is open
%define PROFILE         0x6A0       ; lobby menu: the account's profile
%define KITS            0x57        ; profile: u8 kit count
%define KIT             0x58        ; profile: kit k at +58h + k * 704h
%define KIT_SIZE        0x704
%define SLOTS           0x40        ; kit: 23 slots {u16 item index, u16 amount}
%define SLOT_COUNT      23
%define WEAPON_1        (SLOTS + 4 * 7)
%define WEAPON_2        (SLOTS + 4 * 10)
%define ARMOUR_SLOTS    7           ; slots 0..6
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

; The state, at STATE (patch 32's mode, item and least at +0..+0Bh):
%define ITEM            0x04        ; patch 32's: the open window's item
%define SCRATCH         0x0C        ; the scratch profile (made once), or 0
%define SERIAL          0x10        ; the last scratch revision
%define ARMOUR          0x14        ; 1: the open window's armour is shown worn

; Entries at fixed offsets (patch_026g.py WEAPON_STASH_ENTRIES)
    jmp near armour_start           ; +0   0x265E73's call (start_item_upgrade's camera)
    jmp near armour_exit            ; +5   0x265F44's call (exit_item_upgrade's camera)
    jmp near armour_setup           ; +0Ah 0x284B10's jump (setup_profile)

; +0Fh, weapon_preview_026g.asm's two finds: eax = a profile (or 0), ecx = an
; item id. Out: edx = the kit that shows it, or -1; eax = the profile to show
; it from (the scratch copy when it was made, else as given). Every other
; register, DF and the stack are kept (xmm0 is not, which both callers
; allow).
find_kit_or_show:
    call find_kit
    test edx, edx
    jns .ret                        ; a kit holds it: patch 32's own case
    test eax, eax
    jz .ret
    pushad                          ; [esp+28] = eax, [esp+20] = edx
    mov ebp, eax
    mov ebx, ecx
    call lookup
    jc .none
    cmp eax, 8                      ; a weapon: item_category 8, 13..17 (the movie's isWeaponCategory)
    je .weapon
    sub eax, 13
    cmp eax, 4
    ja .none
.weapon:
    mov edx, 7
    call make_scratch
    jc .none
    mov [esp+20], edx
    mov [esp+28], esi
.none:
    popad
.ret:
    ret

; 0x265E73 `call 0x2845C0` -> call armour_start: esi = the character, edi =
; the item; the setter's arguments in eax, ecx, xmm0, xmm1, [esp+4] (and xmm3
; read after it). Armour the chosen kit does not wear: the character shows
; it worn (armour_setup builds it), then patch 32's start_cam.
armour_start:
    pushad
    sub esp, 48
    movups [esp], xmm0
    movups [esp+16], xmm1
    movups [esp+32], xmm3
    call state
    xchg ebx, eax
    mov byte [ebx+ARMOUR], 0
    mov [ebx+ITEM], edi
    mov eax, [esi+MENU]
    mov eax, [eax+PROFILE]
    test eax, eax
    jz .go
    mov ecx, edi
    call armour_scratch
    test edx, edx
    js .go
    mov byte [ebx+ARMOUR], 1
    call show_kit
.go:
    movups xmm0, [esp]
    movups xmm1, [esp+16]
    movups xmm3, [esp+32]
    add esp, 48
    popad
    jmp START_CAM

; 0x265F44 `call 0x2845C0` -> call armour_exit: esi = the character. After
; armour shown worn, the character shows the account's profile again; then
; patch 32's exit_cam.
armour_exit:
    pushad
    sub esp, 32
    movups [esp], xmm0
    movups [esp+16], xmm1
    call state
    xor ecx, ecx
    xchg cl, [eax+ARMOUR]
    test ecx, ecx
    jz .go
    call show_kit
.go:
    movups xmm0, [esp]
    movups xmm1, [esp+16]
    add esp, 32
    popad
    jmp EXIT_CAM

; 0x284B10 (setup_profile's entry) -> jmp armour_setup: eax = the profile,
; [esp+4] the character, [esp+8] the kit. While armour is shown worn, for the
; character with its upgrade window open: the scratch with it and the chosen
; kit. Then patch 32's setup_hook. Every register but eax kept.
armour_setup:
    push ecx
    push edx
    push eax
    call state
    xchg edx, eax                   ; edx = the state
    pop eax
    cmp byte [edx+ARMOUR], 0
    je .go
    mov ecx, [esp+12]               ; the character
    cmp byte [ecx+UPGRADE_MODE], 0
    je .go
    mov ecx, [edx+ITEM]
    call armour_scratch
    test edx, edx
    js .go
    mov [esp+16], edx               ; the chosen kit; eax the scratch
.go:
    pop edx
    pop ecx
    jmp SETUP_HOOK

; eax = a profile, ecx = an item id: armour the chosen kit does not wear, in
; a scratch: edx = the chosen kit, eax = the scratch. Else edx = -1 and eax
; as given. Every other register kept; xmm0 not.
armour_scratch:
    or edx, -1
    test eax, eax
    jz .ret
    pushad
    mov ebp, eax
    mov ebx, ecx
    call lookup
    jc .none
    dec eax                         ; armour: item_category 1..7
    cmp eax, 6
    ja .none
    inc eax
    ; worn by the chosen kit already: nothing to show
    movzx ecx, byte [ebp+CHOSEN_KIT]
    imul ecx, ecx, KIT_SIZE
    lea edi, [ebp+ecx+KIT+SLOTS]
    movzx ecx, byte [ebp+ITEMS]
    xor edx, edx
.worn:
    movzx esi, word [edi+4*edx]
    cmp esi, ecx
    jae .next
    imul esi, esi, ITEM_SIZE
    cmp [ebp+esi+ITEM_TABLE], ebx
    je .none
.next:
    inc edx
    cmp edx, SLOT_COUNT
    jb .worn
    ; the first armour slot the game lets it into
    mov edi, eax                    ; the category
    mov ecx, [ebp+CLIENT_PLAYERS]
    mov ecx, [ecx+DICTIONARY]
    xor edx, edx
.slot:
    push edx
    push ecx
    push edx
    push edi
    push ecx
    call CAN_MOVE
    pop ecx
    pop edx
    test al, al
    jnz .found
    inc edx
    cmp edx, ARMOUR_SLOTS
    jb .slot
    jmp .none
.found:
    push ebx
    push edx
    mov ecx, [ebp+INVENTORY]        ; lookup's inventory item again (its esi went to CAN_MOVE's frame)
    mov edi, [ebp+INVENTORY+4]
    lea eax, [esp+4]
    push eax
    call FIND_ITEM
    add esp, 4
    mov esi, eax
    pop edx
    pop ebx
    call make_scratch
    jc .none
    mov [esp+20], edx
    mov [esp+28], esi
.none:
    popad
.ret:
    ret

; ebp = a profile, ebx = an item id: CF clear, esi = its inventory item, eax
; = its item_category; CF set when the profile is not the account's (no
; inventory behind it), or the inventory or the dictionary lacks the item.
; ebx and ebp kept.
lookup:
    mov eax, [GAME_ALLOCATOR]
    cmp [ebp+INVENTORY_ALLOC], eax
    jne .no
    push ebx
    mov ecx, [ebp+INVENTORY]
    mov edi, [ebp+INVENTORY+4]
    push esp                        ; &the item id
    call FIND_ITEM
    add esp, 8
    cmp eax, edi
    je .no
    mov esi, eax
    mov eax, [ebp+CLIENT_PLAYERS]
    mov eax, [eax+DICTIONARY]
    movzx ecx, word [esi+4]
    push ecx
    call ITEM_BY_ID
    test eax, eax
    jz .no
    movzx eax, byte [eax+CATEGORY]
    clc
    ret
.no:
    stc
    ret

; ebp = the account's profile, ebx = an item id, esi = its inventory item,
; edx = a kit slot: the scratch (made once) is the account's profile with
; that item in that slot of the chosen kit and a revision of its own: CF
; clear, esi = the scratch, edx = the chosen kit. CF set when no scratch
; could be had or the table is full. ebx and ebp kept.
make_scratch:
    push edx                        ; the slot
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
    jz .fail
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
    pop eax                         ; the slot
    movzx ecx, byte [ebp+CHOSEN_KIT]
    push ecx
    imul ecx, ecx, KIT_SIZE
    lea ecx, [esi+ecx+KIT+SLOTS]
    mov [ecx+eax*4], dx             ; the slot = that record (its amount as the kit's)
    pop ecx
    mov eax, [edi+SERIAL]
    inc eax
    or eax, 0x80000000              ; never a revision srp sends (counters from 1)
    mov [edi+SERIAL], eax
    mov [esi+ecx*4+REVISIONS], eax
    mov edx, ecx
    clc
    ret
.full:
    pop edi
.fail:
    pop edx
    stc
    ret

; esi = the character: setup_profile(the account's profile, esi, the kit
; chosen in the lobby), which armour_setup and patch 32's setup_hook may
; give another profile and kit.
show_kit:
    mov eax, [esi+MENU]
    mov eax, [eax+PROFILE]
    movzx ecx, byte [eax+CHOSEN_KIT]
    push ecx
    push esi
    call SETUP_PROFILE
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
