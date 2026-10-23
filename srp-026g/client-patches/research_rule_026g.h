/* Research (mode 5, capture points): the rule's logic, apart from the game.
 *
 * 0.27d's capture_the_point_rule_core and capturable_zone_core, written again
 * on integers so that every game computes the same bytes (docs/notes/
 * research_mode.md). The native code (research_native_026g.c) reads the
 * players out of the world and the parameters out of the mode config; this
 * file holds no pointer into the game and is tested on the host
 * (test_research_rule_026g.c). Nothing here keeps static storage.
 */
#ifndef RESEARCH_RULE_026G_H
#define RESEARCH_RULE_026G_H

typedef unsigned char rr_u8;
typedef unsigned short rr_u16;
typedef unsigned int rr_u32;

enum {
    RR_PLAYERS = 20,            /* roster places a world keeps (late join: always 20) */
    RR_MAX_ZONES = 3,           /* research units of a map (0.27d: cp_id 0, 1, 2) */
    RR_MAX_SHAPES = 8,          /* collision meshes of one unit's geometry */
    RR_MAX_CAPTURE = 8,         /* capture_time_in_s: 1..8 players */
    RR_MAX_OWNED = 3,           /* resource_per_s: 1..3 units owned */
    RR_NOBODY = 2,              /* a team value: none (owner, capturer, leader: draw) */
    RR_NEVER = 0xFFFFFFFFu,     /* end time not set */
    RR_FULL = 1000000000u,      /* a whole capture, in progress units */
    RR_MICRO = 1000000u,        /* points are kept in millionths */
    RR_RATE_MS = 4,             /* points_rate is per this many ms (resource_per_s x 1.25 is whole there) */
    RR_MAX_DT = 10000u,         /* the most ms one tick may move the rule */
};

/* A sphere (type 0), a box (type 1) or a cylinder (type 2) of a unit's zone,
 * as the collision_geometry meshes of 0.27d's (and 0.31a2's London) units,
 * in world space: centre, the three local axes (unit rows: local = axes *
 * (p - centre)), half extents (a cylinder: x the radius, y the half height
 * about its local y axis; a sphere: x the radius). */
enum { RR_SPHERE = 0, RR_BOX = 1, RR_CYLINDER = 2 };
struct rr_shape {
    rr_u32 type;
    float centre[3];
    float axes[3][3];
    float half[3];
};

struct rr_zone_def {
    rr_u32 cp_id;
    rr_u32 shape_count;
    float position[3];                 /* the unit's object position (HUD, minimap) */
    struct rr_shape shapes[RR_MAX_SHAPES];
};

/* From the mode config (mode.parameters), never serialized. */
struct rr_params {
    rr_u32 capture_count;              /* entries of capture_ms */
    rr_u32 capture_ms[RR_MAX_CAPTURE]; /* a whole capture by n players, ms */
    rr_u32 capture_rate[RR_MAX_CAPTURE]; /* RR_FULL / capture_ms: units per ms */
    rr_u32 restore_rate;               /* capture_rate[0] x restore_multiplier */
    rr_u32 owned_count;
    rr_u32 points_rate[RR_MAX_OWNED];  /* millionths of a point per RR_RATE_MS ms, k units owned */
    rr_u32 target;                     /* resources_count in millionths */
    rr_u32 award;                      /* points shared at a capture or neutralisation */
    rr_u32 zone_count;
    struct rr_zone_def zones[RR_MAX_ZONES];
    float reach;                       /* a player's capsule radius, m */
    float half_height;                 /* half a capsule's height, m, when the game gives none */
};

struct rr_zone {
    rr_u32 progress;                   /* 0..RR_FULL of `capturing`'s capture */
    rr_u8 capturing;                   /* whose progress it is: 0, 1, RR_NOBODY */
    rr_u8 owner;                       /* 0, 1, RR_NOBODY */
    rr_u8 count[2];                    /* players of each team inside */
    rr_u32 present[2];                 /* those players, by roster place */
    rr_u32 marks[RR_PLAYERS];          /* each player's share at entry, RR_FULL = all */
};

/* The rule's whole state: what a world saves and loads. */
struct rr_state {
    rr_u32 points[2];                  /* millionths, 0..target */
    rr_u16 score[2];                   /* points shown, 0..resources_count */
    rr_u32 end_time;                   /* ms the match ends at, RR_NEVER */
    rr_u32 entered;                    /* roster places entered and not left */
    rr_u32 zone_count;
    struct rr_zone zones[RR_MAX_ZONES];
};

/* One player as the world has him this tick: his capsule stands on
 * `position` (the player's transform, the feet: research_mode.md "Inside")
 * and is 2 x `half_height` tall, `reach` (rr_params) round. */
struct rr_player {
    rr_u8 alive;                       /* alive and in the world */
    rr_u8 team;                        /* 0 or 1 (other: never counted) */
    float position[3];                 /* the feet */
    float half_height;                 /* half the capsule's height (0.9 standing) */
};

/* What a tick or an event asks the game to count (statistics events). */
enum { RR_AWARD_CAPTURE = 0, RR_AWARD_NEUTRALISE = 1, RR_AWARD_KILL_INVADER = 2 };
struct rr_award {
    rr_u8 kind;
    rr_u8 player;                      /* roster place */
    rr_u8 zone;                        /* index into the zones */
    rr_u8 team;
    rr_u32 amount;                     /* points of `award`, or 1 for a kill */
};
enum { RR_MAX_AWARDS = 2 * RR_MAX_ZONES * RR_PLAYERS + 1 };

/* Sizes of the serialized state. */
enum { RR_ZONE_BYTES = 4 + 1 + 1 + 2 + 8 + 4 * RR_PLAYERS,
       RR_HEAD_BYTES = 8 + 4 + 4 + 4 + 1 };
rr_u32 rr_serialized_size(const struct rr_state *);

/* A fresh state for these parameters. */
void rr_reset(struct rr_state *, const struct rr_params *);
/* Fills capture_rate, restore_rate from capture_ms and a restore multiplier
 * in thousandths; returns 0 when the parameters cannot be played. */
int rr_prepare(struct rr_params *, rr_u32 restore_permille);

/* Whether a capsule standing on `feet`, 2 x `half_height` tall and
 * params->reach round, overlaps one of the zone's shapes. */
int rr_inside(const struct rr_params *, rr_u32 zone, const float feet[3], float half_height);

/* One tick of the match in play (the time rule's state 3): the zones, then
 * the teams' points. players[i] is roster place i (only `entered` ones are
 * read). Returns the number of awards written to `out`. */
rr_u32 rr_tick(struct rr_state *, const struct rr_params *, const struct rr_player players[RR_PLAYERS],
               rr_u32 dt, rr_u32 time, struct rr_award *out);

/* A kill (event 1 {killer, victim}); writes one award when the victim was
 * inside a unit his team was taking (0.27d's "kill invader"). */
rr_u32 rr_kill(const struct rr_state *, rr_u8 killer, rr_u8 victim, rr_u8 killer_team, rr_u8 victim_team,
               struct rr_award *out);

void rr_entered(struct rr_state *, rr_u8 place);
void rr_left(struct rr_state *, rr_u8 place);

/* 0, 1, or RR_NOBODY: by the shown scores, equal a draw; once a team
 * reached the goal (end_time set), by the millionths. */
rr_u32 rr_leader(const struct rr_state *);
int rr_completed(const struct rr_state *, rr_u32 time);

/* Little-endian bytes, as the rules blob carries them. Write returns the
 * bytes written (rr_serialized_size); read returns the bytes read, or 0
 * for bytes that are not a state of these parameters. */
rr_u32 rr_write(const struct rr_state *, rr_u8 *out);
rr_u32 rr_read(struct rr_state *, const struct rr_params *, const rr_u8 *in, rr_u32 size);

#endif
