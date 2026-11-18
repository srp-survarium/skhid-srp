; Patch 134 (build 2777): shader constants cost the render thread less, with
; the same bytes on the GPU (rust/patches/src/constant_upload.toml's four
; `code` hooks; README "Shader constants: a faster copy, no upload of
; unchanged bytes").
;
; A shader_constant_buffer (0x68 bytes) keeps its constants in a block of
; its own (+58h, the shadow, +5Ch bytes), a dynamic ID3D11Buffer (+60h) and
; m_changed (+64h). Before each draw the backend copies every bound
; constant of the shader's tables into the shadows (set_memory 0x11B5A0, from
; the tables' assign loops 0x3D63D4/0x3D6714/0x3D6904 and 67 other places),
; then update (0x3DDFD0) maps each buffer with m_changed set
; (Map WRITE_DISCARD), copies the whole shadow and unmaps it. In 0.26g
; set_memory sets m_changed always (the 0.10b source set it only when a byte
; differed), so every bound buffer is uploaded before every draw.
;
; - constant_upload_set_memory (0x11B5A0, the whole function): the same copy
;   16 bytes at a time, not one; the registers left as the original leaves
;   them (EAX = ECX = the end written, DL the first differing byte pair's
;   XOR or 0, EDX's upper bytes, EBX, ESI, EDI, EBP and XMM0/XMM1 kept),
;   m_changed set.
; - constant_upload_alloc, constant_upload_zero (0x3DDFA1, 0x3DDFAC, the
;   constructor): the shadow's block is 2 x size + 16 bytes, all zeros: the
;   shadow, then the upload's state (a dword: 0 never uploaded, 1 uploaded)
;   and 12 bytes, then a copy of the shadow as last uploaded.
; - constant_upload_update (0x3DDFD0, update's start): with m_changed set it
;   compares the shadow with that copy (and copies it): when the buffer was
;   uploaded before and no byte differs, the GPU's buffer already holds these
;   bytes, so it only clears m_changed, as the upload does; else the
;   original's Map, copy and Unmap. Nothing else writes these GPU buffers
;   (every Map of the exe checked: update's at 0x3DDFFA is the only one of a
;   constant buffer; a dynamic buffer takes no UpdateSubresource).
bits 32
section .text

%define CB_DATA 58h             ; the shadow
%define CB_SIZE 5Ch             ; its bytes (a constant buffer's, a multiple of 16)
%define CB_CHANGED 64h          ; m_changed
%define TRAILER 16              ; past the shadow: the upload's state, then 12 bytes, then the copy
%define UPLOADED 1              ; the state once update has uploaded the buffer (0 before)
%define CTOR_ALLOC_BACK 0x3DDFA7    ; the constructor's call of malloc_impl (0xE4F90, EAX the bytes)
%define CTOR_ZERO_BACK 0x3DDFB2     ; its push of the block, then memset (0x424500)
%define UPDATE_UPLOAD 0x3DDFDD      ; update after its je: Map, memcpy, Unmap, m_changed = 0

; set_memory(EAX source, ECX bytes, ESI offset, [esp+4] the buffer), ret 4:
; copies min(ECX, size - offset) bytes (unsigned, as the original) from the
; source to shadow + offset.
global constant_upload_set_memory
constant_upload_set_memory:
    push ebp
    mov ebp, [esp + 8]
    push edi
    push ebx
    sub esp, 32
    movdqu [esp], xmm0
    movdqu [esp + 16], xmm1
    mov edi, eax                ; the source
    mov eax, [ebp + CB_SIZE]
    sub eax, esi
    cmp ecx, eax
    jbe .counted
    mov ecx, eax
.counted:
    mov eax, [ebp + CB_DATA]
    add eax, esi                ; the first byte written
    lea ebx, [eax + ecx]        ; the end
    xor dl, dl                  ; no byte has differed
.compare:                       ; 16 at a time until one differs
    mov ecx, ebx
    sub ecx, eax
    cmp ecx, 16
    jb .tail
    movdqu xmm0, [edi]
    movdqu xmm1, [eax]
    pcmpeqb xmm1, xmm0
    pmovmskb ecx, xmm1
    xor ecx, 0FFFFh
    jnz .differs
    movdqu [eax], xmm0
    add eax, 16
    add edi, 16
    jmp .compare
.differs:                       ; the first that differs, as the original's DL
    bsf ecx, ecx
    mov dl, [eax + ecx]
    xor dl, [edi + ecx]
.copy:                          ; the rest, 16 at a time
    mov ecx, ebx
    sub ecx, eax
    cmp ecx, 16
    jb .tail
    movdqu xmm0, [edi]
    movdqu [eax], xmm0
    add eax, 16
    add edi, 16
    jmp .copy
.tail:                          ; under 16 left: the original's loop
    cmp eax, ebx
    je .done
.byte:
    test dl, dl
    jnz .store
    mov dl, [edi]
    xor dl, [eax]
.store:
    mov cl, [edi]
    mov [eax], cl
    inc eax
    inc edi
    cmp eax, ebx
    jne .byte
.done:
    mov ecx, eax
    movdqu xmm0, [esp]
    movdqu xmm1, [esp + 16]
    add esp, 32
    pop ebx
    pop edi
    mov byte [ebp + CB_CHANGED], 1
    pop ebp
    ret 4

; the constructor, `mov ecx,[edi+5Ch] / push eax / mov eax,ecx` before
; malloc_impl (EDI the buffer, EAX the description): room for the copy
global constant_upload_alloc
constant_upload_alloc:
    mov ecx, [edi + CB_SIZE]
    push eax
    lea eax, [ecx*2 + TRAILER]
    jmp CTOR_ALLOC_BACK

; the constructor, `mov edx,[edi+5Ch] / push edx / push 0` before memset
; (EAX the block): all of it zeros, the state "never uploaded"
global constant_upload_zero
constant_upload_zero:
    mov edx, [edi + CB_SIZE]
    lea edx, [edx*2 + TRAILER]
    push edx
    push 0
    jmp CTOR_ZERO_BACK

; update(ESI the buffer), its `sub esp,14h / cmp byte [esi+64h],0`; the
; caller (0x3D6415) keeps EBX, EDI and EBP
global constant_upload_update
constant_upload_update:
    cmp byte [esi + CB_CHANGED], 0
    je .clean
    push edi
    push ebx
    mov eax, [esi + CB_DATA]
    mov ebx, [esi + CB_SIZE]
    lea edi, [eax + ebx]        ; the upload's state
    add ebx, eax                ; the shadow's end
    xor edx, edx
    cmp dword [edi], UPLOADED
    setne dl                    ; never uploaded: upload
    mov dword [edi], UPLOADED
    add edi, TRAILER            ; the copy
.chunk:                         ; compare and copy, 16 at a time
    mov ecx, ebx
    sub ecx, eax
    cmp ecx, 16
    jb .tail
    movdqu xmm0, [eax]
    movdqu xmm1, [edi]
    pcmpeqb xmm1, xmm0
    pmovmskb ecx, xmm1
    movdqu [edi], xmm0
    cmp ecx, 0FFFFh
    setne cl
    or dl, cl
    add eax, 16
    add edi, 16
    jmp .chunk
.tail:
    cmp eax, ebx
    je .compared
    mov cl, [eax]
    cmp cl, [edi]
    setne ch
    mov [edi], cl
    or dl, ch
    inc eax
    inc edi
    jmp .tail
.compared:
    pop ebx
    pop edi
    test dl, dl
    jnz .upload
    mov byte [esi + CB_CHANGED], 0  ; the GPU's buffer holds these bytes
.clean:
    ret
.upload:
    sub esp, 14h
    jmp UPDATE_UPLOAD
