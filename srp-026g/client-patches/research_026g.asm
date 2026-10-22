; Research (exe patch 52): the three hooks of the mode, in front of the C code
; of research_rule_026g.c and research_native_026g.c in the section `.srpr`
; (research_026g.py builds it; patch_026g.py puts it after `.srpw`).
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
;
; -D: CAVE (this code's VA), RN_REGISTER and RN_HUD (the C functions).
bits 32
org CAVE

%define GAME_COOKS_DONE 0x1EC170   ; what the hooked call called
%define HUD_NOT_TDM     0x23B3FC   ; update_ui: past set_last_players
%define WELCOME_SHOW    0x23B35B   ; update_ui: push 1770h ... call 0x2375C0
%define WELCOME_NONE    0x23B36A   ; update_ui: no message for this mode
%define WELCOME_TDM     0xA03AAC   ; st_start_match_welcome_message_3

entries:
    jmp near register
    jmp near hud
    jmp near welcome

register:
    pushad
    push ebx
    call RN_REGISTER
    add esp, 4
    popad
    mov eax, ebx
    add dword [esp], 2              ; return past the replaced call
    jmp GAME_COOKS_DONE

hud:
    pushad
    push edi
    call RN_HUD
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

welcome_key:
    db "st_start_match_welcome_message_5", 0

    times 0x100 - ($ - $$) db 0xCC
