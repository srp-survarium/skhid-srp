/* Research (exe patch 53): the rule on 0.26g build 2777's own interfaces.
 *
 * - a rule cook of class RN_CLASS (the mode configs' rules [93, 91, 1E0h, 94]),
 *   registered by the game's constructor (research_026g.asm, entry
 *   `register`), whose resources are `rn_rule`s: game_match_rule_base (type 5)
 *   with the twenty slots of 0.26g's rules, the tick a delegate of the
 *   world's second tick list, as timelimit_rule_core's;
 * - the hud (entry `hud`, game_world_ui::update_ui past its gates): the
 *   movie's root.research_update, the units' messages, the end's;
 * - the markers (entry `icons`, every frame of update_ui, at its call of
 *   update_player_icons): the units on the screen (the player-icons movie's
 *   root.research_markers) and, in play, research_update too.
 *
 * Every address is a VA of the original exe (image base 0x10000; it is never
 * rebased). The proof of each is in docs/notes/client_patch.md section 3.50
 * and tools/re-kit/v026g. The logic is research_rule_026g.c. The code keeps
 * its mutable state in rn_state, the arena's block research.state.
 * It calls the game's functions through the engine-function library
 * (engine_026g.h: plain cdecl functions, whatever registers the game's take).
 */
#include "engine_026g.h"
#include "research_rule_026g.h"

typedef rr_u8 u8;
typedef rr_u16 u16;
typedef rr_u32 u32;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))
#define THISCALL __attribute__((thiscall))

enum {
    RN_CLASS = 0x1E0,              /* the rule's resource class (free: register_cook's table has 0x206) */
    RN_TYPE = 5,                   /* game_match_rule_type: 0 respawn, 1 gather, 2 timelimit, 3 kd, 4 tdm */
    RN_MODE = 5,                   /* the configs' mode.mode */
    RN_COOK_SIZE = 0x24,           /* the client rule cooks' size (tdm's: 0x2261B54..0x2261B78) */
};

/* ---- the game ---------------------------------------------------------- */

#define COOKS 0xA86E00             /* register_cook's table: begin, end */
#define COOK_TABLE_SIZE 0x206
#define CURRENT_THREAD_ID 0x870118 /* [import] GetCurrentThreadId */
#define COOK_BASE_VTABLE 0x9DDCB8
#define ALLOCATOR 0x1F1D9C4        /* the rule cooks' allocator (engine_malloc's) */
#define ALLOCATION_INFO (0xACCC60 + 8) /* client tdm rule cook's (0x420789: ecx + 8) */
/* The game's functions (engine_026g.h): engine_register_cook 0x102D10,
 * engine_malloc 0xE4F90, engine_match_rule_base 0x313900, engine_cfg_child
 * 0xF6B10 and engine_cfg_has 0xF6C00, engine_movie_invoke 0x511600,
 * engine_show_message 0x2375C0, engine_point_to_screen 0x23DEC0,
 * engine_statistics_notify 0x2F5D40 and the
 * field writers buffer_writer::w<unsigned char/short/int> (tdm_rule_core::
 * serialize 0x316340): engine_write_u8 0x1F3DE0, engine_write_u16 0x2F5DA0
 * and engine_write_u32 0x1F48A0, which append the value to the writer's
 * buffer (its size grown by 0x7AB30) and, with [0x1F1D32C] and [0x1F1D32D]
 * set (the hash mismatch dump), record the field's name, type and place. */

/* the world (game_world_core) */
#define CORE_PLACES 0x18458        /* -> places, 30h each: [3] players, +Ch the current kit */
#define CORE_TICK_LIST 0x18850     /* the second tick list: count, head, tail (timelimit's) */
#define CORE_TICK_ORDER 0x18FB0    /* -> the tick delegates' counter */
#define CORE_STATISTICS 0x18FA8    /* -> the statistics handler (below) */
#define CORE_STATE 0x18FE4         /* the time rule's state: 3 in play */
#define CORE_LOCAL_PLACE 0x18FDC   /* u8: the game's own roster place */
#define CORE_RULES 0x18824         /* begin, end of the rules */
#define PLACE_SIZE 0x30
#define PLACE_KIT 0xC
/* a player (base_player) */
#define PLAYER_ALIVE 0x33C         /* m_is_alive */
#define PLAYER_IN_WORLD 0x33F      /* in the world's tick lists */
#define PLAYER_POSITION 0x43C      /* m_transform's translation (+40Ch row 3): the feet */
#define PLAYER_PHYSICS 0x494       /* -> -> its character controller (0x2D942A) */
/* the controller (get_transform 0x7F5C0: the player's transform is the
 * capsule's centre moved down by heights[(stance + 2) % 3] + heights[stance],
 * half the capsule's height: 0.9 m standing) */
#define CONTROLLER_HEIGHTS 0x1D0   /* float[3] */
#define CONTROLLER_STANCE 0x1F0    /* 0..2 */
#define PLAYER_PROFILE 0x6B8       /* -> its match profile */
#define PROFILE_TEAM 0x45EC
/* [core+18FA8h] is the network client's base_game_statistics_handler (its
 * slot +34h, deserialize 0x26DB60, is what a world's load calls at
 * 0x2D2B0F), not the shared statistics: those are at +8 in it (the handler's
 * slots pass ecx + 8 to shared_statistics' functions, 0x26DA3C), and the
 * offsets below are the handler's. */
#define STATS_SHARED 0x8           /* shared_statistics */
#define STATS_PLAYER 0x1F8         /* a player's record */
/* u16 events[28][3] at the record's start (recalculate_scores 0x26DC30;
 * the death at shared + place x 1F8h + 0Ch, 0x2F4F4E, is event 2's kit 0) */
#define STATS_EVENTS STATS_SHARED
#define STATS_TEAM_POINTS 0x31D4   /* u16 m_team_points[2]: shared +31CCh */
#define STATS_WINNER 0x31E4        /* m_winner_team: shared +31DCh */
/* the shared statistics' event listener (a boost::function, shared +31A0h:
 * 0x2F4D9F; 0 when none) */
#define STATS_LISTENER (STATS_SHARED + 0x31A0)
/* the statistics' events a unit counts (snapshot_rules.md section 3.1) */
#define EVENT_CAPTURE 6            /* hundredths: a unit taken, the shares of `award` */
#define EVENT_KILL_INVADER 10      /* one: an enemy killed inside a unit his team was taking */
#define EVENT_NEUTRALISE 18        /* hundredths: a unit taken from the other team */
#define EVENTS 28
#define KITS 3
/* match_hud_state (fill_match_hud_state) */
#define HUD_TEAM_POINTS 0x90
/* game_world_ui */
#define UI_GAME 0x18
#define UI_HUD 0x8                 /* the hud movie's holder: [[ui+8]+108h]+4 */
#define UI_ICONS 0x10              /* the player-icons movie's (update_player_icons 0x23ACC1, create_hud_icon 0x23BE31) */
#define UI_MODE 0x20
#define GAME_MATCH 0x4D4C          /* -> the match; its world at +1F8h */
#define MATCH_WORLD 0x1F8
/* -> the camera director, its position at +34h: update_player_icons sizes a
 * player's marker by its distance from there (0x23AAF2, 0x23AB63) */
#define GAME_CAMERA 0x98
#define CAMERA_POSITION 0x34
/* A unit's marker stands this far over the unit's position (its object's,
 * where 0.27d's capturable_zone put its marker, 0x299D15), over the props. */
#define MARKER_LIFT 2.0f

/* the rules' default slots (game_match_rule_base and tdm_rule_core) */
#define RULE_DELETE 0x266710
#define RESOURCE_SLOT1 0xF1900
#define RESOURCE_SLOT2 0xF1F00
#define RESOURCE_SLOT3 0xF1F40
#define RESOURCE_NONE 0xFDA90
#define RETURN_4 0x75AF0
#define RETURN_0 0x1AF330
#define PRIORITY_3 0x8FE10
#define RESOLVE_LINKS 0x316540     /* ret 1Ch */
#define RULE_BASE_LOCATOR 0xA4C970 /* game_match_rule_base's RTTI locators */
#define LINK_RESOLVER_LOCATOR 0xA4C95C
/* MSVC's RTTI of the cook (rn_cook_locator): type_info's vftable (every type
 * descriptor's first word, tdm_rule_cook's 0xACCC38 among them) and the base
 * class descriptors of tdm_rule_core_cook's hierarchy (0xA4718C), which is
 * the cook's shape: translate_query_cook (with cook_base) and boost's
 * noncopyable at +20h. */
#define TYPE_INFO_VFTABLE 0x886A78
#define BCD_TRANSLATE_QUERY_COOK 0xA38D74
#define BCD_COOK_BASE 0xA38D24
#define BCD_NONCOPYABLE 0xA40010

/* ---- the state page --------------------------------------------------- */

struct rn_hud {
    void *rule;                    /* the rule the cache is of */
    u8 owner[RR_MAX_ZONES];
    u8 completed;
    u32 shown;                     /* frames since the movie got its last update */
    u32 sum;                       /* a checksum of what it got */
    u8 *icons;                     /* the player-icons movie showing the markers, 0 none */
    u32 icons_shown, icons_sum;    /* as shown and sum, of the markers */
};

struct rn_globals {
    u8 cook[RN_COOK_SIZE];
    u32 registered;                /* 1: done, 2: the class was taken */
    struct rn_hud hud;
};
struct rn_globals rn_state;        /* zeroed: the block research.state (link_026g.py) */
#define G (&rn_state)

/* ---- the rule --------------------------------------------------------- */

struct rn_delegate {
    void *object;
    void *function;
    struct rn_delegate *previous, *next;
    u32 order;
    u8 enabled;
    u8 pad[3];
};

struct rn_rule {
    u8 base[0x110];                /* vtable, unmanaged_resource, +108h link_resolver, +10Ch type */
    struct rn_delegate tick;
    u8 *core;
    void *blackboard;
    void *game;
    u32 valid;                     /* the config was read */
    struct rr_params params;
    struct rr_state state;
};
/* check_research_026g.py reads the rule by these offsets */
_Static_assert(__builtin_offsetof(struct rn_rule, tick) == 0x110, "the delegate follows the base");
_Static_assert(__builtin_offsetof(struct rn_rule, core) == 0x128, "rn_rule.core");
_Static_assert(__builtin_offsetof(struct rn_rule, valid) == 0x134, "rn_rule.valid");
_Static_assert(__builtin_offsetof(struct rn_rule, params) == 0x138, "rn_rule.params");
_Static_assert(__builtin_offsetof(struct rn_rule, state) == 0x7E0, "rn_rule.state");
_Static_assert(sizeof(struct rn_rule) == 0x918, "rn_rule");
_Static_assert(__builtin_offsetof(struct rr_state, zones) == 0x18 && sizeof(struct rr_zone) == 0x60, "rr_state");
_Static_assert(__builtin_offsetof(struct rr_params, zones) == 0x64 && sizeof(struct rr_zone_def) == 0x214, "rr_params");

/* ---- small things freestanding code needs ------------------------------ */

void *memset(void *to, int value, unsigned long size) {
    u8 *at = to;
    while (size--) *at++ = (u8)value;
    return to;
}

void *memcpy(void *to, const void *from, unsigned long size) {
    u8 *a = to;
    const u8 *b = from;
    while (size--) *a++ = *b++;
    return to;
}

/* ---- calls into the game ---------------------------------------------- */

typedef struct engine_cfg {
    u32 data;                      /* a value, or a pointer (tables, vectors, strings) */
    u32 data_high;
    u32 name, name_high;
    u32 crc;
    u16 type, count;
} cfg;
_Static_assert(sizeof(cfg) == 24, "a config value is 24 bytes");
enum { CFG_BOOL = 0, CFG_INT = 1, CFG_FLOAT = 2, CFG_NAMED = 3, CFG_INDEXED = 4, CFG_STRING = 5, CFG_FLOAT3 = 7 };

static const cfg *child(const cfg *table, const char *key) { return engine_cfg_child(table, key); }

static int has(const cfg *table, const char *key) { return (int)engine_cfg_has(table, key); }

static const cfg *item(const cfg *table, u32 index) { return (const cfg *)table->data + index; }

static float number(const cfg *value) {
    union {
        u32 bits;
        float value;
    } as = {value->data};
    return value->type == CFG_FLOAT ? as.value : (float)(int)value->data;
}

static u32 rounded(float value) { return value <= 0 ? 0 : (u32)(value + 0.5f); }

static float root(float value) {
    float out;
    __asm__("sqrtss %1, %0" : "=x"(out) : "x"(value));
    return out;
}

static void vector(const cfg *value, float out[3]) {
    const float *at = (const float *)value->data;
    out[0] = at[0];
    out[1] = at[1];
    out[2] = at[2];
}

static void *allocate(u32 size) { return engine_malloc(*(void **)ALLOCATOR, size, (const void *)ALLOCATION_INFO); }

static const char RN_FILE[] = ".\\research_native_026g.c";
static const char RN_SERIALIZE[] = "srp::research_rule::serialize";

/* One field through the game's writer of its size; `ordinal` stands for the
 * line (the field's place in the state). */
static void field(u8 *writer, u32 size, const void *value, u32 ordinal, const char *name) {
    void (*write)(const void *, void *, const char *, unsigned int, const char *, const char *) =
        size == 1 ? engine_write_u8 : size == 2 ? engine_write_u16 : engine_write_u32;
    write(value, writer, RN_FILE, ordinal, RN_SERIALIZE, name);
}

typedef struct {
    void *object;
    u32 type;
    double value;
    u32 extra[2];
} gfx_value;
_Static_assert(sizeof(gfx_value) == 0x18, "a GFx::Value is 18h bytes");
enum { GFX_NUMBER = 5 };

/* ---- reading the config ----------------------------------------------- */

static const char *const RN_ZONES = "capture_zones";

/* mode.parameters of the mode config (0.27d's keys, and srp's for the
 * units: capture_zones[] {cp_id, position, shapes[] {type, position, axis_x,
 * axis_y, axis_z, half}}, capture_award, capture_reach, capture_half_height). */
static int read_params(struct rr_params *params, const cfg *root) {
    const cfg *mode = child(root, "mode");
    if (number(child(mode, "mode")) != RN_MODE) return 0;
    const cfg *parameters = child(mode, "parameters");
    const cfg *times = child(parameters, "capture_time_in_s");
    const cfg *rates = child(parameters, "resource_per_s");
    if (times->type != CFG_INDEXED || rates->type != CFG_INDEXED) return 0;
    params->capture_count = times->count > RR_MAX_CAPTURE ? RR_MAX_CAPTURE : times->count;
    for (u32 i = 0; i < params->capture_count; ++i)
        params->capture_ms[i] = (u32)(number(item(times, i)) * 1000.0f); /* 0.27d: trunc(s x 1000) */
    params->owned_count = rates->count > RR_MAX_OWNED ? RR_MAX_OWNED : rates->count;
    for (u32 i = 0; i < params->owned_count; ++i) params->points_rate[i] = rounded(number(item(rates, i)) * (1000.0f * RR_RATE_MS));
    params->target = (u32)number(child(parameters, "resources_count")) * RR_MICRO;
    u32 restore = rounded(number(child(parameters, "restore_multiplier")) * 1000.0f);
    params->award = has(parameters, "capture_award") ? (u32)number(child(parameters, "capture_award")) : 100;
    params->reach = has(parameters, "capture_reach") ? number(child(parameters, "capture_reach")) : 0.38f;
    params->half_height =
        has(parameters, "capture_half_height") ? number(child(parameters, "capture_half_height")) : 0.9f;
    const cfg *zones = child(parameters, RN_ZONES);
    if (zones->type != CFG_INDEXED || !zones->count || zones->count > RR_MAX_ZONES) return 0;
    params->zone_count = zones->count;
    for (u32 z = 0; z < zones->count; ++z) {
        const cfg *zone = item(zones, z);
        struct rr_zone_def *def = &params->zones[z];
        def->cp_id = (u32)number(child(zone, "cp_id"));
        vector(child(zone, "position"), def->position);
        const cfg *shapes = child(zone, "shapes");
        if (shapes->type != CFG_INDEXED || !shapes->count || shapes->count > RR_MAX_SHAPES) return 0;
        def->shape_count = shapes->count;
        for (u32 s = 0; s < shapes->count; ++s) {
            const cfg *shape = item(shapes, s);
            struct rr_shape *out = &def->shapes[s];
            out->type = (u32)number(child(shape, "type"));
            if (out->type != RR_SPHERE && out->type != RR_BOX && out->type != RR_CYLINDER) return 0;
            vector(child(shape, "position"), out->centre);
            vector(child(shape, "axis_x"), out->axes[0]);
            vector(child(shape, "axis_y"), out->axes[1]);
            vector(child(shape, "axis_z"), out->axes[2]);
            vector(child(shape, "half"), out->half);
        }
    }
    return rr_prepare(params, restore);
}

/* ---- the world -------------------------------------------------------- */

static u8 *place_of(const struct rn_rule *rule, u32 place) {
    u8 *places = AT(rule->core, CORE_PLACES, u8 *);
    return places ? places + place * PLACE_SIZE : 0;
}

static u8 *player_of(const struct rn_rule *rule, u32 place) {
    u8 *at = place_of(rule, place);
    if (!at || at[PLACE_KIT] >= KITS) return 0;
    return AT(at, 4 * at[PLACE_KIT], u8 *);
}

static u32 team_of(const u8 *player) {
    u8 *profile = player ? AT(player, PLAYER_PROFILE, u8 *) : 0;
    return profile ? AT(profile, PROFILE_TEAM, u32) : RR_NOBODY;
}

/* Half the player's capsule's height, as get_transform (0x7F5C0) takes it
 * off the capsule's centre to give the player's transform; the config's
 * when the player has no controller or it says nothing sensible. */
static float half_height_of(const struct rn_rule *rule, const u8 *player) {
    u8 **physics = AT(player, PLAYER_PHYSICS, u8 **);
    u8 *controller = physics ? *physics : 0;
    if (!controller) return rule->params.half_height;
    int stance = AT(controller, CONTROLLER_STANCE, int);
    if (stance < 0 || stance > 2) return rule->params.half_height;
    float half = AT(controller, CONTROLLER_HEIGHTS + 4 * ((stance + 2) % 3), float) +
                 AT(controller, CONTROLLER_HEIGHTS + 4 * stance, float);
    return half > 0.1f && half < 2.0f ? half : rule->params.half_height;
}

static void players_of(const struct rn_rule *rule, struct rr_player out[RR_PLAYERS]) {
    for (u32 i = 0; i < RR_PLAYERS; ++i) {
        out[i].alive = 0;
        out[i].team = RR_NOBODY;
        if (!(rule->state.entered >> i & 1)) continue;
        const u8 *player = player_of(rule, i);
        if (!player) continue;
        u32 team = team_of(player);
        out[i].alive = player[PLAYER_ALIVE] && player[PLAYER_IN_WORLD];
        out[i].team = team > 1 ? RR_NOBODY : (u8)team;
        for (int axis = 0; axis < 3; ++axis) out[i].position[axis] = AT(player, PLAYER_POSITION + 4 * axis, float);
        out[i].half_height = half_height_of(rule, player);
    }
}

/* An event counted as the game's own sites count one (the kill handler
 * 0x2F4EB0, 0x2F4DE1): the player's cell, then, when the statistics have
 * their event listener (shared +31A0h, a boost::function, not 0 once set),
 * its call through 0x2F5D40 (engine_statistics_notify: the listener, place,
 * event, amount). The listener is base_game_statistics_handler::on_event
 * 0x26DB80 (vtable 0xA0755C slot +44h, client_patch.md 3.16), ecx the
 * handler ([core+18FA8h]): the player's score (m_scores, +31F0h) grows by
 * the event's weight times the amount (hundredths for events 6, 17, 18), so
 * the Tab list shows it at once, not at the next world loaded.
 *
 * Revision 30's build took the handler for the shared statistics: its cell
 * was 4 bytes on (kit 0's award in kit 2's cell, kit 1's and 2's in the
 * next event's: a capture in 7, a teammate cured, a neutralisation in 19,
 * a battery found, both weighed 2.5 by ones, 250 for a capture, and a kill
 * invader in 11, weighed 0), and its listener 8 bytes short (shared
 * +3198h, the batteries' records, 0 in Research: never told, the score
 * moved at the next world loaded). */
static void count(struct rn_rule *rule, u32 place, u32 event, u32 amount) {
    u8 *statistics = AT(rule->core, CORE_STATISTICS, u8 *);
    u8 *at = place < RR_PLAYERS ? place_of(rule, place) : 0;
    if (!statistics || !at || event >= EVENTS) return;
    u32 kit = at[PLACE_KIT] < KITS ? at[PLACE_KIT] : 0;
    u16 *cell = &AT(statistics, STATS_PLAYER * place + STATS_EVENTS + event * 2 * KITS + 2 * kit, u16);
    u32 sum = *cell + amount, added = (sum > 0xFFFF ? 0xFFFF : sum) - *cell;
    *cell = (u16)(sum > 0xFFFF ? 0xFFFF : sum);
    if (!added || !AT(statistics, STATS_LISTENER, u32)) return;
    engine_statistics_notify(statistics + STATS_LISTENER, place, event, added);
}

static void give(struct rn_rule *rule, const struct rr_award *awards, u32 count_of) {
    for (u32 i = 0; i < count_of; ++i) {
        const struct rr_award *award = &awards[i];
        u32 event = award->kind == RR_AWARD_CAPTURE      ? EVENT_CAPTURE
                    : award->kind == RR_AWARD_NEUTRALISE ? EVENT_NEUTRALISE
                                                         : EVENT_KILL_INVADER;
        count(rule, award->player, event, award->amount);
    }
}

/* The statistics' tail: the teams' points and the leader, as tdm_rule_core
 * writes them (0x3161F0). */
static void publish(struct rn_rule *rule) {
    u8 *statistics = AT(rule->core, CORE_STATISTICS, u8 *);
    if (!statistics) return;
    AT(statistics, STATS_TEAM_POINTS, u16) = rule->state.score[0];
    AT(statistics, STATS_TEAM_POINTS + 2, u16) = rule->state.score[1];
    AT(statistics, STATS_WINNER, u32) = rr_leader(&rule->state);
}

/* ---- the rule's slots ------------------------------------------------- */

THISCALL void rn_tick(struct rn_rule *rule, u32 dt, u32 time, u32 step) {
    (void)step;
    if (!rule->valid || !rule->core || AT(rule->core, CORE_STATE, u32) != 3) return;
    struct rr_player players[RR_PLAYERS];
    struct rr_award awards[RR_MAX_AWARDS];
    players_of(rule, players);
    u32 given = rr_tick(&rule->state, &rule->params, players, dt, time, awards);
    give(rule, awards, given);
    publish(rule);
}

/* Slot 7, on_event(type, data, time): the world's dispatcher 0x2D1EA0
 * pushes the core's time ([core+18FC0h]), then its second argument, then
 * its first; the kill site (0x2D9EEE..0x2D9EF7) gives it 1 and
 * &{killer, victim} (tdm 0x316090 and kd 0x3237E0 read the data at
 * [esp+8]). The first build took (type, time, data): it read the time as
 * the kill's bytes. */
THISCALL void rn_on_event(struct rn_rule *rule, u32 type, const u8 *data, u32 time) {
    (void)time;
    if (!rule->valid || !rule->core) return;
    if (type == 0) {
        publish(rule);
    } else if (type == 1 && data && data[0] < RR_PLAYERS && data[1] < RR_PLAYERS) {
        struct rr_award award;
        u32 killer = team_of(player_of(rule, data[0])), victim = team_of(player_of(rule, data[1]));
        if (killer > 1 || victim > 1) return;
        if (rr_kill(&rule->state, data[0], data[1], (u8)killer, (u8)victim, &award)) give(rule, &award, 1);
    }
}

THISCALL void rn_entered(struct rn_rule *rule, u32 place, u32 time) {
    (void)time;
    rr_entered(&rule->state, (u8)place);
}

THISCALL void rn_left(struct rn_rule *rule, u32 place, u32 time) {
    (void)time;
    rr_left(&rule->state, (u8)place);
}

/* timelimit_rule_core::set_game_world_core (0x318050): the tick a delegate
 * at the end of the world's second list, in its order of registration. */
THISCALL void rn_set_core(struct rn_rule *rule, u8 *core) {
    rule->core = core;
    struct rn_delegate *tick = &rule->tick;
    u32 *order = AT(core, CORE_TICK_ORDER, u32 *);
    tick->object = rule;
    tick->function = (void *)rn_tick;
    tick->order = (*order)++;
    tick->enabled = 1;
    u32 *list = (u32 *)(core + CORE_TICK_LIST);
    ++list[0];
    tick->previous = (struct rn_delegate *)list[2];
    tick->next = 0;
    if (!list[1]) {
        list[1] = (u32)tick;
    } else {
        ((struct rn_delegate *)list[2])->next = tick;
    }
    list[2] = (u32)tick;
}

THISCALL void rn_set_blackboard(struct rn_rule *rule, void *blackboard) { rule->blackboard = blackboard; }

/* The fields' names in the hash mismatch dump, in rr_write's order; a unit's
 * by its letter (cp_id 0, 1, 2: A, B, C), short for the section's room. */
#define RN_MARK(z, i) #z ".marks[" #i "]"
#define RN_MARKS(z)                                                                                                \
    RN_MARK(z, 0), RN_MARK(z, 1), RN_MARK(z, 2), RN_MARK(z, 3), RN_MARK(z, 4), RN_MARK(z, 5), RN_MARK(z, 6),          \
        RN_MARK(z, 7), RN_MARK(z, 8), RN_MARK(z, 9), RN_MARK(z, 10), RN_MARK(z, 11), RN_MARK(z, 12), RN_MARK(z, 13),  \
        RN_MARK(z, 14), RN_MARK(z, 15), RN_MARK(z, 16), RN_MARK(z, 17), RN_MARK(z, 18), RN_MARK(z, 19)
#define RN_ZONE(z)                                                                                                 \
    {#z ".progress", #z ".capturing", #z ".owner", #z ".count[0]", #z ".count[1]", #z ".present[0]",                 \
     #z ".present[1]", RN_MARKS(z)}
static const char *const RN_ZONE_FIELDS[RR_MAX_ZONES][7 + RR_PLAYERS] = {RN_ZONE(A), RN_ZONE(B), RN_ZONE(C)};
_Static_assert(RR_MAX_ZONES == 3 && RR_PLAYERS == 20, "RN_ZONE_FIELDS names three zones of twenty marks");

/* The state field by field through the game's writers, as tdm_rule_core's
 * serialize (0x316340) does: the bytes rr_write gives (little-endian, 21 + 96
 * a unit, what the server's decoder reads), and in the hash mismatch dump
 * each field named. */
THISCALL void rn_serialize(struct rn_rule *rule, u8 *writer) {
    const struct rr_state *state = &rule->state;
    u32 n = 0;
    field(writer, 4, &state->points[0], n++, "m_points[0]");
    field(writer, 4, &state->points[1], n++, "m_points[1]");
    field(writer, 2, &state->score[0], n++, "m_score[0]");
    field(writer, 2, &state->score[1], n++, "m_score[1]");
    field(writer, 4, &state->end_time, n++, "m_end_time");
    field(writer, 4, &state->entered, n++, "m_entered");
    u8 zones = (u8)state->zone_count;
    field(writer, 1, &zones, n++, "m_zone_count");
    for (u32 z = 0; z < state->zone_count && z < RR_MAX_ZONES; ++z) {
        const struct rr_zone *zone = &state->zones[z];
        const char *const *names = RN_ZONE_FIELDS[z];
        field(writer, 4, &zone->progress, n++, names[0]);
        field(writer, 1, &zone->capturing, n++, names[1]);
        field(writer, 1, &zone->owner, n++, names[2]);
        field(writer, 1, &zone->count[0], n++, names[3]);
        field(writer, 1, &zone->count[1], n++, names[4]);
        field(writer, 4, &zone->present[0], n++, names[5]);
        field(writer, 4, &zone->present[1], n++, names[6]);
        for (u32 i = 0; i < RR_PLAYERS; ++i) field(writer, 4, &zone->marks[i], n++, names[7 + i]);
    }
}

/* Slot 16 takes two arguments and pops both (`ret 8`): the snapshot's rules
 * loop 0x2D2B42 pushes a dword of its own and the reader and does not clean
 * up, as tdm's 0x3163D0, timelimit's 0x317FD0 and kd's 0x323910 expect. The
 * dword is not read (none of the three reads it either). */
THISCALL void rn_deserialize(struct rn_rule *rule, u8 *reader, u32 unused) {
    (void)unused;
    u8 *at = AT(reader, 4, u8 *);
    u32 size = rr_serialized_size(&rule->state);
    if (!rr_read(&rule->state, &rule->params, at, size)) rr_reset(&rule->state, &rule->params);
    AT(reader, 4, u8 *) = at + size;
    /* A late joiner's first world: the time rule's deserialize (0x31803E)
     * sends event 0 before ours runs, so slot 7's publish() wrote the fresh
     * rule's 0 / 0 and a draw into the statistics' tail; the tail is the
     * loaded state's from here. */
    if (rule->valid && rule->core) publish(rule);
}

/* The teams' points, as tdm's 0x316430 puts its tickets. Its alive counts
 * (+94h, +95h) stay as the hud state's constructor (0x278D00) leaves them,
 * 0FFh, as in modes 2 and 3: only tdm's slot writes them (kd 0x323980 writes
 * +98h on, timelimit's 0x15A4B0 nothing) and only mode 4's last players read
 * them (update_ui 0x23B3DE). */
THISCALL void rn_fill_hud(struct rn_rule *rule, u8 *hud, u32 unused) {
    (void)unused;
    AT(hud, HUD_TEAM_POINTS, u16) = rule->state.score[0];
    AT(hud, HUD_TEAM_POINTS + 2, u16) = rule->state.score[1];
}

THISCALL u32 rn_completed(struct rn_rule *rule, u32 time) { return (u32)rr_completed(&rule->state, time); }

/* The rule's vtables: 0.26g's twenty slots (game_match_rule_base's, with
 * tdm_rule_core's defaults), its RTTI locator before them as MSVC lays them
 * out, and the link resolver's one slot. */
__attribute__((used)) const u32 rn_rule_vtable[1 + 20] = {
    RULE_BASE_LOCATOR,
    RULE_DELETE, RESOURCE_SLOT1, RESOURCE_SLOT2, RESOURCE_SLOT3, RESOURCE_NONE, RESOURCE_NONE, RESOURCE_NONE,
    (u32)rn_on_event, (u32)rn_entered, (u32)rn_left, RETURN_4, (u32)rn_set_core, (u32)rn_set_blackboard,
    RETURN_4, RETURN_0, (u32)rn_serialize, (u32)rn_deserialize, (u32)rn_fill_hud, (u32)rn_completed, PRIORITY_3,
};
__attribute__((used)) const u32 rn_resolver_vtable[2] = {LINK_RESOLVER_LOCATOR, RESOLVE_LINKS};

/* ---- the cook --------------------------------------------------------- */

THISCALL u32 rn_cook_size(void *cook) {
    (void)cook;
    return sizeof(struct rn_rule);
}

/* The client tdm cook's create (0x2063F0) for our rule: query = {match
 * options, ..., +Ch the mode config's root}. */
THISCALL void *rn_cook_create(u8 *cook, u8 *query) {
    struct rn_rule *rule = allocate(sizeof(struct rn_rule));
    if (!rule) return 0;
    /* The hud's cache is of a rule by its address; a new match's rule may
     * be allocated where the last one was: forget the last one's. */
    G->hud.rule = 0;
    engine_match_rule_base(rule, RN_TYPE);
    u8 *base = rule->base;
    memset(base + 0x110, 0, sizeof *rule - 0x110);
    AT(base, 0, const u32 *) = rn_rule_vtable + 1;
    AT(base, 0x108, const u32 *) = rn_resolver_vtable + 1;
    rule->game = AT(cook, 0x20, void *);
    rule->valid = read_params(&rule->params, AT(query, 0xC, const cfg *));
    rr_reset(&rule->state, &rule->params);
    return rule;
}

/* The cook's own RTTI, as MSVC lays it out (x86): research_rule_cook, a
 * translate_query_cook; its complete object locator before the vtable. No
 * code of the game casts a cook or asks its type (no reference to any cook's
 * type descriptor in .text), but the locator names the cook for what it is,
 * not tdm's. The type descriptor's second word is type_info's cache of the
 * undecorated name, written by type_info::name: the section is writable. */
struct rn_type_descriptor {
    u32 vftable;
    u32 spare;
    char name[sizeof ".?AVresearch_rule_cook@srp@@"];
};
struct rn_base_class {
    const struct rn_type_descriptor *type;
    u32 contained, mdisp, pdisp, vdisp, attributes;
    const void *hierarchy;
};
struct rn_hierarchy {
    u32 signature, attributes, count;
    const u32 *bases;
};
struct rn_locator {
    u32 signature, offset, cd_offset;
    const struct rn_type_descriptor *type;
    const struct rn_hierarchy *hierarchy;
};
extern const struct rn_hierarchy rn_cook_hierarchy;
__attribute__((used)) const struct rn_type_descriptor rn_cook_type = {TYPE_INFO_VFTABLE, 0,
                                                                      ".?AVresearch_rule_cook@srp@@"};
__attribute__((used)) const struct rn_base_class rn_cook_base = {&rn_cook_type, 3, 0, 0xFFFFFFFF, 0, 0x40,
                                                                 &rn_cook_hierarchy};
__attribute__((used)) const u32 rn_cook_bases[4] = {(u32)&rn_cook_base, BCD_TRANSLATE_QUERY_COOK, BCD_COOK_BASE,
                                                    BCD_NONCOPYABLE};
__attribute__((used)) const struct rn_hierarchy rn_cook_hierarchy = {0, 1, 4, rn_cook_bases};
__attribute__((used)) const struct rn_locator rn_cook_locator = {0, 0, 0, &rn_cook_type, &rn_cook_hierarchy};

__attribute__((used)) const u32 rn_cook_vtable[1 + 11] = {
    (u32)&rn_cook_locator, 0x36760, 0xFDA90, 0xF7760, 0x372420, 0x372420, 0x36780, 0x1FA420, 0x2F4290, 0x75AF0,
    (u32)rn_cook_create, (u32)rn_cook_size,
};

/* From the game's constructor, after its own client cooks (research_026g.asm):
 * the cook made as tdm_rule_core_cook's (0x2F41E0), with the client's game
 * at +20h, registered unless its class is taken. */
void rn_register(void *game) {
    struct rn_globals *g = G;
    if (g->registered) return;
    u32 *cooks = *(u32 **)COOKS, *end = *(u32 **)(COOKS + 4);
    if (cooks != end && ((u32)(end - cooks) <= RN_CLASS || cooks[RN_CLASS])) {
        g->registered = 2;
        return;
    }
    u8 *cook = g->cook;
    memset(cook, 0, RN_COOK_SIZE);
    AT(cook, 0, const u32 *) = rn_cook_vtable + 1;
    AT(cook, 8, u32) = RN_CLASS;
    AT(cook, 0x10, u32) = 0xFFFFFFFF;
    AT(cook, 0x14, u32) = ((u32(__attribute__((stdcall)) *)(void)) * (u32 *)CURRENT_THREAD_ID)();
    AT(cook, 0x18, u32) = 8;
    AT(cook, 0x20, void *) = game;
    engine_register_cook(cook);
    g->registered = 1;
}

/* ---- the hud ---------------------------------------------------------- */

static struct rn_rule *find_rule(u8 *ui) {
    u8 *game = AT(ui, UI_GAME, u8 *);
    u8 *match = game ? AT(game, GAME_MATCH, u8 *) : 0;
    if (!match) return 0;
    u8 *core = match + MATCH_WORLD;
    struct rn_rule **rule = AT(core, CORE_RULES, struct rn_rule **), **end = AT(core, CORE_RULES + 4, struct rn_rule **);
    for (; rule && rule < end; ++rule)
        if (*rule && AT((*rule)->base, 0, const u32 *) == rn_rule_vtable + 1) return *rule;
    return 0;
}

/* A movie of the ui: [[ui+holder]+108h]+4, 0 while there is none. */
static u8 *movie_of(u8 *ui, u32 holder) {
    u8 *at = AT(ui, holder, u8 *);
    at = at ? AT(at, 0x108, u8 *) : 0;
    return at ? AT(at, 4, u8 *) : 0;
}

static void message(u8 *ui, const char *key) {
    engine_show_message(key, ui, 0x21, 0x14, 6000);
}

static const char *const CAPTURED[RR_MAX_ZONES] = {"st_research_unit_a_captured", "st_research_unit_b_captured",
                                                     "st_research_unit_c_captured"};
static const char *const LOST[RR_MAX_ZONES] = {"st_research_unit_a_lost", "st_research_unit_b_lost",
                                                 "st_research_unit_c_lost"};

/* A movie call's numbers. Calls, not inlined: research.code stays in its
 * three pages. */
struct rn_call {
    u32 n;
    gfx_value values[5 + 10 * RR_MAX_ZONES];
};

static __attribute__((noinline)) void put_number(struct rn_call *call, double value) {
    gfx_value *at = &call->values[call->n++];
    at->object = 0;
    at->type = GFX_NUMBER;
    at->value = value;
    at->extra[0] = at->extra[1] = 0;
}

static __attribute__((noinline)) void put(struct rn_call *call, int value) { put_number(call, value); }

/* Sends the call to `movie` when its numbers differ from the last ones
 * (`*sum`) or 60 frames went by since (`*frames`): a change at once, and
 * every second anyway. */
static void send(u8 *movie, const char *method, struct rn_call *call, u32 *sum, u32 *frames) {
    u32 hash = 0;
    const u32 *word = (const u32 *)call->values;
    for (u32 i = 0; i < call->n * sizeof(gfx_value) / 4; ++i) hash = hash * 31 + word[i];
    if (hash == *sum && ++*frames < 60) return;
    *sum = hash;
    *frames = 0;
    engine_movie_invoke(movie, method, 0, call->values, call->n);
}

static const char RN_MARKERS[] = "root.research_markers";

/* The markers gone from the player-icons movie that showed them, when it is
 * still this ui's (a match of another mode, or none). */
static void hide_markers(u8 *ui) {
    struct rn_hud *hud = &G->hud;
    if (!hud->icons) return;
    if (movie_of(ui, UI_ICONS) == hud->icons) {
        struct rn_call call = {0};
        put(&call, 0);
        put(&call, 0);
        engine_movie_invoke(hud->icons, RN_MARKERS, 0, call.values, call.n);
    }
    hud->icons = 0;
}

static int whole(float value) { return (int)(value + 0.5f); }

/* What the hud and the markers show of a unit's time left: whole seconds
 * (rounded up, as ms), so that the numbers change once a second. */
struct rn_view {
    struct rn_rule *rule;
    u32 local, place;              /* the game's own team (RR_NOBODY none) and roster place */
    u32 left[RR_MAX_ZONES];
};

static struct rn_rule *view_of(u8 *ui, struct rn_view *view) {
    struct rn_rule *rule = AT(ui, UI_MODE, u32) == RN_MODE ? find_rule(ui) : 0;
    if (!rule || !rule->valid || !rule->core) return 0;
    const struct rr_state *state = &rule->state;
    view->rule = rule;
    view->place = AT(rule->core, CORE_LOCAL_PLACE, u8);
    view->local = view->place < RR_PLAYERS ? team_of(player_of(rule, view->place)) : RR_NOBODY;
    /* the rule moves the units only in play (rn_tick) */
    for (u32 z = 0; z < RR_MAX_ZONES; ++z) {
        u32 ms = z < state->zone_count && AT(rule->core, CORE_STATE, u32) == 3 ? rr_ms_left(state, &rule->params, z) : 0;
        view->left[z] = (ms + 999) / 1000 * 1000;
    }
    return rule;
}

static int inside(const struct rn_view *view, const struct rr_zone *zone) {
    return view->local <= 1 && view->place < RR_PLAYERS && (zone->present[view->local] >> view->place & 1);
}

/* The units on the screen, as 0.27d's player icons of kind 2 (its
 * capturable_zone's tick, 0x299D22 -> 0x234BE0: the zone's position through
 * the icons' placer 0x234C20 and the same point_to_screen; off the screen or
 * behind the camera, hidden): root.research_markers(own team, units, then
 * per unit cp_id, on the screen, x, y (pixels), its distance from the camera
 * (m), owner, capturing, progress 0..100, ms left, own player inside). */
static void markers(u8 *ui, const struct rn_view *view) {
    struct rn_hud *hud = &G->hud;
    u8 *icons = movie_of(ui, UI_ICONS);
    u8 *game = AT(ui, UI_GAME, u8 *);
    u8 *camera = game ? AT(game, GAME_CAMERA, u8 *) : 0;
    if (!icons || !camera) return;
    if (icons != hud->icons) hud->icons_shown = 60;
    hud->icons = icons;
    const struct rn_rule *rule = view->rule;
    const struct rr_state *state = &rule->state;
    struct rn_call call = {0};
    put(&call, view->local > 1 ? 0 : (int)view->local);
    put(&call, (int)state->zone_count);
    for (u32 z = 0; z < state->zone_count; ++z) {
        const struct rr_zone *zone = &state->zones[z];
        const struct rr_zone_def *def = &rule->params.zones[z];
        float point[3] = {def->position[0], def->position[1] + MARKER_LIFT, def->position[2]}, at[2] = {0, 0}, d[3];
        int shown = (int)(engine_point_to_screen(game, point, at) & 1);
        for (int axis = 0; axis < 3; ++axis) d[axis] = point[axis] - AT(camera, CAMERA_POSITION + 4 * axis, float);
        put(&call, (int)def->cp_id);
        put(&call, shown);
        put(&call, shown ? whole(at[0]) : 0);
        put(&call, shown ? whole(at[1]) : 0);
        put(&call, whole(root(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])));
        put(&call, zone->owner);
        put(&call, zone->capturing);
        put_number(&call, (int)(zone->progress / (RR_FULL / 1000)) / 10.0);
        put(&call, (int)view->left[z]);
        put(&call, inside(view, zone));
    }
    send(icons, RN_MARKERS, &call, &hud->icons_sum, &hud->icons_shown);
}

/* root.research_update(local team, points 0, 1, the goal, units, then per
 * unit cp_id, x, -z, owner, capturing, progress 0..100, inside 0, inside 1,
 * whether the game's own player is inside; then per unit the ms left of its
 * capture or neutralisation, whole seconds, 0 none), at a change and every
 * 60 frames of the caller's. */
static void units(u8 *ui, const struct rn_view *view) {
    struct rn_hud *hud = &G->hud;
    const struct rn_rule *rule = view->rule;
    const struct rr_state *state = &rule->state;
    u8 *movie = movie_of(ui, UI_HUD);
    if (!movie) return;
    struct rn_call call = {0};
    put(&call, view->local > 1 ? 0 : (int)view->local);
    put(&call, state->score[0]);
    put(&call, state->score[1]);
    put(&call, (int)(rule->params.target / RR_MICRO));
    put(&call, (int)state->zone_count);
    for (u32 z = 0; z < state->zone_count; ++z) {
        const struct rr_zone *zone = &state->zones[z];
        const struct rr_zone_def *def = &rule->params.zones[z];
        put(&call, (int)def->cp_id);
        put_number(&call, def->position[0]);
        put_number(&call, -def->position[2]);
        put(&call, zone->owner);
        put(&call, zone->capturing);
        put_number(&call, (int)(zone->progress / (RR_FULL / 1000)) / 10.0);
        put(&call, zone->count[0]);
        put(&call, zone->count[1]);
        put(&call, inside(view, zone));
    }
    /* after the units' nine numbers each, so that a movie reading nine a
     * unit reads what it did */
    for (u32 z = 0; z < state->zone_count; ++z) put(&call, (int)view->left[z]);
    send(movie, "root.research_update", &call, &hud->sum, &hud->shown);
}

/* Every frame, from update_player_icons' call in update_ui (0x23B207, before
 * update_ui's 40 ms gate and its hud-state checksum, past which the rest of
 * the hud runs about once a second, at a change of the match's seconds):
 * the units' markers while the match is in play (the time rule's state 3),
 * none in another mode, without a rule, in the countdown or after the end;
 * and in play the hud's units too, so that their rings and timers follow
 * the capture. */
void rn_markers(u8 *ui) {
    struct rn_view view;
    if (!view_of(ui, &view) || AT(view.rule->core, CORE_STATE, u32) != 3) {
        hide_markers(ui);
        return;
    }
    markers(ui, &view);
    units(ui, &view);
}

/* game_world_ui::update_ui after the teams' points (0x23B3DE, behind its
 * gates): the units on the hud (research_update), a message when a unit
 * changes hands or the match ends. */
void rn_hud(u8 *ui) {
    struct rn_view view;
    struct rn_hud *hud = &G->hud;
    if (AT(ui, UI_MODE, u32) != RN_MODE) return;
    struct rn_rule *rule = view_of(ui, &view);
    if (!rule) {
        hud->rule = 0;
        return;
    }
    const struct rr_state *state = &rule->state;
    u32 local = view.local;
    if (hud->rule != rule) {
        hud->rule = rule;
        for (u32 z = 0; z < RR_MAX_ZONES; ++z) hud->owner[z] = z < state->zone_count ? state->zones[z].owner : 2;
        hud->completed = state->end_time != RR_NEVER;
        hud->shown = 60;
    }
    if (local <= 1) {
        for (u32 z = 0; z < state->zone_count; ++z) {
            u8 owner = state->zones[z].owner;
            if (owner == hud->owner[z]) continue;
            if (owner == local) message(ui, CAPTURED[z]);
            else if (hud->owner[z] == local) message(ui, LOST[z]);
            hud->owner[z] = owner;
        }
        u8 completed = state->end_time != RR_NEVER;
        if (completed && !hud->completed) {
            u32 leader = rr_leader(state);
            message(ui, leader == RR_NOBODY ? "st_finish_match_mode_05_draw_message"
                        : leader == local   ? "st_finish_match_mode_05_winner_message"
                                            : "st_finish_match_mode_05_loser_message");
        }
        hud->completed = completed;
    }
    units(ui, &view);
}
