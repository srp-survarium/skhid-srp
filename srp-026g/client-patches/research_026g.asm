; Research (exe patch 53): the four hooks of the mode, in front of the C code
; of research_rule_026g.c and research_native_026g.c in the arena's block
; research.code (link_026g.py links it; patch_026g.py's RESEARCH_HOOKS call or
; jump to the four entries by their names).
;
;   +0  register  game's constructor 0x1E9B4C (`mov eax,ebx / call 0x1EC170`,
;                 after its own client cooks, ebx = the game): `call` here;
;                 rn_register(game) makes and registers the rule's cook, then
;                 the two instructions are done here
;   +5  hud       game_world_ui::update_ui 0x23B3DE (`cmp dword [edi+20h],4 /
;                 jne 0x23B3FC`, after the teams' points went to the movie,
;                 edi = the ui): `call` here; rn_hud(ui) every frame, then
;                 the compare and its branch
;   +A  welcome   update_ui 0x23B351 (`cmp eax,4 / jne 0x23B36A`, the match's
;                 first message by mode): `jmp` here; mode 5's is
;                 st_start_match_welcome_message_5 (0.27d's key)
;   +F  icons     update_ui 0x23B207 (`call 0x23AA10`, update_player_icons(ui),
;                 `ret 4`, its argument pushed: every frame, before update_ui's
;                 40 ms and hud-state gates): `call` here; the players' icons,
;                 then rn_markers(ui), the units' markers, and back as the
;                 call's `ret 4`. First patch 62's hide_ui_icons(ui): the
;                 icons' movie told the interface's forced state (Blackout,
;                 patch 65: the players' markers stay, the objects' go)
;
bits 32
global research_register, research_hud, research_welcome, research_icons
extern rn_register, rn_hud, rn_markers ; the C functions
extern hide_ui_icons                   ; patch 62 (hide_ui_026g.asm): the icons told the forced state
section .text

%define GAME_COOKS_DONE 0x1EC170   ; what the hooked call called
%define HUD_NOT_TDM     0x23B3FC   ; update_ui: past set_last_players
%define WELCOME_SHOW    0x23B35B   ; update_ui: push 1770h ... call 0x2375C0
%define WELCOME_NONE    0x23B36A   ; update_ui: no message for this mode
%define WELCOME_TDM     0xA03AAC   ; st_start_match_welcome_message_3
%define PLAYER_ICONS    0x23AA10   ; game_world_ui::update_player_icons(ui), ret 4

entries:
research_register:
    jmp near register
research_hud:
    jmp near hud
research_welcome:
    jmp near welcome
research_icons:
    jmp near icons

register:
    pushad
    push ebx
    call rn_register
    add esp, 4
    popad
    mov eax, ebx
    add dword [esp], 2              ; return past the replaced call
    jmp GAME_COOKS_DONE

hud:
    pushad
    push edi
    call rn_hud
    add esp, 4
    popad
    cmp dword [edi + 0x20], 4
    je .tdm
    mov dword [esp], HUD_NOT_TDM
.tdm:
    ret

welcome:
    cmp eax, 4
    jne .research
    mov ecx, WELCOME_TDM
    jmp WELCOME_SHOW
.research:
    cmp eax, 5
    jne WELCOME_NONE
    mov ecx, welcome_key
    jmp WELCOME_SHOW

icons:
    pushad
    push dword [esp + 0x24]         ; the ui update_ui pushed, over pushad's eight
    call hide_ui_icons
    add esp, 4
    popad
    push dword [esp + 4]            ; the ui update_ui pushed
    call PLAYER_ICONS
    pushad
    push dword [esp + 0x24]         ; the ui again, over pushad's eight
    call rn_markers
    add esp, 4
    popad
    ret 4

welcome_key:
    db "st_start_match_welcome_message_5", 0

    times 0x100 - ($ - $$) db 0xCC
