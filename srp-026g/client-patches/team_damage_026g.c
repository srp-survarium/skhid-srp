/* Patch 63: team damage turned back on its dealer past a threshold, as
 * 0.28a to 0.31a2 had it (docs/notes/team-damage.md; client_patch.md
 * section 3.54). The entries are team_damage_026g.asm's; this is their logic.
 *
 * 0.28a2 (BCJ-decoded exe, image base 0x10000; PROVEN unless tagged):
 * - gameplay/players/default.player `teamdamage_params` {max_amount 3,
 *   max_ratio 0.67, return_ratio 2} (0.28a2, 0.28d0, 0.31a2; not in 0.27d0),
 *   read by 0x5A66F0 into +1D0h, +1D4h, +1D8h of the player's parameters
 *   (the player's +3E0h, +3E4h, +3E8h: LIKELY the same struct at +210h);
 * - a client record per roster place (game_world_core_client, 3Ch bytes,
 *   world+1B808h) holds `m_inflicted_damage` (+34h) and
 *   `m_inflicted_teamdamage` (+38h), saved with the world (0x5FABE6). Nothing
 *   sets them back but the match's start: per match, no decay;
 * - after a hit took health off a player (0x5AA620), its dealer's record
 *   (another place than the victim's, the hit with a source) adds the health
 *   lost to both counts, to the team count only for a teammate (0x5AA940);
 * - before a hit from a teammate is dealt (0x5AA3E0, the victim alive), the
 *   dealer is checked (0x5AA850): team / all > max_ratio and team >
 *   max_amount. Then the victim takes nothing and the dealer takes the same
 *   hit, its damage times return_ratio, through his own hit function (his
 *   own armour; a hit of his own place, so no team rule); his game says "You
 *   were killed for injuring your allies" (st_team_damage_received).
 *
 * In 0.26g (build 2777): the dealer's place is the hit's byte +44h (0xFF none;
 * a fall is the player's own place), its source +20h, its damage +34h (times
 * 1 - invulnerability in damage_model::hit_body_part 0x2E5080; +38h is the
 * armour piercing); the health that kills is the pool "pain" (0.28's
 * "health"), whose loss hit_impl computes at 0x2D9EA9 for the player's
 * on_hit. The counts and the server's numbers are kd_stats_rule's (the
 * object grown by TD_SIZE, its constructor's query's match options, where
 * td_options put the static info's numbers). The counts go with the world:
 * a record of their own after the effects' three writes (game_world_core::
 * serialize 0x2D2570 and deserialize 0x2D2770), so a rollback, a late
 * joiner's world and a replay's seek have them as the games that played. */
#include "engine_026g.h"

typedef unsigned char u8;
typedef unsigned int u32;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))
#define PTR(base, offset) AT(base, offset, u8 *)

enum {
    PLACES = 20,                   /* the roster's most */
    /* a player (base_player) */
    PLAYER_PLACE = 0x114,          /* u8 its roster place */
    PLAYER_CORE = 0x120,           /* the game world core */
    PLAYER_ALIVE = 0x33C,          /* bool m_is_alive */
    PLAYER_DYING = 0x33D,          /* bool m_has_to_die */
    PLAYER_PROFILE = 0x6B8,        /* its place's player_profile in the match options */
    PROFILE_TEAM = 0x45EC,         /* u32 the team */
    /* a hit (hit_info, 48h bytes; base_player::hit 0x2D9AE0 builds one) */
    HIT_SIZE = 0x48,
    HIT_SOURCE = 0x20,             /* the weapon, grenade or trap; 0 for a fall */
    HIT_DAMAGE = 0x34,             /* f32 the damage */
    HIT_DEALER = 0x44,             /* u8 the dealer's place, 0xFF none */
    /* the game world core */
    CORE_PLACES = 0x18458,         /* the places, 30h each: +4 x kit the players, +0Ch the kit */
    CORE_PLACES_END = 0x1845C,
    PLACE_SIZE = 0x30,
    PLACE_KIT = 0x0C,
    CORE_RULES = 0x18824,          /* the rules: begin, end */
    CORE_RULES_END = 0x18828,
    /* kd_stats_rule */
    KD_VTABLE = 0xA177D4,
    KD_COUNT = 0x1B0,              /* u8 its places (the roster at the start) */
    TD_ON = 0x1B8,                 /* u32 1: the server's numbers came */
    TD_AMOUNT = 0x1BC,             /* f32 max_amount */
    TD_RATIO = 0x1C0,              /* f32 max_ratio */
    TD_RETURN = 0x1C4,             /* f32 return_ratio */
    TD_COUNTS = 0x1C8,             /* f32 [PLACES][2]: all the health a place took off others, of it teammates' */
    TD_SIZE = 0xB0,                /* what the rule grew by */
    /* match_options: the "Mode" string (+57840h, 40h bytes) is only made
     * empty by the constructor and logged; its second half holds the
     * static info's numbers (TD_ON .. TD_RETURN's layout) */
    OPTIONS_TEAM_DAMAGE = 0x57860,
    /* the static info's srp block after the squads */
    SRPX_TEAM_DAMAGE = 1,          /* f32 max_amount, max_ratio, return_ratio */
    SAVED = 0x44545253,            /* "SRTD": the world's record */
};

static const char FILE_NAME[] = "team_damage_026g.c";

/* The kd_stats_rule of the world `core`, or 0. */
static u8 *kd_of(u8 *core) {
    if (!core)
        return 0;
    for (u8 **rule = AT(core, CORE_RULES, u8 **); rule && rule < AT(core, CORE_RULES_END, u8 **); ++rule)
        if (*rule && AT(*rule, 0, u32) == KD_VTABLE)
            return *rule;
    return 0;
}

/* The player of `place` (its kit's), or 0. */
static u8 *player_of(u8 *core, u32 place) {
    u8 *begin = PTR(core, CORE_PLACES), *end = PTR(core, CORE_PLACES_END);
    if (!begin || end < begin || place >= (u32)(end - begin) / PLACE_SIZE)
        return 0;
    u8 *at = begin + place * PLACE_SIZE;
    u32 kit = at[PLACE_KIT];
    return kit < 3 ? AT(at, kit * 4, u8 *) : 0;
}

static int team_of(u8 *player, u32 *team) {
    u8 *profile = PTR(player, PLAYER_PROFILE);
    if (!profile)
        return 0;
    *team = AT(profile, PROFILE_TEAM, u32);
    return 1;
}

static int finite(float x) {
    return x == x && x - x == 0.0f;
}

/* The dealer of `hit` on `victim` that the rule counts: another place than
 * the victim's, of the rule's places, with its player, the hit with a
 * source (0x5AA41B, 0x5AA62F: 0.28's +4, 0.26g's +20h). 0 for none. */
static u8 *dealer_of(u8 *victim, const u8 *hit, u8 **kd, u32 *place, int *teammate) {
    u32 dealer = hit[HIT_DEALER], own = victim[PLAYER_PLACE], a, b;
    if (dealer == 0xFF || dealer == own || dealer >= PLACES || !AT(hit, HIT_SOURCE, u8 *))
        return 0;
    u8 *core = PTR(victim, PLAYER_CORE);
    *kd = kd_of(core);
    if (!*kd || dealer >= (*kd)[KD_COUNT])
        return 0;
    u8 *player = player_of(core, dealer);
    if (!player || !team_of(player, &a) || !team_of(victim, &b))
        return 0;
    *place = dealer;
    *teammate = a == b;
    return player;
}

/* base_player::hit_impl's entry (stdcall: the player, the hit): 1 when the
 * hit is turned on its dealer (the victim takes nothing), 0 to deal it. */
int td_reflect(u8 *victim, const u8 *hit) {
    u8 *kd, *dealer;
    u32 place;
    int teammate;
    if (!victim[PLAYER_ALIVE] || victim[PLAYER_DYING])
        return 0;
    dealer = dealer_of(victim, hit, &kd, &place, &teammate);
    if (!dealer || !teammate || AT(kd, TD_ON, u32) != 1)
        return 0;
    float *counts = &AT(kd, TD_COUNTS, float) + 2 * place;
    float ratio = counts[1] / counts[0];
    if (!(ratio > AT(kd, TD_RATIO, float)) || !(counts[1] > AT(kd, TD_AMOUNT, float)))
        return 0;
    u8 turned[HIT_SIZE];
    for (u32 i = 0; i < HIT_SIZE; ++i)
        turned[i] = hit[i];
    AT(turned, HIT_DAMAGE, float) *= AT(kd, TD_RETURN, float);
    engine_hit_impl(dealer, turned);
    return 1;
}

/* hit_impl after the hit took `lost` of the victim's pain (0x2D9EA9). */
void td_count(u8 *victim, const u8 *hit, float lost) {
    u8 *kd;
    u32 place;
    int teammate;
    if (!dealer_of(victim, hit, &kd, &place, &teammate))
        return;
    float *counts = &AT(kd, TD_COUNTS, float) + 2 * place;
    counts[0] += lost;
    if (teammate)
        counts[1] += lost;
}

/* The end of match_options::deserialize 0x1F0E70 (ebx the options, esi the
 * reader {begin, cursor, size}): the srp block after the squads, if any. */
void td_options(u8 *options, u8 **reader) {
    u8 *at = reader[1], *end = reader[0] + (u32)reader[2];
    u32 *rules = &AT(options, OPTIONS_TEAM_DAMAGE, u32);
    rules[0] = rules[1] = rules[2] = rules[3] = 0;
    if (!at || end < at || end - at < 4 || at[0] != 'S' || at[1] != 'R' || at[2] != 'P' || at[3] != 'X')
        return;
    for (at += 4; end - at >= 2;) {
        u32 tag = at[0], len = at[1];
        at += 2;
        if ((u32)(end - at) < len)
            return;
        if (tag == SRPX_TEAM_DAMAGE && len == 12) {
            float amount = AT(at, 0, float), ratio = AT(at, 4, float), turn = AT(at, 8, float);
            if (finite(amount) && finite(ratio) && finite(turn) && amount >= 0.0f && ratio >= 0.0f &&
                ratio <= 1.0f && turn >= 0.0f && turn <= 16.0f) {
                AT(rules, 4, float) = amount;
                AT(rules, 8, float) = ratio;
                AT(rules, 12, float) = turn;
                rules[0] = 1;
            }
        }
        at += len;
    }
}

/* kd_stats_rule's constructor done (`rule`, its query: [query] the match
 * options): its counts none, the options' numbers its own. */
void td_kd_made(u8 *rule, u8 **query) {
    for (u32 i = 0; i < TD_SIZE; ++i)
        rule[TD_ON + i] = 0;
    u8 *options = query ? query[0] : 0;
    if (options && AT(options, OPTIONS_TEAM_DAMAGE, u32) == 1)
        for (u32 i = 0; i < 16; ++i)
            rule[TD_ON + i] = options[OPTIONS_TEAM_DAMAGE + i];
}

/* game_world_core::serialize after the effects (ebp the core, edi the
 * effects' writer): "SRTD", the places, each place's two counts. */
void td_save(u8 *core, void *writer) {
    u8 *kd = kd_of(core);
    u32 magic = SAVED, places = kd ? kd[KD_COUNT] : 0;
    u8 count = places > PLACES ? PLACES : (u8)places;
    engine_write_u32(&magic, writer, FILE_NAME, __LINE__, __func__, "srp_team_damage");
    engine_write_u8(&count, writer, FILE_NAME, __LINE__, __func__, "places");
    for (u32 i = 0; i < count; ++i) {
        engine_write_u32(&AT(kd, TD_COUNTS + 8 * i, u32), writer, FILE_NAME, __LINE__, __func__,
                         "m_inflicted_damage");
        engine_write_u32(&AT(kd, TD_COUNTS + 8 * i + 4, u32), writer, FILE_NAME, __LINE__, __func__,
                         "m_inflicted_teamdamage");
    }
}

/* game_world_core::deserialize after the effects (ebp the core, the
 * effects' reader {begin, cursor, size}): the counts the world has, none
 * for a world without them. */
void td_load(u8 *core, u8 **reader) {
    u8 *kd = kd_of(core), *at = reader[1], *end = reader[0] + (u32)reader[2];
    u32 count = 0, places = kd ? kd[KD_COUNT] : 0;
    if (at && end >= at && end - at >= 5 && AT(at, 0, u32) == SAVED) {
        count = at[4];
        if (count > PLACES || (u32)(end - at - 5) < 8 * count)
            count = 0;
        else
            reader[1] = at + 5 + 8 * count;
    }
    for (u32 i = 0; kd && i < places && i < PLACES; ++i) {
        AT(kd, TD_COUNTS + 8 * i, u32) = i < count ? AT(at, 5 + 8 * i, u32) : 0;
        AT(kd, TD_COUNTS + 8 * i + 4, u32) = i < count ? AT(at, 9 + 8 * i, u32) : 0;
    }
}
