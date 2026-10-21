; Patch 46: on the death screen a drag turns the camera; the wheel does
; nothing (build 2777, client_patch.md section 3.43; the rules:
; ../docs/notes/death-camera.md). Two entries in the section `.srpc`
; (DEATH_CAMERA_* in patch_026g.py, which checks them):
; - death_look (+0): the first ten bytes of the game world's on_mouse_move
;   (0x25D0E0, WARMUP_LOOK_HOOK) jump here, before warmup_look (patch 31),
;   which every move but a death screen's goes on to;
; - death_view (+DEATH_VIEW_OFFSET): player::update_camera's call of the
;   view's update (0x2422B0, at 0x1F364B) calls it.
; Its state (8 bytes) at STATE, the late join's state page +E60h; the drag
; (the moves of the buttons held, the hud's pointer where it began) is
; warmup_look's own state at LOOK_STATE, its end warmup_limits' drag_end.
;   nasm -f bin -DCAVE=0xA429000 -DSTATE=0xA428E60 -DLOOK=0xA425D40 -DLOOK_STATE=0xA428E20 -DDRAG_END=0xA425A98 death_camera_026g.asm -o death_camera_026g.section.bin
;
; The death screen today: a death leaves the player's input handler in the
; first person mode (+358h = 0; on_player_death 0x1F28E0 changes no mode),
; and update_camera (0x1F3480) gives the view's update (0x2422B0) the
; player's eye transform (+44Ch), which the update copies as it is (modes 0
; and 4): the corpse's eyes, still. The hud shows the kits and the cursor
; (0x2383B0), so the game world's handler (priority 7) takes every move and
; the wheel and answers 1; the player's handler (priority 10) gets none. The
; game sends no input for a dead local player (game_world::tick, 0x1F7883:
; the input is built only while player+33Ch is set).
;
; Here, while the kits are shown, the player's handler is in the first
; person mode and the local player (game+53ACh, the client, +4) is dead:
; - a button held: a drag as warmup_look's (the lobby's buttons, right, 5th
;   and 7th, at once; the left one past CLICK, so a click stays the kits').
;   A drag takes the hud's pointer off the screen; its moves turn the camera
;   (STATE: a yaw and a pitch, the player's look's step, the pitch within
;   PITCH_MAX) and are taken (1): neither the cursor's nor the player's.
;   The wheel in a drag is taken with them: it does nothing.
; - no button held: a drag ends (drag_end: the pointer and the system's
;   cursor back where it began); the move, the wheel's too, goes on through
;   warmup_look to the original: the cursor's, which answers 1. The wheel
;   does nothing, as in the original.
; death_view: while that player is dead, in the first person mode, the
; kits shown and the yaw or the pitch not 0, the view is the update's own
; third-person step (0x2422DE) on the eye transform with that yaw and pitch
; and the distance 0: the eyes' place, the look turned about the world's up
; and then about its own right. The handler's warmup fields it borrows
; (+34Ch, +350h, +354h) are put back. Any other time the original update;
; a live player (a respawn) clears the yaw and the pitch.
; Nothing a match computes: the camera alone.
BITS 32
ORG CAVE

%define KITS        0x2E3          ; game world's handler: the kits shown (hud+2DFh, 0x2383B0)
%define INPUT       0x4B60         ; handler: the player's input handler (game world+4D58h)
%define GAME        -0x15C         ; handler: the game (game world+9Ch)
%define WORLD       -0x1F8         ; handler: the game world
%define POINTER     -0x16C         ; handler: the hud's pointer, x and y (game world+8Ch, +90h)
%define CLIENT      0x53AC         ; game: the network client
%define LOCAL        4             ; network client: the local player
%define RENDER      0x5348         ; game: its vtable+1Ch gives the window's size (w, h), 0x2402BE
%define ALIVE       0x33C          ; player: m_is_alive
%define MODE        0x358          ; player's input handler: its mode, 0 the first person
%define VIEW_WORLD  0x334          ; player's input handler: the game world
%define VIEW_KITS   0x4DB          ; game world: the kits shown (hud+2DFh)
%define YAW_W       0x34C          ; player's input handler: the warmup camera's yaw,
%define PITCH_W     0x350          ; pitch
%define DISTANCE_W  0x354          ; and distance (process_warmup_mode 0x242200)
%define LEFT        0x01           ; the mouse's +10h: the buttons held
%define LOBBY       0x52           ; the lobby camera's buttons (0x283D81)
%define CLICK       8              ; a left press moved no more is a click
%define AWAY        -128           ; the hud's pointer in a drag
%define SENSITIVITY 0xA74AEC       ; the option sensitivity, a float (0x24026C)
%define INVERT      0x1F1D9D3      ; the option mouse_invertion, a byte (0x240226)
%define PITCH_MAX   0x3FB2B8C2     ; 1.3962634f: +-80 degrees
%define DEATH_VIEW_OFFSET 0x180   ; death_view's place in the section
%define UPDATE      0x2422B0       ; the view's update (the handler, the transform; ret 8)
%define THIRD       0x2422DE       ; its step for the modes but 0 and 4

; STATE: +0 the yaw, +4 the pitch (floats). LOOK_STATE: +0 the moves of the
; buttons held (above CLICK: a drag), +4, +8 the hud's pointer at its start.
%define S_YAW       0
%define S_PITCH     4
%define S_TRAVEL    0
%define S_POINTER   4

; ecx = the game world's input handler; [esp+4] the input world, then x, y,
; the wheel; `ret 10h`.
death_look:
    cmp byte [ecx+KITS], 0
    je .warmup
    mov eax, [ecx+INPUT]
    test eax, eax
    jz .warmup
    cmp dword [eax+MODE], 0
    jne .warmup
    mov eax, [ecx+GAME]
    test eax, eax
    jz .warmup
    mov eax, [eax+CLIENT]
    test eax, eax
    jz .warmup
    mov eax, [eax+LOCAL]
    test eax, eax
    jz .warmup
    cmp byte [eax+ALIVE], 0
    je .dead
.warmup:
    jmp LOOK
.dead:
    push ebx
    push esi
    push edi
    mov esi, ecx                    ; the handler
    mov ebx, LOOK_STATE             ; the drag
    mov edi, [esp+12+4]             ; the input world
    mov edi, [edi+0x28]             ; its mouse: +4 x, +8 y, +0Ch the wheel
    mov al, [edi+0x10]
    mov ecx, [ebx+S_TRAVEL]
    test al, LOBBY | LEFT
    jz .up
    cmp ecx, CLICK
    ja .turn                        ; in a drag
    test al, LOBBY
    jnz .drag                       ; the lobby's buttons: at once
    mov eax, [edi+4]                ; the left button: |x| + |y| since it went down
    cdq
    xor eax, edx
    sub eax, edx
    add ecx, eax
    mov eax, [edi+8]
    cdq
    xor eax, edx
    sub eax, edx
    add ecx, eax
    mov [ebx+S_TRAVEL], ecx
    cmp ecx, CLICK
    jbe .cursor                     ; still a click
.drag:
    or dword [ebx+S_TRAVEL], -1
    movq xmm0, [esi+POINTER]
    movq [ebx+S_POINTER], xmm0
    push 0
    push AWAY
    push AWAY
    lea ecx, [esi+WORLD]
    mov eax, [ecx]
    call [eax+0x30]                 ; the hud's pointer off the screen
.turn:
    mov ecx, [esi+GAME]             ; the window's height over its width
    mov ecx, [ecx+RENDER]
    sub esp, 8
    push esp
    mov eax, [ecx]
    call [eax+0x1C]                 ; w, h into the 8 bytes (`ret 4`, 0x283E1C)
    movss xmm1, [esp+4]
    divss xmm1, [esp]
    add esp, 8
    movss xmm0, [SENSITIVITY]       ; a unit's turn, as the player's look (0x24026C..0x240353)
    mulss xmm0, [step]
    mulss xmm1, xmm0
    mulss xmm1, [aspect]
    cmp byte [INVERT], 0
    je .upright
    movd eax, xmm1
    xor eax, 0x80000000
    movd xmm1, eax
.upright:
    cvtsi2ss xmm2, dword [edi+4]    ; yaw -= x * step
    mulss xmm2, xmm0
    movss xmm3, [STATE+S_YAW]
    subss xmm3, xmm2
    movss [STATE+S_YAW], xmm3
    mov eax, [edi+8]
    test eax, eax
    jz .taken
    cvtsi2ss xmm2, eax              ; pitch -= y * step * h / w * 0.955 (inverted: +)
    mulss xmm2, xmm1
    movss xmm3, [STATE+S_PITCH]
    subss xmm3, xmm2
    movd eax, xmm3                  ; within +-PITCH_MAX, as integers (warmup_limits)
    mov edx, eax
    and edx, 0x7fffffff
    cmp edx, PITCH_MAX
    jbe .pitch
    and eax, 0x80000000
    or eax, PITCH_MAX
.pitch:
    mov [STATE+S_PITCH], eax
.taken:
    mov al, 1                       ; neither the cursor's nor the player's
    pop edi
    pop esi
    pop ebx
    ret 0x10
.up:
    call DRAG_END                   ; the end of a drag
.cursor:
    mov ecx, esi
    pop edi
    pop esi
    pop ebx
    jmp LOOK                        ; on to warmup_look and the original: the cursor's

align 4, db 0xcc
step:   dd 0.0017453292             ; pi / 180 * 0.1 (0x2402F1: 1/180, pi, 0.1)
aspect: dd 0.95492965               ; 0xA2DAC4

times DEATH_VIEW_OFFSET-($-$$) db 0xcc

; Called instead of the view's update by update_camera (0x1F364B): ebx =
; the player (update_camera's [ebp+8]), [esp+4] its input handler, [esp+8]
; the transform (the eyes', +44Ch, in the first person mode); `ret 8`.
death_view:
    mov eax, [esp+4]
    cmp byte [ebx+ALIVE], 0
    jne .reset
    cmp dword [eax+MODE], 0
    jne .original
    mov edx, [eax+VIEW_WORLD]
    cmp byte [edx+VIEW_KITS], 0
    je .original
    mov edx, [STATE+S_YAW]
    or edx, [STATE+S_PITCH]
    add edx, edx                    ; -0.0 is 0
    jz .original
    push dword [eax+DISTANCE_W]
    push dword [eax+PITCH_W]
    push dword [eax+YAW_W]
    mov edx, [STATE+S_YAW]
    mov [eax+YAW_W], edx
    mov edx, [STATE+S_PITCH]
    mov [eax+PITCH_W], edx
    and dword [eax+DISTANCE_W], 0
    push dword [esp+20]             ; the transform
    push eax                        ; the handler
    call .third
    mov eax, [esp+16]
    pop dword [eax+YAW_W]
    pop dword [eax+PITCH_W]
    pop dword [eax+DISTANCE_W]
    ret 8
.reset:
    and dword [STATE+S_YAW], 0
    and dword [STATE+S_PITCH], 0
.original:
    jmp UPDATE
.third:                             ; 0x2422B0's own start, then its third-person step
    sub esp, 0xd4
    push ebx
    mov ebx, [esp+0xe0]             ; the transform
    push ebp
    mov ebp, [esp+0xe0]             ; the handler
    push esi
    push edi
    jmp THIRD
