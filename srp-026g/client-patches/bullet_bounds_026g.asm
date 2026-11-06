BITS 32
section .text
global bullet_bounds_static

; bullet_physics_world::add, after Bullet has inserted the body.
; EBP+0Ch is the collision group. The level uses 2 (walkers) and 8
; (bullets); transient game bodies also have bit 800h (grenades 818h,
; victory items A18h). Their historical insertion positions must not
; expand the level bounds used by bullet::update_bullet_position.
; The original insertion and the final update_single_aabb still run.
; No register or stack change; only the displaced branch's flags change.
bullet_bounds_static:
    test word [ebp+0xc], 0x800
    jnz .skip
    test byte [ebp+0xc], 0xa
    jz .skip
    jmp 0x758eb
.skip:
    jmp 0x75a37
