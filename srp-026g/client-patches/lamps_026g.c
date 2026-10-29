/* Breakable map lamps, build 2777. See docs/notes/breakable_lamps.md.
 * The project owns every binding. Bullet workers only change Intact -> Broken;
 * history saves/restores those states. Rendering changes on the game thread.
 * No resource reference, collision object or renderer list is edited by hand. */
#include "engine_026g.h"

typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;
#define THISCALL __attribute__((thiscall))
#define MAX_LAMPS 256
#define MAX_VIEWS 1024
#define MAGIC 0x31504d4cu /* LMP1 */

struct engine_cfg { u32 data, high, name, name_high, crc; u16 type, count; };
struct vec { float x[3]; };
struct box { struct vec low, high; };
enum condition { INTACT, BROKEN };
enum appearance { DETACHED, HIDDEN, SHOWN };
enum event { INSERT, REMOVE, PRESENT };
enum action { KEEP, ADD, DROP };
enum kind { VISUAL, LIGHT };
struct lamp { struct box hit; enum condition condition; };
struct view {
    void *object;
    u32 *original;
    u32 lamp;
    enum condition visible_when;
    enum kind kind;
    enum appearance appearance;
};
struct project {
    void *object, *scene, *physics;
    u32 count, views, break_material;
    struct lamp lamps[MAX_LAMPS];
    struct view view[MAX_VIEWS];
};
static struct {
    enum { EMPTY, LOADED } tag;
    union { struct project project; } data;
} owner;

extern u32 bl_visual_vtable[], bl_light_vtable[]; /* RTTI, then native-shaped slots */

static void *pointer(const void *p, u32 offset) { return *(void *const *)((const u8 *)p + offset); }
static u32 word(const void *p, u32 offset) { return *(const u32 *)((const u8 *)p + offset); }
static struct vec vector(const void *p) { return *(const struct vec *)p; }
static int finite(float f) { return f >= -3.402823466e38f && f <= 3.402823466e38f; }
static void require(int ok) { if (!ok) __builtin_trap(); }
static const struct engine_cfg *child(const struct engine_cfg *c, const char *key) {
    require(c && c->type == 3 && engine_cfg_has(c, key));
    return engine_cfg_child(c, key);
}
static u32 integer(const struct engine_cfg *c, const char *key) {
    c = child(c, key); require(c->type == 1); return c->data;
}
static float number(const struct engine_cfg *c, const char *key) {
    c = child(c, key);
    require(c->type == 2 || c->type == 1);
    float f = c->type == 2 ? *(const float *)&c->data : (float)(int)c->data;
    require(finite(f)); return f;
}
static const struct engine_cfg *entries(const struct engine_cfg *c) {
    require(c->type == 3 || c->type == 4);
    return (const struct engine_cfg *)c->data;
}
static struct project *project(void) { return owner.tag == LOADED ? &owner.data.project : 0; }
static enum condition condition(const struct lamp *lamp) {
    return __atomic_load_n(&lamp->condition, __ATOMIC_RELAXED);
}

/* The one presentation transition: Detached has no renderer membership;
 * Hidden is attached to the scene but absent from the renderer. */
static enum action transition(enum appearance *state, enum event event, int visible) {
    enum appearance next = *state;
    switch (event) {
    case REMOVE: next = DETACHED; break;
    case INSERT: next = visible ? SHOWN : HIDDEN; break;
    case PRESENT: if (*state != DETACHED) next = visible ? SHOWN : HIDDEN; break;
    }
    enum action action = *state == SHOWN ? (next == SHOWN ? KEEP : DROP)
                                        : (next == SHOWN ? ADD : KEEP);
    *state = next;
    return action;
}
static struct view *view_of(void *object) {
    struct project *p = project();
    if (p) for (u32 i = 0; i < p->views; ++i) if (p->view[i].object == object) return &p->view[i];
    __builtin_trap(); /* only bound objects carry our vtables */
}
static void drive(struct view *v, enum event event) {
    struct project *p = project();
    enum action a = transition(&v->appearance, event, condition(&p->lamps[v->lamp]) == v->visible_when);
    if (v->kind == VISUAL) {
        if (a == ADD) engine_visual_insert(v->object, p->scene);
        if (a == DROP) engine_visual_remove(v->object, p->scene);
    } else {
        if (a == ADD) engine_light_insert(v->object);
        if (a == DROP) engine_light_remove(v->object);
    }
}
void THISCALL bl_visual_insert(void *object, void *scene) {
    require(scene == project()->scene); drive(view_of(object), INSERT);
}
void THISCALL bl_visual_remove(void *object, void *scene) {
    require(scene == project()->scene); drive(view_of(object), REMOVE);
}
void THISCALL bl_light_insert(void *object) { drive(view_of(object), INSERT); }
void THISCALL bl_light_remove(void *object) { drive(view_of(object), REMOVE); }

static void bind_view(struct project *p, void *object, u32 lamp, enum kind kind, enum condition when) {
    require(object && p->views < MAX_VIEWS);
    for (u32 i = 0; i < p->views; ++i) require(p->view[i].object != object);
    struct view *v = &p->view[p->views++];
    *v = (struct view){ object, *(u32 **)object, lamp, when, kind, DETACHED };
    require(v->original == (u32 *)(kind == VISUAL ? 0xa029b4u : 0xa0adecu));
    *(u32 **)object = (kind == VISUAL ? bl_visual_vtable : bl_light_vtable) + 1;
}
void bl_detach(void *object) {
    struct project *p = project();
    if (!p || p->object != object) return;
    for (u32 i = 0; i < p->views; ++i) {
        struct view *v = &p->view[i];
        drive(v, REMOVE);
        *(u32 **)v->object = v->original;
    }
    owner.tag = EMPTY;
}
void bl_bind(void *object, void *physics) {
    struct project *p = project();
    if (p && p->object == object) return;
    const struct engine_cfg *root = pointer(pointer(object, 0x17c), 0x108);
    if (!engine_cfg_has(root, "srp_lamps")) return;
    /* Unconfigured scenery can coexist. A second lamp project must not
     * steal the active match's bindings. */
    require(!p && physics);
    const struct engine_cfg *spec = child(root, "srp_lamps");
    require(integer(spec, "version") == 2);
    u32 break_material = integer(spec, "break_material");
    require(break_material < 128);
    const struct engine_cfg *list = child(spec, "lamps"), *rows = entries(list);
    require(list->count && list->count <= MAX_LAMPS);
    owner.tag = LOADED;
    p = project();
    p->object = object; p->scene = pointer(object, 0x180);
    /* insert(physics_world, game_world_core) supplies the same world as
     * bullet_manager +98h. Scene +98h is its camera_director, not physics. */
    p->physics = physics;
    p->count = list->count; p->views = 0; p->break_material = break_material;
    u8 *visuals = pointer(object, 0x1e8);
    u32 visual_count = word(object, 0x1ec);
    void **objects = pointer(object, 0x184), **end = pointer(object, 0x188);
    for (u32 i = 0; i < p->count; ++i) {
        const struct engine_cfg *row = &rows[i];
        struct lamp *lamp = &p->lamps[i];
        lamp->condition = INTACT;
        const char *low[] = {"min_x", "min_y", "min_z"};
        const char *high[] = {"max_x", "max_y", "max_z"};
        for (u32 k = 0; k < 3; ++k) {
            lamp->hit.low.x[k] = number(row, low[k]); lamp->hit.high.x[k] = number(row, high[k]);
            require(lamp->hit.low.x[k] < lamp->hit.high.x[k]);
        }
        const struct engine_cfg *vs = child(row, "visuals"), *ve = entries(vs);
        for (u32 j = 0; j < vs->count; ++j) {
            u32 index = integer(&ve[j], "index"), when = integer(&ve[j], "state");
            require(index < visual_count && when <= BROKEN);
            bind_view(p, visuals + index * 0x48, i, VISUAL, (enum condition)when);
        }
        const struct engine_cfg *ls = child(row, "lights"), *le = entries(ls);
        for (u32 j = 0; j < ls->count; ++j) {
            struct vec pos = {{number(&le[j], "x"), number(&le[j], "y"), number(&le[j], "z")}};
            u32 found = 0;
            for (void **o = objects; o != end; ++o) {
                if (word(*o, 0) != 0xa0adec) continue;
                struct vec at = vector((u8 *)*o + 0x140);
                if (at.x[0] == pos.x[0] && at.x[1] == pos.x[1] && at.x[2] == pos.x[2]) {
                    bind_view(p, *o, i, LIGHT, INTACT); ++found;
                }
            }
            require(found == 1);
        }
    }
}

void bl_present(void) {
    struct project *p = project();
    if (p) for (u32 i = 0; i < p->views; ++i) drive(&p->view[i], PRESENT);
}

/* Slab intersection of the actual bullet segment with the authored bulb
 * bounds. Geometry beyond the native hit (wall, player, etc.) is excluded. */
static int intersects(struct box b, struct vec start, struct vec direction, float length) {
    float lo = 0, hi = length;
    for (u32 k = 0; k < 3; ++k) {
        float d = direction.x[k];
        if (d == 0) {
            if (start.x[k] < b.low.x[k] || start.x[k] > b.high.x[k]) return 0;
        } else {
            float a = (b.low.x[k] - start.x[k]) / d, z = (b.high.x[k] - start.x[k]) / d;
            if (a > z) { float t = a; a = z; z = t; }
            if (a > lo) lo = a;
            if (z < hi) hi = z;
            if (lo > hi) return 0;
        }
    }
    return 1;
}

/* A fixture's break has its own glass feedback, even if its collision
 * surface is metal or the bulb has no collision. The native manager queues
 * these on bullet workers and drains them after joining those workers.
 * Quiet (restored/resimulated) bullets change state without repeating FX. */
static void shatter(const struct project *p, const struct lamp *lamp,
                    void *bullet, void *manager, struct vec direction) {
    if (((const u8 *)bullet)[0x80] || !pointer(manager, 0x90)) return;
    const void *materials = pointer(manager, 0x94), *material = pointer(bullet, 0x48);
    if (!materials || !material) return;
    u32 id = *(const u16 *)((const u8 *)material + 0x64);
    if (id >= 128) return;
    const void *pair = engine_material_pair(materials, id, p->break_material);
    if (!pair) return;
    struct vec position, normal;
    for (u32 k = 0; k < 3; ++k) {
        position.x[k] = (lamp->hit.low.x[k] + lamp->hit.high.x[k]) * 0.5f;
        normal.x[k] = -direction.x[k];
    }
    const void *particles = pointer(pair, 0x24);
    if (particles && particles != pointer(pair, 0x28))
        engine_play_particle(position.x, manager, particles, direction.x, normal.x, word(pair, 0x74));
    if (pointer(pair, 0x18))
        engine_play_sound(manager, (const u32 *)((const u8 *)pair + 0x18), position.x);
}
void bl_ray(void *bullet, const void *result, const void *frame) {
    struct project *p = project();
    void *manager = pointer(bullet, 0x44);
    if (!p || pointer(manager, 0x98) != p->physics) return;
    struct vec start = vector((const u8 *)frame + 0x90), dir = vector((const u8 *)frame + 0x20);
    float length = *(const float *)((const u8 *)frame + 0x14);
    if (!finite(length) || length <= 0) return;
    for (u32 k = 0; k < 3; ++k) if (!finite(start.x[k]) || !finite(dir.x[k])) return;
    if (word(result, 0x1c)) {
        struct vec hit = vector(result);
        float stop = 0;
        for (u32 k = 0; k < 3; ++k) stop += (hit.x[k] - start.x[k]) * dir.x[k];
        if (!finite(stop) || stop < 0) return;
        if (stop < length) length = stop;
    }
    for (u32 i = 0; i < p->count; ++i) {
        struct lamp *lamp = &p->lamps[i];
        if (condition(lamp) == INTACT && intersects(lamp->hit, start, dir, length)) {
            enum condition expected = INTACT;
            if (__atomic_compare_exchange_n(&lamp->condition, &expected, BROKEN, 0,
                                            __ATOMIC_RELAXED, __ATOMIC_RELAXED))
                shatter(p, lamp, bullet, manager, dir);
        }
    }
}

static struct project *for_manager(void *manager) {
    struct project *p = project();
    return p && p->physics == pointer(manager, 0x98) ? p : 0;
}
void bl_serialize(void *manager, void *writer) {
    struct project *p = for_manager(manager);
    if (!p) return;
    u32 values[2 + MAX_LAMPS / 32] = {MAGIC, p->count};
    for (u32 i = 0; i < p->count; ++i)
        if (condition(&p->lamps[i]) == BROKEN) values[2 + i / 32] |= 1u << (i % 32);
    for (u32 i = 0; i < 2 + (p->count + 31) / 32; ++i)
        engine_write_u32(&values[i], writer, "lamps_026g.c", i, "srp::lamps::serialize", "lamps");
}
void bl_deserialize(void *manager, void *reader) {
    struct project *p = for_manager(manager);
    if (!p) return;
    const u32 *at = pointer(reader, 4);
    /* buffer_reader is {begin, cursor, byte_count}, not {begin, cursor,
     * end}: 0x27A5AB computes begin + byte_count. Bound the complete suffix
     * before reading it or changing any state. */
    u32 begin = word(reader, 0), cursor = word(reader, 4), size = word(reader, 8);
    u32 bytes = 4 * (2 + (p->count + 31) / 32);
    require(cursor >= begin && cursor - begin <= size && bytes <= size - (cursor - begin));
    require(at[0] == MAGIC && at[1] == p->count);
    for (u32 i = 0; i < p->count; ++i)
        p->lamps[i].condition = (at[2 + i / 32] >> (i % 32)) & 1 ? BROKEN : INTACT;
    *(const u32 **)((u8 *)reader + 4) = at + 2 + (p->count + 31) / 32;
}
