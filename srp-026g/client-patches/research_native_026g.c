/* Research (exe patch 53): the rule on 0.26g build 2777's own interfaces.
 *
 * - a rule cook of class RN_CLASS (the mode configs' rules [93, 91, 1E0h, 94]),
 *   registered by the game's constructor (research_026g.asm, entry
 *   `register`), whose resources are `rn_rule`s: game_match_rule_base (type 5)
 *   with the twenty slots of 0.26g's rules, the tick a delegate of the
 *   world's second tick list, as timelimit_rule_core's;
 * - the hud (entry `hud`, every frame of game_world_ui::update_ui): the
 *   movie's root.research_update, the units' messages, the end's.
 *
 * Every address is a VA of the original exe (image base 0x10000; it is never
 * rebased). The proof of each is in docs/notes/client_patch.md section 3.50
 * and tools/re-kit/v026g. The logic is research_rule_026g.c. The code keeps
 * its mutable state in the section's own page (RN_STATE, -DRN_STATE_VA).
 */
#include "research_rule_026g.h"

#ifndef RN_STATE_VA
#error "RN_STATE_VA: the section's state page (research_026g.py)"
#endif

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

#define REGISTER_COOK 0x102D10     /* edi = the cook */
#define COOKS 0xA86E00             /* register_cook's table: begin, end */
#define COOK_TABLE_SIZE 0x206
#define CURRENT_THREAD_ID 0x870118 /* [import] GetCurrentThreadId */
#define COOK_BASE_VTABLE 0x9DDCB8
#define ALLOCATOR 0x1F1D9C4        /* the rule cooks' allocator (esi of 0xE4F90) */
#define ALLOCATE 0xE4F90           /* esi allocator, eax size, push its info; ret 4 */
#define ALLOCATION_INFO (0xACCC60 + 8) /* client tdm rule cook's (0x420789: ecx + 8) */
#define RULE_BASE_CTOR 0x313900    /* eax = this, push type; ret 4 */
#define CONFIG_CHILD 0xF6B10       /* eax = table, edi = key: the child */
#define CONFIG_HAS 0xF6C00         /* eax = table, esi = key: al */
#define BUFFER_RESIZE 0x7AB30      /* eax = buffer, push size; ret 4 */
#define HUD_INVOKE 0x511600        /* thiscall movie (name, result, args, count); ret 10h */
#define HUD_MESSAGE 0x2375C0       /* ecx = key; (ui, 21h, 14h, ms); ret 10h */

/* the world (game_world_core) */
#define CORE_PLACES 0x18458        /* -> places, 30h each: [3] players, +Ch the current kit */
#define CORE_TICK_LIST 0x18850     /* the second tick list: count, head, tail (timelimit's) */
#define CORE_TICK_ORDER 0x18FB0    /* -> the tick delegates' counter */
#define CORE_STATISTICS 0x18FA8    /* -> shared_statistics */
#define CORE_STATE 0x18FE4         /* the time rule's state: 3 in play */
#define CORE_LOCAL_PLACE 0x18FDC   /* u8: the game's own roster place */
#define CORE_RULES 0x18824         /* begin, end of the rules */
#define PLACE_SIZE 0x30
#define PLACE_KIT 0xC
/* a player (base_player) */
#define PLAYER_ALIVE 0x33C         /* m_is_alive */
#define PLAYER_IN_WORLD 0x33F      /* in the world's tick lists */
#define PLAYER_POSITION 0x43C      /* m_transform's translation (+40Ch row 3) */
#define PLAYER_PROFILE 0x6B8       /* -> its match profile */
#define PROFILE_TEAM 0x45EC
/* shared_statistics */
#define STATS_PLAYER 0x1F8         /* a player's record */
#define STATS_EVENTS 0xC           /* u16 events[28][3] */
#define STATS_TEAM_POINTS 0x31D4   /* u16 m_team_points[2] */
#define STATS_WINNER 0x31E4        /* m_winner_team */
/* the statistics' events a unit counts (snapshot_rules.md section 3.1) */
#define EVENT_CAPTURE 6            /* hundredths: a unit taken, the shares of `award` */
#define EVENT_KILL_INVADER 10      /* one: an enemy killed inside a unit his team was taking */
#define EVENT_NEUTRALISE 18        /* hundredths: a unit taken from the other team */
#define EVENTS 28
#define KITS 3
/* match_hud_state (fill_match_hud_state) */
#define HUD_TEAM_POINTS 0x90
#define HUD_ALIVE 0x94
/* game_world_ui */
#define UI_GAME 0x18
#define UI_HUD 0x8
#define UI_MODE 0x20
#define GAME_MATCH 0x4D4C          /* -> the match; its world at +1F8h */
#define MATCH_WORLD 0x1F8

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
#define COOK_LOCATOR 0xA47134      /* the client tdm rule cook's (its vtable 0x9FD8AC) */

/* ---- the state page --------------------------------------------------- */

struct rn_hud {
    void *rule;                    /* the rule the cache is of */
    u8 owner[RR_MAX_ZONES];
    u8 completed;
    u32 shown;                     /* frames since the movie got its last update */
    u32 sum;                       /* a checksum of what it got */
};

struct rn_globals {
    u8 cook[RN_COOK_SIZE];
    u32 registered;                /* 1: done, 2: the class was taken */
    struct rn_hud hud;
};
#define G ((struct rn_globals *)RN_STATE_VA)

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

typedef struct {
    u32 data;                      /* a value, or a pointer (tables, vectors, strings) */
    u32 data_high;
    u32 name, name_high;
    u32 crc;
    u16 type, count;
} cfg;
_Static_assert(sizeof(cfg) == 24, "a config value is 24 bytes");
enum { CFG_BOOL = 0, CFG_INT = 1, CFG_FLOAT = 2, CFG_NAMED = 3, CFG_INDEXED = 4, CFG_STRING = 5, CFG_FLOAT3 = 7 };

static const cfg *child(const cfg *table, const char *key) {
    const cfg *result;
    __asm__ volatile("call *%[fn]"
                     : "=a"(result), "+D"(key)
                     : "a"(table), [fn] "r"(CONFIG_CHILD)
                     : "ecx", "edx", "memory", "cc");
    return result;
}

static int has(const cfg *table, const char *key) {
    u32 result;
    __asm__ volatile("call *%[fn]"
                     : "=a"(result), "+S"(key)
                     : "a"(table), [fn] "r"(CONFIG_HAS)
                     : "ecx", "edx", "edi", "memory", "cc");
    return (u8)result;
}

static const cfg *item(const cfg *table, u32 index) { return (const cfg *)table->data + index; }

static float number(const cfg *value) {
    union {
        u32 bits;
        float value;
    } as = {value->data};
    return value->type == CFG_FLOAT ? as.value : (float)(int)value->data;
}

static u32 rounded(float value) { return value <= 0 ? 0 : (u32)(value + 0.5f); }

static void vector(const cfg *value, float out[3]) {
    const float *at = (const float *)value->data;
    out[0] = at[0];
    out[1] = at[1];
    out[2] = at[2];
}

static void *allocate(u32 size) {
    void *result;
    __asm__ volatile("push %[info]\n\tcall *%[fn]"
                     : "=a"(result)
                     : "a"(size), "S"(*(u32 *)ALLOCATOR), [info] "i"(ALLOCATION_INFO), [fn] "r"(ALLOCATE)
                     : "ecx", "edx", "memory", "cc");
    return result;
}

static void rule_base(void *rule, u32 type) {
    u32 ignored;
    __asm__ volatile("push %[type]\n\tcall *%[fn]"
                     : "=a"(ignored)
                     : "a"(rule), [type] "r"(type), [fn] "r"(RULE_BASE_CTOR)
                     : "ecx", "edx", "memory", "cc");
}

static void resize(void *buffer, u32 size) {
    u32 ignored;
    __asm__ volatile("push %[size]\n\tcall *%[fn]"
                     : "=a"(ignored)
                     : "a"(buffer), [size] "r"(size), [fn] "r"(BUFFER_RESIZE)
                     : "ecx", "edx", "memory", "cc");
}

typedef struct {
    void *object;
    u32 type;
    double value;
    u32 extra[2];
} gfx_value;
_Static_assert(sizeof(gfx_value) == 0x18, "a GFx::Value is 18h bytes");
enum { GFX_NUMBER = 5 };

typedef THISCALL u32 (*invoke_fn)(void *, const char *, void *, const gfx_value *, u32);
typedef THISCALL void (*message_fn)(const char *, void *, u32, u32, u32);

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
    for (u32 i = 0; i < params->owned_count; ++i) params->points_rate[i] = rounded(number(item(rates, i)) * 1000.0f);
    params->target = (u32)number(child(parameters, "resources_count")) * RR_MICRO;
    u32 restore = rounded(number(child(parameters, "restore_multiplier")) * 1000.0f);
    params->award = has(parameters, "capture_award") ? (u32)number(child(parameters, "capture_award")) : 100;
    params->reach = has(parameters, "capture_reach") ? number(child(parameters, "capture_reach")) : 0.4f;
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
    }
}

static void count(struct rn_rule *rule, u32 place, u32 event, u32 amount) {
    u8 *statistics = AT(rule->core, CORE_STATISTICS, u8 *);
    u8 *at = place_of(rule, place);
    if (!statistics || !at || event >= EVENTS) return;
    u32 kit = at[PLACE_KIT] < KITS ? at[PLACE_KIT] : 0;
    u16 *cell = &AT(statistics, STATS_PLAYER * place + STATS_EVENTS + event * 2 * KITS + 2 * kit, u16);
    u32 sum = *cell + amount;
    *cell = (u16)(sum > 0xFFFF ? 0xFFFF : sum);
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

THISCALL void rn_on_event(struct rn_rule *rule, u32 type, u32 time, const u8 *data) {
    (void)time;
    if (!rule->valid || !rule->core) return;
    if (type == 0) {
        publish(rule);
    } else if (type == 1 && data && data[0] != 0xFF) {
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

THISCALL void rn_serialize(struct rn_rule *rule, u8 *writer) {
    u8 *buffer = AT(writer, 0x14, u8 *);
    u32 size = rr_serialized_size(&rule->state), at = AT(buffer, 0xC, u32);
    resize(buffer, at + size);
    rr_write(&rule->state, AT(buffer, 4, u8 *) + at);
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
}

THISCALL void rn_fill_hud(struct rn_rule *rule, u8 *hud, u32 unused) {
    (void)unused;
    AT(hud, HUD_TEAM_POINTS, u16) = rule->state.score[0];
    AT(hud, HUD_TEAM_POINTS + 2, u16) = rule->state.score[1];
    hud[HUD_ALIVE] = 0;
    hud[HUD_ALIVE + 1] = 0;
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
    rule_base(rule, RN_TYPE);
    u8 *base = rule->base;
    memset(base + 0x110, 0, sizeof *rule - 0x110);
    AT(base, 0, const u32 *) = rn_rule_vtable + 1;
    AT(base, 0x108, const u32 *) = rn_resolver_vtable + 1;
    rule->game = AT(cook, 0x20, void *);
    rule->valid = read_params(&rule->params, AT(query, 0xC, const cfg *));
    rr_reset(&rule->state, &rule->params);
    return rule;
}

__attribute__((used)) const u32 rn_cook_vtable[1 + 11] = {
    COOK_LOCATOR, 0x36760, 0xFDA90, 0xF7760, 0x372420, 0x372420, 0x36780, 0x1FA420, 0x2F4290, 0x75AF0,
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
    __asm__ volatile("call *%[fn]" : "+D"(cook) : [fn] "r"(REGISTER_COOK) : "eax", "ecx", "edx", "memory", "cc");
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

static void message(u8 *ui, const char *key) {
    ((message_fn)HUD_MESSAGE)(key, ui, 0x21, 0x14, 6000);
}

static const char *const CAPTURED[RR_MAX_ZONES] = {"st_research_unit_a_captured", "st_research_unit_b_captured",
                                                     "st_research_unit_c_captured"};
static const char *const LOST[RR_MAX_ZONES] = {"st_research_unit_a_lost", "st_research_unit_b_lost",
                                                 "st_research_unit_c_lost"};

/* Every frame of the match's hud (game_world_ui::update_ui, after the teams'
 * points): root.research_update(local team, points 0, 1, the goal, units,
 * then per unit cp_id, x, -z, owner, capturing, progress 0..100, inside 0,
 * inside 1, whether the game's own player is inside), and a message when a
 * unit changes hands or the match ends. */
void rn_hud(u8 *ui) {
    if (AT(ui, UI_MODE, u32) != RN_MODE) return;
    struct rn_rule *rule = find_rule(ui);
    struct rn_hud *hud = &G->hud;
    if (!rule || !rule->valid || !rule->core) {
        hud->rule = 0;
        return;
    }
    const struct rr_state *state = &rule->state;
    u32 place = AT(rule->core, CORE_LOCAL_PLACE, u8);
    u32 local = place < RR_PLAYERS ? team_of(player_of(rule, place)) : RR_NOBODY;
    if (hud->rule != rule) {
        hud->rule = rule;
        for (u32 z = 0; z < RR_MAX_ZONES; ++z) hud->owner[z] = z < state->zone_count ? state->zones[z].owner : 2;
        hud->completed = state->end_time != RR_NEVER;
        hud->sum = 0;
        hud->shown = 0;
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
    gfx_value values[5 + 9 * RR_MAX_ZONES];
    u32 n = 0, sum = 0;
#define PUT(v)                                                                                                     \
    do {                                                                                                           \
        values[n].object = 0;                                                                                      \
        values[n].type = GFX_NUMBER;                                                                               \
        values[n].value = (v);                                                                                     \
        values[n].extra[0] = values[n].extra[1] = 0;                                                               \
        ++n;                                                                                                       \
    } while (0)
    PUT(local > 1 ? 0 : local);
    PUT(state->score[0]);
    PUT(state->score[1]);
    PUT(rule->params.target / RR_MICRO);
    PUT(state->zone_count);
    for (u32 z = 0; z < state->zone_count; ++z) {
        const struct rr_zone *zone = &state->zones[z];
        const struct rr_zone_def *def = &rule->params.zones[z];
        u32 progress = zone->progress / (RR_FULL / 1000);
        PUT(def->cp_id);
        PUT(def->position[0]);
        PUT(-def->position[2]);
        PUT(zone->owner);
        PUT(zone->capturing);
        PUT(progress / 10.0);
        PUT(zone->count[0]);
        PUT(zone->count[1]);
        u32 inside = local <= 1 && place < RR_PLAYERS && (zone->present[local] >> place & 1);
        PUT(inside);
        sum = sum * 31 + progress * 7 + zone->owner * 3 + zone->capturing + zone->count[0] * 11 + zone->count[1] * 13 +
              inside * 17;
    }
#undef PUT
    sum = sum * 31 + state->score[0] * 1009 + state->score[1] * 17 + local;
    /* the movie hears of a change at once, and every second (60 frames) anyway */
    if (sum == hud->sum && ++hud->shown < 60) return;
    hud->sum = sum;
    hud->shown = 0;
    u8 *movie = AT(AT(AT(ui, UI_HUD, u8 *), 0x108, u8 *), 4, u8 *);
    ((invoke_fn)HUD_INVOKE)(movie, "root.research_update", 0, values, n);
}
