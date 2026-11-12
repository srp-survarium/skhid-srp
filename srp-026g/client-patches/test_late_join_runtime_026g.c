/* Host tests for the asynchronous lifecycle; native ABI adapters are tested
 * separately. Run with AddressSanitizer/UBSan to check callback ownership. */
#include "late_join_runtime_026g.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <pthread.h>

static struct lj_state state;
static struct lj_context *query;
static unsigned allocations, frees, queries, retains, releases, installs, acks;
static unsigned last_serial, last_status;
static int allocation_fails, install_fails, query_mode;
static int client, world, dictionary, other_world, player_a, player_b;
static void *completed[3] = {&player_a, 0, &player_b};
static const void *destroy_on_release;

static void *allocate(lj_u32 size) {
    if (allocation_fails) return 0;
    ++allocations;
    return malloc(size);
}
static void release_memory(void *p) { ++frees; free(p); }
static int decode(const lj_u8 *wire, lj_u32 size, void *profile, void *dict) {
    assert(dict == &dictionary);
    if (size != 1 || wire[0] != 0x42) return 0;
    memset(profile, 0, 0x45fc);
    ((lj_u8 *)profile)[0x57] = 3;
    ((lj_u8 *)profile)[0x758 + 0x704] = 2;
    ((lj_u8 *)profile)[8] = 0x42;
    return 1;
}
static void retain_world(void *p) { assert(p == &world || p == &other_world); ++retains; }
static void release_world(void *p) {
    assert(p == &world || p == &other_world);
    ++releases;
    if (destroy_on_release) {
        const void *profile = destroy_on_release;
        destroy_on_release = 0;
        lj_player_destroyed(&state, &player_a, profile);
        lj_player_destroyed(&state, &player_b, profile);
    }
}
static void start_query(struct lj_context *c) {
    ++queries;
    query = c;
    assert(c->kit_mask == 5);
    assert(((lj_u8 *)c->profile)[8] == 0x42);
    if (query_mode == 1) lj_complete(c, completed);
    if (query_mode == 2) {
        void *none[3] = {0, 0, 0};
        lj_deactivate(&state);
        lj_complete(c, none); /* Can free c before returning from query. */
    }
}
static int install(struct lj_context *c) {
    ++installs;
    assert(c->pending && c->owned && c->world == state.world);
    assert(c->place == state.roster && c->kit_mask == 5);
    assert(c->players[0] == &player_a && c->players[2] == &player_b);
    assert(((lj_u8 *)c->profile)[8] == 0x42);
    return !install_fails;
}
static void ack(void *nc, lj_u32 seed, lj_u32 serial, lj_u8 place, lj_u8 status) {
    assert(nc == &client && seed == state.seed && place > 0 && place < 20);
    ++acks;
    last_serial = serial;
    last_status = status;
}
static void reset(void) {
    assert(allocations == frees && retains == releases);
    memset(&state, 0, sizeof(state));
    state.api = (struct lj_api){allocate, release_memory, decode, retain_world,
        release_world, start_query, install, ack};
    allocations = frees = queries = retains = releases = installs = acks = 0;
    last_serial = last_status = 0;
    allocation_fails = install_fails = query_mode = 0;
    destroy_on_release = 0;
    query = 0;
    lj_activate(&state, &client, &world, &dictionary, 0x80000001, 2);
}
static void packet(lj_u8 bytes[15], unsigned serial) {
    const lj_u8 prefix[15] = {255, 'L', 'J', 1, 1, 0, 0, 128, 0, 0, 0, 0, 2, 4, 0x42};
    memcpy(bytes, prefix, 15);
    bytes[8] = serial;
}
static void successful_load(void) {
    reset();
    lj_u8 bytes[15]; packet(bytes, 1);
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(queries == 1 && retains == 1 && acks == 0);
    assert(query->wire[0] == 0x42 && query->squad == 4);
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(queries == 1 && last_status == LJ_BUSY);
    bytes[14] = 0x43;
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(queries == 1 && last_status == LJ_FAILED && query->wire[0] == 0x42);
    packet(bytes, 3);
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(query->serial == 3);
    packet(bytes, 2); /* Old proposals cannot replace the newest callback ACK. */
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(query->serial == 3);
    lj_complete(query, completed);
    assert(installs == 1 && state.roster == 3 && releases == 1);
    assert(last_serial == 3 && last_status == LJ_INSTALLED);
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(last_serial == 2 && last_status == LJ_INSTALLED && queries == 1);
    const void *profile = query->profile;
    lj_deactivate(&state);
    assert(frees == 0);
    lj_player_destroyed(&state, &player_a, profile);
    assert(frees == 0 && ((const lj_u8 *)profile)[8] == 0x42);
    lj_player_destroyed(&state, &player_b, profile);
    assert(frees == 1 && !state.contexts);
}
static void cancelled_load(void) {
    reset();
    lj_u8 bytes[15]; packet(bytes, 1);
    lj_prepare(&state, bytes, sizeof(bytes));
    const void *profile = query->profile;
    lj_activate(&state, &client, &other_world, &dictionary, 0x80000002, 2);
    assert(!query->owned && frees == 0 && releases == 0);
    lj_complete(query, completed);
    assert(installs == 0 && acks == 0 && state.roster == 2 && releases == 1);
    lj_player_destroyed(&state, &player_b, profile);
    assert(frees == 0);
    lj_player_destroyed(&state, &player_a, profile);
    assert(frees == 1);
    lj_deactivate(&state);
}
static void partial_failure(void) {
    reset();
    lj_u8 bytes[15]; packet(bytes, 1);
    lj_prepare(&state, bytes, sizeof(bytes));
    const void *profile = query->profile;
    void *partial[3] = {&player_a, 0, 0};
    lj_complete(query, partial);
    assert(installs == 0 && last_status == LJ_FAILED && state.roster == 2);
    lj_player_destroyed(&state, &player_a, profile);
    assert(frees == 0); /* Keep failed proposal immutable until match retirement. */
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(queries == 1 && last_status == LJ_FAILED);
    lj_deactivate(&state);
    assert(frees == 1);
}
static void rejected_install(void) {
    reset();
    install_fails = 1;
    lj_u8 bytes[15]; packet(bytes, 1);
    lj_prepare(&state, bytes, sizeof(bytes));
    const void *profile = query->profile;
    lj_complete(query, completed);
    assert(installs == 1 && last_status == LJ_FAILED && state.roster == 2);
    lj_player_destroyed(&state, &player_a, profile);
    lj_player_destroyed(&state, &player_b, profile);
    lj_deactivate(&state);
}
static void bad_requests(void) {
    reset();
    lj_u8 bytes[15]; packet(bytes, 1);
    for (unsigned n = 0; n < 14; ++n) lj_prepare(&state, bytes, n);
    assert(!queries && !acks && !allocations);
    bytes[4] = 2;
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(!acks);
    packet(bytes, 1); bytes[12] = 3;
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(last_status == LJ_FAILED && !allocations);
    packet(bytes, 1); bytes[14] = 0x43;
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(last_status == LJ_FAILED && allocations == frees && !retains);
    allocation_fails = 1;
    packet(bytes, 1);
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(last_status == LJ_BUSY && !queries);
    lj_deactivate(&state);
}
static void immediate_callbacks(void) {
    reset();
    query_mode = 1;
    lj_u8 bytes[15]; packet(bytes, 1);
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(installs == 1 && last_status == LJ_INSTALLED);
    const void *profile = query->profile;
    lj_deactivate(&state);
    lj_player_destroyed(&state, &player_a, profile);
    lj_player_destroyed(&state, &player_b, profile);
    reset();
    query_mode = 2;
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(frees == 1 && releases == 1 && acks == 0 && installs == 0);
    reset();
    lj_prepare(&state, bytes, sizeof(bytes));
    destroy_on_release = query->profile;
    lj_deactivate(&state);
    lj_complete(query, completed);
    assert(frees == 1 && releases == 1 && installs == 0);
}

struct destruction { void *player; const void *profile; };
static void *destroy_thread(void *arg) {
    struct destruction *d = arg;
    lj_player_destroyed(&state, d->player, d->profile);
    return 0;
}
static void concurrent_destruction(void) {
    for (unsigned i = 0; i < 50; ++i) {
        reset();
        lj_u8 bytes[15]; packet(bytes, 1);
        lj_prepare(&state, bytes, sizeof(bytes));
        lj_complete(query, completed);
        struct destruction a = {&player_a, query->profile}, b = {&player_b, query->profile};
        pthread_t first, second;
        assert(!pthread_create(&first, 0, destroy_thread, &a));
        assert(!pthread_create(&second, 0, destroy_thread, &b));
        lj_deactivate(&state);
        assert(!pthread_join(first, 0) && !pthread_join(second, 0));
        assert(frees == 1 && !state.contexts);
    }
}

static void wrong_solo_identity(void) {
    reset();
    lj_u8 bytes[15]; packet(bytes, 1);
    bytes[13] = 0;
    bytes[14] = 7; /* Place 2 must have identity 6, never another player's. */
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(last_status == LJ_FAILED && allocations == 0 && queries == 0);
    bytes[14] = 6;
    bytes[13] = 1; /* No squads in FFA. */
    lj_prepare(&state, bytes, sizeof(bytes));
    assert(last_status == LJ_FAILED && allocations == 0 && queries == 0);
}

int main(void) {
    wrong_solo_identity();
    successful_load();
    cancelled_load();
    partial_failure();
    rejected_install();
    bad_requests();
    immediate_callbacks();
    concurrent_destruction();
    assert(allocations == frees && retains == releases);
    puts("PASS: asynchronous late-join ownership, cancellation, retries and cleanup");
    return 0;
}
