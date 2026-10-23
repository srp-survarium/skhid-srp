; Experiment (patch 56, not for a release yet): a weapon's upgrade window
; shows the weapon above the workbench without making the lobby character
; again (client_patch.md section 3.53). Patch 32 asks setup_profile for a
; whole new player holding the weapon (the player's query, its models, the
; armour's camouflage and decal bakes: about 200 ms); here only the weapon is
; made, as the inventory makes a held one (inventory_cook::translate_query
; 0x2C7250: a query of class 46h, "weapon", with the record's data), and its
; models are put in the lobby's scene at patch 45's transform. The character
; keeps the kit it shows, its own weapon in its hand. The last weapon made is
; kept after the close: the same record's next window shows it at once.
; Block fastweapon.code of `.srpx` (0xA432000), its state fastweapon.state
; (0xA433000); patch 32's state at PREVIEW, patch 45's at TABLE.
;   nasm -f bin -DCAVE=0xA432000 -DSTATE=0xA433000 -DPREVIEW=0xA428E00 -DTABLE=0xA428F00 fast_weapon_026g.asm -o fast_weapon_026g.section.bin
;
; Entries (a jump table):
;   +0  setup   0x284B16 `sub esp,6Ch / push ebx / mov ebx,[ebp+8]` (7 bytes,
;               setup_profile after its prologue; patch 32's setup_hook jumps
;               here after giving the window's kit; eax = the profile,
;               [ebp+8] the character, [ebp+0Ch] the kit) -> call + 2 nops:
;               for the lobby's own character with a weapon's window open
;               (patch 32's mode 1) whose kit holds the window's item, the
;               weapon is asked for (`ask`) and setup_profile returns at once:
;               nothing of the character changes. Anything else: the three
;               instructions, and setup_profile goes on.
;   +5  frame   0x284A7E `call 0x2474A0` (lobby_character::update's
;               player::mt_draw; ecx = the player, ebx = the character): for
;               the lobby's own character with the window this code took, the
;               weapon at the bench (its models put in the scene once, moved
;               to the bench's transform every frame) and mt_draw called from
;               here, so that patch 45's is_table (mt_draw's return address
;               0x284A83) is false and the character draws its own weapon in
;               its hand; once closed the weapon's models leave the scene.
;               Otherwise a jump to mt_draw: patch 45 as before.
;   +10 unload  0x21D093 `call 0x2855B0` in the lobby menu's unload (0x21D080,
;               ebp = the menu, before it lets its characters' players go):
;               the weapon of that menu leaves the scene and is let go; a
;               query in flight is let go when it comes.
;   +15 ready   the weapon query's callback (bound by 0x27D280, the game's
;               member callback, as the late join's; invoked by 0x202300 with
;               ecx = the state, [esp+4] = the queries_result; `ret 4`, as
;               lobby_character::on_player_ready 0x284F20).
; The code finds nothing by call/pop: the exe is never rebased; its state,
; patch 32's and patch 45's are absolute.
BITS 32
ORG CAVE

%ifndef STATE
%error STATE is required
%endif
%ifndef PREVIEW
%error PREVIEW (patch 32's state) is required
%endif
%ifndef TABLE
%error TABLE (patch 45's state) is required
%endif

; The game's functions
%define MT_DRAW         0x2474A0    ; player::mt_draw, ecx = the player; `ret`
%define UNLOAD_NEXT     0x2855B0    ; the lobby menu's unload's call (esi = its +114h); `ret`
%define QUERY_RESOURCES 0x0E5B60    ; eax = requests, ecx = count, cdecl (&function, allocator, &user data, parent, 1)
%define MEMBER_CALLBACK 0x27D280    ; edi = the function, stdcall (pmf, 0, object, 0); `ret 10h`
%define WEAPON_TYPE     0x2C8090    ; type_to_int of the weapon query's data: eax (ecx, edx clobbered)
%define WEAPON_HELPER   0xA0E7D0    ; its variant<32> helper
%define WEAPON_PATH     0xA04840    ; "weapon"
%define WEAPON_CLASS    0x46        ; weapon_cook
%define RECORD_CTOR     0x2F7C10    ; inventory_item_descr's ctor, ecx = this
%define RECORD_COPY     0x270390    ; its copy-assign, eax = source, edi = destination
%define RELEASE         0x0E0210    ; a resource's last reference gone: ecx = it, eax = its count
%define ADD_MODEL       0x021CD0    ; eax = &instance, stdcall (renderer, &scene, transform, transform); `ret 10h`
%define REMOVE_MODEL    0x021F40    ; eax = &instance, stdcall (renderer, &scene); `ret 8`
%define UPDATE_MODEL    0x022280    ; eax = &instance, stdcall (renderer, &scene, transform, transform); `ret 10h`
%define CALC_LOCATOR    0x243BC0    ; weapon::calculate_locator: edi = out, stdcall (locator, transform, bones, identity)
%define GAME_ALLOCATOR  0x1F1D9C4
%define LOG             0x111ED0    ; cdecl (&function, [LOG_A], LOG_B, file, line, function, channel, level, format, ...)
%define LOG_FILTER      0x111860    ; cdecl (channel, level): al = it is written
%define LOG_FILTERED    0x1F1D60C
%define LOG_A           0x1F1D610
%define LOG_B           0x21E73B8
%define LOG_SINK        0x1F1D614
%define LOG_MANAGER     0x9DAF4C
%define CHANNEL_GAME    0x9FA4E4    ; "game"
%define LEVEL_INFO      4

; The lobby
%define MENU            0xD2BC      ; lobby_character: the lobby menu
%define UPGRADE_MODE    0xD515      ; lobby_character: 1 while an upgrade window is open
%define MENU_CHARACTER  0x694       ; lobby menu: its own lobby_character
%define MENU_SCENE      0x4         ; lobby menu (a weapon's world): the scene
%define MENU_GAME       0x9C        ; lobby menu: the game
%define GAME_DICTIONARY 0x53A4      ; game: the items dictionary
%define GAME_CLIENT     0x53AC      ; game: the network client (vtable+34h, then +577B0h: the query's sixth word)
%define CLIENT_DATA     0x34
%define CLIENT_WORD     0x577B0
%define GAME_RENDER     0x5360      ; game: +200064h the scene renderer
%define RENDERER        0x200064
%define GAME_CLOCK      0x584C      ; game: the lobby's time, ms
%define KITS            0x57        ; profile: u8 kit count
%define KIT             0x58        ; profile: kit k at +58h + k * 704h, 23 slots {u16 index, u16 amount} at +40h
%define KIT_SIZE        0x704
%define WEAPON_1        (0x40 + 4 * 7)
%define WEAPON_2        (0x40 + 4 * 10)
%define ITEMS           0x1564      ; profile: u8 item count
%define ITEM_TABLE      0x1568      ; profile: records of B4h
%define ITEM_SIZE       0xB4
%define MAX_ITEMS       0x45
%define R_ID            0x00        ; record: the item id
%define R_DICT          0x04        ; record: u16 items_dict id
%define R_FIXED         0x14        ; record: the bytes before its first vector
%define R_FIRST         0x14        ; record: buffer_vector {begin, end, capacity end} of 8-byte entries
%define R_SECOND        0x60        ; record: buffer_vector of 18h-byte upgrades
%define FIRST_SIZE      8
%define SECOND_SIZE     0x18
%define RESULT_FIRST    0x12C       ; queries_result: the first query's resource
%define W_RESOURCE      0x10        ; weapon: the resource the query gives (its inventory_item; weapon_cook's
                                    ; delete_resource 0x24B190 takes 10h off it, inventory_cook keeps it)
%define REFS            0xD0        ; a resource's reference count
%define W_WORLD         0x180C      ; weapon: its world (the lobby menu)
%define W_SCOPE         0x1808      ; weapon: its rifle scope item, or 0
%define W_MODEL         0x1810      ; weapon: its skeleton model instance
%define W_SCOPE_LOCATOR 0x744       ; weapon: the scope's locator (+20h a float4x4, +60h u16 bone)
%define L_BONE          0x60
%define S_MODEL         0x108       ; the scope item: its skeleton model instance (not aimed)
%define MODEL_RENDER    0x108       ; skeleton model instance: its render instance
%define IN_SCENE        0x10D       ; render instance: added to a scene
%define FOREGROUND      0x114       ; render instance: drawn in the first person's depth range
%define MOVABLE         0x115       ; render instance: movable
%define SKELETON_VTABLE 0x9F0F24    ; skeleton_render_model_instance
%define RENDER_MODEL    0x62B4      ; skeleton_render_model_instance: its skeleton_render_model
%define BOX_MIN         0x108
%define BOX_MAX         0x114
%define INVERSE_BINDS   0x140       ; skeleton_render_model: buffer_vector of float4x4 {begin, end}

; Patch 32's state (PREVIEW) and patch 45's (TABLE)
%define P_MODE          0x00        ; byte: 1 while the window's weapon is shown
%define P_ITEM          0x04        ; the window's item
%define T_SPIN          0x20        ; the weapon's turn about the vertical (the drag's yaw)
%define T_TILT          0x24        ; and about the bench's length
%define T_READY         0x28        ; 1: the window's weapon is at the bench (its camera waits for it)
%define T_STAMP         0x48        ; the interface: +1 at each transform written
%define T_MATRIX        0x50        ; the interface: the transform the weapon is drawn with

; The state (STATE)
%define FAST            0x00        ; 1: this window's weapon is this code's
%define ITEM            0x04        ; the window's item
%define HAVE            0x08        ; the weapon kept (one reference ours), or 0
%define SHOWN           0x0C        ; 1: its models are in the scene
%define PENDING         0x10        ; 1: a query is in flight (its record at REC)
%define WANT            0x14        ; 1: a newer record waits at WANT_REC
%define DROP            0x18        ; 1: the query in flight is not wanted any more
%define ASKED           0x1C        ; the lobby's time when the window asked for what it shows
%define TIMING          0x20        ; 1 until that is drawn (then a log line)
%define KEPT            0x24        ; 1: it was the weapon kept, no query
%define WORLD           0x28        ; the lobby menu of the query in flight
%define QUERIES         0x2C        ; queries made (for the checks)
%define READY_SEEN      0x30        ; weapons that came (for the checks)
%define RECORDS_MADE    0x34        ; 1: the three records are constructed
%define MATRIX          0x40        ; float4x4: the weapon's transform
%define SCOPE_MATRIX    0x80        ; float4x4: the scope's
%define BIND            0xC0        ; float4x4: the scope locator's bone in the bind pose
%define REC             0x100       ; the record of the query in flight
%define WANT_REC        0x1C0
%define HAVE_REC        0x280       ; the record HAVE was made from
%define STATE_SIZE      0x340

    jmp near setup
    jmp near frame
    jmp near unload
    jmp near ready

; 0x284B16 -> call setup + 2 nops. eax = the profile, ebp = setup_profile's
; frame ([ebp+8] the character, [ebp+0Ch] the kit). Returns from
; setup_profile (`ret 8`) when it takes the window, else does the three
; instructions with their stack effect (ecx and edx are dead at the site:
; 0x284B1D and 0x284B2B write them first; no flag is read before one is set).
setup:
    pushad
    mov edi, STATE
    mov ebx, [ebp+8]
    mov ecx, [ebx+MENU]
    test ecx, ecx
    jz .original
    cmp [ecx+MENU_CHARACTER], ebx
    jne .original                   ; a squad mate's: as before
    cmp byte [ebx+UPGRADE_MODE], 0
    je .off
    cmp byte [PREVIEW+P_MODE], 0
    je .off                         ; armour, or no kit holds it: patch 32's way
    test eax, eax
    jz .off
    movzx ecx, byte [ebp+0Ch]
    cmp ecx, 3
    jae .off
    cmp cl, [eax+KITS]
    jae .off
    imul ecx, ecx, KIT_SIZE
    movzx esi, word [eax+ecx+KIT+WEAPON_1]
    cmp si, -1
    jne .held
    movzx esi, word [eax+ecx+KIT+WEAPON_2]
.held:
    cmp esi, MAX_ITEMS
    jae .off
    movzx ecx, byte [eax+ITEMS]
    cmp esi, ecx
    jae .off
    imul esi, esi, ITEM_SIZE
    lea esi, [eax+esi+ITEM_TABLE]   ; the record the window's player would hold
    mov ecx, [PREVIEW+P_ITEM]
    cmp [esi+R_ID], ecx
    jne .off
    mov dword [edi+FAST], 1
    mov [edi+ITEM], ecx
    call ask
    popad
    mov esp, ebp                    ; setup_profile returns: nothing of the character changes
    pop ebp
    ret 8
.off:
    mov dword [edi+FAST], 0
    mov dword [edi+TIMING], 0       ; a window closed before its weapon came: no line
.original:
    popad
    pop edx
    sub esp, 6Ch
    push ebx
    mov ebx, [ebp+8]
    jmp edx

; esi = the record the window shows, ebx = the character, edi = the state.
; The weapon kept, the one coming, or a query for it.
ask:
    call make_records
    mov eax, [ebx+MENU]
    mov eax, [eax+MENU_GAME]
    mov edx, [eax+GAME_CLOCK]       ; the lobby's time now
    cmp dword [edi+HAVE], 0
    je .coming
    lea ecx, [edi+HAVE_REC]
    call same
    jne .coming
    mov dword [edi+WANT], 0         ; the weapon kept is the one: nothing in flight is
    cmp dword [edi+PENDING], 0
    je .kept
    mov dword [edi+DROP], 1
.kept:
    cmp dword [edi+SHOWN], 0
    jne .done                       ; at the bench already (a refresh)
    cmp dword [edi+TIMING], 0
    jne .done
    mov [edi+ASKED], edx
    mov dword [edi+TIMING], 1
    mov dword [edi+KEPT], 1
    ret
.coming:
    cmp dword [edi+TIMING], 0
    jne .timed                      ; the window still waits: from its first ask
    mov [edi+ASKED], edx
    mov dword [edi+TIMING], 1
.timed:
    mov dword [edi+KEPT], 0
    cmp dword [edi+PENDING], 0
    je .query
    lea ecx, [edi+REC]
    call same
    jne .later
    mov dword [edi+WANT], 0         ; the one in flight: it is wanted again
    mov dword [edi+DROP], 0
    ret
.later:
    push edi                        ; another one in flight: this one after it
    lea edi, [edi+WANT_REC]
    mov eax, esi
    call RECORD_COPY
    pop edi
    mov dword [edi+WANT], 1
    mov dword [edi+DROP], 1
    ret
.query:
    push edi
    lea edi, [edi+REC]
    mov eax, esi
    call RECORD_COPY
    pop edi
    mov eax, [ebx+MENU]
    mov [edi+WORLD], eax
    call query
.done:
    ret

; The three records of the state constructed once (their vectors point into
; them). edi = the state; every register but eax, ecx, edx kept.
make_records:
    cmp dword [edi+RECORDS_MADE], 0
    jne .ret
    lea ecx, [edi+REC]
    call RECORD_CTOR
    lea ecx, [edi+WANT_REC]
    call RECORD_CTOR
    lea ecx, [edi+HAVE_REC]
    call RECORD_CTOR
    mov dword [edi+RECORDS_MADE], 1
.ret:
    ret

; ZF set when the records at esi and ecx are alike: the item, its dict id,
; the words before the vectors and both vectors' entries. Keeps every
; register but eax.
same:
    push esi
    push edi
    push ecx
    push edx
    mov edi, ecx
    mov eax, [esi+R_ID]
    cmp eax, [edi+R_ID]
    jne .out
    mov ax, [esi+R_DICT]
    cmp ax, [edi+R_DICT]
    jne .out
    mov eax, [esi+8]
    cmp eax, [edi+8]
    jne .out
    mov eax, [esi+0Ch]
    cmp eax, [edi+0Ch]
    jne .out
    mov eax, [esi+10h]
    cmp eax, [edi+10h]
    jne .out
    mov edx, R_FIRST
    call .vector
    jne .out
    mov edx, R_SECOND
    call .vector
.out:
    pop edx
    pop ecx
    pop edi
    pop esi
    ret
; the vector at +edx of both: ZF when as long and byte for byte the same
.vector:
    push esi
    push edi
    mov ecx, [esi+edx+4]
    mov esi, [esi+edx]
    sub ecx, esi
    mov eax, [edi+edx+4]
    mov edi, [edi+edx]
    sub eax, edi
    cmp eax, ecx
    jne .vector_out
    repe cmpsb                      ; ecx 0: ZF from the cmp above (equal)
.vector_out:
    pop edi
    pop esi
    ret

; A query of the weapon of REC (class 46h, "weapon"), its data as the
; inventory's (0x2C7340..0x2C73D5): the record's two vectors, the
; dictionary, the world (the lobby menu, the weapon's +180Ch), the dict id,
; the network client's word, 1. edi = the state, [edi+WORLD] the menu. Every
; register kept.
query:
    pushad
    mov ebp, edi
    sub esp, 60h
    ; [esp] the variant (30h), [esp+30h] the user data (one pointer),
    ; [esp+38h] the request {path, class}, [esp+40h] the function (20h)
    mov edi, esp
    xor eax, eax
    mov ecx, 60h / 4
    rep stosd
    mov esi, esp
    mov ebx, [ebp+WORLD]
    mov dword [esi], WEAPON_HELPER
    lea eax, [ebp+REC+R_FIRST]
    mov [esi+8], eax
    lea eax, [ebp+REC+R_SECOND]
    mov [esi+0Ch], eax
    mov eax, [ebx+MENU_GAME]
    mov ecx, [eax+GAME_DICTIONARY]
    mov [esi+10h], ecx
    mov [esi+14h], ebx
    movzx ecx, word [ebp+REC+R_DICT]
    mov [esi+18h], ecx
    mov ecx, [eax+GAME_CLIENT]
    mov edx, [ecx]
    call [edx+CLIENT_DATA]
    mov eax, [eax+CLIENT_WORD]
    mov [esi+1Ch], eax
    mov dword [esi+20h], 1
    mov [esi+28h], esi
    call WEAPON_TYPE
    mov [esi+2Ch], eax
    mov [esp+30h], esi
    mov dword [esp+38h], WEAPON_PATH
    mov dword [esp+3Ch], WEAPON_CLASS
    push 0
    push ebp                        ; the object: the state
    push 0
    push 0
    mov dword [esp], ready          ; the member function
    lea edi, [esp+10h+40h]
    call MEMBER_CALLBACK
    mov dword [ebp+PENDING], 1      ; before the query: its callback may not wait
    mov dword [ebp+DROP], 0
    inc dword [ebp+QUERIES]
    push 1
    push 0                          ; no parent: it runs on its own
    lea eax, [esp+8+30h]
    push eax
    push dword [GAME_ALLOCATOR]
    lea eax, [esp+10h+40h]
    push eax
    lea eax, [esp+14h+38h]
    mov ecx, 1
    call QUERY_RESOURCES
    add esp, 14h
    add esp, 60h
    popad
    ret

; The query's callback: ecx = the state, [esp+4] = the queries_result (which
; holds the weapon's resource until this returns). The weapon is kept (a
; reference of ours; HAVE the weapon, its resource at +10h) unless it is not
; wanted any more; a newer record's query follows.
ready:
    pushad
    mov edi, ecx
    mov eax, [esp+24h]
    mov esi, [eax+RESULT_FIRST]
    mov dword [edi+PENDING], 0
    inc dword [edi+READY_SEEN]
    test esi, esi
    jz .next
    cmp dword [edi+DROP], 0
    jne .next
    lock inc dword [esi+REFS]
    sub esi, W_RESOURCE             ; the weapon
    call let_go                     ; the one kept before, if any
    mov [edi+HAVE], esi
    push edi
    lea eax, [edi+REC]
    lea edi, [edi+HAVE_REC]
    call RECORD_COPY
    pop edi
.next:
    mov dword [edi+DROP], 0
    cmp dword [edi+WANT], 0
    je .done
    mov dword [edi+WANT], 0
    push edi
    lea eax, [edi+WANT_REC]
    lea edi, [edi+REC]
    call RECORD_COPY
    pop edi
    call query
.done:
    popad
    ret 4

; HAVE out of the scene and let go. edi = the state; every register kept.
let_go:
    pushad
    mov esi, [edi+HAVE]
    test esi, esi
    jz .done
    call hide
    mov dword [edi+HAVE], 0
    or eax, -1
    lock xadd [esi+W_RESOURCE+REFS], eax
    cmp eax, 1
    jne .done                       ; others hold it
    lea ecx, [esi+W_RESOURCE]
    lea eax, [esi+W_RESOURCE+REFS]
    call RELEASE
.done:
    popad
    ret

; The models of the weapon at esi out of its world's scene. edi = the state.
; Every register kept.
hide:
    cmp dword [edi+SHOWN], 0
    je .ret
    pushad
    mov dword [edi+SHOWN], 0
    call scene_of                   ; ebx = the renderer, [esp] the scene
    call model_of
    jz .scope
    call remove
.scope:
    call scope_of
    jz .gone
    call remove
.gone:
    add esp, 4
    popad
.ret:
    ret
; eax = &instance in the scene: out of it. ebx = the renderer, [esp+4] the scene.
remove:
    mov ecx, [eax]
    cmp byte [ecx+IN_SCENE], 0
    je .ret
    lea ecx, [esp+4]
    push ecx
    push ebx
    call REMOVE_MODEL
.ret:
    ret

; esi = a weapon: ebx = its world's scene renderer, and the scene pushed
; under the return address (the caller pops it). eax, ecx clobbered.
scene_of:
    pop ecx
    mov eax, [esi+W_WORLD]
    push dword [eax+MENU_SCENE]
    mov eax, [eax+MENU_GAME]
    mov eax, [eax+GAME_RENDER]
    mov ebx, [eax+RENDERER]
    jmp ecx

; esi = a weapon: eax = &its model's render instance (ZF clear), or ZF set
; when it has none. Keeps every other register.
model_of:
    mov eax, [esi+W_MODEL]
    test eax, eax
    jz .ret
    add eax, MODEL_RENDER
    cmp dword [eax], 0
.ret:
    ret
; the same for its scope's model (not aimed)
scope_of:
    mov eax, [esi+W_SCOPE]
    test eax, eax
    jz .ret
    mov eax, [eax+S_MODEL]
    test eax, eax
    jz .ret
    add eax, MODEL_RENDER
    cmp dword [eax], 0
.ret:
    ret

; 0x284A7E `call 0x2474A0` -> call frame. ecx = the player, ebx = the
; character, [esp] = 0x284A83. Every register mt_draw is given is kept.
frame:
    pushad
    mov edi, STATE
    mov eax, [ebx+MENU]
    cmp [eax+MENU_CHARACTER], ebx
    jne .tail                       ; a squad mate's
    cmp dword [edi+FAST], 0
    je .closed
    cmp byte [ebx+UPGRADE_MODE], 0
    je .closed
    cmp byte [PREVIEW+P_MODE], 0
    je .closed
    mov esi, [edi+HAVE]
    test esi, esi
    jz .own                         ; not here yet: the character as it is
    mov eax, [edi+HAVE_REC+R_ID]
    cmp eax, [edi+ITEM]
    je .bench
    call hide                       ; another item's (a window over a window)
    jmp .own
.bench:
    call place
    call show
    mov dword [TABLE+T_READY], 1    ; patch 45: the window's weapon is at the bench
    lea esi, [edi+MATRIX]
    push edi
    mov edi, TABLE+T_MATRIX
    mov ecx, 16
    rep movsd
    pop edi
    inc dword [TABLE+T_STAMP]
    cmp dword [edi+TIMING], 0
    je .own
    cmp dword [edi+WANT], 0
    jne .own                        ; a newer one is coming: the time is its
    cmp dword [edi+PENDING], 0
    je .timed
    cmp dword [edi+DROP], 0
    je .own
.timed:
    mov dword [edi+TIMING], 0
    mov eax, [ebx+MENU]
    mov eax, [eax+MENU_GAME]
    mov eax, [eax+GAME_CLOCK]
    sub eax, [edi+ASKED]
    mov ecx, log_new
    cmp dword [edi+KEPT], 0
    je .log
    mov ecx, log_kept
.log:
    push ecx
    push eax
    push dword [edi+ITEM]
    push log_drawn
    call log
    add esp, 16
.own:
    popad
    call MT_DRAW                    ; from here: patch 45 leaves the held item in the hand
    ret
.closed:
    mov esi, [edi+HAVE]
    test esi, esi
    jz .tail
    call hide                       ; the window closed: out of the scene, kept
.tail:
    popad
    jmp MT_DRAW

; The weapon's (esi) models into the scene, once, at MATRIX and
; SCOPE_MATRIX; then moved there. edi = the state, ebx = the character.
; Every register kept.
show:
    pushad
    call scene_of
    cmp dword [edi+SHOWN], 0
    jne .update
    mov dword [edi+SHOWN], 1
    call model_of
    jz .scope
    call flags
    lea ecx, [edi+MATRIX]
    call add
.scope:
    call scope_of
    jz .update
    call flags
    lea ecx, [edi+SCOPE_MATRIX]
    call add
.update:
    call model_of
    jz .scope_update
    lea ecx, [edi+MATRIX]
    call update
.scope_update:
    call scope_of
    jz .done
    lea ecx, [edi+SCOPE_MATRIX]
    call update
.done:
    add esp, 4
    popad
    ret
; eax = &instance: not in the first person's depth range, movable (as
; weapon::on_show leaves a third person's: 0x243F63, 0x243F6A)
flags:
    mov edx, [eax]
    mov byte [edx+FOREGROUND], 0
    mov byte [edx+MOVABLE], 1
    ret
; eax = &instance, ecx = the transform, ebx = the renderer, [esp+4] the scene
add:
    mov edx, [eax]
    cmp byte [edx+IN_SCENE], 0
    jne .ret
    push ecx
    push ecx
    lea edx, [esp+0Ch]
    push edx
    push ebx
    call ADD_MODEL
.ret:
    ret
update:
    mov edx, [eax]
    cmp byte [edx+IN_SCENE], 0
    je .ret
    push ecx
    push ecx
    lea edx, [esp+0Ch]
    push edx
    push ebx
    call UPDATE_MODEL
.ret:
    ret

; MATRIX: patch 45's bench transform for the weapon at esi in its bind pose
; (rows, v * M): rotY(spin) * rotZ(tilt), the weapon's middle at BENCH;
; SCOPE_MATRIX: the scope's locator through it (calculate_locator, the
; locator's bone in the bind pose). edi = the state. Every register kept.
place:
    pushad
    lea eax, [edi+MATRIX]
    fld dword [TABLE+T_SPIN]
    fsincos                         ; st0 = cos a, st1 = sin a
    fld dword [TABLE+T_TILT]
    fsincos                         ; st0 = cos b, st1 = sin b, st2 = cos a, st3 = sin a
    fld st1
    fchs
    fstp dword [eax+10h]            ; row 1: (-sin b, cos b, 0)
    fst dword [eax+14h]
    fld st0
    fmul st0, st3
    fstp dword [eax+00h]            ; row 0: (cos a cos b, cos a sin b, sin a)
    fld st1
    fmul st0, st3
    fstp dword [eax+04h]
    fld st3
    fstp dword [eax+08h]
    fld st0
    fmul st0, st4
    fchs
    fstp dword [eax+20h]            ; row 2: (-sin a cos b, -sin a sin b, cos a)
    fld st1
    fmul st0, st4
    fchs
    fstp dword [eax+24h]
    fld st2
    fstp dword [eax+28h]
    fcompp
    fcompp
    xorps xmm0, xmm0
    movss [eax+0Ch], xmm0
    movss [eax+18h], xmm0
    movss [eax+1Ch], xmm0
    movss [eax+2Ch], xmm0
    movups [eax+30h], xmm0
    call middle                     ; xmm0 = the middle (w 0)
    mov edx, eax
    call transform
    movups xmm1, [bench]
    subps xmm1, xmm0
    movups [eax+30h], xmm1          ; row 3: BENCH - middle * the rows (w 1)
    ; the scope
    mov ecx, [esi+W_SCOPE]
    test ecx, ecx
    jz .done
    mov ebx, identity               ; the bones: none read without a bone
    movzx ecx, word [esi+W_SCOPE_LOCATOR+L_BONE]
    cmp ecx, 0FFFFh
    je .locator
    call bones_of                   ; edx = the inverse binds, eax = their count
    jz .done
    cmp ecx, eax
    jae .done
    shl ecx, 6
    add edx, ecx                    ; its inverse bind
    lea ebx, [edi+BIND]
    call rigid_inverse
    sub ebx, ecx                    ; bones[i] is BIND
.locator:
    lea eax, [edi+MATRIX]
    lea edx, [esi+W_SCOPE_LOCATOR]
    push edi
    push identity
    push ebx
    push eax
    push edx
    lea edi, [edi+SCOPE_MATRIX]
    call CALC_LOCATOR
    pop edi
.done:
    popad
    ret

; esi = the weapon: edx = its model's inverse binds, eax = how many (ZF set
; when there are none to read). Keeps every other register.
bones_of:
    call render_model
    jz .ret
    mov edx, [eax+INVERSE_BINDS]
    mov eax, [eax+INVERSE_BINDS+4]
    sub eax, edx
    shr eax, 6
    test edx, edx
    jz .ret
    test eax, eax
.ret:
    ret

; esi = the weapon: eax = its skeleton_render_model (ZF clear), or ZF set.
render_model:
    mov eax, [esi+W_MODEL]
    test eax, eax
    jz .ret
    mov eax, [eax+MODEL_RENDER]
    test eax, eax
    jz .ret
    cmp dword [eax], SKELETON_VTABLE
    jne .none
    mov eax, [eax+RENDER_MODEL]
    test eax, eax
    ret
.none:
    xor eax, eax
.ret:
    ret

; [ebx] = the inverse of the rigid float4x4 at [edx] (rows: R, then t):
; R transposed, then -(t . row j of R) for each j. Uses xmm1..xmm3; keeps
; every register.
rigid_inverse:
%assign j 0
%rep 3
    movss xmm1, [edx+j*4]           ; row j of the result: column j of R
    movss [ebx+j*16], xmm1
    movss xmm1, [edx+16+j*4]
    movss [ebx+j*16+4], xmm1
    movss xmm1, [edx+32+j*4]
    movss [ebx+j*16+8], xmm1
    mov dword [ebx+j*16+12], 0
    movss xmm1, [edx+48]
    mulss xmm1, [edx+j*16]
    movss xmm2, [edx+52]
    mulss xmm2, [edx+j*16+4]
    addss xmm1, xmm2
    movss xmm2, [edx+56]
    mulss xmm2, [edx+j*16+8]
    addss xmm1, xmm2
    xorps xmm3, xmm3
    subss xmm3, xmm1
    movss [ebx+48+j*4], xmm3
%assign j j+1
%endrep
    mov dword [ebx+60], 0x3F800000
    ret

; xmm0 = the weapon's (esi) middle in its bind space (w 0): MIDDLES by the
; dict id of HAVE_REC, else its box's centre turned as the instance's box
; (rotY(pi)) with x 0, else (0, 0, MIDDLE_Z). edi = the state. Uses
; xmm1..xmm2; keeps every register.
middle:
    push eax
    push ecx
    push edx
    movzx eax, word [edi+HAVE_REC+R_DICT]
    mov edx, middles
.find:
    movzx ecx, word [edx]
    test ecx, ecx
    jz .box
    add edx, 8
    cmp ecx, eax
    jne .find
    movsx eax, word [edx-6]
    cvtsi2ss xmm0, eax
    movsx eax, word [edx-4]
    cvtsi2ss xmm1, eax
    movsx eax, word [edx-2]
    cvtsi2ss xmm2, eax
    unpcklps xmm0, xmm1
    movlhps xmm0, xmm2
    movss xmm1, [millimetre]
    shufps xmm1, xmm1, 0
    mulps xmm0, xmm1
    jmp .done
.box:
    movups xmm0, [fallback]
    call render_model
    jz .done
    cmp dword [eax+BOX_MIN], 0xBF800000
    jne .own
    cmp dword [eax+BOX_MAX], 0x3F800000
    je .done                        ; the exporter's default box: the fallback
.own:
    xorps xmm0, xmm0
    movss xmm1, [eax+BOX_MIN+4]
    addss xmm1, [eax+BOX_MAX+4]
    movss xmm2, [eax+BOX_MIN+8]
    addss xmm2, [eax+BOX_MAX+8]
    unpcklps xmm0, xmm1
    movlhps xmm0, xmm2
    movups xmm1, [flip_half]
    mulps xmm0, xmm1
.done:
    pop edx
    pop ecx
    pop eax
    ret

; xmm0 = (x, y, z, w) * the matrix at edx (rows; w not read): x row 0 +
; y row 1 + z row 2 + row 3. Uses xmm1..xmm3.
transform:
    movaps xmm1, xmm0
    shufps xmm1, xmm1, 0x00
    movups xmm3, [edx]
    mulps xmm1, xmm3
    movaps xmm2, xmm0
    shufps xmm2, xmm2, 0x55
    movups xmm3, [edx+16]
    mulps xmm2, xmm3
    addps xmm1, xmm2
    movaps xmm2, xmm0
    shufps xmm2, xmm2, 0xAA
    movups xmm3, [edx+32]
    mulps xmm2, xmm3
    addps xmm1, xmm2
    movups xmm3, [edx+48]
    addps xmm1, xmm3
    movaps xmm0, xmm1
    ret

; 0x21D093 `call 0x2855B0` -> call unload. ebp = the lobby menu (its unload
; 0x21D080), esi = its +114h. Every register kept.
unload:
    pushad
    mov edi, STATE
    mov esi, [edi+HAVE]
    test esi, esi
    jz .flight
    cmp [esi+W_WORLD], ebp
    jne .flight
    call let_go
.flight:
    cmp dword [edi+PENDING], 0
    je .done
    cmp [edi+WORLD], ebp
    jne .done
    mov dword [edi+DROP], 1         ; the query in flight is let go when it comes
    mov dword [edi+WANT], 0
.done:
    mov dword [edi+FAST], 0
    mov dword [edi+TIMING], 0
    popad
    jmp UNLOAD_NEXT

; cdecl (format, a, b, c): a line of the game's log, channel "game", info,
; as patch 48's (upgrade_preview_026g.asm `log`). Every register but eax,
; ecx, edx kept.
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
    push eax                        ; the function the line goes through, 20h bytes
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
    mov [esp+8], eax
    mov eax, LOG_MANAGER | 1        ; (not `mov dword [esp], imm`: that is ready's marker)
    mov [esp], eax
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

align 16, db 0
bench:            dd 4.25, 1.30, 3.05, 1.0  ; patch 45's BENCH (w 1, for row 3)
flip_half:        dd -0.5, 0.5, -0.5, 0.0   ; the box's centre, turned as the instance's box (rotY(pi))
fallback:         dd 0.0, 0.0, 0.2, 0.0     ; patch 45's MIDDLE_Z
identity:         dd 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
millimetre:       dd 0.001
log_where:        db "srp", 0
log_drawn:        db "srp: fast weapon: item %u at the bench %u ms after the window asked (%s)", 0
log_new:          db "made", 0
log_kept:         db "kept", 0
; MIDDLES, patch 45's (weapon_table_026g.asm): items_dict id, the gun's
; middle in its bind space (mm)
middles:          dw 12, 5, -2, 240     ; toz_122 (weapons/toz_122)
                  dw 13, 10, -37, 88    ; ak_74u (weapons/ak74u)
                  dw 14, 15, -4, 270    ; rem_700 (weapons/rem_700)
                  dw 16, 1, -38, 275    ; toz_34 (weapons/toz34/toz_34)
                  dw 56, 1, 1, 96       ; magnum (weapons/magnum/magnum)
                  dw 77, -28, -39, 231  ; rpd (weapons/rpd/rpd)
                  dw 80, 10, -30, 167   ; akmn (weapons/akmn/akmn)
                  dw 90, 4, -8, 40      ; winchester_1894 (weapons/winchester_1894)
                  dw 107, 15, 11, 221   ; mosin_nagant (weapons/mosin_nagant/mosin_nagant)
                  dw 108, 0, -31, 276   ; izh_27_lancaster (weapons/izh_27_lancaster/izh_27_lancaster_)
                  dw 110, 10, -25, 309  ; mp_153 (weapons/mp153/mp153)
                  dw 125, 10, -17, 162  ; benelli_m4 (weapons/benelli_m4/benelli_m4)
                  dw 216, -5, -31, 21   ; vepr (weapons/vepr)
                  dw 220, 8, -28, 169   ; a_545 (weapons/a_545/a_545)
                  dw 303, 11, -16, 178  ; a9_91 (weapons/a9_91/a9_91)
                  dw 337, 0, -31, 276   ; izh_27_lancaster_premium (weapons/izh_27_lancaster/izh_27_lancaster_)
                  dw 2214, 11, -29, 174 ; valmet (weapons/valmet_rk62/valmet_rk62)
                  dw 2215, 10, -6, 141  ; soc_vepr (weapons/soc_vepr/soc_vepr)
                  dw 2227, 10, -17, 256 ; saiga_12 (weapons/saiga_12s/saiga_12s)
                  dw 2228, 10, -17, 256 ; saiga_12_premium (weapons/saiga_12s/saiga_12s_camo)
                  dw 2241, 10, -30, 167 ; akmn_premium (weapons/akmn/akmn_camo)
                  dw 2242, 10, -30, 167 ; akmn_premium_2 (weapons/akmn/akmn_camo_st)
                  dw 2243, 10, -37, 88  ; ak_74u_premium (weapons/aks74u_camo/ak74u_camo)
                  dw 2244, -5, -31, 21  ; vepr_premium (weapons/vepr_camo/vepr_camo)
                  dw 2249, 15, -4, 270  ; rem_700_premium (weapons/rem_700_camo_st)
                  dw 2251, 0, -31, 276  ; izh_27_lancaster_premium_2 (weapons/izh_27_lancaster_camo/izh_27_lancaster_camo_st)
                  dw 0
