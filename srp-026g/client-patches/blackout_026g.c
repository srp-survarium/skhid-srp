/* Blackout, exe patch 65 (prototype; the rules: ../docs/notes/blackout.md;
 * client_patch.md section 3.58): a battery match with no interface but the
 * players' markers over their heads.
 *
 * - bo_options: srp's block after the static info's squads (the entries
 *   patch 63's td_options reads; match_options::deserialize's end, 0x1F0F30)
 *   may hold tag 2, one byte 1: this match is a Blackout match. The options
 *   keep it in the last 16 bytes of their unused "Mode" string (+57870h, the
 *   magic BO_MAGIC or zeros) and bo_blackout says it for the match: one
 *   flag, set when a match's options are read (0xE4 at 0x27B928, a replay's
 *   chunk 1 at 0x1F096C), before the match's interface is reset.
 * - bo_match: that flag: patch 62's reset of a match's interface
 *   (initialize_match, 0x234AE8) then forces it hidden for the whole match
 *   and F11 cannot show it (hide_ui_026g.asm, hide_ui_force: the one place
 *   that forces the state; each movie keeps what a forced interface keeps:
 *   the hud the pre-match countdown, the player icons the players' markers).
 * - bo_tab, bo_input: Tab held shows the ammo and the quick-use slots (the
 *   hud movie) and stops the player (the input's moves and acts).
 *
 * Nothing a match computes: the interface and the local input only (no
 * simulation_changes.tsv row). Every address is a VA of the original exe
 * (image base 0x10000; never rebased). */
typedef unsigned char u8;
typedef unsigned int u32;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))

enum {
    /* match_options (0x578A0 bytes; match_options::deserialize 0x1F0E70) */
    OPTIONS_BLACKOUT = 0x57870,    /* the "Mode" string's (+57840h, 40h bytes) last 16: patch 65's */
    BO_MAGIC = 0x4F425253,         /* "SRBO" */
    SRPX_BLACKOUT = 2,             /* srp's entry: one byte, 1 */
};

/* Whether the options last read (a match's or a replay's) are a Blackout
 * match's: the one place the flag lives, set before that match's interface
 * is reset (the options name the map and the players its world is made of; a
 * late joiner's place comes into a world made from them). */
static u32 bo_blackout;

/* Tab held (the Tab list the game asks for), and the blocked actions held
 * through it (a key held through Tab acts again only after it is let go). */
static u32 bo_tab_down, bo_held;

u32 bo_match(void);

/* The input's actions mask (process_first_person_mode, handler+33Ch; the
 * bits: srp's drive.rs::key). While Tab is held in a Blackout match the
 * player neither moves nor acts: these are taken out as if not pressed. */
enum {
    MOVES = 0x1 | 0x2 | 0x4 | 0x8 | 0x200,              /* forward, back, left, right, sprint */
    ACTS = 0x10 | 0x20 | 0x40 | 0x80 | 0xFC000 |        /* jump, fire, reload, aim, quick use 1..6 */
           0x100000 | 0x200000 | 0x400000 | 0x800000 |  /* hold breath, use, drop, grenade */
           0x8000000,                                   /* melee */
    BLOCKED = MOVES | ACTS,
};

/* match_options::deserialize's end (ebx the options, esi the reader {begin,
 * the cursor, the size}): the Blackout entry of srp's block, as td_options
 * reads the block (an entry of another tag or length is stepped over). */
void bo_options(u8 *options, u8 **reader) {
    u8 *at = reader[1], *end = reader[0] + (u32)reader[2];
    u32 on = 0;
    if (at && end >= at && end - at >= 4 && at[0] == 'S' && at[1] == 'R' && at[2] == 'P' && at[3] == 'X') {
        for (at += 4; end - at >= 2;) {
            u32 tag = at[0], len = at[1];
            at += 2;
            if ((u32)(end - at) < len)
                break;
            if (tag == SRPX_BLACKOUT && len == 1 && at[0] == 1)
                on = 1;
            at += len;
        }
    }
    AT(options, OPTIONS_BLACKOUT, u32) = on ? BO_MAGIC : 0;
    bo_blackout = on;
    bo_tab_down = bo_held = 0;
}

/* game_world::on_keyboard_action's show_player_list: Tab down or up. */
void bo_tab(u32 down) {
    bo_tab_down = (down & 0xFF) != 0;
}

/* The input's actions mask as built: in a Blackout match, while Tab is held,
 * no move and no act (the look stays); after it, an act whose key was held
 * through Tab stays out until the key is let go (nothing fires on Tab's
 * release), a move comes back at once. */
void bo_input(u32 *mask) {
    if (!bo_match()) {
        bo_held = 0;
        return;
    }
    if (bo_tab_down) {
        bo_held = *mask & ACTS;
        *mask &= ~(u32)BLOCKED;
        return;
    }
    bo_held &= *mask;
    *mask &= ~bo_held;
}

/* 1 when the options last read are a Blackout match's. */
u32 bo_match(void) {
    return bo_blackout;
}
