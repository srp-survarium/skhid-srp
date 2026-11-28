; Patch 220: cuboid::test_inexact with four planes at a time, the same verdicts
; (rust/patches/src/frustum_test.toml's `code` hook).
;
; cuboid::test_inexact (0xF2BC0; EAX the cuboid: six aabb_planes of 14h
; bytes, [esp+4] the box: min xyz, max xyz; `ret 4`) is the render's frustum
; test of every octree node and model (select_models, the shadow cascades,
; the particles). It calls aabb_plane::test (0x1073B0) on each plane in turn:
; the plane's +10h bits pick, through the index triples at 0x9CC880, the
; corner farthest along the normal (bits 0..2: an axis' max when set, its
; min when clear) and the nearest one (bits 3..5); the far corner's distance
; ((nx*x + nz*z) + ny*y) + d below 0 is outside (2, at once), else the near
; corner's below 0 is intersect (3); six planes with neither are inside (1),
; any other count intersect (3).
;
; The same here: the four planes 0..3 and then 4, 5 in the lanes of SSE
; registers, each corner coordinate picked by its bit, each distance the
; same products added in the same order (the packed operations round every
; lane as the scalar ones do), "below 0" the same ordered compare (a NaN
; distance is not below). Outside when any plane's far distance is below 0,
; else intersect when any near one is, else inside: the loop's verdict, as
; plane k returns 2 only past planes that did not. A plane whose bits are
; not under 40h (the table has eight triples; aabb_plane::normalize 0x1072E0
; writes far | (far ^ 7) << 3, always under 40h) goes to the game's own loop.
; EAX the verdict; ECX, EDX, XMM0..7 and the flags changed, as the game's
; leaves them (it uses them all); EBX, ESI, EDI, EBP kept.
; check_frustum_test_026g.py runs both on the same cuboids and boxes.
bits 32
section .text

%define TEST_INEXACT_BACK 0xF2BC6   ; test_inexact past push ebx / push ebp / mov ebp,[esp+0Ch]

%define MIN_X 0                 ; the stack's 16-aligned room: each min broadcast,
%define MIN_Y 16                ; then each (min xor max), so that
%define MIN_Z 32                ; min xor (mask and (min xor max)) is the max
%define DIF_X 48                ; where the mask is set, the min where clear
%define DIF_Y 64
%define DIF_Z 80
%define NEAR_A 96               ; planes 0..3's near verdicts
%define ROOM 112

; %1 the register given the corner coordinate of four planes: the plane bit
; %2 of XMM6's bits picks DIF/MIN %3's max or min
%macro corner 3
    movdqa %1, xmm6
    pslld %1, 31 - (%2)
    psrad %1, 31
    andps %1, [esp + DIF_%3]
    xorps %1, [esp + MIN_%3]
%endmacro

; ECX the four lanes' "distance below 0" bits of the corner picked by bits
; %1, %1+1, %1+2: X XMM0, Y XMM2, Z XMM4, D XMM5, the bits XMM6
%macro below 1
    corner xmm7, %1, X
    mulps xmm7, xmm0
    corner xmm1, %1 + 2, Z
    mulps xmm1, xmm4
    addps xmm7, xmm1
    corner xmm1, %1 + 1, Y
    mulps xmm1, xmm2
    addps xmm7, xmm1
    addps xmm7, xmm5
    xorps xmm1, xmm1
    cmpltps xmm7, xmm1
    movmskps ecx, xmm7
%endmacro

global frustum_test_inexact
frustum_test_inexact:           ; the jump from 0xF2BC0: EAX the cuboid, [esp+4] the box
    mov ecx, [eax + 10h]
    or ecx, [eax + 24h]
    or ecx, [eax + 38h]
    or ecx, [eax + 4Ch]
    or ecx, [eax + 60h]
    or ecx, [eax + 74h]
    test ecx, ~3Fh
    jnz .game
    mov ecx, [esp + 4]          ; the box
    mov edx, esp
    sub esp, ROOM + 12
    and esp, -16
%assign axis 0
%rep 3
    movss xmm0, [ecx + 4 * axis]        ; min
    movss xmm1, [ecx + 4 * axis + 12]   ; max
    xorps xmm1, xmm0
    shufps xmm0, xmm0, 0
    shufps xmm1, xmm1, 0
    movaps [esp + MIN_X + 16 * axis], xmm0
    movaps [esp + DIF_X + 16 * axis], xmm1
%assign axis axis + 1
%endrep
    ; planes 0..3: X Y Z D across the lanes
    movups xmm0, [eax]
    movups xmm1, [eax + 14h]
    movups xmm2, [eax + 28h]
    movups xmm3, [eax + 3Ch]
    movaps xmm4, xmm0
    unpcklps xmm0, xmm1         ; x0 x1 y0 y1
    unpckhps xmm4, xmm1         ; z0 z1 d0 d1
    movaps xmm5, xmm2
    unpcklps xmm2, xmm3         ; x2 x3 y2 y3
    unpckhps xmm5, xmm3         ; z2 z3 d2 d3
    movaps xmm1, xmm0
    movlhps xmm0, xmm2          ; X
    movhlps xmm2, xmm1          ; Y
    movaps xmm3, xmm4
    movlhps xmm4, xmm5          ; Z
    movhlps xmm5, xmm3          ; D
    movd xmm6, [eax + 10h]
    movd xmm1, [eax + 24h]
    punpckldq xmm6, xmm1
    movd xmm1, [eax + 38h]
    movd xmm3, [eax + 4Ch]
    punpckldq xmm1, xmm3
    punpcklqdq xmm6, xmm1       ; the bits
    below 0
    test ecx, ecx
    jnz .outside
    below 3
    mov [esp + NEAR_A], ecx
    ; planes 4, 5 in lanes 0, 1 (lanes 2, 3: their y and d, bits 0)
    movups xmm0, [eax + 50h]
    movups xmm1, [eax + 64h]
    movaps xmm4, xmm0
    unpcklps xmm0, xmm1         ; x4 x5 y4 y5: X
    unpckhps xmm4, xmm1         ; z4 z5 d4 d5: Z
    movaps xmm2, xmm0
    movhlps xmm2, xmm0          ; y4 y5: Y
    movaps xmm5, xmm4
    movhlps xmm5, xmm4          ; d4 d5: D
    movd xmm6, [eax + 60h]
    movd xmm1, [eax + 74h]
    punpckldq xmm6, xmm1
    below 0
    test ecx, 3
    jnz .outside
    below 3
    and ecx, 3                  ; lanes 2, 3 count for nothing
    or ecx, [esp + NEAR_A]
    mov eax, 1                  ; inside
    jz .done
    mov eax, 3                  ; intersect
    jmp .done
.outside:
    mov eax, 2
.done:
    mov esp, edx
    ret 4
.game:                          ; the game's own loop
    push ebx
    push ebp
    mov ebp, [esp + 0Ch]
    jmp TEST_INEXACT_BACK
