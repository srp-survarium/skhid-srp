; A weapon wears the camouflage its item holds (build 2777, patch 44,
; client_patch.md section 3.41), and its decals (patch 47, section 3.44). Code
; of its own section `.srpw`, after the late join's state page; patch_026g.py
; checks the entries. DECALS is weapon_decals_026g.asm (`.srpw`+1200h), which
; says what a surface is painted with: lookup (+0) and fill (+5).
;   nasm -f bin -DCAVE=0xA42A000 -DDECALS=0xA42B200 weapon_camo_026g.asm -o weapon_camo_026g.section.bin
;
; The stock game bakes a camouflage only onto the character's parts: the
; profile's visual (0x204560) fills a camouflage_decal_cook_parameters (P)
; for every part whose merged config has `data/camouflage` and asks for a
; class-6Ch bake per part (0x129830); the lobby's decal editor does the same
; for one part of a model already shown (0x2641A0). A weapon's merged config
; holds its upgrades as well (0x2F14C0, `_u<modification>` in its name), so a
; camouflage on a weapon puts `data/camouflage` into it when the weapon's
; config has a `data` table, but weapon_cook never asks for a bake.
;
; weapon_cook::on_weapon_subresources_ready (0x24ACF0, `ret 14h`), when the
; weapon's model is loaded and before it is given to the weapon (0x243A30),
; calls `weapon_ready` in place of `lea eax,[esp+20h] / push eax` (0x24B0D2).
; With `data/camouflage` a non-empty text or decals in `data/decals`
; (DECALS' lookup), it asks, for each surface of the model's render instance
; (at most MAX_SURFACES), for the bake the lobby's editor asks for: one P
; from the game's allocator, the camouflage's diffuse, `_nmap` and `_smap`
; names and the decals' layers (DECALS' fill), the render instance (one
; reference more), the surface's index, the scale (`data/camo_scale`, else
; 4.0 as every stock bake), and a name of the baked files of its own,
; `srpw1_<camouflage>_<object/model>_<surface>` (and the decals', by fill)
; with '/' and '\' as '_'.
; The query runs on its own (no parent): the weapon is shown plain and
; takes the baked textures when they are ready, as a part in the decal
; editor does. Its callback `bake_done` lets go of the render instance and
; frees P (0x2648E0 does the same). The baker refuses a surface whose
; diffuse has no `_aoac` texture, and the surface stays plain.
; Every register and the flags are kept; eax is what the original's `lea`
; leaves.
BITS 32
ORG CAVE

%ifndef DECALS
%error DECALS is required
%endif
LOOKUP              equ DECALS + 0  ; patch 44's frame: eax = 0: nothing to paint; [ebp+CAMO] = the camouflage or ""
FILL                equ DECALS + 5  ; edi = P, ebp = patch 44's frame: the camouflage's and the decals' layers

has_key             equ 0x0F6C00    ; eax = table, esi = key: al = it is there
child               equ 0x0F6B10    ; eax = table, edi = key: eax = the child (no end check)
string_appendf      equ 0x0E45B0    ; eax = buffer_string, cdecl (format, ...)
allocate            equ 0x0E4F90    ; esi = allocator, eax = size, [esp+4] = where: eax; `ret 4`
allocation_name     equ 0x420789    ; ecx = 0AC1220h: eax (the editor's argument of allocate)
deallocate          equ 0x1539D0    ; ecx = allocator, eax = memory, cdecl (&the pointer)
parameters_ctor     equ 0x119760    ; stdcall (P): eax = P
get_all_surfaces    equ 0x159700    ; stdcall (render instance, &buffer_vector)
query_resources     equ 0x0E5B60    ; eax = requests, ecx = count, cdecl (&function, allocator, &user data, parent, 1)
release             equ 0x011540    ; ecx = &intrusive pointer
type_id             equ 0x119590    ; type_to_int<camouflage_decal_cook_parameters*>: eax
GAME_ALLOCATOR      equ 0x1F1D9C4
ALLOCATION_WHERE    equ 0xAC1220
VARIANT_HELPER      equ 0x9EBE74    ; concrete_type_helper<P*>
BIND_MANAGER        equ 0x9ED148    ; boost::function: bind(void(*)(A*,B*), a, b), trivially copied
EMPTY               equ 0x9DB708    ; ""
BAKE_CLASS          equ 0x6C        ; camouflage_decal_baker_cook
STOCK_SCALE         equ 0x9E4EB4    ; 4.0, every stock bake's
KEY_DATA            equ 0x9FD6C4
KEY_OBJECT          equ 0xA02AA0
KEY_MODEL           equ 0xA048B8
P_SIZE              equ 0x8B8
P_CAMO_MATRIX       equ 0x4E0       ; the camouflage layer's float4x4; the constructor leaves it
P_MODEL             equ 0x790       ; render instance, counted at +0D0h
P_SURFACE           equ 0x794
P_SCALE             equ 0x798
P_ONE               equ 0x79C
P_EDITOR            equ 0x7A0
P_NAME              equ 0x7A4
MODEL_RENDER        equ 0x108       ; skeleton_model_instance: its render instance
CONFIG_ROOT         equ 0x108
REFERENCES          equ 0xD0
T_FLOAT             equ 2
T_STRING            equ 5
MAX_SURFACES        equ 16          ; a bake each; 0.69d0's AKMSU has 11 (8 left its magazine bare)
SURFACES            equ 64          ; the inline buffer of get_all_surfaces' vector

; the frame below ebp (ebp = esp after pushad)
ROOT        equ -4
DATA        equ -8
CAMO        equ -0Ch
MODEL       equ -10h
RENDER      equ -14h
COUNT       equ -18h
SCALE       equ -1Ch
VECTOR      equ -2Ch                ; begin, end, capacity end
INLINE      equ VECTOR - SURFACES * 4
VARIANT     equ INLINE - 30h        ; variant<32>: helper, P at +8, &helper at +28h, type at +2Ch
USER_DATA   equ VARIANT - 4         ; one pointer: the variant
REQUEST     equ USER_DATA - 8       ; {name, class}
FUNCTION    equ REQUEST - 20h       ; {manager|1, 0, f, a, b, ...}
FRAME       equ -FUNCTION
SITE_MODEL  equ 28h + 20h           ; [ebp+..]: the original's [esp+20h], the model
SITE_CONFIG equ 28h + 3Ch           ; the original's [esp+3Ch], the weapon's merged config

; 0x24B0D2: `call weapon_ready` in place of `lea eax,[esp+20h] / push eax`.
weapon_ready:
    pushad
    pushfd
    mov ebp, esp
    sub esp, FRAME
    call bake_weapon
    mov esp, ebp
    popfd
    popad
    lea eax, [esp+4+20h]            ; the original's lea
    xchg eax, [esp]                 ; its push, where the return address is
    push eax
    lea eax, [esp+8+20h]
    ret

bake_weapon:
    mov eax, [ebp+SITE_CONFIG]
    test eax, eax
    jz .done
    mov eax, [eax+CONFIG_ROOT]
    test eax, eax
    jz .done
    mov [ebp+ROOT], eax
    mov esi, KEY_DATA
    call has_key
    test al, al
    jz .done
    mov eax, [ebp+ROOT]
    mov edi, KEY_DATA
    call child
    mov [ebp+DATA], eax
    call LOOKUP                     ; the camouflage (or ""), the decals
    test eax, eax
    jz .done
    mov eax, [STOCK_SCALE]
    mov [ebp+SCALE], eax
    mov eax, [ebp+DATA]
    mov esi, key_camo_scale
    call has_key
    test al, al
    jz .model
    mov eax, [ebp+DATA]
    mov edi, key_camo_scale
    call child
    cmp word [eax+14h], T_FLOAT
    jne .model
    mov eax, [eax]
    mov [ebp+SCALE], eax
.model:
    mov eax, [ebp+ROOT]
    mov esi, KEY_OBJECT
    call has_key
    test al, al
    jz .done
    mov eax, [ebp+ROOT]
    mov edi, KEY_OBJECT
    call child
    mov [ebp+MODEL], eax
    mov esi, KEY_MODEL
    call has_key
    test al, al
    jz .done
    mov eax, [ebp+MODEL]
    mov edi, KEY_MODEL
    call child
    cmp word [eax+14h], T_STRING
    jne .done
    mov eax, [eax]
    test eax, eax
    jz .done
    mov [ebp+MODEL], eax
    mov eax, [ebp+SITE_MODEL]
    test eax, eax
    jz .done
    mov eax, [eax+MODEL_RENDER]
    test eax, eax
    jz .done
    mov [ebp+RENDER], eax
    lea eax, [ebp+INLINE]
    mov [ebp+VECTOR], eax
    mov [ebp+VECTOR+4], eax
    lea ecx, [eax+SURFACES*4]
    mov [ebp+VECTOR+8], ecx
    lea eax, [ebp+VECTOR]
    push eax
    push dword [ebp+RENDER]
    call get_all_surfaces
    mov eax, [ebp+VECTOR+4]
    sub eax, [ebp+VECTOR]
    shr eax, 2
    cmp eax, MAX_SURFACES
    jbe .count
    mov eax, MAX_SURFACES
.count:
    mov [ebp+COUNT], eax
    xor ebx, ebx
.surface:
    cmp ebx, [ebp+COUNT]
    jae .done
    push ebx
    call bake_surface
    pop ebx
    inc ebx
    jmp .surface
.done:
    ret

; ebx = the surface's index
bake_surface:
    mov esi, [GAME_ALLOCATOR]
    mov ecx, ALLOCATION_WHERE
    call allocation_name
    push eax
    mov eax, P_SIZE
    call allocate
    test eax, eax
    jz .done
    push eax
    call parameters_ctor
    mov edi, eax
    push edi
    lea edi, [edi+P_CAMO_MATRIX]
    xor eax, eax
    mov ecx, 16
    rep stosd
    pop edi
    mov eax, 0x3F800000             ; the identity
    mov [edi+P_CAMO_MATRIX], eax
    mov [edi+P_CAMO_MATRIX+14h], eax
    mov [edi+P_CAMO_MATRIX+28h], eax
    mov [edi+P_CAMO_MATRIX+3Ch], eax
    push ebx
    push dword [ebp+MODEL]
    push dword [ebp+CAMO]
    push name_format
    lea eax, [edi+P_NAME]
    call string_appendf
    add esp, 16
    call FILL                       ; the layers, and the decals' part of the name
    mov eax, [edi+P_NAME]           ; its text: '/' and '\' become '_'
    mov ecx, [edi+P_NAME+4]
.name:
    cmp eax, ecx
    jae .named
    cmp byte [eax], '/'
    je .underscore
    cmp byte [eax], '\'
    jne .next
.underscore:
    mov byte [eax], '_'
.next:
    inc eax
    jmp .name
.named:
    mov eax, [ebp+RENDER]
    mov [edi+P_MODEL], eax
    lock inc dword [eax+REFERENCES]
    mov [edi+P_SURFACE], ebx
    mov eax, [ebp+SCALE]
    mov [edi+P_SCALE], eax
    mov dword [edi+P_ONE], 1
    mov byte [edi+P_EDITOR], 0
    call type_id
    mov edx, eax
    lea esi, [ebp+VARIANT]
    push edi
    mov edi, esi
    xor eax, eax
    mov ecx, 30h / 4
    rep stosd
    pop edi
    mov dword [esi], VARIANT_HELPER
    mov [esi+8], edi
    mov [esi+28h], esi
    mov [esi+2Ch], edx
    mov [ebp+USER_DATA], esi
    mov dword [ebp+REQUEST], EMPTY
    mov dword [ebp+REQUEST+4], BAKE_CLASS
    lea eax, [ebp+FUNCTION]
    mov dword [eax], BIND_MANAGER | 1
    mov dword [eax+4], 0
    mov dword [eax+8], bake_done
    mov dword [eax+0Ch], 0
    mov [eax+10h], edi
    push 1
    push 0                          ; no parent: it runs on its own
    lea ecx, [ebp+USER_DATA]
    push ecx
    push dword [GAME_ALLOCATOR]
    push eax
    lea eax, [ebp+REQUEST]
    mov ecx, 1
    call query_resources
    add esp, 14h
.done:
    ret

; The bake's end, cdecl (a, P): bind's function, as 0x2648E0's end.
bake_done:
    push ebx
    mov ebx, [esp+0Ch]
    test ebx, ebx
    jz .done
    lea ecx, [ebx+P_MODEL]
    call release
    push ebx                        ; the pointer deallocate clears
    mov eax, esp
    push eax
    mov eax, ebx
    mov ecx, [GAME_ALLOCATOR]
    call deallocate
    add esp, 8
.done:
    pop ebx
    ret

key_camo_scale: db "camo_scale", 0
name_format: db "srpw1_%s_%s_%d", 0
