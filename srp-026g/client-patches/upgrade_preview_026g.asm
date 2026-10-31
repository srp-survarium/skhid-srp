; A weapon's upgrade window shows an upgrade dropped on the weapon at once
; (build 2777, patch 48, client_patch.md section 3.45). Patch 32's lobby
; character shows the window's weapon (weapon_preview_026g.asm,
; weapon_stash_026g.asm); the stock game shows what the server's account
; says, so a scope or a camouflage dropped on the weapon was seen only after
; the request had gone out (the lobby client sends at its own 500 ms tick),
; the server's inventory and account had come back, and the character's
; player had been made again. Here the drop itself changes what the
; character is built from; the server stays the authority:
;   relocate  relocate_upgrade_item(host, upgrade, slot)'s handler, before
;             it sends `46 01`: for the window's weapon, the upgrade is
;             queued as "put" and the character is made again at once
;   drop      drop_upgrade_item(host, upgrade)'s handler, before patch 30
;             sends `46 02`: queued as "take off", the same
;   account   each account the server sends (0x87; srp sends one after
;             every answer to `46 01` / `46 02`, a refusal too) answers the
;             oldest queued change: it leaves the queue, the account's
;             record has it (or not, when refused)
;   find      patch 32's two finds: for the window's weapon once a change
;             was queued, the character is built from patch 32's scratch
;             profile: the account's, the queued changes applied to the
;             weapon's record in order (as the server applies them:
;             items_rules::attach_upgrade_in / detach_upgrade), with a
;             kit revision of its own that stays the same while the
;             weapon's upgrades are the same (setup_profile's early return:
;             the account confirming the change shown makes nothing again)
;   again     setup_profile while the character's player is being made of
;             the same kit's id and revision: returns (what is asked is
;             being made; a call waiting for it is dropped, this is newer)
;   made      on_player_ready: one log line with the time the player took
;             (the lobby's own character)
;   early     setup_profile's early return (what is asked is what is shown)
;             only while no player is being made (the lobby's own
;             character): a player being made of another revision of that
;             kit (patch 32's scratch: a window closed at once) is made
;             again after it, as the original does for another kit
; Nothing sent to the server changes. A scope's record holds the variant
; made for the weapon (srp's ScopeFits): found in the items dictionary, the
; first category-22 entry whose config's `srp_scope_fit` names the scope
; and the weapon (by name or dict id); none: nothing is shown before the
; server's answer.
;   nasm -f bin -DCAVE=0xA42C000 -DSTATE=0xA428D60 -DPREVIEW=0xA428E00 -DFIND=0xA42A36F upgrade_preview_026g.asm -o upgrade_preview_026g.section.bin
; (STATE: its own, UPGRADE_PREVIEW_STATE_OFFSET of `.srpd`; PREVIEW: patch
; 32's state; FIND: weapon_stash_026g.asm's find_kit_or_show)
BITS 32
ORG CAVE

%ifndef STATE
%error STATE is required
%endif
%ifndef PREVIEW
%error PREVIEW (patch 32's state) is required
%endif
%ifndef FIND
%error FIND (weapon_stash_026g.asm's find_kit_or_show) is required
%endif

; The game's functions
%define SETUP_PROFILE   0x284B10    ; eax = a profile, [esp+4] the character, [esp+8] the kit; `ret 8`
%define SETUP_DONE      0x284CF8    ; setup_profile's epilogue: pop edi, esi, ebx; leave; `ret 8`
%define ACCOUNT_ITEMS   0x270320    ; the 0x87 reader's last call (esi = the lobby client)
%define FIND_ITEM       0x277AA0    ; ecx = begin, edi = end, [esp+4] = &id (the caller pops): eax = the item or end
%define ITEM_BY_ID      0x1F0090    ; eax = items_dictionary, [esp+4] = dict id; `ret 4`: eax = the entry or 0
%define HAS_KEY         0x0F6C00    ; eax = table, esi = key: al
%define CHILD           0x0F6B10    ; eax = table, edi = key: eax = the child
%define PROFILE_COPY    0x284D10    ; esi = destination, edi = source
%define PROFILE_CTOR    0x2F73F0    ; ecx = this
%define ALLOCATE        0x0E4F90    ; esi = allocator, eax = size, [esp+4] = where: eax; `ret 4`
%define ALLOCATION_NAME 0x420789    ; ecx = ALLOCATION_WHERE: eax
%define GAME_ALLOCATOR  0x1F1D9C4
%define ALLOCATION_WHERE 0xAC1220
%define LOG             0x111ED0    ; cdecl (&function, [LOG_A], LOG_B, file, line, function, channel, level, format, ...)
%define LOG_FILTER      0x111860    ; cdecl (channel, level): al = it is written
%define LOG_FILTERED    0x1F1D60C
%define LOG_A           0x1F1D610
%define LOG_B           0x21E73B8
%define LOG_SINK        0x1F1D614   ; the function the game's lines go through (0, none)
%define LOG_MANAGER     0x9DAF4C    ; its boost::function manager
%define CHANNEL_GAME    0x9FA4E4    ; "game"
%define LEVEL_INFO      4

; The lobby
%define MENU_CHARACTER  0x694       ; lobby menu: its own lobby_character
%define MENU_PROFILE    0x6A0       ; lobby menu: the account's profile
%define MENU_CLOCK      0x9C        ; lobby menu: [+9Ch]+584Ch, its time in ms (the camera's tick reads it)
%define CLOCK_MS        0x584C
%define MENU            0xD2BC      ; lobby_character: the lobby menu
%define UPGRADE_MODE    0xD515      ; lobby_character: 1 while an upgrade window is open
%define QUERY           0xD2C4      ; lobby_character: the player's query, -1 none
%define AGAIN           0xD2C8      ; lobby_character: a setup_profile waits for the query
%define SHOWN_KIT_ID    0x4         ; lobby_character: the kit's id setup_profile last took
%define COPIES          0x18        ; lobby_character: three profiles of PROFILE_SIZE
%define KIT_ID          0x754       ; profile: kit k's id at +754h + k * 704h
%define KIT_SIZE        0x704
%define ITEMS           0x1564      ; profile: u8 item count
%define ITEM_TABLE      0x1568      ; profile: records of B4h, +0 the item id
%define ITEM_SIZE       0xB4
%define REVISIONS       0x45FC      ; profile: u32 revision of kit k at +45FCh + 4k
%define CHOSEN_KIT      0x4632      ; profile: the kit chosen in the lobby
%define PROFILE_SIZE    0x4634
%define INVENTORY       0x4634      ; the account's profile: the inventory vector after it
%define INVENTORY_ALLOC 0x463C      ;   {begin, end, allocator, storage end}
%define CLIENT_PLAYERS  (0x1E56C0 - 0x1E6344) ; from the account's profile: lc+1E56C0h
%define DICTIONARY      0x53A4      ; [lc+1E56C0h]+53A4h: the items dictionary
%define ITEM_DICT       0x4         ; inventory item / record: u16 dict id
%define UPGRADES        0x60        ; record: buffer_vector {begin, end, capacity end} of
%define UP_SIZE         0x18        ;   {u32 id, u32 dict id, u64 value, u8 slot}
%define UP_DICT         0x4
%define UP_VALUE        0x8
%define UP_SLOT         0x10
%define UP_COMPARED     0x11        ;   what two records are compared by (not the padding)
%define ENTRIES         0x130       ; items_dictionary: entries of 3A0h
%define ENTRY_COUNT     0x134
%define ENTRY_SIZE      0x3A0
%define ENTRY_NAME      0x4         ; entry: its name (buffer_string's text)
%define ENTRY_CONFIG    0x114       ; entry: its config, the root at +108h
%define CONFIG_ROOT     0x108
%define ENTRY_CATEGORY  0x228
%define SCOPE_CATEGORY  22
%define NODE_TYPE       0x14        ; config node: +0 value or children, u16 type, u16 count
%define NODE_COUNT      0x16
%define NODE_SIZE       0x18
%define T_INTEGER       1
%define T_INDEXED       4
%define T_STRING        5

; Patch 32's state (PREVIEW)
%define MODE            0x00        ; byte: 1 while the window's weapon is shown
%define WINDOW_ITEM     0x04        ; the window's item
%define SCRATCH         0x0C        ; weapon_stash_026g.asm's scratch profile, or 0
%define SERIAL          0x10        ; its last scratch revision

; The state (STATE)
%define HOST            0x00        ; the item the queue is of, 0 none
%define COUNT           0x04        ; queued changes
%define LAST_REV        0x08        ; the revision last given
%define LAST_KIT        0x0C        ;   for this kit (-1: none)
%define LAST_BYTES      0x10        ;   and these upgrades (bytes)
%define STARTED         0x14        ; the lobby's time when its character's last query started
%define MAKING_KIT      0x18        ;   and the kit it is of
%define LAST            0x1C        ; the upgrades last given (3 records)
%define OPS             0x64        ; the queue: {u32 upgrade, u32 record's dict id, u32 slot or TAKE_OFF}
%define OP_SIZE         0x0C
%define MAX_OPS         5
%define STATE_SIZE      (OPS + MAX_OPS * OP_SIZE)
%define TAKE_OFF        0xFF

; Entries at fixed offsets (patch_026g.py UPGRADE_PREVIEW_ENTRIES)
    jmp near relocate               ; +0   0x29A9B2 (relocate_upgrade_item's handler)
    jmp near drop                   ; +5   0x29AACF (drop_upgrade_item's handler)
    jmp near account                ; +0Ah 0x270313 (the 0x87 reader's end)
    jmp near find                   ; +0Fh patch 32's two finds (weapon_preview_026g.asm -DFIND)
    jmp near again                  ; +14h 0x284B89 (setup_profile past its early return)
    jmp near made                   ; +19h 0x284F7F (on_player_ready)
    jmp near early                  ; +1Eh 0x284B7C (setup_profile's early return)
kit_apply:
    dd 0                            ; +23h patch 68's weapon_kit_preview_apply, written by the build (0: none)

; 0x29A9B2 `mov edx,[esi+4620h]` -> call relocate + nop: edi = the host,
; ebx = the upgrade, cl = the slot, esi = the account's profile, eax = the
; lobby client; the handler's [esp+10h] (here [esp+14h]) the lobby menu.
relocate:
    pushad
    movzx edx, cl
    mov eax, [esp+32+14h]
    call preview
    popad
    mov edx, [esi+4620h]
    ret

; 0x29AACF `mov ecx,[esi+4620h]` -> call drop + nop: edi = the upgrade,
; the host the Flash call's first argument ([ebp+8] -> +8 -> +8, as patch
; 30 reads it), eax = the lobby client (patch 30's drop_upgrade reads it
; next), the lobby menu as above.
drop:
    pushad
    mov ebx, edi
    mov edi, [ebp+8]
    mov edi, [edi+8]
    mov edi, [edi+8]
    mov edx, TAKE_OFF
    mov eax, [esp+32+14h]
    call preview
    popad
    mov ecx, [esi+4620h]
    ret

; eax = the lobby menu, edi = the host's id, ebx = the upgrade's id, edx =
; its slot or TAKE_OFF. For the window's weapon (patch 32's mode 1): the
; change is queued and the character made again from it. Any register.
preview:
    mov esi, PREVIEW
    cmp byte [esi+MODE], 0
    je .ret
    cmp [esi+WINDOW_ITEM], edi
    jne .ret
    mov ecx, [eax+MENU_CHARACTER]
    test ecx, ecx
    jz .ret
    cmp byte [ecx+UPGRADE_MODE], 0
    je .ret
    mov ebp, [eax+MENU_PROFILE]
    test ebp, ebp
    jz .ret
    push ecx                        ; the character
    push edx                        ; the slot
    mov ecx, ebx
    call item_dict
    jc .none
    cmp byte [esp], TAKE_OFF
    je .queue
    call record_dict
    jc .none
.queue:
    mov esi, STATE
    cmp [esi+HOST], edi
    je .room
    mov [esi+HOST], edi             ; another weapon: a queue of its own
    and dword [esi+COUNT], 0
    or dword [esi+LAST_KIT], -1
.room:
    cmp dword [esi+COUNT], MAX_OPS
    jb .push
    call pop_op                     ; full: the oldest goes
.push:
    imul ecx, [esi+COUNT], OP_SIZE
    lea ecx, [esi+ecx+OPS]
    mov [ecx], ebx
    mov [ecx+4], eax
    pop edx
    mov [ecx+8], edx
    inc dword [esi+COUNT]
    push edx
    push ebx
    push edi
    push log_preview
    call log
    add esp, 16
    pop ecx                         ; the character
    movzx eax, byte [ebp+CHOSEN_KIT]
    push eax
    push ecx
    mov eax, ebp
    call SETUP_PROFILE              ; setup_hook gives it the window's kit, find the queue
.ret:
    ret
.none:
    add esp, 8
    ret

; ebp = the account's profile, ecx = an item id: CF clear and eax = its dict
; id; CF set when the inventory lacks it. ecx, ebx, esi, edi, ebp kept.
item_dict:
    push ecx
    push edi
    mov eax, [GAME_ALLOCATOR]
    cmp [ebp+INVENTORY_ALLOC], eax
    jne .no
    push ecx
    mov ecx, [ebp+INVENTORY]
    mov edi, [ebp+INVENTORY+4]
    push esp                        ; &the id
    call FIND_ITEM
    add esp, 8
    cmp eax, edi
    je .no
    movzx eax, word [eax+ITEM_DICT]
    clc
    jmp .ret
.no:
    stc
.ret:
    pop edi
    pop ecx
    ret

; eax = an upgrade's dict id, edi = the host's id, ebp = the account's
; profile: CF clear and eax = the dict id its record on the host holds: a
; scope's (category 22) the variant made for the host's weapon, any other
; upgrade's its own. CF set: no variant. Every other register kept.
record_dict:
    pushad
    mov esi, [ebp+CLIENT_PLAYERS]
    mov esi, [esi+DICTIONARY]
    push eax
    mov eax, esi
    call ITEM_BY_ID
    test eax, eax
    jz .no
    cmp byte [eax+ENTRY_CATEGORY], SCOPE_CATEGORY
    jne .own
    sub esp, 16                     ; the scope's dict id and name, the weapon's
    mov ecx, [esp+16+28]
    mov [esp], ecx
    mov ecx, [eax+ENTRY_NAME]
    mov [esp+4], ecx
    mov ecx, edi
    call item_dict
    jc .none
    mov [esp+8], eax
    push eax
    mov eax, esi
    call ITEM_BY_ID
    test eax, eax
    jz .none
    mov eax, [eax+ENTRY_NAME]
    mov [esp+12], eax
    mov ecx, [esi+ENTRY_COUNT]
    mov esi, [esi+ENTRIES]
.entry:
    test ecx, ecx
    jz .none
    cmp byte [esi+ENTRY_CATEGORY], SCOPE_CATEGORY
    jne .next
    mov eax, [esi+ENTRY_CONFIG]
    test eax, eax
    jz .next
    mov eax, [eax+CONFIG_ROOT]
    test eax, eax
    jz .next
    mov edx, key_fit
    call get
    jz .next
    mov ebx, eax                    ; srp_scope_fit
    mov edx, key_scope
    call get
    jz .next
    mov edx, [esp]
    mov edi, [esp+4]
    call names
    jne .next
    mov eax, ebx
    mov edx, key_weapons
    call get
    jz .next
    cmp word [eax+NODE_TYPE], T_INDEXED
    jne .next
    movzx ebx, word [eax+NODE_COUNT]
    mov eax, [eax]
    mov edx, [esp+8]
    mov edi, [esp+12]
.weapon:
    test ebx, ebx
    jz .next
    call names
    je .found
    add eax, NODE_SIZE
    dec ebx
    jmp .weapon
.found:
    movzx eax, word [esi]           ; the variant's dict id
    mov [esp+16+28], eax
    add esp, 16
.own:
    popad
    clc
    ret
.next:
    add esi, ENTRY_SIZE
    dec ecx
    jmp .entry
.none:
    add esp, 16
.no:
    popad
    stc
    ret

; eax = a config table, edx = a key: eax = its child, or 0 (ZF set). ebx,
; ecx, esi, edi, ebp kept.
get:
    push ecx
    push esi
    push edi
    mov esi, edx
    mov edi, edx
    push eax
    call HAS_KEY
    mov ecx, eax
    pop eax
    test cl, cl
    jz .none
    call CHILD
    jmp .ret
.none:
    xor eax, eax
.ret:
    pop edi
    pop esi
    pop ecx
    test eax, eax
    ret

; eax = a config node, edx = a dict id, edi = that item's name: ZF set when
; the node names the item (an integer, its dict id; a string, its name).
; Every register kept.
names:
    cmp word [eax+NODE_TYPE], T_INTEGER
    jne .string
    cmp [eax], edx
    ret
.string:
    cmp word [eax+NODE_TYPE], T_STRING
    jne .ret
    push ecx
    push esi
    push edi
    mov esi, [eax]
    test esi, esi
    jnz .char
    or esi, 1                       ; no text: ZF clear
    jmp .out
.char:
    mov cl, [esi]
    cmp cl, [edi]
    jne .out
    inc esi
    inc edi
    test cl, cl
    jnz .char
.out:
    pop edi
    pop esi
    pop ecx
.ret:
    ret

; 0x270313 `call 0x270320` (the 0x87 reader's last call) -> call account:
; the account answers the oldest queued change. Every register kept.
account:
    call pop_op
    jmp ACCOUNT_ITEMS

; The oldest queued change leaves the queue. Every register kept.
pop_op:
    pushad
    mov esi, STATE
    mov ecx, [esi+COUNT]
    jecxz .ret
    dec dword [esi+COUNT]
    lea edi, [esi+OPS]
    lea esi, [edi+OP_SIZE]
    mov ecx, (MAX_OPS - 1) * OP_SIZE / 4
    rep movsd
.ret:
    popad
    ret

; Patch 32's two `call FIND`: eax = a profile (or 0), ecx = an item id. The
; stash's find_kit_or_show (edx = the kit that shows it, or -1; eax = the
; profile to show it from), then, for the item a change was queued on,
; shown from the account's profile: eax = the scratch, the queue applied to
; the item's record, the kit's revision the last one given while the
; item's upgrades are those it was given for. Every other register kept, as
; find_kit_or_show (xmm0 not).
find:
    push eax
    call FIND
    pushad                          ; [esp+28] eax, [esp+24] ecx, [esp+20] edx, [esp+32] the profile given
    mov ebp, [esp+32]
    test edx, edx
    js .ret
    test ebp, ebp
    jz .ret
    mov eax, [GAME_ALLOCATOR]
    cmp [ebp+INVENTORY_ALLOC], eax  ; the account's profile (a character's copy or a squad mate's has none)
    jne .ret
    mov ebx, STATE
    test ecx, ecx
    jz .ret
    cmp [ebx+HOST], ecx
    jne .ret
    mov edi, PREVIEW
    cmp [edi+WINDOW_ITEM], ecx
    jne .ret
    mov esi, [edi+SCRATCH]
    cmp [esp+28], esi
    je .made                        ; the stash's scratch (a weapon no kit holds)
    test esi, esi
    jnz .copy
    mov esi, [GAME_ALLOCATOR]       ; the scratch, once (as the stash's make_scratch)
    mov ecx, ALLOCATION_WHERE
    call ALLOCATION_NAME
    push eax
    mov eax, PROFILE_SIZE
    call ALLOCATE
    test eax, eax
    jz .ret
    mov ecx, eax
    call PROFILE_CTOR
    mov [PREVIEW+SCRATCH], eax
    mov esi, eax
.copy:
    mov edi, ebp
    call PROFILE_COPY
.made:
    mov ecx, [esp+24]
    movzx eax, byte [esi+ITEMS]
    lea edi, [esi+ITEM_TABLE]
.record:
    test eax, eax
    jz .ret                         ; not in the table: as found
    cmp [edi], ecx
    je .apply
    add edi, ITEM_SIZE
    dec eax
    jmp .record
.apply:
    call apply
    mov edx, [esp+20]
    call same
    mov eax, [ebx+LAST_REV]
    je .revision
    mov eax, [PREVIEW+SERIAL]       ; a revision of its own, as the stash's
    inc eax
    or eax, 0x80000000
    mov [PREVIEW+SERIAL], eax
    mov [ebx+LAST_REV], eax
    mov [ebx+LAST_KIT], edx
    push esi
    push edi
    mov esi, [edi+UPGRADES]
    mov ecx, [edi+UPGRADES+4]
    sub ecx, esi
    mov [ebx+LAST_BYTES], ecx
    lea edi, [ebx+LAST]
    rep movsb
    pop edi
    pop esi
    mov eax, [ebx+LAST_REV]
.revision:
    mov [esi+REVISIONS+edx*4], eax
    mov [esp+28], esi
.ret:
    popad
    add esp, 4
    ret

; edi = a record, ebx = the state: the queue applied in order, as the
; server does. With patch 68 (kit_apply, written by the build): its
; weapon_kit_preview_apply (cdecl: the record, the queue, its count; the Rust
; patch weapon_kit), on the weapon kit's logical slots (decals 0 and 1,
; camouflage 2, scope 3, muzzle 4, grip 5), packed again into at most three
; records. Without it (a build with no arena): "put" (slot 0..3) leaves a
; record of the same upgrade, slot and dict id as it is, else takes the
; upgrade and what is in the slot off and adds {upgrade, dict id, 0, slot}
; when there is room; "take off" takes the upgrade off. Every register kept.
apply:
    cmp dword [kit_apply], 0
    je apply_plain
    pushad
    push dword [ebx+COUNT]
    lea eax, [ebx+OPS]
    push eax
    push edi
    call [kit_apply]
    add esp, 12
    popad
    ret
apply_plain:
    pushad
    lea esi, [ebx+OPS]
    mov ecx, [ebx+COUNT]
.op:
    jecxz .done
    push ecx
    mov eax, [esi]
    mov edx, [esi+8]
    mov ebp, [edi+UPGRADES]
.held:
    cmp ebp, [edi+UPGRADES+4]
    jae .take
    cmp [ebp], eax
    jne .other
    cmp [ebp+UP_SLOT], dl
    jne .other
    mov ecx, [esi+4]
    cmp [ebp+UP_DICT], ecx
    je .next                        ; there already
.other:
    add ebp, UP_SIZE
    jmp .held
.take:
    call take_off
    cmp dl, TAKE_OFF
    je .next
    mov ebp, [edi+UPGRADES+4]
    lea ecx, [ebp+UP_SIZE]
    cmp ecx, [edi+UPGRADES+8]
    ja .next                        ; no room (the server refuses it)
    mov [ebp], eax
    mov ecx, [esi+4]
    mov [ebp+UP_DICT], ecx
    xor ecx, ecx
    mov [ebp+UP_VALUE], ecx
    mov [ebp+UP_VALUE+4], ecx
    mov [ebp+UP_SLOT], edx
    mov [ebp+UP_SLOT+4], ecx
    add dword [edi+UPGRADES+4], UP_SIZE
.next:
    pop ecx
    add esi, OP_SIZE
    dec ecx
    jmp .op
.done:
    popad
    ret

; edi = a record, eax = an upgrade, dl = a slot: the record's upgrades of
; that id or in that slot go. Every register kept.
take_off:
    pushad
    mov ebx, edi
    mov esi, [ebx+UPGRADES]
    mov edi, esi
.each:
    cmp esi, [ebx+UPGRADES+4]
    jae .end
    cmp [esi], eax
    je .skip
    cmp [esi+UP_SLOT], dl
    je .skip
    mov ecx, UP_SIZE / 4
    rep movsd
    jmp .each
.skip:
    add esi, UP_SIZE
    jmp .each
.end:
    mov [ebx+UPGRADES+4], edi
    popad
    ret

; edi = a record, edx = a kit, ebx = the state: ZF set when the record's
; upgrades and the kit are those the last revision was given for. Every
; register kept.
same:
    pushad
    cmp [ebx+LAST_KIT], edx
    jne .ret
    mov ecx, [edi+UPGRADES+4]
    mov esi, [edi+UPGRADES]
    sub ecx, esi
    cmp ecx, [ebx+LAST_BYTES]
    jne .ret
    lea edi, [ebx+LAST]
.each:
    test ecx, ecx
    jz .ret
    push ecx
    mov ecx, UP_COMPARED
    repe cmpsb
    pop ecx
    jne .ret
    add esi, UP_SIZE - UP_COMPARED
    add edi, UP_SIZE - UP_COMPARED
    sub ecx, UP_SIZE
    jmp .each
.ret:
    popad
    ret

; 0x284B89 `cmp dword [ebx+0D2C4h],-1` -> call again + 2 nops: setup_profile
; past its early return; ebx = the character, edi = the profile, edx = the
; kit, eax = the kit's id, ecx = the shown copy's index. For the lobby's own
; character: with no player being made, a query starts (its time and kit
; are kept); with one being made of the same kit, id and revision, that is
; what is asked: setup_profile returns (and a call waiting for that player
; is dropped: this one is newer). Else the original's comparison, for its
; `je`.
again:
    push eax
    push ecx
    mov eax, [ebx+MENU]
    cmp [eax+MENU_CHARACTER], ebx
    jne .original                   ; a squad mate's: the original's
    cmp dword [ebx+QUERY], -1
    jne .making
    mov eax, [eax+MENU_CLOCK]
    mov eax, [eax+CLOCK_MS]
    mov [STATE+STARTED], eax
    mov [STATE+MAKING_KIT], edx
    jmp .original
.making:
    cmp [STATE+MAKING_KIT], edx
    jne .original
    mov ecx, [esp]                  ; the copy being made: the shown one's next
    inc ecx
    cmp ecx, 3
    jb .copy
    sub ecx, 3
.copy:
    imul ecx, ecx, PROFILE_SIZE
    lea ecx, [ebx+ecx+COPIES]
    push esi
    imul esi, edx, KIT_SIZE
    mov eax, [esp+8]                ; the kit's id
    cmp [ecx+esi+KIT_ID], eax
    pop esi
    jne .original
    mov eax, [ecx+REVISIONS+edx*4]
    cmp eax, [edi+REVISIONS+edx*4]
    jne .original
    pop ecx
    pop eax
    mov [ebx+SHOWN_KIT_ID], eax     ; the original's next write
    mov byte [ebx+AGAIN], 0
    add esp, 4
    jmp SETUP_DONE
.original:
    pop ecx
    pop eax
    cmp dword [ebx+QUERY], -1
    ret

; 0x284B7C `cmp byte [ebx+0D2C8h],0` -> call early + 2 nops: setup_profile's
; last test before its early return (`jz 284CF8h`, the only reader of the
; flags): the kit asked, its id and revision, are the shown copy's and the
; last asked id (+4) is its id. The original returns when no call waits
; (+D2C8h 0), whatever player is being made; for the lobby's own character
; also only when none is (+D2C4h -1). Patch 32's scratch keeps the chosen
; kit's id with a revision of its own, so a window closed while the
; scratch's player is still being made asked for the shown copy and
; returned: the scratch's player came after, the window's weapon in the
; hands (the owner, 2026-10-23). Now that call waits for it (+D2C8h, at
; 0x284BB2) and on_player_ready makes the account's again (0x2850C5); a call
; for what is being made returns at `again`. ZF set: the early return.
; Every register kept.
early:
    cmp byte [ebx+AGAIN], 0
    jne .ret                        ; a call waits: on, as the original
    push eax
    mov eax, [ebx+MENU]
    cmp [eax+MENU_CHARACTER], ebx
    jne .original                   ; a squad mate's: the original's
    cmp dword [ebx+QUERY], -1       ; ZF: no player being made
    pop eax
    ret
.original:
    cmp eax, eax
    pop eax
.ret:
    ret

; 0x284F7F `cmp byte [ebx+0D2C9h],0` -> call made + 2 nops: on_player_ready,
; ebx = the character. For the lobby's own character, a log line with the
; time since its query started. Then the original's comparison (its `jne`
; follows a `mov`).
made:
    pushad
    mov eax, [ebx+MENU]
    cmp [eax+MENU_CHARACTER], ebx
    jne .done
    mov eax, [eax+MENU_CLOCK]
    mov eax, [eax+CLOCK_MS]
    sub eax, [STATE+STARTED]
    push eax
    push eax
    push eax
    push log_made
    call log
    add esp, 16
.done:
    popad
    cmp byte [ebx+0xD2C9], 0
    ret

; cdecl (format, a, b, c): a line of the game's log, channel "game", info,
; as the game's own (0x29A8E9..0x29A958, relocate_upgrade_item's "ui:
; attach_upgrade"): the function it is written through is LOG_SINK's,
; {LOG_MANAGER | 1, 0, LOG_SINK} (trivially copied, nothing to destroy
; after), or none when LOG_SINK is 0 (an empty function writes nothing).
; Every register but eax, ecx, edx kept.
log:
    cmp dword [LOG_FILTERED], 0
    je .write
    push LEVEL_INFO
    push CHANNEL_GAME
    call LOG_FILTER
    add esp, 8
    test al, al
    jz .ret
.write:
    push ebp
    mov ebp, esp
    xor eax, eax
    push eax                        ; an empty function (its manager 0), 20h bytes
    push eax
    push eax
    push eax
    push eax
    push eax
    push eax
    push eax
    mov eax, [LOG_SINK]
    test eax, eax
    jz .function
    mov dword [esp], LOG_MANAGER | 1
    mov [esp+8], eax
.function:
    mov eax, esp
    push dword [ebp+20]
    push dword [ebp+16]
    push dword [ebp+12]
    push dword [ebp+8]
    push LEVEL_INFO
    push CHANNEL_GAME
    push log_where
    push 0
    push log_where
    push LOG_B
    push dword [LOG_A]
    push eax
    call LOG
    mov esp, ebp
    pop ebp
.ret:
    ret

key_fit:        db "srp_scope_fit", 0
key_scope:      db "scope", 0
key_weapons:    db "weapons", 0
log_where:      db "srp", 0
log_preview:    db "srp: upgrade preview: item %u, upgrade %u, slot %u", 0
log_made:       db "srp: lobby character's player made in %u ms", 0
