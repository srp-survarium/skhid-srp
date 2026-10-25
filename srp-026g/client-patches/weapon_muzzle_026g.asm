; Patch 68, the muzzle device (build 2777): a weapon whose config names a
; model at data/srp_muzzle_model (a muzzle device's modification adds it: the
; weapon kit's slot 4) loads that model with the weapon and draws it at the
; weapon's barrel_point, data/srp_muzzle_offset metres forward (each weapon's
; own: the stock muzzle brakes are part of the weapons' models). The arena's
; block weapon_kit.code; weapon_muzzle_026g.c computes. README patch 68;
; client_patch.md section 3.56; ../docs/notes/weapon-kit.md.
;
; The weapon cook (0x24A920) asks for a weapon's sub-resources in one query
; (0xE5B60): the scope's config when it names one (class 4Dh), the model
; (10h), each user animation (31h), the bullet shells' and the shot effects'
; particles (3Dh); their count, plus one for the model, sizes the requests'
; alloca (0x24AA04, `lea eax,[ecx+eax+1]`: now +2, a place for the device's).
; - request (0x24ABF9, after the last request): the device's model (class
;   0Fh, a static model's instance, as the scope cook's 0x258672) when the
;   config names one;
; - made (0x24A8F7, weapon_cook 0x24A780 after the weapon's arrays): the
;   weapon is EXTENSION bytes more (0x24A7A5's size, `lea eax,[eax*4+4710h]`
;   -> +4760h), its own part after the last array zeroed;
; - loaded (0x24ACF0, the query's callback, at its entry): the device's model,
;   the last result, kept in the weapon's part;
; - the weapon's model's own calls: on_show's add (0x243F37), on_hide's remove
;   (0x244012), add_aimed_rifle_scope_to_scene's remove (0x244C08: a scope
;   that hides the weapon) and remove_aimed_rifle_scope_to_scene's add
;   (0x244CF5): the original's, then the same for the device's model with its
;   own matrix; draw's update (0x244F3A): the original's, then the device's
;   matrix from barrel_point's (+0x620, computed earlier in draw) and its
;   update;
; - released (0x24B23A, weapon_cook's delete_resource after the shot effects'
;   pointers): the device's model released.
BITS 32

global weapon_muzzle_request, weapon_muzzle_made_hook, weapon_muzzle_loaded_hook
global weapon_muzzle_add, weapon_muzzle_remove, weapon_muzzle_update, weapon_muzzle_release
extern weapon_muzzle_model, weapon_muzzle_made, weapon_muzzle_loaded, weapon_muzzle_place
extern weapon_muzzle_extension
section .text

%define REQUEST_PUSH 0x5FCB0        ; buffer_vector<request>::push_back: edi the request, esi the vector
%define ADD_MODEL 0x21CD0           ; eax the render model; (channel, &scene, &matrix, &matrix); `ret 10h`
%define REMOVE_MODEL 0x21F40        ; eax the render model; (channel, &scene); `ret 8`
%define UPDATE_MODEL 0x22280        ; eax the render model; (channel, &scene, &matrix, &matrix); `ret 10h`
%define RELEASE 0x11540             ; intrusive_ptr release: ecx the pointer's place
%define CLASS_STATIC_MODEL 0x0F
%define PART_MODEL 0x00
%define PART_ADDED 0x08
%define PART_MATRIX 0x10
%define RENDER 0x108                ; a model instance's render model

; 0x24ABF9 `mov ecx,[ebp-10h] / movzx ecx,word [ecx+16h]` -> jmp + 2 nops.
; ebx the weapon config's root, [ebp-40h] the requests' vector.
weapon_muzzle_request:
    pushad
    push ebx
    call weapon_muzzle_model
    add esp, 4
    test eax, eax
    jz .done
    push CLASS_STATIC_MODEL
    push eax
    mov edi, esp                    ; the request {path, class}
    lea esi, [ebp-0x40]
    call REQUEST_PUSH
    add esp, 8
.done:
    popad
    mov ecx, [ebp-0x10]
    movzx ecx, word [ecx+0x16]
    jmp 0x24AC00

; 0x24A8F7 `lea edi,[ebx+6A0h] / mov esi,eax` -> jmp + 3 nops: weapon_cook
; 0x24A780's end, ebx the weapon (its arrays made), eax its matrix.
weapon_muzzle_made_hook:
    pushad
    push ebx
    call weapon_muzzle_made
    add esp, 4
    popad
    lea edi, [ebx+0x6A0]
    mov esi, eax
    jmp 0x24A8FF

; 0x24ACF0 `sub esp,24h / push ebx / push ebp` -> jmp: the callback's entry,
; [esp+4] the results, [esp+8] the weapon config's resource, [esp+0Ch] the
; weapon.
weapon_muzzle_loaded_hook:
    pushad
    push dword [esp+32+12]
    push dword [esp+36+8]
    push dword [esp+40+4]
    call weapon_muzzle_loaded
    add esp, 12
    popad
    sub esp, 0x24
    push ebx
    push ebp
    jmp 0x24ACF5

; In place of the weapon model's `call 0x21CD0` (on_show 0x243F37,
; remove_aimed 0x244CF5): esi the weapon. The original's, then the device's.
weapon_muzzle_add:
    push dword [esp+16]
    push dword [esp+16]
    push dword [esp+16]
    push dword [esp+16]
    call ADD_MODEL
    pushad
    push esi
    call weapon_muzzle_extension
    add esp, 4
    mov edi, eax
    mov eax, [edi+PART_MODEL]
    test eax, eax
    jz .done
    cmp dword [edi+PART_ADDED], 0
    jne .done
    push esi
    call weapon_muzzle_place
    add esp, 4
    lea ecx, [edi+PART_MATRIX]
    push ecx
    push ecx
    push dword [esp+32+8+8]         ; &scene
    push dword [esp+32+12+4]        ; the channel
    mov eax, [edi+PART_MODEL]
    add eax, RENDER
    call ADD_MODEL
    mov dword [edi+PART_ADDED], 1
.done:
    popad
    ret 16

; In place of the weapon model's `call 0x21F40` (on_hide 0x244012,
; add_aimed 0x244C08): esi the weapon.
weapon_muzzle_remove:
    push dword [esp+8]
    push dword [esp+8]
    call REMOVE_MODEL
    pushad
    push esi
    call weapon_muzzle_extension
    add esp, 4
    mov edi, eax
    mov eax, [edi+PART_MODEL]
    test eax, eax
    jz .done
    cmp dword [edi+PART_ADDED], 0
    je .done
    push dword [esp+32+8]           ; &scene
    push dword [esp+32+4+4]         ; the channel
    mov eax, [edi+PART_MODEL]
    add eax, RENDER
    call REMOVE_MODEL
    mov dword [edi+PART_ADDED], 0
.done:
    popad
    ret 8

; In place of draw's `call 0x22280` for the weapon model (0x244F3A): ebx the
; weapon, barrel_point's matrix at +0x620 this frame's.
weapon_muzzle_update:
    push dword [esp+16]
    push dword [esp+16]
    push dword [esp+16]
    push dword [esp+16]
    call UPDATE_MODEL
    pushad
    push ebx
    call weapon_muzzle_extension
    add esp, 4
    mov edi, eax
    cmp dword [edi+PART_MODEL], 0
    je .done
    cmp dword [edi+PART_ADDED], 0
    je .done
    push ebx
    call weapon_muzzle_place
    add esp, 4
    lea ecx, [edi+PART_MATRIX]
    push ecx
    push ecx
    push dword [esp+32+8+8]         ; &scene
    push dword [esp+32+12+4]        ; the channel
    mov eax, [edi+PART_MODEL]
    add eax, RENDER
    call UPDATE_MODEL
.done:
    popad
    ret 16

; 0x24B23A `mov ebx,[edi+1818h]` -> jmp + nop: delete_resource, edi the
; weapon: the device's model released.
weapon_muzzle_release:
    pushad
    push edi
    call weapon_muzzle_extension
    add esp, 4
    lea ecx, [eax+PART_MODEL]
    mov esi, ecx
    call RELEASE
    mov dword [esi], 0
    popad
    mov ebx, [edi+0x1818]
    jmp 0x24B240
