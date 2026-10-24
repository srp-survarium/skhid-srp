; Patch 64 (build 2777): the interface settings, saved choices of the
; gameplay options that reach the match's movies; the first is the
; interface's level (Full, Minimal, Disabled). The arena's blocks
; interface.code, interface.data and interface.state (link_026g.py links
; them; patch_026g.py's INTERFACE_HOOKS call and jump to the four entries by
; their names). client_patch.md section 3.57; the rules:
; ../docs/notes/interface-level.md. The logic is interface_026g.c's.
;
; The variables. The hud's constructor (0x233810, run by the game world's
; constructor before the configs) registers its saved variables once (a
; static guard, 0xA2AE1F8): show_quests_progress at 0x2339DE is a
; cc_value<bool> (0x1DDB0, eax the object, the name, the value's address,
; saved 1, kind 2 pushed; `ret 10h`), then minimap_position, a cc_u32
; (0x1DCD0, then the vtable 0x9E7CCC). There each setting's variable is made
; the same way, a cc_u32 from 0 to its count - 1, saved in user.cfg.
;
; The options' row. options_tab::options_tab (0x262300) makes the gameplay
; tab's (tab 0) 16 rows (`mov edi,10h` / `mov byte [ebx+4],10h` and the
; array 0x2632E0); its last, option 15, is stored at 0x262731 (`mov
; ecx,[ebx] / mov [ecx+3Ch],edi`, then 0x262736 returns the tab). There
; each setting gets an options_item_int (0x261100, as minimap_position's,
; option 9) at 16 + i; patch_026g.py makes the tab's count 16 + the
; settings'. fill_settings_data (0x2005E0) makes the gameplay labels with
; 0x202150 at 0x2005F2 (the count pushed at 0x2005EF, made 16 + the
; settings' too) and its loop takes the count from `mov bl,10h` at 0x200B08
; (the same): each setting's label key goes at 16 + i.
;
; The match. The game world calls the hud's update_ui (0x23B200, eax the
; hud, three arguments, `ret 0Ch`) at 0x1F8023 every frame of a match; after
; it each movie gets a setting's value when it changed (interface_sync).
BITS 32
%include "engine_026g.inc"
global interface_register, interface_options, interface_labels, interface_ui
extern interface_registered, interface_made, interface_labelled, interface_sync
section .text

%define CC_BOOL 0x1DDB0            ; cc_value<bool>::cc_value, `ret 10h`
%define OPTIONS_BACK 0x262736      ; options_tab::options_tab after option 15 is stored
%define NEW_LABELS 0x202150        ; fill_settings_data's labels (cdecl: the allocator, the count)
%define UPDATE_UI 0x23B200         ; game_world_ui::update_ui, `ret 0Ch`

; The hud's constructor's registration of show_quests_progress (0x2339DE):
; the variable made as the site asked, then the settings'.
interface_register:
    push dword [esp+16]            ; kind
    push dword [esp+16]            ; saved
    push dword [esp+16]            ; the value's address
    push dword [esp+16]            ; the name
    call CC_BOOL                   ; eax the object, as the site gave it
    pushad
    call interface_registered
    popad
    ret 16

; options_tab::options_tab, the gameplay tab's option 15 stored (0x262731;
; ebx the tab, edi option 15): the settings' rows after it.
interface_options:
    mov ecx, [ebx]
    mov [ecx+0x3C], edi
    pushad
    push ebx
    call interface_made
    add esp, 4
    popad
    jmp OPTIONS_BACK

; fill_settings_data's call making the gameplay labels (0x2005F2): the
; labels, then the settings' label keys after the game's 16.
interface_labels:
    push dword [esp+8]             ; the count
    push dword [esp+8]             ; the allocator
    call NEW_LABELS
    add esp, 8
    pushad
    push eax                       ; the labels
    call interface_labelled
    add esp, 4
    popad
    ret

; The game world's call of the hud's update_ui (0x1F8023, eax the hud):
; update_ui, then the settings to the movies.
interface_ui:
    push eax                       ; the hud
    push dword [esp+16]
    push dword [esp+16]
    push dword [esp+16]
    call UPDATE_UI                 ; eax the hud; pops the three
    pushad
    push dword [esp+32]            ; the hud
    call interface_sync
    add esp, 4
    popad
    add esp, 4
    ret 12
