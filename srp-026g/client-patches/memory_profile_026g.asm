; Patch 127 (build 2777): the memory profiler's gates on dlmalloc's three
; entries (rust/patches/src/memory_profile.toml's `code` hooks; the profiler
; is the Rust patch memory_profile, docs/notes/memory-profile.md).
;
; Every allocation of the engine's heaps (each doug_lea arena's, ptmalloc3's
; "global multithreaded") ends in mspace_malloc, mspace_realloc or
; mspace_free. Each gate tests one byte, the profiler's switch
; (SRP_MEMORY_PROFILE, vostok::memory_profile): 0, the game's own body runs
; (its replayed first instructions, then a jump back), and that is all a
; game without SKHID_MEMORY_PROFILE pays. On, the gate calls the stock body
; itself and tells the profiler through two generated entries (cdecl, each
; saves the whole machine):
;
; - memory_profile_on_freeing (the game's ESP at its call, the memory) before
;   mspace_free and mspace_realloc give a block back, so no other thread can
;   be handed the block before the profiler forgot it;
; - memory_profile_on_allocated (the game's ESP, the memory or 0) after
;   mspace_malloc and mspace_realloc returned one.
;
; What the game sees is the stock body's: its result, its registers (ECX
; and EDX as the body left them), its flags (the entry restores them); the
; gate's own words are below the game's ESP. The switch is written once, at
; the start (memory.arenas_ready), before any thread but the main one.
bits 32
section .text

extern SRP_MEMORY_PROFILE
extern memory_profile_on_allocated
extern memory_profile_on_freeing

%define MALLOC_ON 0x101335      ; mspace_malloc after sub esp,8 / push ebx / push ebp
%define REALLOC_ON 0x1005A5     ; mspace_realloc after mov ecx,[esp+8] / push esi
%define FREE_ON 0x1015E6        ; mspace_free after sub esp,0Ch / push edi / mov edi,eax

global memory_profile_malloc
memory_profile_malloc:          ; EAX the mspace, [esp+4] the bytes; cdecl, EAX the memory or 0
    cmp byte [SRP_MEMORY_PROFILE], 0
    jne .on
.stock:
    sub esp, 8
    push ebx
    push ebp
    jmp MALLOC_ON
.on:
    push dword [esp + 4]        ; the bytes again, for the body called below (EAX still the mspace)
    call .stock
    add esp, 4
    push ecx
    push edx
    lea ecx, [esp + 8]          ; the game's ESP at its call: its return address there
    push eax                    ; the memory
    push ecx
    call memory_profile_on_allocated
    pop ecx
    pop eax
    pop edx
    pop ecx
    ret

global memory_profile_realloc
memory_profile_realloc:         ; EAX the mspace, [esp+4] the memory, [esp+8] the bytes; cdecl
    cmp byte [SRP_MEMORY_PROFILE], 0
    jne .on
.stock:
    mov ecx, [esp + 8]
    push esi
    jmp REALLOC_ON
.on:
    push ecx
    push edx
    lea ecx, [esp + 8]
    push dword [ecx + 4]        ; the memory given back
    push ecx
    call memory_profile_on_freeing
    add esp, 8
    pop edx
    pop ecx
    push dword [esp + 8]        ; the bytes
    push dword [esp + 8]        ; the memory
    call .stock
    add esp, 8
    push ecx
    push edx
    lea ecx, [esp + 8]
    push eax                    ; the memory now (0: none, the old one kept)
    push ecx
    call memory_profile_on_allocated
    pop ecx
    pop eax
    pop edx
    pop ecx
    ret

global memory_profile_free
memory_profile_free:            ; ESI the mspace, EAX the memory
    cmp byte [SRP_MEMORY_PROFILE], 0
    jne .on
.stock:
    sub esp, 0Ch
    push edi
    mov edi, eax
    jmp FREE_ON
.on:
    push ecx
    push edx
    lea ecx, [esp + 8]
    push eax
    push ecx
    call memory_profile_on_freeing
    add esp, 8
    pop edx
    pop ecx
    jmp .stock                  ; the body returns to the game's caller itself
