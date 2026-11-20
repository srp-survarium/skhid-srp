/* Build-2777 native slot installation. Runs on the game's resource-callback
 * thread, between simulation updates. No player-entry event is emitted here. */
#include "late_join_runtime_026g.h"

_Static_assert(sizeof(void *) == 4, "native adapter is x86 only");

struct lj_bindings {
    void (*core)(void *core, void *slot);
    void (*physics)(void *physics, void *slot);
    void (*animations)(void *player, void *callbacks);
    void (*ready)(void *player);
    void *slot_callback;
};

#define PTR(at) (*(lj_u8 **)(at))
#define WORD(at) (*(lj_u32 *)(at))

/* Finalize each kit in roster order; later kits append without renumbering. */
void lj_native_animations(lj_u8 *table, void (*append)(void *, void *),
                          int (*less)(void *, void *)) {
    lj_u8 *vector = table + 0xe044, *head = table + 0xe034;
    lj_u8 **begin = (lj_u8 **)PTR(vector);
    lj_u32 first = (lj_u32)(PTR(vector + 4) - PTR(vector)) / 4;
    for (lj_u8 *node = PTR(head + 4); node != head;) {
        lj_u8 *entry = node - 12;
        lj_u32 id = WORD(entry + 8);
        if (id >= first || begin[id] != entry) append(vector, entry);
        if (PTR(node + 8)) {
            node = PTR(node + 8);
            while (PTR(node + 4)) node = PTR(node + 4);
        } else {
            lj_u8 *parent = PTR(node);
            while (node == PTR(parent + 8)) {
                node = parent;
                parent = PTR(parent);
            }
            if (PTR(node + 8) != parent) node = parent;
        }
    }
    lj_u32 count = (lj_u32)(PTR(vector + 4) - PTR(vector)) / 4;
    for (lj_u32 i = first + 1; i < count; ++i) {
        lj_u8 *entry = begin[i];
        lj_u32 j = i;
        while (j > first && less(entry, begin[j - 1])) {
            begin[j] = begin[j - 1];
            --j;
        }
        begin[j] = entry;
    }
    for (lj_u32 i = first; i < count; ++i) WORD(begin[i] + 8) = i;
    table[0x10050] = 1;
}

/* The slot just bound put `added` nodes at the front of the list the world
 * saves its game objects by (core+0x18914; bind_slot_reserved in the
 * assembly). The states the game keeps to roll back to (core+0x2FBC the
 * oldest, item+0xC0BAC the next) were saved without them: their game-objects
 * bytes (the buffer at item+0xBDF30: +4 the bytes, +8 the room, +12 the
 * size; made by 0x3246D0 with room for 0x2000, given the multithreaded
 * allocator since patch 98, so it may grow, which this shift does not) get
 * the new nodes' bytes in front, so a state loaded later reads in step.
 * Such a node is a grenade or a trap that nobody has touched, and each
 * saves itself as one zero byte (grenade_core::serialize 0x303DA0,
 * booby_trap_core::serialize 0x3052E0). A state with no room for them is
 * left as it is: the largest seen is 525 bytes of the 8192. */
static void shift_saved_objects(lj_u8 *core, lj_u32 added) {
    for (lj_u8 *item = PTR(core + 0x2fbc); item; item = PTR(item + 0xc0bac)) {
        lj_u8 *buffer = item + 0xbdf30, *bytes = PTR(buffer + 4);
        lj_u32 size = WORD(buffer + 12);
        if (!bytes || size > WORD(buffer + 8) || added > WORD(buffer + 8) - size) continue;
        for (lj_u32 i = size; i; --i) bytes[i - 1 + added] = bytes[i - 1];
        for (lj_u32 i = 0; i < added; ++i) bytes[i] = 0;
        WORD(buffer + 12) = size + added;
    }
}

int lj_native_install(struct lj_context *c, const struct lj_bindings *bind) {
    lj_u8 *nc = c->client, *pvp = c->world;
    lj_u8 *core = pvp + 0x1f8;
    if (!c->kit_mask || (c->kit_mask & ~7u)) return 0;
    for (lj_u32 i = 0; i < 3; ++i)
        if (!!c->players[i] != !!(c->kit_mask & (1u << i))) return 0;
    if (c->place >= 20 || WORD(nc + 0x1f1c64) != 2 || PTR(nc + 0x1f1c40) != pvp)
        return 0;
    lj_u8 *options = PTR(pvp + 0x124);
    lj_u8 *begin = PTR(core + 0x18458), *end = PTR(core + 0x1845c);
    lj_u8 *capacity = PTR(core + 0x18460), *statistics = PTR(core + 0x18fa8);
    if (!options || !begin || !statistics || options[0x577b5] != c->place ||
        end != begin + 48 * c->place || (lj_u32)capacity < (lj_u32)end + 48 ||
        WORD(options + 0x577b0) != c->seed)
        return 0;
    lj_u8 *profile = options + c->place * 0x45fc;
    if (!c->state->api.decode(c->wire, c->wire_size, profile, c->state->dictionary)) return 0;

    /* All operations that can reject the proposal precede slot publication.
     * Each query result already has a reference while its callback runs. */
    for (lj_u32 i = 0; i < 48; ++i) end[i] = 0;
    end[12] = end[13] = end[0x24] = 0xff;
    PTR(end + 0x10) = end;
    PTR(end + 0x14) = bind->slot_callback;
    WORD(end + 0x20) = 0xffffffff;
    end[0x2c] = (lj_u8)c->place;
    for (lj_u32 kit = 0; kit < 3; ++kit) {
        lj_u8 *player = c->players[kit];
        if (!player) continue;
        __atomic_add_fetch((lj_u32 *)(player + 0xd0), 1, __ATOMIC_RELAXED);
        PTR(end + kit * 4) = player;
        /* 0x2CBC60 passes this animation table to 0x2D8100; the PVP
         * constructor retains it at +0x19238. Spatial tree is +0x130. */
        bind->animations(player, PTR(pvp + 0x19238));
    }
    PTR(core + 0x1845c) = end + 48;
    options[0x5782c + c->place] = (lj_u8)c->squad;
    options[0x577b5] = (lj_u8)(c->place + 1);
    /* core stores statistics_handler; its shared_statistics subobject is +8
     * (native serialize thunk 0x26DB40), with the live count at +0x31E0. */
    statistics[8 + 0x31e0] = (lj_u8)(c->place + 1);
    lj_u8 *listed = PTR(core + 0x18914);
    bind->core(core, end);
    lj_u32 added = 0;
    for (lj_u8 *node = PTR(core + 0x18914); node && node != listed && added < 0x2000;
         node = PTR(node + 8))
        ++added;
    /* More nodes than a state has room for is no list of a slot's. */
    if (added && added < 0x2000) shift_saved_objects(core, added);
    bind->physics(pvp + 0x138, end);
    bind->ready(PTR(end + end[12] * 4));
    return 1;
}
