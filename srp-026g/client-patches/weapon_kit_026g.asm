; Patch 68 (build 2777): the weapon kit's room on the item config cook's
; stack (rust/patches/src/weapon_kit.toml's one `code` hook; the logic is
; the Rust patch weapon_kit, docs/notes/weapon-kit.md).
;
; item_config_cook::translate_query (0x2F1750) reads an item's upgrade
; records through [ebp+40h] alone (0x2F1799, 0x2F18D0). Its `call 0x2F2060`
; at 0x2F1771 (which copies the query to [ebp+38h..44h]) jumps here: the
; call, then room for six records and their buffer_vector (the kind
; survarium::weapon_kit_room, fields_026g.tsv) below translate_query's
; frame, as its own allocas are (its epilogue takes esp from ebp,
; 0x2F2041), and on at 0x2F1776, where the lohook weapon_kit.query_records
; fills it for an item whose records hold a kit. Per call, on the cook's own
; stack: nothing shared between two cooks.
bits 32
section .text

%define QUERY_COPY 0x2F2060     ; item_config_query::get_item: esi the query's holder, edi ebp+38h
%define QUERY_BACK 0x2F1776     ; translate_query after that call
%define ROOM 0xA0               ; six records of 18h and a buffer_vector of 0Ch, rounded to 16

global weapon_kit_query_room
weapon_kit_query_room:
    call QUERY_COPY
    sub esp, ROOM
    jmp QUERY_BACK
