; Patch 67 (build 2777): the kill feed shows a kill through a wall, a kill by
; a ricochet and a kill by pain. The arena's block kill_feed.code (link_026g.py links it;
; patch_026g.py's KILL_FEED_HOOKS call the two entries by their names, and
; KILL_FEED_ARGUMENT widens one load). client_patch.md section 3.55; the
; rules: ../docs/notes/kill-feed-icons.md.
;
; A death makes a kill event in every game alike: base_player::
; generate_killed_event (0x2DA170; esi = the victim, ebp = the killing
; bullet or 0, from hit_impl 0x2D9F15) takes a node of the world's events
; (0x2D43B0 zeroes its +0..+0Bh) and writes the victim (+0), the killer (+1),
; the weapon's icon (+2) and the cause icon (+4: 1 a headshot, 3 a suicide or
; no killer, else 0), then queues it (0x2D0A60). When it is played, the hud's
; event handler (0x23B5F0, edi = the node) gives game_world_ui::
; on_player_killed (0x237C70; the hud, victim, killer, weapon icon, cause icon;
; `ret 14h`) the five, which sets the movie's description (action_id,
; who_name ... extra_icon, mastery_icon) and invokes root.add_log_message.
;
; - kill_feed_event, at 0x2DA3D1 (`mov [eax+4],bl / mov edx,[esi+120h]`, the
;   node in eax): the cause icon, then +5 the flags, 0.69d0's rule for its
;   penetration_icon and ricochet_icon (its generate_killed_event 0x2003046B0,
;   from the hit's copies of the bullet's segments and reflections), for a
;   bullet when someone else killed (the cause icon not 3) and not by melee
;   (generate_killed_event's own flag, [esp+14h]): 4 when the bullet
;   ricocheted before this hit (its reflections, the byte +88h, not 0), else
;   1 when it went through something (its trajectory's segments, +84h, one
;   more for each pierce and each reflection (0x32EE20), not 0); 2 when the
;   victim's pain pool is empty and the body and head pools are not
;   (damage_model+108h's parts by name, 0x2E58B0; a part's health +0A0h, a
;   float clamped at 0; one hit that empties pain and body or head together
;   is a death by damage, the owner's rule). All are the
;   simulation's own state, the same in every game; nothing is written but
;   the node's byte, which only the hud reads;
; - KILL_FEED_ARGUMENT, at 0x23B6F0: the handler's `movzx edx,byte [edi+4]`
;   reads the word (`movzx edx,word [edi+4]`), so the fifth argument carries
;   the flags in its second byte; on_player_killed reads its first alone
;   (0x2380EA);
; - kill_feed_row, at 0x23814F (`mov edx,[esp+30h] / mov ecx,[esp+2Ch]`, after
;   extra_icon, before mastery_icon): the description's penetration_icon,
;   ricochet_icon and pain_icon, 0 or 1 (GFx ints, set as extra_icon is), then
;   the two moves.
BITS 32
global kill_feed_event, kill_feed_row
section .text

%define FIND_PART 0x2E58B0         ; a damage model's part by name: eax = its parts (+108h), ecx = &the name;
                                   ; eax the part or 0; keeps ebx esi edi ebp
%define DAMAGE_MODEL 0x2E0         ; the player's damage model
%define PARTS 0x108                ; the damage model's parts, a list by name
%define HEALTH 0xA0                ; a part's health (float; clamped at 0)
%define SEGMENTS 0x84              ; the bullet's trajectory segments past the first (dword)
%define REFLECTIONS 0x88           ; the bullet's reflections (byte)
%define CAUSE 4                    ; the event's cause icon (byte)
%define NO_KILLER 3                ; the cause icon of a death with no killer or by the victim himself
%define EVENT 0x0C                 ; kill_feed_event's frame after its two pushes: the site's esp at [esp+EVENT]
%define MELEE 0x14                 ; generate_killed_event's [esp+14h]: the bullet's damage is its weapon's melee
                                   ; (`[ebp+40h]` the weapon's +53Ch, 0x2DA2D7; no headshot then)
%define FLAGS 5                    ; the event's byte after it: ours
%define WORLD 0x120                ; the player's game world
%define WALL 1
%define PAIN 2
%define RICOCHET 4
%define GFX_INT 3                  ; a GFx value's type: int (extra_icon's)
%define SET_MEMBER 0x14            ; a GFx object interface's SetMember (thiscall: data, name, &value, is a
                                   ; display object; ret 10h)
%define ROW 8                      ; kill_feed_row's frame: the site's esp at [esp+ROW]
%define VALUE 0x14                 ; on_player_killed's frame: a GFx value (+14h interface, +18h type, +1Ch data)
%define OBJECT 0x2C                ; the description (+2Ch interface, +30h type, +34h data)
%define ARGUMENT 0x105             ; the fifth argument's second byte: the flags

; generate_killed_event at 0x2DA3D1: eax = the event, esi = the victim,
; ebp = the bullet or 0, bl = the cause icon. Back with eax, ebx, esi, edi,
; ebp as they were and edx = the game world, as the two instructions leave
; them.
kill_feed_event:
    mov [eax+CAUSE], bl
    push eax
    push ebx
    movzx ecx, bl                  ; the cause icon
    xor ebx, ebx
    test ebp, ebp
    jz .pain                       ; no bullet
    cmp ecx, NO_KILLER
    je .pain                       ; no killer, or the victim himself
    cmp byte [esp+EVENT+MELEE], 0
    jne .pain                      ; a melee hit
    cmp byte [ebp+REFLECTIONS], 0
    je .wall
    or bl, RICOCHET                ; a ricochet, before or after any pierce: no wall
    jmp .pain
.wall:
    cmp dword [ebp+SEGMENTS], 0
    je .pain                       ; straight from the muzzle
    or bl, WALL
.pain:
    mov ecx, pain_name
    call part
    test eax, eax
    jz .done
    cmp dword [eax+HEALTH], 0
    jg .done                       ; pain left (a health's bits: above 0 exactly for a float above 0)
    mov ecx, body_name
    call part
    test eax, eax
    jz .done
    cmp dword [eax+HEALTH], 0
    jle .done                      ; the body pool killed
    mov ecx, head_name
    call part
    test eax, eax
    jz .done
    cmp dword [eax+HEALTH], 0
    jle .done                      ; the head pool killed
    or bl, PAIN
.done:
    mov edx, ebx
    pop ebx
    pop eax
    mov [eax+FLAGS], dl
    mov edx, [esi+WORLD]
    ret

; The victim's (esi) part named by [ecx] in eax, or 0.
part:
    mov eax, [esi+DAMAGE_MODEL]
    test eax, eax
    jz .none
    add eax, PARTS
    jmp FIND_PART
.none:
    ret

; on_player_killed at 0x23814F: the description gets penetration_icon,
; ricochet_icon and pain_icon from the flags, as it got extra_icon; then the site's moves.
%macro SET_FLAG 2                  ; the name, the flag
    mov edx, [esp+ROW+OBJECT+4]
    mov ecx, [esp+ROW+OBJECT]
    and edx, 0x8F
    cmp dl, 0x0A
    sete dl
    movzx edx, dl
    push edx                       ; a display object
    lea edx, [esp+4+ROW+VALUE]
    push edx                       ; &the value
    mov edx, [esp+8+ROW+OBJECT+8]
    mov dword [esp+8+ROW+VALUE+4], GFX_INT
    xor eax, eax
    test bl, %2
    setnz al
    mov [esp+8+ROW+VALUE+8], eax
    mov eax, [ecx]
    mov eax, [eax+SET_MEMBER]
    push dword %1
    push edx
    call eax
%endmacro

kill_feed_row:
    push ebx
    movzx ebx, byte [esp+ROW+ARGUMENT]
    SET_FLAG penetration_icon, WALL
    SET_FLAG ricochet_icon, RICOCHET
    SET_FLAG pain_icon, PAIN
    pop ebx
    mov edx, [esp+4+OBJECT+4]
    mov ecx, [esp+4+OBJECT]
    ret

section .rodata
pain_name: dd pain_text            ; FIND_PART takes the address of a name's pointer
body_name: dd body_text
head_name: dd head_text
pain_text: db "pain", 0
body_text: db "body", 0
head_text: db "head", 0
penetration_icon: db "penetration_icon", 0
ricochet_icon: db "ricochet_icon", 0
pain_icon: db "pain_icon", 0
