; Patch 134 (build 2777): time_channel::domain without a division per step
; (rust/patches/src/animation_math.toml's `code` hook; the sines, cosines and
; atan2 of the same patch are its Rust `call` hooks, animation_math.rs).
;
; time_channel<float4>::domain (0x3B3340; EAX the channel {+0 the knot count
; n, +4 the offset of its memory from the channel: n float4 domains, then n
; float knots}, XMM2 the time, [esp+4] the current domain, `ret 4`; called
; only by evaluate_frame 0x3B31A6, for each channel of each bone of each
; animation every frame) clamps the time to the first and last knots and
; looks for the domain j with knots[j] <= t <= knots[j+1], trying
; j = (current + i) mod n for i = 0, 1, ... (the source's
; `( i + current_domain ) % m_knots_count`, its bound dropped by the
; compiler: it never stops before it finds one), a `div` each try. It stores
; j at the current and returns it.
;
; The same search here, try for try: the first j is current mod n (no
; division when current < n, as it is after the first call), and each next
; j is the previous + 1, 0 at n -- the same (current + i) mod n, also where
; current + i wraps past 2^32 (then 0 mod n = 0). The clamp, the compares
; (their operands and jumps) and every memory read are the original's; what
; the caller can see after the return is too: EAX = EDX = j, ECX = n, XMM0
; the clamped time, XMM1 knots[j+1], EBX ESI EDI EBP kept, the argument
; popped. A count of 0 divides by 0 as the original's first try does.
; check_animation_math_026g.py runs both on the same channels.
bits 32
section .text

global animation_math_domain
animation_math_domain:          ; the jump from 0x3B3340: eax the channel, xmm2 t, [esp+4] the current
    push ebx
    push esi
    push edi
    mov ecx, [eax]              ; n
    mov esi, ecx
    shl esi, 4
    add esi, [eax + 4]
    add esi, eax                ; the knots: channel + offset + 16 n
    movss xmm1, [esi]           ; knots[0]
    movss xmm0, [esi + ecx*4 - 4]   ; knots[n - 1]
    comiss xmm1, xmm2
    jb .above_first
    movaps xmm0, xmm1           ; t <= knots[0]: knots[0]
    jmp .clamped
.above_first:
    comiss xmm0, xmm2
    jb .clamped                 ; t > knots[n - 1] (or unordered): knots[n - 1]
    movaps xmm0, xmm2           ; t
.clamped:
    mov ebx, [esp + 16]         ; the current domain's place
    mov edi, [ebx]              ; current + i, i = 0
    mov eax, edi
    cmp edi, ecx
    jb .try                     ; current < n: j = current
    xor edx, edx
    div ecx                     ; j = current mod n
    mov eax, edx
.try:
    comiss xmm0, [esi + eax*4]
    jb .next                    ; t < knots[j] (or unordered)
    movss xmm1, [esi + eax*4 + 4]
    comiss xmm1, xmm0
    jae .found                  ; knots[j + 1] >= t
.next:
    inc eax
    cmp eax, ecx
    jb .wrapped
    xor eax, eax                ; j + 1 = n: 0
.wrapped:
    add edi, 1
    jnz .try
    xor eax, eax                ; current + i wrapped to 0: 0 mod n
    jmp .try
.found:
    mov [ebx], eax
    mov edx, eax
    pop edi
    pop esi
    pop ebx
    ret 4
