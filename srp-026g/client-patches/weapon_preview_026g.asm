; Patch 32: a weapon's upgrade window ("Optika", the scope slot) shows the
; weapon (build 2777, client_patch.md section 3.30). Five entries at fixed offsets
; (WEAPON_PREVIEW_* in patch_026g.py, which checks each hook's target). At
; .srp+2E50h, after the late-join code (which ends at +2E4Eh) and before
; `.srpd` (+3000h); its state at STATE, 0xE00 of `.srpd`.
;   nasm -f bin -DCAVE=0xA427E50 -DSTATE=0xA428E00 -DFIND=0xA42936F weapon_preview_026g.asm -o weapon_preview_026g.section.bin
; (FIND: weapon_stash_026g.asm's find_kit_or_show, which also shows a weapon no kit holds;
; its start, exit and setup_profile hooks reach start_cam, exit_cam and setup_hook through it)
;
; The lobby's 3D view is one character, survarium::lobby_character (lobby
; menu+694h), which shows one kit of the account's profile (lobby
; menu+6A0h: up to 3 kits of 704h at +58h, the item table at +1568h):
; setup_profile (0x284B10; eax = a profile, [esp+4] the character, [esp+8]
; the kit) builds the player of that kit, who holds its weapon 1 (kit slot
; 7) or, without one, its weapon 2 (slot 10). The movie's
; start_item_upgrade(item) (0x29A870 -> 0x265DE0) aims the lobby camera
; (the setter 0x2845C0) at the character's chest: look-at height 1.3 (0.8
; for the kit's legs armour, slot 4), side angle 0.37, distance 1.5, the
; closest the camera's tick (0x283EF0) allows. exit_item_upgrade (0x29A890
; -> 0x265F10) aims it back.
;
; Here, for a window whose item is the weapon a kit holds:
;   start_cam   the character shows that kit (setup_profile, whose kit
;               setup_hook replaces while the window is open, so that every
;               refresh, as after an attach, keeps it) and the camera looks
;               at the weapon: WEAPON_HEIGHT, WEAPON_DISTANCE
;   exit_cam    the character shows the kit chosen in the lobby again
;   setup_hook  setup_profile's kit, for the open window's character
;   min_step    the camera's least distance (two sites, flags kept): 1.5,
;               or NEAR_MIN from a weapon's window until the camera is
;               back at 1.5
;   min_final   the same at the tick's last clamp, which ends NEAR_MIN
; weapon_draw_026g.asm reads the mode too: while it is 1 the player is not
; drawn for the decal editor (bind pose, weapon put away).
; Any other window (armour; a weapon no kit holds, as one only in the
; stash, or a weapon 2 behind a weapon 1) does what the original does. The
; code finds its own address (call/pop): a rebase changes nothing.
BITS 32
ORG CAVE

%ifndef STATE
%error STATE is required
%endif
%ifndef FIND
%define FIND find_kit               ; the build's: weapon_stash_026g.asm's find_kit_or_show (-DFIND)
%endif

%define SETUP_PROFILE   0x284B10    ; lobby_character::setup_profile, `ret 8`
%define CAMERA_SET      0x2845C0    ; lobby camera: xmm0 height, xmm1 distance, [esp+4] side angle; `ret 4`
%define MENU            0xD2BC      ; lobby_character: the lobby menu
%define UPGRADE_MODE    0xD515      ; lobby_character: 1 while an upgrade window is open
%define PROFILE         0x6A0       ; lobby menu: the account's profile
%define KITS            0x57        ; profile: u8 kit count
%define KIT             0x58        ; profile: kit k at +58h + k * 704h
%define KIT_SIZE        0x704
%define SLOTS           0x40        ; kit: 23 slots {u16 item index, u16 amount}
%define WEAPON_1        (SLOTS + 4 * 7)
%define WEAPON_2        (SLOTS + 4 * 10)
%define ITEMS           0x1564      ; profile: u8 item count
%define ITEM_TABLE      0x1568      ; profile: items of B4h, +0 the item id
%define ITEM_SIZE       0xB4
%define CHOSEN_KIT      0x4632      ; profile: the kit chosen in the lobby

; The state, at STATE (the offsets below):
%define MODE            0           ; 1: the open window's weapon is shown
%define ITEM            4           ; the open window's item
%define NEAR            8           ; 1: the least distance is NEAR_MIN
%define CONST(label)    ((label) - STATE)

; 0x265E73 `call 0x2845C0` in start_item_upgrade's 0x265DE0 -> call
; start_cam: esi = the character, edi = the item, eax = the camera, ecx =
; where the setter keeps the distance it replaces; xmm0, xmm1 and [esp+4]
; are the original's values; xmm3 is read after the call.
start_cam:
    pushad
    sub esp, 48
    movups [esp], xmm0
    movups [esp+16], xmm1
    movups [esp+32], xmm3
    call state
    xchg ebx, eax                   ; ebx = the state
    mov ebp, [ebx+MODE]             ; a weapon shown before (a window opened over another)
    mov [ebx+ITEM], edi
    mov eax, [esi+MENU]
    mov eax, [eax+PROFILE]
    mov ecx, edi
    call FIND
    inc edx
    setnz byte [ebx+MODE]           ; 1: a kit holds it
    or ebp, [ebx+MODE]
    jz .camera                      ; no weapon now or before: the original
    call show_kit                   ; the weapon's kit (setup_hook), or the chosen one back
.camera:
    movups xmm0, [esp]
    movups xmm1, [esp+16]
    movups xmm3, [esp+32]
    cmp byte [ebx+MODE], 0
    je .set
    movss xmm0, [ebx+CONST(weapon_height)]
    movss xmm1, [ebx+CONST(weapon_distance)]
    mov byte [ebx+NEAR], 1
.set:
    add esp, 48
    popad
    jmp CAMERA_SET

; 0x265F44 `call 0x2845C0` in exit_item_upgrade's 0x265F10 -> call
; exit_cam: esi = the character, eax = the camera, ecx = a local the setter
; writes, xmm0 1.2, xmm1 the distance kept at the start, [esp+4] 0.
exit_cam:
    pushad
    sub esp, 32
    movups [esp], xmm0
    movups [esp+16], xmm1
    call state
    xchg ebx, eax
    xor ebp, ebp
    xchg ebp, [ebx+MODE]
    test ebp, ebp
    jz .camera
    call show_kit                   ; MODE is 0: the kit chosen in the lobby
.camera:
    movups xmm0, [esp]
    movups xmm1, [esp+16]
    cmp byte [ebx+NEAR], 0
    je .set
    maxss xmm1, [ebx+CONST(least)]  ; a distance kept while a weapon was looked at
.set:
    add esp, 32
    popad
    jmp CAMERA_SET

; 0x284B10 (setup_profile's `push ebp / mov ebp,esp / and esp,-8`) -> jmp
; setup_hook. eax = the profile, [esp+4] the character, [esp+8] the kit.
setup_hook:
    push eax
    call state
    xchg edx, eax                   ; edx = the state, eax = the profile
    pop eax
    cmp byte [edx+MODE], 0
    je .original
    push ecx
    push edx
    mov ecx, [esp+12]               ; the character: only the lobby's own has an upgrade window
    cmp byte [ecx+UPGRADE_MODE], 0
    je .restore
    mov ecx, [edx+ITEM]
    call FIND
    test edx, edx
    js .restore
    mov [esp+16], edx               ; the weapon's kit
.restore:
    pop edx
    pop ecx
.original:
    push ebp
    mov ebp, esp
    and esp, -8
    jmp SETUP_PROFILE + 6

; 0x284081 and 0x28428C `movss xmm1,[0xA7AFA4]` (1.5, the least distance)
; -> call min_step + 3 nops. No flag changes: 0x284089 jumps on the flags
; of the comparison before it.
min_step:
    push eax
    push ecx
    call state
    mov ecx, [eax+NEAR]
    movss xmm1, [eax+ecx*4+CONST(least)]
    pop ecx
    pop eax
    ret

; 0x284316 `movss xmm0,[0xA7AFA4]` -> call min_final + 3 nops; xmm1 = the
; distance it clamps (a comparison follows). Once no weapon is looked at
; and the camera is back at the original's least, the least is 1.5 again.
min_final:
    push eax
    push ecx
    call state
    mov ecx, [eax+NEAR]
    jecxz .load
    cmp byte [eax+MODE], 0
    jne .load
    comiss xmm1, [eax+CONST(back)]
    jb .load
    xor ecx, ecx
    mov [eax+NEAR], ecx
.load:
    movss xmm0, [eax+ecx*4+CONST(least)]
    pop ecx
    pop eax
    ret

; eax = the state where this image was loaded. No flag changes.
state:
    call .pc
.pc:
    pop eax
    lea eax, [eax + STATE - .pc]
    ret

; esi = the character: setup_profile(the account's profile, esi, the kit
; chosen in the lobby), which setup_hook may give another kit.
show_kit:
    mov eax, [esi+MENU]
    mov eax, [eax+PROFILE]          ; not 0: a start found the weapon in it
    movzx ecx, byte [eax+CHOSEN_KIT]
    push ecx
    push esi
    call SETUP_PROFILE
    ret

; eax = a profile (or 0), ecx = an item id (no item has the id 0): edx =
; the first kit (below the profile's count) whose held weapon
; (slot 7, or slot 10 when 7 is empty) is that item, or -1. Keeps every
; other register.
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
    jae .kit                        ; an empty slot (FFFFh) or past the table
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

least:            dd 1.5            ; the original's least distance (the float at 0xA7AFA4)
near_min:         dd 0.6            ; NEAR_MIN: the least from a weapon's window
back:             dd 1.49           ; at or past it the least is 1.5 again
weapon_height:    dd 1.05           ; WEAPON_HEIGHT: the look-at height (chest 1.3, legs 0.8)
weapon_distance:  dd 0.85           ; WEAPON_DISTANCE (the chest's 1.5)
