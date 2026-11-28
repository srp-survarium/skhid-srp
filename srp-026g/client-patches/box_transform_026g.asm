; Patch 221: aabb::modify without a branch per term, the same floats
; (rust/patches/src/box_transform.toml's `code` hook).
;
; aabb::modify (0xF1FD0; EAX the box: min xyz, max xyz, ECX the 4x4
; transform: rows i, j, k, then c at +30h; a plain `ret`) moves a box by a
; transform, Arvo's way: each of min and max starts at the moved min corner
; (lane x ((i.x*min.x + min.z*k.x) + min.y*j.x) + c.x; lanes y, z
; ((k*min.z + j*min.y) + i*min.x) + c), then for each row r in turn and each
; axis a, the term row_r.a * size_r (size = max - min) is added to min.a when
; its sign bit is set, else to max.a. select_lod (0x16EF55) and the models'
; and particles' bounds call it for every visible thing every frame, nine
; branches on signs a call.
;
; The same here, an axis a lane: the same products, sums in the same order,
; each row's sum taken where the sign says, so every lane rounds as the
; scalar operation does (also with the MXCSR's FTZ or DAZ). With no NaN
; anywhere (every input reaches an output by additions and products, so a
; NaN reaches an output) the operands' order inside a product or a sum does
; not change its bits; when an output is NaN (an input was, and two NaN
; operands give the first one's payload) the box is left as it was and the
; game's own code makes it. The inputs are read whole before the box is
; written, as the game's. EAX kept, ECX and EDX the signs of k.z*size.z and
; k.y*size.z (what the game's last tests leave), XMM0..7 and the flags
; changed (the game's uses them all), EBX ESI EDI EBP kept.
; check_box_transform_026g.py runs both on the same boxes and transforms.
bits 32
section .text

%define MODIFY_BACK 0xF1FD7     ; aabb::modify past sub esp,38h / movss xmm0,[ecx]

; one row's terms (%1, their signs' masks the lanes) into min (XMM1) or max (XMM0)
%macro add_row 1
    movaps xmm5, xmm1
    addps xmm5, %1              ; min + term
    movaps xmm6, xmm0
    addps xmm6, %1              ; max + term
    movaps xmm7, %1
    psrad xmm7, 31              ; the sign bits
    xorps xmm5, xmm1
    andps xmm5, xmm7
    xorps xmm1, xmm5            ; min + term where the sign is set
    xorps xmm0, xmm6
    andps xmm0, xmm7
    xorps xmm0, xmm6            ; max + term where it is clear
%endmacro

global box_transform_modify
box_transform_modify:           ; the jump from 0xF1FD0: EAX the box, ECX the transform
    movss xmm0, [eax]           ; a float a load, as the game's: a load inside
    movss xmm5, [eax + 4]       ; one store of the caller's (select_lod copies
    movss xmm6, [eax + 8]       ; the box with three movq) takes its bytes
    unpcklps xmm0, xmm5         ; from the store buffer
    movlhps xmm0, xmm6          ; min
    movss xmm1, [eax + 0Ch]
    movss xmm5, [eax + 10h]
    movss xmm6, [eax + 14h]
    unpcklps xmm1, xmm5
    movlhps xmm1, xmm6
    subps xmm1, xmm0            ; size = max - min
    movups xmm2, [ecx]          ; i
    movups xmm3, [ecx + 10h]    ; j
    movups xmm4, [ecx + 20h]    ; k
    movaps xmm5, xmm0
    shufps xmm5, xmm5, 0
    mulps xmm5, xmm2            ; i * min.x
    movaps xmm6, xmm0
    shufps xmm6, xmm6, 55h
    mulps xmm6, xmm3            ; j * min.y
    shufps xmm0, xmm0, 0AAh
    mulps xmm0, xmm4            ; k * min.z
    movaps xmm7, xmm1
    shufps xmm7, xmm7, 0
    mulps xmm2, xmm7            ; row i's terms: i * size.x
    movaps xmm7, xmm1
    shufps xmm7, xmm7, 55h
    mulps xmm3, xmm7            ; row j's
    shufps xmm1, xmm1, 0AAh
    mulps xmm4, xmm1            ; row k's
    movaps xmm1, xmm0
    movss xmm1, xmm5            ; i.x*min.x, k.y*min.z, k.z*min.z
    movaps xmm7, xmm6
    movss xmm7, xmm0            ; k.x*min.z, j.y*min.y, j.z*min.y
    addps xmm1, xmm7
    movss xmm5, xmm6            ; j.x*min.y, i.y*min.x, i.z*min.x
    addps xmm1, xmm5
    movq xmm0, [ecx + 30h]
    movss xmm5, [ecx + 38h]
    movlhps xmm0, xmm5
    addps xmm1, xmm0            ; + c: the moved min corner, min
    movaps xmm0, xmm1           ; and max
    add_row xmm2
    add_row xmm3
    add_row xmm4
    movaps xmm5, xmm1
    cmpunordps xmm5, xmm0
    movmskps edx, xmm5
    test edx, 7
    jnz .game
    movq [eax], xmm1
    movhlps xmm1, xmm1
    movss [eax + 8], xmm1
    movq [eax + 0Ch], xmm0
    movhlps xmm0, xmm0
    movss [eax + 14h], xmm0
    movmskps edx, xmm4
    mov ecx, edx
    shr edx, 1
    and edx, 1                  ; k.y*size.z's sign
    shr ecx, 2
    and ecx, 1                  ; k.z*size.z's
    ret
.game:                          ; a NaN: the game's own code, from its start
    sub esp, 38h
    movss xmm0, [ecx]
    jmp MODIFY_BACK
