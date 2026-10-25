; Patch 71 (build 2777): the freeze detector. The arena's blocks freeze.code,
; freeze.data and freeze.state with freeze_026g.c (link_026g.py links them;
; patch_026g.py's FREEZE_HOOKS jump and call to the entries by their names).
; client_patch.md section 3.58; the rules: ../docs/notes/freeze-detector.md.
;
; Each hooked function's first instructions (five to eight bytes, no address,
; no branch) become a jump here; the entry counts the call, and for a timed
; function keeps the caller's return address and the time stamp counter in a
; slot of its own and puts its leave in the return address' place. It then
; runs the function's own first instructions (the same bytes) and jumps back
; past them: the function runs with the registers, the flags and the stack the
; caller gave it. When it returns (its own `ret n`, the arguments popped) it
; comes to the leave, which adds the microseconds it took to the function's
; total, frees the slot and returns to the caller with the function's
; registers and flags. Two slots a function: a second thread, or the function
; called again from inside itself, takes the second; a third call is counted
; and not timed. The clock is the time stamp counter (one instruction, edx:eax
; alone), turned into microseconds with frz_tsc_mult, which frz_frame works
; out against QueryPerformanceCounter; until the first second, 0.
;
; frz_totals[i] = {calls, microseconds}, each only ever growing (a `lock`
; add from any thread); freeze_026g.c takes the frame's own by subtraction.
;
;   index  function (entry)                                      what
;   0      engine_world::logic_tick (0x1A030)                    logic frames, timed; then frz_logic_done
;   1      res_render_output::present (0x3CF7F0)                 the swap chain's present, timed; then frz_frame
;   2      game_world_core::move (0x2D2E40)                      the simulation brought up to the clock, timed
;   3      game_world_core::atomic_tick (0x2D1E20)               ticks simulated (again after a rollback too)
;   4      game_world_core::deserialize (0x2D2770)               a world's history loaded: a rollback
;   5      network_client::on_match_packet_received (0x27A300)   match messages handled
;   6      udp_match_connection::process_incoming_packet (0x3F3BB0)  datagrams received
;   7      the camouflage/decal baker's on_textures_loaded (0x12E510)       bakes, timed
;   8      the camouflage/decal baker's on_baked_texture_loaded (0x12DB40)  baked textures loaded from
;                                                                         user_data/streamed_textures, timed
;   9      resources::query_resources_and_wait (0xE5C20)         synchronous loads, timed (both callers'
;                                                                 query_*_and_wait 0xE5DF0, 0xE5F30 come here)
;   10     query_result::do_create_resource (0x1096C0)           resources a cook made
;   11     resources::dispatch_callbacks (0xE5AB0)               the resources' callbacks run, timed
;   12     scene_renderer::play_particle_system (0x22660)        particle systems started
;   13     Scaleform::GFx::Movie::Invoke (0x511600)              ActionScript invokes
;   14     GFx MovieImpl::Advance (0x51F1C0)                     movies advanced, timed
;   15     renderer::render_flash_ui (0x13CED0)                  the movies drawn, timed
;   16     bullet_manager::emit_bullet (0x322830)                bullets fired (weapon_core::instant_fire,
;                                                                 a grenade's throw)
;   17     bullet::process_collision (0x32EBD0)                  a bullet's hits (a pierce, a reflection, a stop)
;   18     bullet_manager::add_decal (0x321FA0)                  bullet decals asked for
;   19     bullet_manager::play_particle (0x3223E0)              bullet impact effects asked for
;   20     sound_instance_proxy_internal::play (0x5ABA0)         sounds started
;   21     (freeze_tick)                                         ticks simulated again: a tick whose time (edi)
;                                                                 is not past the latest simulated
;   22     (freeze_shot)                                         bullets fired inside such a tick
;   23     base_player::generate_killed_event (0x2DA170)         kill events (patch 67's kill_feed_event, each)
;   24     game_world_ui::on_player_killed (0x237C70)            kill feed rows (patch 67's kill_feed_row, each)
;
; The console variable: game::game's call of the game world's constructor
; (0x1E90A3, before the configs run) makes it, then registers srp_freeze_ms, a
; cc_u32 (cc_value<u32> 0x1DCD0, as game_world_ui's minimap_position 0x2338F5)
; on frz_limit_ms (40; 0 to 100000, 0 none), kept in user.cfg.
BITS 32
%include "engine_026g.inc"
global freeze_present, freeze_logic, freeze_sim, freeze_tick, freeze_rollback, freeze_message, freeze_datagram
global freeze_bake, freeze_baked_load, freeze_sync_load, freeze_cook, freeze_callbacks, freeze_particle
global freeze_invoke, freeze_advance, freeze_flash_render, freeze_game_world
global freeze_shot, freeze_hit, freeze_decal, freeze_impact, freeze_sound, freeze_kill, freeze_kill_row
global frz_log
global frz_totals, frz_state, frz_world, frz_tsc_mult, frz_limit_ms, frz_variable, frz_sim_time, frz_resim
extern frz_frame, frz_logic_done
section .text

%define LOG             0x111ED0    ; cdecl (&function, [LOG_A], LOG_B, file, line, function, channel, level, format, ...)
%define LOG_FILTER      0x111860    ; cdecl (channel, level): al = it is written
%define LOG_FILTERED    0x1F1D60C
%define LOG_A           0x1F1D610
%define LOG_B           0x21E73B8
%define LOG_SINK        0x1F1D614   ; the function the game's lines go through (0, none)
%define LOG_MANAGER     0x9DAF4C    ; its boost::function manager
%define CHANNEL_GAME    0x9FA4E4    ; "game"
%define LEVEL_INFO      4
%define GAME_WORLD      0x1F67B0    ; game_world::game_world: the world, the game; `ret 8`
%define CC_U32_VTABLE   0x9E7CCC    ; cc_u32, as minimap_position's (0x233911)
%define LIMIT_MOST      100000

%define SLOT  16                    ; a timed function's slot: {busy, the caller's return, the start (8)}
%define BUSY  0
%define RET   4
%define START 8

; TIMED name, index, site, length, {bytes}: a timed entry at `name`, its two
; slots name_slots in .bss; `bytes` are the site's own `length` bytes. With a
; sixth argument, a C function the leave calls after the time is added.
%macro TIMED 5-6
%1:
    pushfd
    push eax
    push ecx
    push edx
    lock inc dword [frz_totals + 8*%2]
    mov ecx, 1
    xor eax, eax
    lock cmpxchg [%1_slots + BUSY], ecx
    jz %%first
    xor eax, eax
    lock cmpxchg [%1_slots + SLOT + BUSY], ecx
    jnz %%go                        ; both slots taken: counted, not timed
    mov ecx, %1_slots + SLOT
    mov edx, %%leave1
    jmp %%take
%%first:
    mov ecx, %1_slots
    mov edx, %%leave0
%%take:
    mov eax, [esp+16]               ; the caller's return address
    mov [ecx+RET], eax
    mov [esp+16], edx               ; the leave's in its place
    rdtsc
    mov [ecx+START], eax
    mov [ecx+START+4], edx
%%go:
    pop edx
    pop ecx
    pop eax
    popfd
    db %5
    jmp %3 + %4
%%leave0:
    push dword [%1_slots + RET]
    pushfd
    push eax
    push ecx
    push edx
    mov ecx, %1_slots
    jmp %%stop
%%leave1:
    push dword [%1_slots + SLOT + RET]
    pushfd
    push eax
    push ecx
    push edx
    mov ecx, %1_slots + SLOT
%%stop:
    mov edx, frz_totals + 8*%2 + 4
    call frz_stop
%if %0 == 6
    call %6
%endif
    pop edx
    pop ecx
    pop eax
    popfd
    ret
section .bss
alignb 16
global %1_slots                     ; check_freeze_026g.py reads them
%1_slots:
    resb 2*SLOT
section .text
%endmacro

; COUNTED name, index, site, length, {bytes}: a counted entry.
%macro COUNTED 5
%1:
    pushfd
    lock inc dword [frz_totals + 8*%2]
    popfd
    db %5
    jmp %3 + %4
%endmacro

; A timed call's end: ecx = its slot, edx = &its microseconds' total. The
; time since the slot's start, in microseconds ((ticks x frz_tsc_mult) >> 32),
; added to the total; then the slot freed. eax, ecx, edx and the flags
; changed.
frz_stop:
    push ebx
    push esi
    mov ebx, ecx
    mov esi, edx
    rdtsc
    sub eax, [ebx+START]
    sbb edx, [ebx+START+4]
    jns .positive
    xor eax, eax                    ; another core's counter behind: nothing
    xor edx, edx
.positive:
    mov ecx, eax
    mov eax, edx
    mul dword [frz_tsc_mult]        ; the high dword's part: its low dword
    xchg eax, ecx
    mul dword [frz_tsc_mult]        ; the low dword's: its high dword
    add edx, ecx
    lock add [esi], edx
    mov dword [ebx+BUSY], 0
    pop esi
    pop ebx
    ret

; engine_world::logic_tick (0x1A030, stdcall: the engine world; `ret 4`): the
; world kept for frz_logic_done, then timed, the state named after it.
freeze_logic:
    push eax
    mov eax, [esp+8]
    mov [frz_world], eax
    pop eax
    TIMED freeze_logic_timed, 0, 0x1A030, 6, {0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8}, frz_logic_done

; res_render_output::present (0x3CF7F0, eax the output; [esp+4] the interval
; flag; `ret 4`, eax its HRESULT): timed, then the frame's end.
    TIMED freeze_present, 1, 0x3CF7F0, 6, {0x83, 0xEC, 0x0C, 0x56, 0x8B, 0xF0}, frz_frame

    TIMED freeze_sim, 2, 0x2D2E40, 6, {0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x40}
; game_world_core::atomic_tick (0x2D1E20; edi the time the world ticks to,
; tick_events 0x2D22B7): counted; a time not past the latest ticked to is a
; tick played again (after a rollback), counted again and marked for the
; bullets fired in it. Only the logic thread ticks a world; frz_logic_done
; starts the latest again out of a match.
freeze_tick:
    pushfd
    lock inc dword [frz_totals + 8*3]
    cmp edi, [frz_sim_time]
    ja .ahead
    lock inc dword [frz_totals + 8*21]
    mov byte [frz_resim], 1
    jmp .go
.ahead:
    mov [frz_sim_time], edi
    mov byte [frz_resim], 0
.go:
    popfd
    db 0x51, 0xB8, 0x1F, 0x85, 0xEB, 0x51
    jmp 0x2D1E26

; bullet_manager::emit_bullet (0x322830): a bullet fired, and one fired again
; when inside a tick played again.
freeze_shot:
    pushfd
    lock inc dword [frz_totals + 8*16]
    cmp byte [frz_resim], 0
    je .go
    lock inc dword [frz_totals + 8*22]
.go:
    popfd
    db 0x51, 0x53, 0x8B, 0x5C, 0x24, 0x0C
    jmp 0x322836

    COUNTED freeze_hit, 17, 0x32EBD0, 8, {0x83, 0xEC, 0x24, 0x53, 0x8B, 0x5C, 0x24, 0x30}
    COUNTED freeze_decal, 18, 0x321FA0, 8, {0x83, 0xEC, 0x14, 0x53, 0x8B, 0x5C, 0x24, 0x30}
    COUNTED freeze_impact, 19, 0x3223E0, 5, {0x83, 0xEC, 0x20, 0x53, 0x55}
    COUNTED freeze_sound, 20, 0x5ABA0, 8, {0x8B, 0x44, 0x24, 0x04, 0x8B, 0x54, 0x24, 0x0C}
    COUNTED freeze_kill, 23, 0x2DA170, 8, {0x83, 0xEC, 0x34, 0x53, 0x8A, 0x5C, 0x24, 0x40}
    COUNTED freeze_kill_row, 24, 0x237C70, 6, {0x81, 0xEC, 0xE0, 0x00, 0x00, 0x00}
    COUNTED freeze_rollback, 4, 0x2D2770, 8, {0x83, 0xEC, 0x68, 0x53, 0x8B, 0x5C, 0x24, 0x74}
    COUNTED freeze_message, 5, 0x27A300, 6, {0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x64}
    COUNTED freeze_datagram, 6, 0x3F3BB0, 6, {0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8}
    TIMED freeze_bake, 7, 0x12E510, 6, {0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8}
    TIMED freeze_baked_load, 8, 0x12DB40, 6, {0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8}
    TIMED freeze_sync_load, 9, 0xE5C20, 6, {0x81, 0xEC, 0xC0, 0x00, 0x00, 0x00}
    COUNTED freeze_cook, 10, 0x1096C0, 6, {0x51, 0x53, 0x8B, 0x5C, 0x24, 0x0C}
    TIMED freeze_callbacks, 11, 0xE5AB0, 6, {0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8}
    COUNTED freeze_particle, 12, 0x22660, 6, {0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8}
    COUNTED freeze_invoke, 13, 0x511600, 5, {0x83, 0xEC, 0x10, 0x53, 0x56}
    TIMED freeze_advance, 14, 0x51F1C0, 6, {0x81, 0xEC, 0x90, 0x00, 0x00, 0x00}
    TIMED freeze_flash_render, 15, 0x13CED0, 6, {0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x0C}

; game::game's call of the game world's constructor (0x1E90A3; [esp+4] the
; world, [esp+8] the game, eax the world, ebx the game; `ret 8`): the
; constructor, then srp_freeze_ms registered once; back with the
; constructor's registers.
freeze_game_world:
    push dword [esp+8]
    push dword [esp+8]
    call GAME_WORLD
    pushad
    pushfd
    cmp byte [registered], 0
    jne .done
    mov byte [registered], 1
    push 2                          ; the kind minimap_position has
    push 1                          ; saved to user.cfg
    push LIMIT_MOST
    push 0
    push frz_limit_ms
    push variable_name
    push frz_variable
    call engine_cc_u32
    add esp, 28
    mov eax, CC_U32_VTABLE
    mov [frz_variable], eax         ; a cc_u32
.done:
    popfd
    popad
    ret 8

; cdecl (text): one line of the game's log, channel "game", info, "%s" of
; the text, as upgrade_preview_026g.asm's `log` (the game's own
; relocate_upgrade_item 0x29A8E9..0x29A958): written through LOG_SINK's
; function, {LOG_MANAGER | 1, 0, LOG_SINK}, or none when LOG_SINK is 0.
; Every register but eax, ecx, edx kept.
frz_log:
    cmp dword [LOG_FILTERED], 0
    je .write
    push LEVEL_INFO
    push CHANNEL_GAME
    call LOG_FILTER
    add esp, 8
    test al, al
    jz .ret
.write:
    push ebp
    mov ebp, esp
    xor eax, eax
    push eax                        ; an empty function (its manager 0), 20h bytes
    push eax
    push eax
    push eax
    push eax
    push eax
    push eax
    push eax
    mov eax, [LOG_SINK]
    test eax, eax
    jz .function
    mov dword [esp], LOG_MANAGER | 1
    mov [esp+8], eax
.function:
    mov eax, esp
    push dword [ebp+8]
    push format
    push LEVEL_INFO
    push CHANNEL_GAME
    push log_where
    push 0
    push log_where
    push LOG_B
    push dword [LOG_A]
    push eax
    call LOG
    mov esp, ebp
    pop ebp
.ret:
    ret

section .rodata
variable_name:  db "srp_freeze_ms", 0
log_where:      db "srp", 0
format:         db "%s", 0

section .data
alignb 4
frz_limit_ms:   dd 40               ; srp_freeze_ms

section .bss
alignb 16
frz_totals:     resd 2*32           ; {calls, microseconds} per counter (25 used)
frz_state:      resd 1              ; freeze_026g.c's S_*
frz_sim_time:   resd 1              ; the latest time a world ticked to
frz_resim:      resb 4              ; 1 inside a tick played again
frz_world:      resd 1              ; the engine world
frz_tsc_mult:   resd 1              ; 2^32 / TSC ticks per microsecond (0: not known yet)
frz_variable:   resb 0x4C           ; the cc_u32 (cc_value<u32>, 0x1DCD0)
registered:     resb 1
