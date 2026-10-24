/* Blackout, exe patch 65 (prototype; the rules: ../docs/notes/blackout.md;
 * client_patch.md section 3.58): a battery match with no interface, the teams
 * told apart by a plain armband on every character's upper arms, blue (team
 * 0) or red (team 1): the content pack blackout-bands@2, a band for each
 * torso model made of its own sleeve and skinned as the sleeve is.
 *
 * - bo_options: srp's block after the static info's squads (the entries
 *   patch 63's td_options reads; match_options::deserialize's end, 0x1F0F30)
 *   may hold tag 2, one byte 1: this match is a Blackout match. The options
 *   keep it in the last 16 bytes of their unused "Mode" string (+57870h, the
 *   magic BO_MAGIC or zeros), and the options last read are remembered: a
 *   match's (0xE4 at 0x27B928) or a replay's (its chunk 1 at 0x1F096C).
 * - bo_match: whether the options last read are a Blackout match's: patch
 *   62's reset of a match's interface (initialize_match, 0x234AE8) then
 *   hides it for the whole match and F11 cannot show it (hide_ui_026g.asm,
 *   hide_ui_force: the one place that forces the state).
 * - bo_visual: profile_skin_visual_cook::on_configs_loaded (0x204560), after
 *   its parts and the head (0x2056F0): for a profile of those options (a
 *   match's player; the lobby's profiles are the lobby's own), one more part
 *   record: the band of the visual's torso (its first record of the torso,
 *   part 2, one of BO_TORSOS) in the profile's team's colour,
 *   `ported/blackout/<the torso's base_model, '/' as '_'>_<blue|red>`, and
 *   the visual's name (its resource's, the cache's key) told apart by team.
 *
 * Nothing a match computes: textures and the interface only (no
 * simulation_changes.tsv row). Every address is a VA of the original exe
 * (image base 0x10000; never rebased). */
typedef unsigned char u8;
typedef unsigned int u32;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))

enum {
    /* match_options (0x578A0 bytes; match_options::deserialize 0x1F0E70) */
    PROFILES = 20,                 /* player_profile[20] at +0 */
    PROFILE_SIZE = 0x45FC,         /* match.md: the player table's stride */
    PROFILE_TEAM = 0x45EC,         /* u32 the team (team_damage_026g.c) */
    OPTIONS_BLACKOUT = 0x57870,    /* the "Mode" string's (+57840h, 40h bytes) last 16: patch 65's */
    BO_MAGIC = 0x4F425253,         /* "SRBO" */
    SRPX_BLACKOUT = 2,             /* srp's entry: one byte, 1 */
    /* skeleton_combined_cook_data (the visual; constructor 0x127C30): the
     * bind pose (+0) and skeleton (+118h), then 20 part records of C0Ch
     * (constructor 0x127CB0): +0 base_model, +114h part_name, +228h material,
     * each a buffer_string {begin, end, the buffer's end} over its own 104h
     * bytes; +C08h the part (0 the head, 1 + the slot table's index 0xA79870
     * for an item's: gloves 1, torso 2). +F324h the records in use (the head's is
     * record 0; the cook starts at 1, 0x204735). skeleton_combined_model_cook
     * queries each record's resources/models/<base_model>.skinned_model/
     * render/<part_name>/converted_model and export_properties and its
     * settings (0x128260). */
    VISUAL_RECORDS = 0x234,
    RECORD_SIZE = 0xC0C,
    RECORDS = 20,
    RECORD_MODEL = 0x000,
    RECORD_PART = 0x114,
    RECORD_KIND = 0xC08,
    KIND_TORSO = 2,
    VISUAL_COUNT = 0xF324,
};

#include "blackout_torsos_026g.h"

/* The bands (pack blackout-bands@2): models/<BAND_DIR><torso>_<colour>.skinned_model, surface `band`. */
static const char BAND_DIR[] = "ported/blackout/";
static const char *const BAND_COLOUR[2] = {"_blue", "_red"};
static const char BAND_PART[] = "band";
static const char *const BAND_NAME[2] = {"srpbo0_", "srpbo1_"};

enum { BAND_TEXT = 0x104 };        /* a record's buffer_string: 104h bytes */

static int same(const u8 *a, const char *b) {
    while (*a && *a == (u8)*b) {
        ++a;
        ++b;
    }
    return *a == (u8)*b;
}

/* The visual's torso (its first record of part 2) if the bands pack has one
 * for it: its base_model's text, else 0. */
static const u8 *torso_of(u8 *visual, u32 count) {
    for (u32 i = 0; i < count && i < RECORDS; ++i) {
        u8 *record = visual + VISUAL_RECORDS + i * RECORD_SIZE;
        if (AT(record, RECORD_KIND, u32) != KIND_TORSO)
            continue;
        const u8 *text = *(u8 **)(record + RECORD_MODEL);
        for (u32 t = 0; t < sizeof BO_TORSOS / sizeof BO_TORSOS[0]; ++t)
            if (same(text, BO_TORSOS[t]))
                return text;
        return 0;
    }
    return 0;
}

/* The options last read (a match's or a replay's). */
static u8 *bo_last;

/* A buffer_string {begin, end, the buffer's end} made `text` (as the game's
 * own copies: at most up to the buffer's end, then the terminator). */
static void set_text(u8 **string, const char *text) {
    u8 *at = string[0];
    while (*text && at < string[2])
        *at++ = (u8)*text++;
    *at = 0;
    string[1] = at;
}

/* The cook's name, {begin, the cursor, the buffer's end}: `text` after it. */
static void append(u8 **name, const char *text) {
    u8 *at = name[1];
    while (*text && at + 1 < name[2])
        *at++ = (u8)*text++;
    *at = 0;
    name[1] = at;
}

/* match_options::deserialize's end (ebx the options, esi the reader {begin,
 * the cursor, the size}): the Blackout entry of srp's block, as td_options
 * reads the block (an entry of another tag or length is stepped over). */
void bo_options(u8 *options, u8 **reader) {
    u8 *at = reader[1], *end = reader[0] + (u32)reader[2];
    u32 on = 0;
    bo_last = options;
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
}

/* 1 when the options last read are a Blackout match's. */
u32 bo_match(void) {
    return bo_last && AT(bo_last, OPTIONS_BLACKOUT, u32) == BO_MAGIC;
}

/* on_configs_loaded at 0x2056F0 (the parts and the head done): `visual` the
 * cook data, `profile` its profile ([ebp+10h]), `name` its name's buffer
 * ([esp+348h]). */
void bo_visual(u8 *visual, u8 *profile, u8 **name) {
    if (!visual || !profile || !bo_match())
        return;
    if (profile < bo_last || profile >= bo_last + PROFILES * PROFILE_SIZE ||
        (u32)(profile - bo_last) % PROFILE_SIZE != 0)
        return;
    u32 team = AT(profile, PROFILE_TEAM, u32), count = visual[VISUAL_COUNT];
    if (team > 1 || count >= RECORDS)
        return;
    const u8 *torso = torso_of(visual, count);
    if (!torso)
        return;
    /* ported/blackout/<the torso's base_model, '/' as '_'>_<colour> */
    char band[BAND_TEXT];
    u32 at = 0;
    for (const char *c = BAND_DIR; *c; ++c)
        band[at++] = *c;
    for (const u8 *c = torso; *c && at < BAND_TEXT - 8; ++c)
        band[at++] = *c == '/' ? '_' : (char)*c;
    for (const char *c = BAND_COLOUR[team]; *c; ++c)
        band[at++] = *c;
    band[at] = 0;
    u8 *record = visual + VISUAL_RECORDS + count * RECORD_SIZE;
    set_text((u8 **)(record + RECORD_MODEL), band);
    set_text((u8 **)(record + RECORD_PART), BAND_PART);
    AT(record, RECORD_KIND, u32) = KIND_TORSO;
    visual[VISUAL_COUNT] = (u8)(count + 1);
    append(name, BAND_NAME[team]);
}
