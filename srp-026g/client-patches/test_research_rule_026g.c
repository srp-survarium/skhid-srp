/* Tests of Research's rules (research_rule_026g.c) on the host:
 *
 *   cc -std=c11 -g -O1 -Wall -Wextra -Werror -fsanitize=address,undefined \
 *     research_rule_026g.c test_research_rule_026g.c -o /tmp/srp-research-test && /tmp/srp-research-test
 *
 * Each case is a rule of docs/notes/research_mode.md. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "research_rule_026g.h"

static int failures;
#define CHECK(condition)                                                                                         \
    do {                                                                                                         \
        if (!(condition)) {                                                                                      \
            fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #condition);                                      \
            ++failures;                                                                                          \
        }                                                                                                        \
    } while (0)

/* 0.27d's parameters, units A, B, C: a 5 m cylinder, a 4 x 2 x 3 box turned a quarter, a 6 m sphere. */
static struct rr_params params_027d(void) {
    struct rr_params p;
    memset(&p, 0, sizeof p);
    static const unsigned capture[8] = {20000, 18000, 15000, 14899, 14800, 14699, 14600, 14500};
    p.capture_count = 8;
    memcpy(p.capture_ms, capture, sizeof capture);
    p.owned_count = 3;
    p.points_rate[0] = 75;
    p.points_rate[1] = 150;
    p.points_rate[2] = 350;
    p.target = 100 * RR_MICRO;
    p.award = 100;
    p.reach = 0.4f;
    p.half_height = 0.9f;
    p.zone_count = 3;
    for (int z = 0; z < 3; ++z) {
        struct rr_zone_def *d = &p.zones[z];
        d->cp_id = (rr_u32)z;
        d->shape_count = 1;
        d->position[0] = 100.0f * (float)z;
        struct rr_shape *s = &d->shapes[0];
        s->centre[0] = 100.0f * (float)z;
        s->type = z == 0 ? RR_CYLINDER : z == 1 ? RR_BOX : RR_SPHERE;
        for (int i = 0; i < 3; ++i) s->axes[i][i] = 1;
        if (z == 1) {
            /* turned 90 degrees about y: local x is world z */
            memset(s->axes, 0, sizeof s->axes);
            s->axes[0][2] = 1;
            s->axes[1][1] = 1;
            s->axes[2][0] = -1;
        }
        s->half[0] = z == 0 ? 5.0f : z == 1 ? 4.0f : 6.0f;
        s->half[1] = z == 1 ? 2.0f : 1.8f;
        s->half[2] = z == 1 ? 3.0f : 5.0f;
    }
    CHECK(rr_prepare(&p, 500));
    return p;
}

static struct rr_player players[RR_PLAYERS];
static struct rr_award awards[RR_MAX_AWARDS];

static void place(int i, int team, int zone) {
    players[i].alive = 1;
    players[i].team = (rr_u8)team;
    players[i].position[0] = zone < 0 ? 1000.0f : 100.0f * (float)zone;
    players[i].position[1] = 0.5f;
    players[i].position[2] = 0;
}

static void away(int i) { players[i].position[0] = 1000.0f; }

/* Ticks of 50 ms for `ms`; returns the awards of all of them in `all` (count). */
static rr_u32 run(struct rr_state *s, const struct rr_params *p, rr_u32 ms, rr_u32 *time, struct rr_award *all) {
    rr_u32 n = 0;
    for (rr_u32 t = 0; t < ms; t += 50) {
        *time += 50;
        rr_u32 got = rr_tick(s, p, players, 50, *time, awards);
        for (rr_u32 i = 0; i < got && all; ++i) all[n++] = awards[i];
        if (!all) n += got;
    }
    return n;
}

static void fresh(struct rr_state *s, const struct rr_params *p) {
    rr_reset(s, p);
    memset(players, 0, sizeof players);
    for (rr_u32 i = 0; i < RR_PLAYERS; ++i) {
        rr_entered(s, (rr_u8)i);
        players[i].team = 2;
        away(i);
    }
}

static void test_inside(void) {
    struct rr_params p = params_027d();
    float in_cylinder[3] = {5.3f, 2.6f, 0}, out_cylinder[3] = {5.5f, 0, 0}, above[3] = {0, 2.8f, 0};
    CHECK(rr_inside(&p, 0, in_cylinder));
    CHECK(!rr_inside(&p, 0, out_cylinder));
    CHECK(!rr_inside(&p, 0, above));
    /* the box: 4 m along world z, 3 m along world x */
    float box_z[3] = {100, 0, 4.3f}, box_x[3] = {103.3f, 0, 0}, box_far_x[3] = {103.5f, 0, 0};
    CHECK(rr_inside(&p, 1, box_z));
    CHECK(rr_inside(&p, 1, box_x));
    CHECK(!rr_inside(&p, 1, box_far_x));
    /* the sphere: the capsule's segment reaches down to it */
    float over[3] = {200, 7.2f, 0}, high[3] = {200, 7.4f, 0}, side[3] = {206.3f, 0, 0};
    CHECK(rr_inside(&p, 2, over));
    CHECK(!rr_inside(&p, 2, high));
    CHECK(rr_inside(&p, 2, side));
}

static void test_capture_alone(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    struct rr_award all[64];
    rr_u32 time = 0;
    fresh(&s, &p);
    place(3, 0, 0);
    rr_u32 n = run(&s, &p, 19950, &time, all);
    CHECK(n == 0 && s.zones[0].owner == RR_NOBODY && s.zones[0].capturing == 0);
    n = run(&s, &p, 100, &time, all);
    CHECK(s.zones[0].owner == 0);
    CHECK(n == 1 && all[0].kind == RR_AWARD_CAPTURE && all[0].player == 3 && all[0].amount == 100);
    CHECK(s.zones[0].count[0] == 1 && s.zones[0].present[0] == 1u << 3);
}

static void test_capture_by_two(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    struct rr_award all[64];
    rr_u32 time = 0;
    fresh(&s, &p);
    place(0, 1, 0);
    place(1, 1, 0);
    run(&s, &p, 17950, &time, all);
    CHECK(s.zones[0].owner == RR_NOBODY);
    rr_u32 n = run(&s, &p, 100, &time, all);
    CHECK(s.zones[0].owner == 1);
    CHECK(n == 2 && all[0].amount == 50 && all[1].amount == 50);
}

static void test_contested(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    fresh(&s, &p);
    place(0, 0, 0);
    run(&s, &p, 5000, &time, 0);
    rr_u32 progress = s.zones[0].progress;
    place(1, 1, 0);
    run(&s, &p, 5000, &time, 0);
    CHECK(s.zones[0].progress == progress && s.zones[0].capturing == 0);
}

static void test_fall_back(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    fresh(&s, &p);
    place(0, 0, 0);
    run(&s, &p, 10000, &time, 0); /* half */
    CHECK(s.zones[0].progress > RR_FULL / 2 - RR_FULL / 100 && s.zones[0].progress <= RR_FULL / 2);
    away(0);
    run(&s, &p, 10000, &time, 0); /* half speed: a quarter back */
    CHECK(s.zones[0].progress > RR_FULL / 4 - RR_FULL / 100 && s.zones[0].progress < RR_FULL / 4 + RR_FULL / 100);
    run(&s, &p, 30000, &time, 0);
    CHECK(s.zones[0].progress == 0);
}

static void test_drain_then_take(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    struct rr_award all[64];
    rr_u32 time = 0;
    fresh(&s, &p);
    place(0, 0, 0);
    run(&s, &p, 20050, &time, all);
    CHECK(s.zones[0].owner == 0);
    away(0);
    place(5, 1, 0);
    rr_u32 n = run(&s, &p, 19950, &time, all);
    CHECK(n == 0 && s.zones[0].owner == 0 && s.zones[0].progress < RR_FULL / 100);
    n = run(&s, &p, 100, &time, all);
    CHECK(s.zones[0].owner == RR_NOBODY && s.zones[0].capturing == 1);
    CHECK(n == 1 && all[0].kind == RR_AWARD_NEUTRALISE && all[0].player == 5 && all[0].amount == 100);
    n = run(&s, &p, 20000, &time, all);
    CHECK(s.zones[0].owner == 1 && n == 1 && all[0].kind == RR_AWARD_CAPTURE && all[0].amount == 100);
}

static void test_owner_refills(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    fresh(&s, &p);
    place(0, 0, 0);
    run(&s, &p, 20050, &time, 0);
    away(0);
    place(1, 1, 0);
    run(&s, &p, 5000, &time, 0);
    away(1);
    rr_u32 drained = s.zones[0].progress;
    run(&s, &p, 30000, &time, 0); /* owned: nothing falls or fills with nobody */
    CHECK(s.zones[0].owner == 0 && s.zones[0].progress == drained);
    place(0, 0, 0);
    run(&s, &p, 5050, &time, 0);
    CHECK(s.zones[0].owner == 0 && s.zones[0].progress == RR_FULL);
}

static void test_water_fill(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    struct rr_award all[64];
    rr_u32 time = 0;
    fresh(&s, &p);
    place(0, 0, 0);
    /* alone to a third, then a second player comes: 1/3 of the capture is left at 2 players */
    run(&s, &p, 6000, &time, all);
    rr_u32 left = RR_FULL - s.zones[0].progress;
    place(1, 0, 0);
    rr_u32 n = run(&s, &p, 20000, &time, all);
    CHECK(s.zones[0].owner == 0 && n == 2);
    /* the second's mark is what was left when it came; the first one's the whole capture */
    double second = 100.0 * left / RR_FULL / 2, first = 100.0 - second;
    CHECK(all[0].player == 0 && all[0].amount == (rr_u32)(first + 0.5));
    CHECK(all[1].player == 1 && all[1].amount == (rr_u32)(second + 0.5));
}

static void test_points_and_end(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    fresh(&s, &p);
    /* team 1 owns all three units */
    for (int z = 0; z < 3; ++z) {
        s.zones[z].owner = 1;
        s.zones[z].capturing = 1;
        s.zones[z].progress = RR_FULL;
    }
    run(&s, &p, 100000, &time, 0);
    CHECK(s.points[1] == 100000u * 350 && s.score[1] == 35 && s.score[0] == 0);
    CHECK(rr_leader(&s) == 1);
    run(&s, &p, 185700, &time, 0); /* 285.7 s */
    CHECK(s.end_time == RR_NEVER);
    run(&s, &p, 50, &time, 0); /* 285.75 s: 100 points */
    CHECK(s.points[1] == p.target && s.score[1] == 100 && s.end_time == time + 3000);
    CHECK(!rr_completed(&s, time + 2999) && rr_completed(&s, time + 3000));
    rr_u32 end = s.end_time;
    run(&s, &p, 5000, &time, 0);
    CHECK(s.end_time == end);
    /* nothing moves after the end: a unit stays as it was */
    place(0, 0, 0);
    rr_u32 before = s.zones[0].progress;
    run(&s, &p, 5000, &time, 0);
    CHECK(s.zones[0].progress == before && s.zones[0].owner == 1);
}

static void test_one_unit_rate(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    fresh(&s, &p);
    s.zones[1].owner = 0;
    s.zones[1].capturing = 0;
    s.zones[1].progress = RR_FULL;
    run(&s, &p, 60000, &time, 0);
    CHECK(s.points[0] == 60000u * 75 && s.score[0] == 5 && rr_leader(&s) == 0);
    s.zones[2].owner = 1;
    s.zones[2].capturing = 1;
    s.zones[2].progress = RR_FULL;
    run(&s, &p, 60000, &time, 0);
    CHECK(s.score[0] == 9 && s.score[1] == 5);
}

static void test_kill_invader(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    struct rr_award a;
    fresh(&s, &p);
    place(2, 1, 0);
    run(&s, &p, 1000, &time, 0);
    CHECK(rr_kill(&s, 7, 2, 0, 1, &a) == 1 && a.kind == RR_AWARD_KILL_INVADER && a.player == 7 && a.amount == 1);
    CHECK(rr_kill(&s, 7, 3, 0, 1, &a) == 0);  /* not inside */
    CHECK(rr_kill(&s, 8, 2, 1, 1, &a) == 0);  /* a teammate */
    CHECK(rr_kill(&s, 2, 2, 1, 1, &a) == 0);  /* himself */
    run(&s, &p, 20000, &time, 0);
    CHECK(s.zones[0].owner == 1);
    CHECK(rr_kill(&s, 7, 2, 0, 1, &a) == 0);  /* inside his own unit */
}

static void test_left_and_dead(void) {
    struct rr_params p = params_027d();
    struct rr_state s;
    rr_u32 time = 0;
    fresh(&s, &p);
    place(4, 0, 0);
    run(&s, &p, 1000, &time, 0);
    CHECK(s.zones[0].count[0] == 1);
    rr_left(&s, 4);
    CHECK(s.zones[0].count[0] == 0 && !(s.entered >> 4 & 1));
    run(&s, &p, 1000, &time, 0);
    CHECK(s.zones[0].count[0] == 0); /* a place that left is not read */
    rr_entered(&s, 4);
    players[4].alive = 0;
    run(&s, &p, 1000, &time, 0);
    CHECK(s.zones[0].count[0] == 0);
}

static void test_wire(void) {
    struct rr_params p = params_027d();
    struct rr_state s, back;
    rr_u8 bytes[1024];
    rr_u32 time = 0;
    fresh(&s, &p);
    place(1, 0, 0);
    place(2, 1, 1);
    run(&s, &p, 7000, &time, 0);
    CHECK(rr_serialized_size(&s) == 21 + 3 * 96);
    CHECK(rr_write(&s, bytes) == 309);
    CHECK(bytes[20] == 3);
    CHECK(rr_read(&back, &p, bytes, 309) == 309);
    CHECK(memcmp(&back, &s, sizeof s) == 0);
    /* a tick from the loaded state gives the same as from the saved one */
    struct rr_award a1[RR_MAX_AWARDS], a2[RR_MAX_AWARDS];
    rr_tick(&s, &p, players, 50, time + 50, a1);
    rr_tick(&back, &p, players, 50, time + 50, a2);
    CHECK(memcmp(&back, &s, sizeof s) == 0);
    rr_u8 bad[1024];
    memcpy(bad, bytes, 309);
    bad[21 + 4] = 3; /* a team 3 */
    CHECK(rr_read(&back, &p, bad, 309) == 0);
    memcpy(bad, bytes, 309);
    bad[21 + 6] = 2; /* a count unlike its mask */
    CHECK(rr_read(&back, &p, bad, 309) == 0);
    CHECK(rr_read(&back, &p, bytes, 308) == 0);
}

static void test_prepare(void) {
    struct rr_params p = params_027d();
    CHECK(p.capture_rate[0] == RR_FULL / 20000 && p.restore_rate == RR_FULL / 20000 / 2);
    p.zone_count = 4;
    CHECK(!rr_prepare(&p, 500));
    p = params_027d();
    p.capture_ms[2] = 0;
    CHECK(!rr_prepare(&p, 500));
}

int main(void) {
    test_inside();
    test_capture_alone();
    test_capture_by_two();
    test_contested();
    test_fall_back();
    test_drain_then_take();
    test_owner_refills();
    test_water_fill();
    test_points_and_end();
    test_one_unit_rate();
    test_kill_invader();
    test_left_and_dead();
    test_wire();
    test_prepare();
    if (failures) {
        fprintf(stderr, "%d failures\n", failures);
        return 1;
    }
    puts("research rule: all cases pass");
    return 0;
}
