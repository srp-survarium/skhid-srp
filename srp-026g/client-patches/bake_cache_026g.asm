; Patch 70 (build 2777): the camouflage/decal baker keeps its textures, and
; bakes or loads a texture once while it is asked for again. The arena's
; block bake_cache.code with bake_cache_026g.c (link_026g.py links them;
; patch_026g.py's BAKE_CACHE_HOOKS call and jump to the entries by their
; names, BAKE_CACHE_KEEP takes the delete out of game::exit).
; client_patch.md section 3.57; the rules: ../docs/notes/baked-textures.md.
;
; - bake_cache_start, game::game's `call 0x1E8DA0` (0x1E9455; eax = the
;   physical path of user_data/streamed_textures, a buffer_string): bc_open;
;   the stock delete (0x1E8DA0, SHFileOperationA FO_DELETE) only when it says
;   so (no stamp, another revision's), then bc_stamp. game::exit's own call
;   (0x1EC832) is nops: the folder stays.
; - bake_cache_adopt, check_loaded_baked_texture's `cmp eax,[edi+114h]`
;   (0x131C90; eax the CRC of the bytes loaded, edi the record): a record of
;   CRC 0 (one bc_open made from a kept file) takes the CRC loaded; then the
;   compare, its flags for the `jz 0x131DC2` after it.
; - bake_cache_lookup, translate_query's `call 0x131ED0` (0x12D4CD; eax the
;   cook, [esp+4] &the path; ebx the surface, [ebp+8] the query, edi the
;   model instance, [frame+1Ch] the parameters): bc_lookup; a path under way
;   leaves translate_query at 0x12D42D (its end after a refused query: the
;   surfaces' list emptied, its model instance released) with the query not
;   finished; a path whose loaded texture is held gives the query that
;   texture at once (complete) and leaves the same way; else the look-up (a
;   tail jump).
; - bake_cache_baked, process_compression's `call 0x420789` (0x12F18F, its
;   texture made and put in the slot: the texture [frame+10h], the query
;   [frame+604h], the surface [frame+610h], the type [frame+620h]) and
;   bake_cache_loaded, on_baked_texture_loaded's (0x12DDB4: the texture edi,
;   the parameters [ebp+0Ch], the surface [ebp+10h], the query [ebp+14h]):
;   the waiting queries get the texture (share, complete), then
;   `lea eax,[ecx+8]` as 0x420789.
; - bake_cache_finish, finish_query's entry (0xF0D00, stdcall: the query, the
;   result, two more; `ret 10h`): when the query owns a path that nothing
;   shared (an error, a query made again, a texture not where it was looked
;   for), the function, then each waiting query made again (translate_query
;   0x12D1D0). Every other query: the function, at once.
;
; The game's own sequences, as on_baked_texture_loaded does them (0x12DD53..
; 0x12DE0E): the slot as 0x12C6D0 sets it, the texture's resource (0x12DE30 on
; 0x108 bytes of the allocator [1F1D960h], 0xE4F90), the query's resource
; (0xF0080) and finish_query (query, 3, 1, 0).
BITS 32
global bake_cache_start, bake_cache_adopt, bake_cache_lookup, bake_cache_baked, bake_cache_loaded
global bake_cache_finish, bake_cache_crc32, bc_release_texture
extern bc_open, bc_stamp, bc_lookup, bc_take, bc_owners, bc_crc_tables, bc_crc_ready, bc_crc_build
section .text

%define WIPE 0x1E8DA0              ; SHFileOperationA(FO_DELETE) of the buffer_string at eax
%define FIND_RECORD 0x131ED0       ; the list's first record of a path (eax the cook, the path; `ret 4`)
%define NOT_BAKED 0x12D42D         ; translate_query after a refused query's finish: its end
%define TRANSLATE 0x12D1D0         ; translate_query (ecx the cook, the query; `ret 4`)
%define FINISH 0xF0D00             ; finish_query (the query, the result, 1, 0; `ret 10h`)
%define FINISH_BODY 0xF0D06        ; past its `push ebp / mov ebp,esp / and esp,-8`
%define RESOURCE_NAME 0x420789     ; `lea eax,[ecx+8]`
%define ALLOCATE 0xE4F90           ; esi the allocator, eax the size, [esp+4] the name; `ret 4`
%define RESOURCE 0x12DE30          ; the texture's resource, constructed at eax
%define SET_RESOURCE 0xF0080       ; edx the query; the pointer, the allocator, the size; `ret 0Ch`
%define RELEASE_TEXTURE 0x3BEE00   ; eax the texture, [esp+4] the device; `ret 4`
%define DESTROY_MODEL 0xE0210      ; ecx a model instance, eax &its references
%define ALLOCATOR 0x1F1D960
%define DEVICE 0x226047C
%define NAMES 0xAC3334
%define RESOURCE_ALLOCATOR 0x21E76C8
%define RESOURCE_SIZE 0x108
%define CRC 0x114                  ; a record's CRC
%define REFS 0xD0                  ; a model instance's or resource's references
%define TYPE 0x7A0                 ; the parameters' type
%define FRAME 0x1C                 ; translate_query's [esp+1Ch]: the parameters
%define MAX_WAITERS 16             ; bake_cache_026g.c's PER_OWNER
%define WAITER 20                  ; struct waiter: query, surface, model, type, cook

bake_cache_start:
    push ebx
    mov ebx, eax
    push dword [ebx+4]
    push dword [ebx]
    call bc_open
    add esp, 8
    test eax, eax
    jz .kept
    mov eax, ebx
    call WIPE
    push dword [ebx+4]
    push dword [ebx]
    call bc_stamp
    add esp, 8
.kept:
    pop ebx
    ret

bake_cache_adopt:
    cmp dword [edi+CRC], 0
    jne .compare
    mov [edi+CRC], eax
.compare:
    cmp eax, [edi+CRC]
    ret

bake_cache_lookup:
    push eax                        ; the cook; the frame at esp+0Ch
    sub esp, 4                      ; a held texture; the frame at esp+10h
    mov ecx, esp
    push ecx                        ; bc_lookup(path, query, surface, parameters, cook, &texture)
    push eax
    push dword [esp+18h+FRAME]
    push ebx
    push dword [ebp+8]
    mov ecx, [esp+20h]              ; &the path
    push dword [ecx]
    call bc_lookup
    add esp, 18h
    cmp eax, 2
    je .held
    pop ecx
    test eax, eax
    pop eax
    jnz .waits
    jmp FIND_RECORD
.waits:
    add esp, 8                      ; the return address and the path
    jmp NOT_BAKED
.held:                              ; the query gets the held texture at once
    mov ecx, [esp]                  ; the texture
    mov edx, [esp+4]                ; the cook
    push edx                        ; a waiter: query, surface, no model, type, cook
    mov eax, [esp+14h+FRAME]
    push dword [eax+TYPE]
    push 0
    push ebx
    push dword [ebp+8]
    mov eax, esp
    push eax
    push ecx
    call complete
    add esp, 8+WAITER+8+8           ; complete's two, the waiter, the texture and cook, the return and the path
    jmp NOT_BAKED

bake_cache_baked:
    push ecx                        ; the frame at esp+8
    push 1                          ; a bake: its texture not held
    push dword [esp+0Ch+620h]       ; the type
    push dword [esp+10h+610h]       ; the surface
    push dword [esp+14h+10h]        ; the texture
    push dword [esp+18h+604h]       ; the query
    call share
    add esp, 14h
    pop ecx
    lea eax, [ecx+8]
    ret

bake_cache_loaded:
    push ecx
    push 2                          ; a load: its texture held for the game
    mov eax, [ebp+0Ch]
    push dword [eax+TYPE]
    push dword [ebp+10h]
    push edi
    push dword [ebp+14h]
    call share
    add esp, 14h
    pop ecx
    lea eax, [ecx+8]
    ret

; share(query, texture, surface, type, check): the waiting queries of the path
; the query owns get the texture, when it is in the surface's slot (check 2: a
; load's, held for the game besides).
share:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    sub esp, MAX_WAITERS*WAITER
    mov edi, esp
    push dword [ebp+18h]
    push MAX_WAITERS
    push edi
    push dword [ebp+14h]
    push dword [ebp+10h]
    push dword [ebp+0Ch]
    push dword [ebp+8]
    call bc_take
    add esp, 1Ch
    test eax, eax
    jle .done
    mov ebx, eax
    mov esi, edi
.next:
    push esi
    push dword [ebp+0Ch]
    call complete
    add esp, 8
    add esi, WAITER
    dec ebx
    jnz .next
.done:
    lea esp, [ebp-0Ch]
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

; complete(texture, waiter): the waiter's slot holds the texture, its query
; has the texture's resource and is finished; its model instance released.
complete:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    mov edi, [ebp+0Ch]
    mov eax, [ebp+8]
    inc dword [eax+4]
    mov edx, [edi+4]
    mov ecx, [edi+0Ch]
    lea edx, [edx+ecx*4]
    mov ecx, [edx]
    mov [edx], eax
    test ecx, ecx
    jz .resource
    add dword [ecx+4], -1
    jnz .resource
    push dword [DEVICE]
    mov eax, ecx
    call RELEASE_TEXTURE
.resource:
    mov esi, [ALLOCATOR]
    mov ecx, NAMES
    call RESOURCE_NAME
    push eax
    mov eax, RESOURCE_SIZE
    call ALLOCATE
    test eax, eax
    jz .none
    call RESOURCE
    jmp .made
.none:
    xor eax, eax
.made:
    push RESOURCE_SIZE
    push RESOURCE_ALLOCATOR
    push ecx
    mov ecx, esp
    mov dword [ecx], 0
    test eax, eax
    jz .set
    mov [ecx], eax
    add eax, REFS
    mov edx, 1
    lock xadd [eax], edx
.set:
    mov edx, [edi]
    call SET_RESOURCE
    push 0
    push 1
    push 3
    push dword [edi]
    call FINISH
    push edi
    call release_model
    add esp, 4
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret

; bake_cache_crc32, in place of the game's CRC-32 step 0x42050 (its first
; bytes jump here): eax &the CRC (updated), edx the bytes, [esp+4] how many;
; `ret 4`. As the original leaves them: ecx the new CRC (the count when it
; is 0), edx past the bytes, eax, ebx, esi, edi and ebp kept. The same
; value: the bytes to a dword one at a time, then eight a step through
; bc_crc_tables (T7..T0), the rest one at a time (T0 is the game's T).
%define T(k) (bc_crc_tables + (k) * 1024)
bake_cache_crc32:
    mov ecx, [esp+4]
    test ecx, ecx
    jz .none
    push ebx
    push esi
    push edi
    push ebp
    push eax                        ; &the CRC
    cmp dword [bc_crc_ready], 0
    jne .ready
    push ecx
    push edx
    call bc_crc_build
    pop edx
    pop ecx
.ready:
    mov esi, edx                    ; the bytes
    mov edi, ecx                    ; how many
    mov eax, [esp]                  ; &the CRC (bc_crc_build may have changed eax)
    mov eax, [eax]                  ; the CRC
.head:
    test esi, 3
    jz .body
    movzx edx, byte [esi]
    xor dl, al
    shr eax, 8
    xor eax, [T(0) + edx*4]
    inc esi
    dec edi
    jnz .head
    jmp .done
.body:
    cmp edi, 8
    jb .tail
.eight:
    mov ebx, [esi]
    xor ebx, eax
    mov ecx, [esi+4]
    movzx edx, bl
    mov eax, [T(7) + edx*4]
    movzx edx, bh
    xor eax, [T(6) + edx*4]
    shr ebx, 16
    movzx edx, bl
    xor eax, [T(5) + edx*4]
    movzx edx, bh
    xor eax, [T(4) + edx*4]
    movzx edx, cl
    xor eax, [T(3) + edx*4]
    movzx edx, ch
    xor eax, [T(2) + edx*4]
    shr ecx, 16
    movzx edx, cl
    xor eax, [T(1) + edx*4]
    movzx edx, ch
    xor eax, [T(0) + edx*4]
    add esi, 8
    sub edi, 8
    cmp edi, 8
    jae .eight
.tail:
    test edi, edi
    jz .done
    movzx edx, byte [esi]
    xor dl, al
    shr eax, 8
    xor eax, [T(0) + edx*4]
    inc esi
    dec edi
    jmp .tail
.done:
    mov ecx, eax                    ; the new CRC
    pop eax                         ; &the CRC
    mov [eax], ecx
    mov edx, esi                    ; past the bytes
    pop ebp
    pop edi
    pop esi
    pop ebx
.none:
    ret 4

; bc_release_texture(texture), cdecl: a held texture's reference let go, as
; 0x12C6D0 lets one go.
bc_release_texture:
    mov eax, [esp+4]
    add dword [eax+4], -1
    jnz .kept
    push dword [DEVICE]
    call RELEASE_TEXTURE
.kept:
    ret

; release_model(waiter): the reference bc_lookup took.
release_model:
    mov eax, [esp+4]
    mov ecx, [eax+8]
    test ecx, ecx
    jz .kept
    lea eax, [ecx+REFS]
    or edx, -1
    lock xadd [eax], edx
    jnz .kept
    call DESTROY_MODEL
.kept:
    ret

; retranslate(waiter): its query made again (it bakes or loads, or waits for
; another), then the reference released.
retranslate:
    push ebx
    mov ebx, [esp+8]
    mov ecx, [ebx+10h]
    push dword [ebx]
    call TRANSLATE
    push ebx
    call release_model
    add esp, 4
    pop ebx
    ret

bake_cache_finish:
    cmp dword [bc_owners], 0
    jne .owner
.function:
    push ebp
    mov ebp, esp
    and esp, -8
    jmp FINISH_BODY
.owner:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi
    sub esp, MAX_WAITERS*WAITER
    mov edi, esp
    push 0
    push MAX_WAITERS
    push edi
    push 0
    push 0
    push 0
    push dword [ebp+8]
    call bc_take
    add esp, 1Ch
    test eax, eax
    jg .waiting
    lea esp, [ebp-0Ch]
    pop edi
    pop esi
    pop ebx
    pop ebp
    jmp .function
.waiting:
    mov ebx, eax
    push dword [ebp+14h]
    push dword [ebp+10h]
    push dword [ebp+0Ch]
    push dword [ebp+8]
    call .function
    mov esi, edi
.again:
    push esi
    call retranslate
    add esp, 4
    add esi, WAITER
    dec ebx
    jnz .again
    lea esp, [ebp-0Ch]
    pop edi
    pop esi
    pop ebx
    pop ebp
    ret 10h
