; Patch 38: a spot that hits a battery's body spots the battery (build 2777,
; base_player::apply_new_input 0x2D86E0). The no-cooldown half of the patch
; is one immediate in place (patch_026g.py SPOT_COOLDOWN).
;   nasm -f bin -DCAVE=0xA4257C0 spot_026g.asm -o spot_026g.section.bin
; client_patch.md section 3.35; the rules: ../docs/notes/spotting.md.
;
; A spot (input bit 28) casts a 100 m ray from the eye (0x2D87EB, collision
; filter 80Ah) and walks its hits in order; [esp+10h] is the hits' groups so
; far. A hit whose group has 800h (spottable) is looked up through the hit
; object's owner (+0Ch): the owner's slot 2 gives a player, its slot 1 a
; geometry whose first spottable is asked. A battery's use box
; `!victory_item_collision!` (group 880h) answers through slot 1. Its rigid
; body (group 0A18h, 0x301CF5) has the owner victory_item_core+58h
; (0x301CC0), vtable 0xA13EEC, whose slots 1 and 2 are both 0x300040
; `xor eax,eax; ret`: no player, no geometry, so the original takes the
; 800h bit out of the mask and goes on, keeping the body's 8h (hittable),
; and a walkable hit behind it (2h) then stops the walk.
BITS 32
ORG CAVE

%define OWNER_VTABLE 0xA13EEC      ; victory_item_core's at +58h, the rigid body's owner
%define SPOTTED 0x2D888F           ; a target was spotted: the walk ends, the hits are freed
%define NOT_SPOTTED 0x2D8A8F       ; `and dword [esp+10h],0F7FFh`, then the next hit
%define GEOMETRY 0x2D8A14          ; the original: the owner's slot 1 (the geometry)

; 0x2D8A0F `mov ecx,[esi]; mov ecx,[ecx+0Ch]` (no player behind the hit) ->
; `jmp body`. esi = the hit's object pointer (hit+1Ch), edi = the spotter.
; When the owner is a battery's, the battery's spottable interface is just
; before it (+54h, vtable 0xA13EE0): asked and spotted the way the use box's
; path asks and spots its geometry's first spottable (0x2D8A50..0x2D8A65):
; can_be_spotted_by (0x301CA0, always yes) and set_spotted (0x301C40: the
; spotter's team bit into +1A4h, event 4 to the match statistics). esi is
; free on both ways out (the walk reads the next hit from [esp+14h]; the
; end clears it) and kept by the calls.
body:
    mov ecx, [esi]
    mov ecx, [ecx+0x0C]
    cmp dword [ecx], OWNER_VTABLE
    jne .geometry
    lea esi, [ecx-4]
    mov eax, [esi]
    push edi
    mov ecx, esi
    call [eax+4]                   ; can_be_spotted_by(spotter), `ret 4`
    test al, al
    jz .not_spotted
    mov eax, [esi]
    push edi
    mov ecx, esi
    call [eax]                     ; set_spotted(spotter), `ret 4`
    jmp SPOTTED
.not_spotted:
    jmp NOT_SPOTTED
.geometry:
    jmp GEOMETRY                   ; ecx = the owner, as there
times 64-($-$$) db 0xcc
