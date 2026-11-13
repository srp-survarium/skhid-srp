/* Ownership and admission preparation for the native client. Native adapters
 * are deliberately separate from this lifecycle: they must implement the ABI
 * contracts in the header. The native hooks live in late_join_026g.asm.
 */
#include "late_join_runtime_026g.h"

static void lock(struct lj_state *s) {
    while (__atomic_test_and_set(&s->lock, __ATOMIC_ACQUIRE))
        __asm__ volatile("pause");
}
static void unlock(struct lj_state *s) {
    __atomic_clear(&s->lock, __ATOMIC_RELEASE);
}
static lj_u32 u32(const lj_u8 *p) {
    return (lj_u32)p[0] | (lj_u32)p[1] << 8 | (lj_u32)p[2] << 16 | (lj_u32)p[3] << 24;
}
static int equal(const lj_u8 *a, const lj_u8 *b, lj_u32 n) {
    for (lj_u32 i = 0; i < n; ++i) if (a[i] != b[i]) return 0;
    return 1;
}
static int current(const struct lj_context *c) {
    const struct lj_state *s = c->state;
    return c->owned && s->client == c->client && s->world == c->world && s->seed == c->seed;
}
/* The lock protects both the list and destruction notifications. Once
 * detached, no caller can still own or discover this context. */
static struct lj_context *collect(struct lj_state *s) {
    struct lj_context *dead = 0, **link = &s->contexts;
    while (*link) {
        struct lj_context *c = *link;
        if (!c->owned && !c->pending && !c->players[0] && !c->players[1] && !c->players[2]) {
            *link = c->next;
            c->next = dead;
            dead = c;
        } else {
            link = &c->next;
        }
    }
    return dead;
}
static void free_list(struct lj_state *s, struct lj_context *dead) {
    while (dead) {
        struct lj_context *next = dead->next;
        s->api.free(dead);
        dead = next;
    }
}

void lj_deactivate(struct lj_state *s) {
    lock(s);
    s->client = s->world = s->dictionary = 0;
    s->seed = s->roster = 0;
    for (struct lj_context *c = s->contexts; c; c = c->next) c->owned = 0;
    struct lj_context *dead = collect(s);
    unlock(s);
    free_list(s, dead);
}

void lj_activate(struct lj_state *s, void *client, void *world, void *dictionary,
                 lj_u32 seed, lj_u32 roster) {
    lj_deactivate(s);
    s->client = client;
    s->world = world;
    s->dictionary = dictionary;
    s->seed = seed;
    s->roster = roster;
}

void lj_prepare(struct lj_state *s, const lj_u8 *packet, lj_u32 length) {
    if (!s->client || !s->world || !s->dictionary || !packet || length < 14 || length > 32782)
        return;
    if (packet[0] != 0xff || packet[1] != 'L' || packet[2] != 'J' || packet[3] != 1)
        return;
    lj_u32 seed = u32(packet + 4), serial = u32(packet + 8), place = packet[12], squad = packet[13];
    if (seed != s->seed || !serial || !place || place >= 20) return;
    const lj_u8 *body = packet + 14;
    lj_u32 size = length - 14;
    /* Game-thread ownership pins every current-match descriptor, including a
     * failed one: retransmission must not submit a different immutable body. */
    struct lj_context *found = 0;
    lock(s);
    for (struct lj_context *c = s->contexts; c; c = c->next) {
        if (current(c) && c->place == place) { found = c; break; }
    }
    unlock(s);
    if (found) {
        lj_u8 status = LJ_FAILED;
        if (found->squad == squad && found->wire_size == size && equal(found->wire, body, size)) {
            if (serial > found->serial) found->serial = serial;
            status = (lj_u8)found->status;
        }
        s->api.ack(s->client, seed, serial, (lj_u8)place, status);
        return;
    }
    /* A solo identity belongs to exactly this place, in no squad. */
    if (place != s->roster || (size && body[0] >= 4 && body[0] < 24 && (body[0] != 4 + place || squad != 0))) {
        s->api.ack(s->client, seed, serial, (lj_u8)place, LJ_FAILED);
        return;
    }
    struct lj_context *c = s->api.allocate((lj_u32)sizeof(*c) + size);
    if (!c) {
        s->api.ack(s->client, seed, serial, (lj_u8)place, LJ_BUSY);
        return;
    }
    c->state = s;
    c->client = s->client;
    c->world = s->world;
    c->seed = seed;
    c->serial = serial;
    c->place = place;
    c->squad = squad;
    c->pending = c->owned = 1;
    c->status = LJ_BUSY;
    c->kit_mask = 0;
    c->wire_size = size;
    for (lj_u32 i = 0; i < size; ++i) c->wire[i] = body[i];
    for (lj_u32 i = 0; i < 3; ++i) c->players[i] = 0;
    if (!s->api.decode(c->wire, size, c->profile, s->dictionary)) {
        s->api.free(c);
        s->api.ack(s->client, seed, serial, (lj_u8)place, LJ_FAILED);
        return;
    }
    const lj_u8 *profile = (const lj_u8 *)c->profile;
    for (lj_u32 i = 0; i < profile[0x57]; ++i)
        if (!u32(profile + 0x758 + 0x704 * i)) c->kit_mask |= 1u << i;
    s->api.retain_world(c->world);
    lock(s);
    c->next = s->contexts;
    s->contexts = c;
    unlock(s);
    /* The query is allowed to invoke completion immediately. Do not touch c
     * afterwards: cancellation plus immediate failure can already free it. */
    s->api.query(c);
}

void lj_complete(struct lj_context *c, void *const players[3]) {
    struct lj_state *s = c->state;
    void *world = c->world;
    int success = 1;
    lock(s);
    for (lj_u32 i = 0; i < 3; ++i) {
        c->players[i] = players[i];
        if (!!players[i] != !!(c->kit_mask & (1u << i))) success = 0;
    }
    unlock(s);
    if (current(c)) {
        success = success && c->place == s->roster && s->api.install(c);
        c->status = success ? LJ_INSTALLED : LJ_FAILED;
        if (success) ++s->roster;
        s->api.ack(c->client, c->seed, c->serial, (lj_u8)c->place, (lj_u8)c->status);
    }
    lock(s);
    c->pending = 0;
    struct lj_context *dead = collect(s);
    unlock(s);
    free_list(s, dead);
    /* Releasing the old world may synchronously destroy players and free c.
     * The context and its profile are never accessed after this call. */
    s->api.release_world(world);
}

void lj_player_destroyed(struct lj_state *s, void *player, const void *profile) {
    lock(s);
    for (struct lj_context *c = s->contexts; c; c = c->next) {
        if ((const void *)c->profile != profile) continue;
        for (lj_u32 i = 0; i < 3; ++i) if (c->players[i] == player) c->players[i] = 0;
        break;
    }
    struct lj_context *dead = collect(s);
    unlock(s);
    free_list(s, dead);
}
