BITS 32
section .text
global match_stat_map

; lobby_menu::fill_match_statistic (0x29AB20) at 0x29ABE1, `mov ecx,[esi+313Ch]`:
; the map name the results screen shows, translate_text's key (0x29AC04).
; Its one writer is the match loading screen (0x22052A), so it is NULL in a
; game that has loaded no match (a game started anew and reseated into one
; that then ended), and translate_text's value_exists reads it (0xF6C47).
; A NULL name reads as "": the screen shows no map name. Only ECX and the
; flags change (the next flag reader is after the `add ecx` at 0x29ABF6).
match_stat_map:
    mov ecx, [esi+0x313c]
    test ecx, ecx
    jnz .back
    mov ecx, no_map
.back:
    jmp 0x29abe7

section .rodata
no_map: db 0
