/* Research's rule logic (research_rule_026g.h; the rules in
 * docs/notes/research_mode.md). Integers only where a value is kept, so that
 * a world saved and loaded, or computed by another game, is the same; the
 * one float test (a player inside a unit's zone) reads the world's floats
 * the same way in every game. */
#include "research_rule_026g.h"

#define LIMIT(value, most) ((value) > (most) ? (most) : (value))

static void put32(rr_u8 *out, rr_u32 value) {
    out[0] = (rr_u8)value;
    out[1] = (rr_u8)(value >> 8);
    out[2] = (rr_u8)(value >> 16);
    out[3] = (rr_u8)(value >> 24);
}

static rr_u32 get32(const rr_u8 *in) {
    return (rr_u32)in[0] | (rr_u32)in[1] << 8 | (rr_u32)in[2] << 16 | (rr_u32)in[3] << 24;
}

static rr_u32 popcount(rr_u32 mask) {
    rr_u32 count = 0;
    for (; mask; mask &= mask - 1) ++count;
    return count;
}

rr_u32 rr_serialized_size(const struct rr_state *state) {
    return RR_HEAD_BYTES + state->zone_count * RR_ZONE_BYTES;
}

int rr_prepare(struct rr_params *params, rr_u32 restore_permille) {
    if (!params->capture_count || params->capture_count > RR_MAX_CAPTURE || !params->owned_count ||
        params->owned_count > RR_MAX_OWNED || !params->zone_count || params->zone_count > RR_MAX_ZONES ||
        !params->target || restore_permille > 1000)
        return 0;
    for (rr_u32 i = 0; i < params->capture_count; ++i) {
        if (params->capture_ms[i] < 100 || params->capture_ms[i] > 3600000) return 0;
        params->capture_rate[i] = RR_FULL / params->capture_ms[i];
    }
    for (rr_u32 i = 0; i < params->owned_count; ++i)
        if (!params->points_rate[i] || params->points_rate[i] > params->target) return 0;
    for (rr_u32 z = 0; z < params->zone_count; ++z)
        if (!params->zones[z].shape_count || params->zones[z].shape_count > RR_MAX_SHAPES) return 0;
    rr_u32 rate = params->capture_rate[0];
    params->restore_rate = rate / 1000 * restore_permille + rate % 1000 * restore_permille / 1000;
    return 1;
}

void rr_reset(struct rr_state *state, const struct rr_params *params) {
    rr_u8 *bytes = (rr_u8 *)state;
    for (rr_u32 i = 0; i < sizeof *state; ++i) bytes[i] = 0;
    state->end_time = RR_NEVER;
    state->zone_count = params->zone_count;
    for (rr_u32 z = 0; z < params->zone_count; ++z) {
        state->zones[z].capturing = RR_NOBODY;
        state->zones[z].owner = RR_NOBODY;
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i) state->zones[z].marks[i] = RR_FULL;
    }
}

static float absolute(float value) { return value < 0 ? -value : value; }

static float root(float value) {
    float out;
    __asm__("sqrtss %1, %0" : "=x"(out) : "x"(value));
    return out;
}

static float positive(float value) { return value > 0 ? value : 0; }

/* The squared distance from a point to a shape (0 inside it). */
static float distance2(const struct rr_shape *shape, const float p[3]) {
    float d[3] = {p[0] - shape->centre[0], p[1] - shape->centre[1], p[2] - shape->centre[2]};
    float l[3];
    for (int axis = 0; axis < 3; ++axis)
        l[axis] = shape->axes[axis][0] * d[0] + shape->axes[axis][1] * d[1] + shape->axes[axis][2] * d[2];
    if (shape->type == RR_SPHERE) {
        float out = positive(root(l[0] * l[0] + l[1] * l[1] + l[2] * l[2]) - shape->half[0]);
        return out * out;
    }
    if (shape->type == RR_BOX) {
        float x = positive(absolute(l[0]) - shape->half[0]), y = positive(absolute(l[1]) - shape->half[1]),
              z = positive(absolute(l[2]) - shape->half[2]);
        return x * x + y * y + z * z;
    }
    float radial = positive(root(l[0] * l[0] + l[2] * l[2]) - shape->half[0]);
    float axial = positive(absolute(l[1]) - shape->half[1]);
    return radial * radial + axial * axial;
}

/* 0.27d's test is the physics' overlap of the unit's trigger shapes and the
 * player's capsule (capturable_zone_core's enter 0x324350 gets the colliding
 * objects' owners). Here: the capsule's segment, vertical, from reach above
 * the feet to reach below its top, and the shape: the nearest they come
 * (the distance along the segment is convex: a ternary search) is at most
 * the capsule's radius. Bullet's collision margin is not counted. */
static int inside_shape(const struct rr_params *params, const struct rr_shape *shape, const float feet[3],
                        float half_height) {
    float r = params->reach, lo = feet[1] + r, hi = feet[1] + 2 * half_height - r;
    if (hi < lo) lo = hi = feet[1] + half_height;
    float p[3] = {feet[0], lo, feet[2]};
    for (int i = 0; i < 40 && hi - lo > 0.0005f; ++i) {
        float a = lo + (hi - lo) / 3, b = hi - (hi - lo) / 3, da, db;
        p[1] = a;
        da = distance2(shape, p);
        p[1] = b;
        db = distance2(shape, p);
        if (da <= db) hi = b;
        else lo = a;
    }
    p[1] = (lo + hi) / 2;
    return distance2(shape, p) <= r * r;
}

int rr_inside(const struct rr_params *params, rr_u32 zone, const float feet[3], float half_height) {
    const struct rr_zone_def *def = &params->zones[zone];
    for (rr_u32 s = 0; s < def->shape_count; ++s)
        if (inside_shape(params, &def->shapes[s], feet, half_height)) return 1;
    return 0;
}

/* `amount` shared over the players of `mask` as 0.27d's handler of a
 * capture event (capture_the_point_rule_core 0x31E920) water-fills it: while
 * any mark is left, the smallest one m is taken off every mark still above
 * zero, and each of those c players gets m / c of `amount`; each share is
 * then rounded (+0.5, down). So the capture is split, moment by moment, among
 * those who were there, and the whole amount goes out when one was there
 * from the start (mark 1). Marks count in 1/65536ths of a capture here
 * (RR_FULL >> 14 is 61035) so that every product fits 32 bits. */
enum { RR_MARK_SHIFT = 14, RR_MARK_ONE = RR_FULL >> RR_MARK_SHIFT };
static rr_u32 share(const struct rr_zone *zone, rr_u32 mask, rr_u32 amount, rr_u8 kind, rr_u8 zone_index,
                    rr_u8 team, struct rr_award *out) {
    rr_u32 mark[RR_PLAYERS], sum[RR_PLAYERS];
    if (!mask || !amount || amount > 4000) return 0;
    for (rr_u32 i = 0; i < RR_PLAYERS; ++i) {
        mark[i] = mask >> i & 1 ? zone->marks[i] >> RR_MARK_SHIFT : 0;
        sum[i] = 0;
    }
    for (;;) {
        rr_u32 least = 0, count = 0;
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i)
            if (mark[i] && (!least || mark[i] < least)) least = mark[i];
        if (!least) break;
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i) count += mark[i] != 0;
        rr_u32 each = least * amount / count;
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i)
            if (mark[i]) {
                sum[i] += each;
                mark[i] -= least;
            }
    }
    rr_u32 written = 0;
    for (rr_u32 i = 0; i < RR_PLAYERS; ++i) {
        rr_u32 given = (sum[i] + RR_MARK_ONE / 2) / RR_MARK_ONE;
        if (!given) continue;
        out[written].kind = kind;
        out[written].player = (rr_u8)i;
        out[written].zone = zone_index;
        out[written].team = team;
        out[written].amount = given;
        ++written;
    }
    return written;
}

static rr_u32 mark_of(const struct rr_zone *zone, rr_u32 team) {
    /* 0.27d's capturable_zone_core enter: the part of the capture still to
     * come when the player came in: for the team taking it 1 - progress,
     * for the other team (who must drain it first) the progress. */
    return zone->capturing == team ? RR_FULL - zone->progress : zone->progress;
}

static rr_u32 tick_zone(struct rr_zone *zone, rr_u8 index, const struct rr_params *params, rr_u32 dt,
                        struct rr_award *out) {
    rr_u32 written = 0;
    if (zone->count[0] && zone->count[1]) return 0;
    if (!zone->count[0] && !zone->count[1]) {
        if (zone->owner == RR_NOBODY && zone->progress) {
            rr_u32 fall = dt * params->restore_rate;
            zone->progress = fall > zone->progress ? 0 : zone->progress - fall;
        }
        return 0;
    }
    rr_u8 team = zone->count[0] ? 0 : 1;
    if (zone->owner == team && zone->progress == RR_FULL) return 0;
    rr_u32 n = LIMIT(zone->count[team], params->capture_count);
    rr_u32 rate = dt * params->capture_rate[n - 1];
    if (zone->capturing == team) {
        zone->progress = LIMIT(zone->progress + rate, RR_FULL);
    } else if (rate > zone->progress) {
        if (zone->owner != RR_NOBODY)
            written += share(zone, zone->present[team], params->award, RR_AWARD_NEUTRALISE, index, team,
                             out + written);
        zone->progress = LIMIT(rate - zone->progress, RR_FULL);
        zone->owner = RR_NOBODY;
        zone->capturing = team;
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i) zone->marks[i] = RR_FULL;
    } else {
        zone->progress -= rate;
    }
    if (zone->progress == RR_FULL && zone->capturing == team && zone->owner != team) {
        zone->owner = team;
        written += share(zone, zone->present[team], params->award, RR_AWARD_CAPTURE, index, team, out + written);
    }
    return written;
}

rr_u32 rr_tick(struct rr_state *state, const struct rr_params *params, const struct rr_player players[RR_PLAYERS],
               rr_u32 dt, rr_u32 time, struct rr_award *out) {
    rr_u32 written = 0;
    dt = LIMIT(dt, RR_MAX_DT);
    /* who is inside, by the world of this tick */
    for (rr_u32 z = 0; z < state->zone_count; ++z) {
        struct rr_zone *zone = &state->zones[z];
        rr_u32 present[2] = {0, 0};
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i) {
            const struct rr_player *player = &players[i];
            if (!(state->entered >> i & 1) || !player->alive || player->team > 1) continue;
            if (rr_inside(params, z, player->position, player->half_height)) present[player->team] |= 1u << i;
        }
        for (rr_u32 t = 0; t < 2; ++t) {
            rr_u32 came = present[t] & ~zone->present[t];
            for (rr_u32 i = 0; i < RR_PLAYERS; ++i)
                if (came >> i & 1) zone->marks[i] = mark_of(zone, t);
            zone->present[t] = present[t];
            zone->count[t] = (rr_u8)popcount(present[t]);
        }
    }
    /* the end is set: nothing moves any more */
    if (state->end_time != RR_NEVER) return 0;
    for (rr_u32 z = 0; z < state->zone_count; ++z)
        written += tick_zone(&state->zones[z], (rr_u8)z, params, dt, out + written);
    /* the teams' research: by the units each owns */
    for (rr_u32 t = 0; t < 2; ++t) {
        rr_u32 owned = 0;
        for (rr_u32 z = 0; z < state->zone_count; ++z)
            if (state->zones[z].owner == t) ++owned;
        if (owned) {
            owned = LIMIT(owned, params->owned_count);
            rr_u32 gain = dt * params->points_rate[owned - 1] / RR_RATE_MS;
            state->points[t] = LIMIT(state->points[t] + gain, params->target);
        }
        rr_u32 score = (state->points[t] + RR_MICRO / 2) / RR_MICRO;
        state->score[t] = (rr_u16)LIMIT(score, params->target / RR_MICRO);
    }
    if (state->points[0] >= params->target || state->points[1] >= params->target) state->end_time = time + 3000;
    return written;
}

rr_u32 rr_kill(const struct rr_state *state, rr_u8 killer, rr_u8 victim, rr_u8 killer_team, rr_u8 victim_team,
               struct rr_award *out) {
    if (killer >= RR_PLAYERS || victim >= RR_PLAYERS || killer == victim || killer_team > 1 ||
        victim_team > 1 || killer_team == victim_team || state->end_time != RR_NEVER)
        return 0;
    for (rr_u32 z = 0; z < state->zone_count; ++z) {
        const struct rr_zone *zone = &state->zones[z];
        if (!(zone->present[victim_team] >> victim & 1)) continue;
        /* 0.27d (0x31E9E7..0x31EA11): inside a unit his team does not
         * own: taking it, or draining the killer's team's */
        if (zone->owner == victim_team) continue;
        out->kind = RR_AWARD_KILL_INVADER;
        out->player = killer;
        out->zone = (rr_u8)z;
        out->team = killer_team;
        out->amount = 1;
        return 1;
    }
    return 0;
}

rr_u32 rr_ms_left(const struct rr_state *state, const struct rr_params *params, rr_u32 index) {
    if (index >= state->zone_count || state->end_time != RR_NEVER || !params->capture_count) return 0;
    const struct rr_zone *zone = &state->zones[index];
    if (!zone->count[0] == !zone->count[1]) return 0;
    rr_u8 team = zone->count[0] ? 0 : 1;
    if (zone->owner == team && zone->progress == RR_FULL) return 0;
    rr_u32 rate = params->capture_rate[LIMIT(zone->count[team], params->capture_count) - 1];
    /* tick_zone: the team's own capture fills; another's, or the
     * ownership, drains first; from nothing, the capture starts */
    rr_u32 left = zone->capturing == team ? RR_FULL - zone->progress : zone->progress ? zone->progress : RR_FULL;
    return rate ? left / rate + (left % rate != 0) : 0;
}

void rr_entered(struct rr_state *state, rr_u8 place) {
    if (place < RR_PLAYERS) state->entered |= 1u << place;
}

void rr_left(struct rr_state *state, rr_u8 place) {
    if (place >= RR_PLAYERS) return;
    state->entered &= ~(1u << place);
    for (rr_u32 z = 0; z < state->zone_count; ++z)
        for (rr_u32 t = 0; t < 2; ++t)
            if (state->zones[z].present[t] >> place & 1) {
                state->zones[z].present[t] &= ~(1u << place);
                --state->zones[z].count[t];
            }
}

rr_u32 rr_leader(const struct rr_state *state) {
    /* At the goal by the millionths: 100 shown for both (99.5 rounds up)
     * is not a draw when one team reached 100. */
    if (state->end_time != RR_NEVER && state->points[0] != state->points[1])
        return state->points[0] > state->points[1] ? 0 : 1;
    if (state->score[0] > state->score[1]) return 0;
    if (state->score[1] > state->score[0]) return 1;
    return RR_NOBODY;
}

int rr_completed(const struct rr_state *state, rr_u32 time) {
    return state->end_time != RR_NEVER && time >= state->end_time;
}

rr_u32 rr_write(const struct rr_state *state, rr_u8 *out) {
    rr_u8 *at = out;
    put32(at, state->points[0]);
    put32(at + 4, state->points[1]);
    at[8] = (rr_u8)state->score[0];
    at[9] = (rr_u8)(state->score[0] >> 8);
    at[10] = (rr_u8)state->score[1];
    at[11] = (rr_u8)(state->score[1] >> 8);
    put32(at + 12, state->end_time);
    put32(at + 16, state->entered);
    at[20] = (rr_u8)state->zone_count;
    at += RR_HEAD_BYTES;
    for (rr_u32 z = 0; z < state->zone_count; ++z) {
        const struct rr_zone *zone = &state->zones[z];
        put32(at, zone->progress);
        at[4] = zone->capturing;
        at[5] = zone->owner;
        at[6] = zone->count[0];
        at[7] = zone->count[1];
        put32(at + 8, zone->present[0]);
        put32(at + 12, zone->present[1]);
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i) put32(at + 16 + 4 * i, zone->marks[i]);
        at += RR_ZONE_BYTES;
    }
    return (rr_u32)(at - out);
}

rr_u32 rr_read(struct rr_state *state, const struct rr_params *params, const rr_u8 *in, rr_u32 size) {
    if (size < RR_HEAD_BYTES || in[20] != params->zone_count ||
        size < RR_HEAD_BYTES + params->zone_count * RR_ZONE_BYTES)
        return 0;
    struct rr_state read;
    rr_reset(&read, params);
    read.points[0] = get32(in);
    read.points[1] = get32(in + 4);
    read.score[0] = (rr_u16)(in[8] | in[9] << 8);
    read.score[1] = (rr_u16)(in[10] | in[11] << 8);
    read.end_time = get32(in + 12);
    read.entered = get32(in + 16);
    if (read.points[0] > params->target || read.points[1] > params->target) return 0;
    const rr_u8 *at = in + RR_HEAD_BYTES;
    for (rr_u32 z = 0; z < params->zone_count; ++z) {
        struct rr_zone *zone = &read.zones[z];
        zone->progress = get32(at);
        zone->capturing = at[4];
        zone->owner = at[5];
        zone->count[0] = at[6];
        zone->count[1] = at[7];
        zone->present[0] = get32(at + 8);
        zone->present[1] = get32(at + 12);
        for (rr_u32 i = 0; i < RR_PLAYERS; ++i) zone->marks[i] = get32(at + 16 + 4 * i);
        if (zone->progress > RR_FULL || zone->capturing > RR_NOBODY || zone->owner > RR_NOBODY ||
            zone->count[0] != popcount(zone->present[0]) || zone->count[1] != popcount(zone->present[1]) ||
            (zone->present[0] | zone->present[1]) >> RR_PLAYERS)
            return 0;
        at += RR_ZONE_BYTES;
    }
    *state = read;
    return (rr_u32)(at - in);
}
