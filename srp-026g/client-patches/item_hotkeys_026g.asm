; Patch 149: a key per item type uses it (the rules: ../docs/notes/item-hotkeys.md;
; client_patch.md section 3.82). The arena's block item_hotkeys.data: the game's
; action table again, its 60 rows and fourteen more, a row per item type a key
; can use.
;
; The game's action table is 60 rows of 20 bytes at 0xA74AF0 (static data:
; {name, id, kinds, toggle, the default key's name}), and the key names'
; table begins right after it (0xA74FA0), so no row can be added in place. Four
; functions of the key binder read it, each by its address and its 4B0h bytes:
; the binder's construction (0x20755B: every row's description in the binding
; of its id, to the end 0xA74FA0), set_default_controls (0x2078A0: each row's
; default key bound), id_to_action_name (0x2079D0) and action_name_to_ptr
; (0x207AD0: `bind <action> <key>`, user.cfg's lines and the options' accept).
; Everything else reads a row through the binder's description pointer
; (get_binded_action 0x207D50: +4 the id, +8 the kinds, +0Ch the toggle).
; `actions` below is that table: the game's 60 rows in their order (one
; changed: `artefact`'s default key is H, patch 78's carry key; the game's row
; has none), then the fourteen. patch_026g.py's ITEM_HOTKEYS_TABLE_REFS,
; ITEM_HOTKEYS_TABLE_END and ITEM_HOTKEYS_TABLE_COUNTS point the four
; functions at it.
;
; Their ids are 4Ah..57h: the key binder holds a binding per id (0Ch bytes,
; indexed by it), 48h of them in the game; patch_026g.py's
; ITEM_HOTKEYS_BINDER makes them 60h (the binder's allocation, its
; construction, get_binded_action, bind and the save). 48h and 49h stay
; without a row: 48h is the options' "no key being set" (game_options +3Ch),
; 49h get_binded_action's "no action" (and the binding an unbind of an unknown
; name clears, 0x20801D). Kinds 5 as the quick-use keys' (the game world's 1,
; the player's input handler's 4), no toggle, no default key: a player binds
; them in Options, Controls (patch 62's `controls`, hide_ui_026g.asm).
BITS 32
global actions                     ; the action table, 74 rows (patch_026g.py's ITEM_HOTKEYS_TABLE_REFS)

; A row: its name, id, kinds, toggle and default key's name (0: none).
%macro ACTION 5
    dd %1, %2, %3, %4, %5
%endmacro

section .data
align 4
actions:                           ; the game's 60 rows (0xA74AF0), in their order
    ACTION 0x9FE074, 0x00, 5, 0, 0x9FE06C    ; left, kLEFT
    ACTION 0x9FE064, 0x01, 5, 0, 0x9FE05C    ; right, kRIGHT
    ACTION 0x9FE058, 0x02, 5, 0, 0x9FE054    ; up, kUP
    ACTION 0x9FE04C, 0x03, 5, 0, 0x9FE044    ; down, kDOWN
    ACTION 0x9FE03C, 0x04, 5, 0, 0x9FE034    ; jump, kSPACE
    ACTION 0x9FE07C, 0x05, 5, 0, 0x9FE028    ; crouch, kLCONTROL
    ACTION 0x9FE018, 0x06, 5, 1, 0x9FE014    ; crouch_toggle, kC
    ACTION 0x9FE00C, 0x07, 1, 0, 0x9FE004    ; accel, kLSHIFT
    ACTION 0x9FDFF4, 0x08, 5, 1, 0x9FDFF0    ; sprint_toggle, kX
    ACTION 0x9FDFD4, 0x09, 5, 0, 0x9FDFD0    ; set_next_fire_queue_type, kN
    ACTION 0x9FDFBC, 0x0A, 5, 0, 0x9FDFB8    ; set_next_ammo_type, kM
    ACTION 0x9EEA88, 0x0B, 5, 0, 0x9FDFB4    ; forward, kW
    ACTION 0x9FCC9C, 0x0C, 5, 0, 0x9FDFB0    ; back, kS
    ACTION 0x9FDFA8, 0x0D, 5, 0, 0x9FDFA4    ; lstrafe, kA
    ACTION 0x9FDF9C, 0x0E, 5, 0, 0x9FDF98    ; rstrafe, kD
    ACTION 0x9FDF90, 0x11, 5, 0, 0x9FDF8C    ; cam_1, kF1
    ACTION 0x9FDF84, 0x12, 5, 0, 0x9FDF80    ; cam_2, kF2
    ACTION 0x9FDF78, 0x13, 5, 0, 0x9FDF74    ; cam_3, kF3
    ACTION 0x9FDF68, 0x15, 5, 0, 0x000000    ; cam_zoom_in, none
    ACTION 0x9FDF58, 0x16, 5, 0, 0x000000    ; cam_zoom_out, none
    ACTION 0x9FDF4C, 0x17, 5, 0, 0x9FDF48    ; next_player, kF6
    ACTION 0x9FDF3C, 0x18, 5, 0, 0x9FDF38    ; prev_player, kF5
    ACTION 0x9FDF24, 0x19, 5, 0, 0x9FDF1C    ; toggle_replay_ui, kF11
    ACTION 0x9FDF14, 0x1A, 5, 0, 0x000000    ; torch, none
    ACTION 0x9FDF04, 0x1B, 5, 0, 0x000000    ; night_vision, none
    ACTION 0x9FDEF4, 0x1C, 5, 0, 0x000000    ; show_detector, none
    ACTION 0x9FDEEC, 0x1D, 5, 0, 0x9FDEE8    ; wpn_1, k1
    ACTION 0x9FDEE0, 0x1E, 5, 0, 0x9FDEDC    ; wpn_2, k2
    ACTION 0x9FDECC, 0x1F, 5, 0, 0x9FDEC8    ; commit_suicide, kK
    ACTION 0x9FDEC0, 0x21, 5, 0, 0x9FDEBC    ; melee, kF
    ACTION 0x9FDEB4, 0x22, 5, 0, 0x9FDEB0    ; spot, kQ
    ACTION 0x9FE084, 0x23, 5, 0, 0x9FDCA8    ; artefact, kH (patch 78: the game's row has none)
    ACTION 0x9FDEA4, 0x24, 5, 0, 0x9FDE94    ; wpn_next, mouse_wheel_up
    ACTION 0x9FDE88, 0x25, 5, 0, 0x9FDE74    ; wpn_prev, mouse_wheel_down
    ACTION 0x9FDE68, 0x26, 5, 0, 0x9FDE60    ; wpn_fire, mouse1
    ACTION 0x9FDE54, 0x27, 5, 0, 0x9FDE50    ; wpn_reload, kR
    ACTION 0x9FDE48, 0x28, 5, 0, 0x9FDE40    ; wpn_aim, mouse2
    ACTION 0x9FDE30, 0x29, 5, 1, 0x9FDE28    ; wpn_aim_toggle, mouse3
    ACTION 0x9FDE18, 0x2A, 4, 0, 0x9FE004    ; delay_breath, kLSHIFT
    ACTION 0x9FA79C, 0x34, 5, 0, 0x9FDE10    ; pause, kPAUSE
    ACTION 0x9FDDF8, 0x37, 5, 0, 0x9FDDF4    ; serialize_player_state, kO
    ACTION 0x9FDDD8, 0x38, 5, 0, 0x9FDDD4    ; deserialize_player_state, kP
    ACTION 0x9FDDC8, 0x39, 5, 0, 0x9FDDC4    ; quick_use_1, k3
    ACTION 0x9FDDB8, 0x3A, 5, 0, 0x9FDDB4    ; quick_use_2, k4
    ACTION 0x9FDDA8, 0x3B, 5, 0, 0x9FDDA4    ; quick_use_3, k5
    ACTION 0x9FDD98, 0x3C, 5, 0, 0x9FDD94    ; quick_use_4, k6
    ACTION 0x9FDD88, 0x3D, 5, 0, 0x9FDD84    ; quick_use_5, k7
    ACTION 0x9FDD78, 0x3E, 5, 0, 0x9FDD74    ; quick_use_6, k8
    ACTION 0x9FDD64, 0x20, 5, 0, 0x9FDD60    ; throw_grenade, kG
    ACTION 0x9FDD58, 0x3F, 5, 0, 0x9FDD54    ; chat, kT
    ACTION 0x9FDD50, 0x35, 5, 0, 0x9FDD4C    ; use, kE
    ACTION 0x9FDD44, 0x36, 5, 0, 0x9FDD40    ; drop, kV
    ACTION 0x9FDD34, 0x40, 2, 0, 0x9FE014    ; character, kC
    ACTION 0x9FD55C, 0x41, 2, 0, 0x9FDD30    ; inventory, kI
    ACTION 0x9FDD28, 0x42, 2, 0, 0x9FDFB0    ; shop, kS
    ACTION 0x9E1AA4, 0x43, 2, 0, 0x9FDDF4    ; options, kO
    ACTION 0x9FDD20, 0x44, 2, 0, 0x9FDEBC    ; friends, kF
    ACTION 0x9FDD10, 0x45, 8, 0, 0x9FDD08    ; send_message, kRETURN
    ACTION 0x9FDD00, 0x46, 8, 0, 0x9FDCF8    ; send_to, kTAB
    ACTION 0x9FDCF4, 0x47, 8, 0, 0x9FDDD4    ; ptt, kP
                                   ; patch 149's fourteen, one per item type, ids 4Ah..57h
    ACTION use_onyx, 0x4A, 5, 0, 0
    ACTION use_spring, 0x4B, 5, 0, 0
    ACTION use_lifebone, 0x4C, 5, 0, 0
    ACTION use_bandages, 0x4D, 5, 0, 0
    ACTION use_medkit, 0x4E, 5, 0, 0
    ACTION use_military_medkit, 0x4F, 5, 0, 0
    ACTION use_blackmarket_medkit, 0x50, 5, 0, 0
    ACTION use_science_medkit, 0x51, 5, 0, 0
    ACTION use_antitoxin, 0x52, 5, 0, 0
    ACTION use_painkiller, 0x53, 5, 0, 0
    ACTION use_rgd5, 0x54, 5, 0, 0
    ACTION use_rdgm, 0x55, 5, 0, 0
    ACTION use_trap, 0x56, 5, 0, 0
    ACTION use_oxygen_tank, 0x57, 5, 0, 0

; Their names: what user.cfg's `bind <action> <key>` names.
use_onyx:
    db "use_onyx", 0
use_spring:
    db "use_spring", 0
use_lifebone:
    db "use_lifebone", 0
use_bandages:
    db "use_bandages", 0
use_medkit:
    db "use_medkit", 0
use_military_medkit:
    db "use_military_medkit", 0
use_blackmarket_medkit:
    db "use_blackmarket_medkit", 0
use_science_medkit:
    db "use_science_medkit", 0
use_antitoxin:
    db "use_antitoxin", 0
use_painkiller:
    db "use_painkiller", 0
use_rgd5:
    db "use_rgd5", 0
use_rdgm:
    db "use_rdgm", 0
use_trap:
    db "use_trap", 0
use_oxygen_tank:
    db "use_oxygen_tank", 0
