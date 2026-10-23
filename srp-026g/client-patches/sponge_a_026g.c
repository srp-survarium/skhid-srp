/* The Sponge (artefact_sponge, data type 12), exe patch 55: 0.28d0's
 * artefact_sponge_core (0x5136E0..0x515270), its wrapper and cook
 * (0x46A740..0x46AEA6) and its cook data reader (0x5439E0) written again on
 * 0.26g build 2777's own interfaces.
 *
 * The object is artefact<artefact_rattle_core>'s (Rattle, type 8: a bare
 * artefact_base and the wrapper's drawable at +340h, 360h bytes, built by the
 * game's own constructor 0x25FA10), then the sponge's own fields (struct
 * sponge). Every address is a VA of the original exe (image base 0x10000; it
 * is never rebased); the proof of each is in docs/notes/client_patch.md
 * (patch 55) and tools/re-kit/v026g. The game's functions are called through
 * the engine-function library (engine_026g.h: a cdecl function per engine
 * function, its registers and cleanup from the re-kit); the arena's link
 * (link_026g.py) places this code in the block sponge.code and its mutable
 * state (sp_g: the zone being ticked, for the zone's hit hook) in
 * sponge.state.
 *
 * 0.28 names a slot of its own artefact_base vtable (42 slots) by its 0.28
 * offset; 0.26g's (36 slots) by its own: action is 0.28 +28h, 0.26g +2Ch.
 */
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef int i32;

#include "engine_026g.h"

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))
#define THISCALL __attribute__((thiscall))
#define STDCALL __attribute__((stdcall))
#define CDECL __attribute__((cdecl))

/* ---- the game ---------------------------------------------------------- */

/* The game's functions this code calls are the engine library's (engine_026g.tsv):
 * the config's child and has (0xF6B10, 0xF6C00), a damage type's index
 * (0x304370, the grenade set's reader 0x309CA0), the cooks' malloc and free
 * (0xE4F90, 0x1539D0), Rattle's constructor, loader, set_world, draw and
 * destructor (0x25FA10, 0x2605F0, 0x25FB30, 0x25FB80, 0x260860), the cook's
 * finish (0x2EE8D0), the resource query (0xE5B60), a resource's release
 * (0xE0210), artefact_base's tick, serialize and deserialize (0x313200,
 * 0x313670, 0x313760), the modifiers' getter (0x2D5AE0), find_pool (0x2E58B0),
 * the hand's transform (0x309F00), the physics' shape and body (0x7A390,
 * 0x357100, 0x7A1B0, 0x42A390, 0x3576C0.., 0x3577D0..), the splash (0x339630),
 * remove_from_slot (0x2FC680), the zone's sensor, tick and switch-off
 * (0x3080B0, 0x319A70, 0x319D30), the geometry's load and destructor
 * (0x2F8C30, 0x2F8BE0), the matrix helpers (0x573C0, 0x725E0, 0xEE700), the
 * field writers (0x1F3DE0, 0x1F48A0, 0x301DE0, 0x81A70), the scene's model
 * calls (0x21CD0, 0x22280, 0x21F40) and the variant's type id (0x125790). */
/* allocation (the cooks' allocator, as research_native_026g.c) */
#define ALLOCATOR 0x1F1D9C4
#define GEOMETRY_INFO (0xACED34 + 8) /* the collision geometry's allocation info (0x304719: 0x420789 is lea eax,[ecx+8]) */
/* the artefact cook (vtable 0x9FC70C, class 76h): Rattle's object is 360h bytes
 * (0x25E7C9), its vtable 0xA06124 (36 slots), its drawable's 0xA061B8 */
#define CONFIG_VARIANT_VTABLE 0x9ED1E4 /* concrete_type_helper<binary_config_value> (0x2F2C8F) */
#define RESULT_STRIDE 0x2E0        /* queries_result: an entry's size, its resource at +12Ch */
#define RESULT_RESOURCE 0x12C
#define RESULT_QUERY 0x20          /* the query the results are for (0x25E9DF) */
#define CLASS_MODEL 0xF
#define CLASS_PARTICLE 0x3D
#define CLASS_DAMAGE_ZONE 0x75     /* damage_zone_cook (0x311139; client vtable 0xA0563C, 0x1F6E6C) */
#define REFCOUNT 0xD0              /* a resource's intrusive count */
/* artefact_base (0.26g; client_patch.md 3.51, docs/notes/artefacts.md) */
#define I_WORLD 0x248              /* game_world_core* (set_world) */
#define I_INVENTORY 0x250
#define I_SLOT 0x254
#define I_COUNT 0x260              /* u16 */
#define A_STATE 0x280
#define V_ENABLE_PASSIVE 0x7C
#define V_DISABLE_PASSIVE 0x84
#define V_DISABLE_ACTIVE 0x88
#define D_SCENE 0xC                /* the drawable's (+340h): the scene */
#define D_LAST 0x10                /* the state it last drew */
#define D_MODEL 0x14               /* the model resource; its instance at +108h */
/* inventory */
#define INV_SLOTS 0x10C
#define INV_GRENADE_SLOT 0x17C     /* m_grenade_slot, 17h none */
#define INV_DIRTY 0x184
#define INV_OWNER 0x174            /* the holder's interface: vt+8 the damage model's owner, vt+0Ch the player */
#define NO_SLOT 0x17
/* the player (base_player) */
#define P_GAME_OBJECT 0x110        /* the hit's instigator (the grenade's arm 0x30A1F1) */
#define GO_ID 4                    /* u8: the killer a hit names (0x2D9AF4) */
#define P_KIT 0x115
#define P_PROFILE 0x6B8
#define P_ALIVE 0x33C
#define M_DAMAGE 11                /* the splash's damages (0x30A3D3) */
#define M_RADIUS 12                /* splash_radius (0x30A3A0) */
#define M_THROW 27                 /* the throw's power (0x30A2CC) */
#define POOLS 0x108                /* the damage model's pools */
#define POOL_REGEN 0x2E8           /* a pool's regeneration modifiers {?, head, tail}, next at node+20h */
#define REGEN_CALLBACK 0x312590    /* Lifebone's: (info, modifier) speed, timeout, threshold */
#define REGEN_FUNCTION_VTABLE (0xA1566C | 1) /* its boost::function's (in place) */
/* the world (game_world_core) */
#define W_PHYSICS 0x18FA4          /* the physics world (rigid bodies, the zones' ghosts: 0x2D3B1E) */
#define W_HITS 0x18F58             /* the splash's hit manager (0x30A4CF) */
#define W_TIME_BASE 0x18FBC        /* the tick grid's origin (50: 0x2D00CD; 0.28 +1D05Ch) */
#define W_PLACES 0x18458           /* Research's: places, 30h each, [4 x kit] the player, +0Ch the kit */
#define W_PLACES_END 0x1845C       /* their end (active_player_ptr 0x2D0AB0: the count) */
#define W_ACTIVE_PLACES 0x18FD8    /* u32: a bit for each place in the match now (active_player_ptr) */
/* the rigid body (grenade_core's 0x303890, 0.28's 0x514400 the same numbers) */
#define PHYSICS_ALLOCATOR 0x1F1D384
#define B_SHAPE 0x10
#define B_BULLET 0x14              /* the Bullet body: +100h, +104h damping */
#define BV_SET_TRANSFORM 0x18
#define BV_GET_TRANSFORM 0x1C
#define BV_OBJECT 0x4
#define PW_ADD 0x2C                /* (body, 818h, 8) */
#define PW_UPDATE 0x20             /* (the body's object) */
#define PW_REMOVE 0x38             /* (body, 1) */
/* the damage zone (damage_zone_core, client damage_zone) */
#define Z_BASE 0x108               /* base_game_object: +14h geometries, +18h their count, +1Ch active */
#define Z_GEOMETRIES 0x11C
#define Z_GEOMETRY_COUNT 0x120
#define Z_ACTIVE 0x124
#define Z_SERIAL 0x130             /* serializable: [0] serialize, [1] deserialize, [2] after load (world) */
#define Z_WORLD 0x1A8
#define Z_LAST_HIT 0x1CC
#define Z_HITTING 0x1D1
#define Z_SOUNDS 0x41C             /* client: the sound zones' world matrices, count +420h */
#define Z_EFFECTS 0x43C            /* the effect zones', count +440h */
#define Z_DRAWABLE 0x408           /* the client zone's drawable */
#define ZV_ON_ACTIVATE 0x1C        /* thiscall (flag) */
#define GV_SET_TRANSFORM 0x20      /* collision_geometry (vtable 0xA13428) */
#define GV_GET_TRANSFORM 0x24
#define GEOMETRY_VTABLE 0xA13428
#define GEOMETRY_VTABLE2 0xA13454
#define RET_ZERO 0x300040          /* xor eax,eax; ret */
#define FIELD_10H 0x346480         /* lea eax,[ecx+10h]; ret: the inflictor's position */
#define BIND_VTABLE 0xA09374       /* boost::function's vtable of a bound member function taking a
                                      queries_result (manager 0x27E890, invoker 0x202300: this, results) */

enum {
    TYPE_SPONGE = 12,
    ST_NONE = 0, ST_PASSIVE = 3, ST_ACTIVE = 4, ST_DEAD = 5, ST_ARMED = 6, ST_ZONE = 7,
    NEVER = 0xFFFFFFFFu,
    TICK = 50,
    MAX_PLACES = 20, KITS = 3, PLACE_SIZE = 0x30, PLACE_KIT = 0xC,
    MAX_ZONES = 4,                 /* sound and effect zones moved with the explosion */
};

/* ---- the object -------------------------------------------------------- */

typedef struct { float m[16]; } matrix;

/* hit_inflictor (0.28 +3C4h: {vt, u16 FFFFh, u8 0, the instigator, the item}),
 * then where it is: its slot 2 (0.28's 0x514550, a pointer to the position)
 * is the game's getter 0x346480 (`lea eax,[ecx+10h] / ret`), so the position
 * is kept at +10h, set wherever m_transform moves (place). */
struct inflictor {
    const u32 *vtable;
    u16 id;
    u8 flag, pad;
    void *instigator;              /* the thrower's game object (P_GAME_OBJECT) at the explosion */
    void *item;                    /* the sponge */
    float position[3];             /* +10h */
};
_Static_assert(__builtin_offsetof(struct inflictor, position) == 0x10, "the getter's +10h");

/* Lifebone's regeneration modifier: the callback reads +1Ch..+33h. */
struct modifier {
    u8 unused[0x1C];
    float regen_add, regen_mul, timeout_add, timeout_mul, threshold_add, threshold_mul;
};

/* a regeneration modifier's node: boost::function {vt|1, ?, fn, adj, this,
 * &modifier, ..} and the list's next at +20h (Lifebone's at +490h) */
struct node {
    u32 vtable;
    u32 unused;
    u32 function, adjust;
    void *self;
    const struct modifier *modifier;
    u32 rest[2];
    struct node *next;
    u32 tail;
};

/* the splash's hit parameters (the grenade set's +278h, 0.28's +3F8h) */
struct splash {
    void *inflictor;
    i32 bodies[8];                 /* -1 after the last */
    u32 type;
    float radius, min, max, pierce, stamina, stopping;
};

struct sponge {
    u8 base[0x340];                /* artefact_base */
    u8 drawable[0x20];             /* +340h the wrapper's (Rattle's): its draw reads the artefact by these offsets */
    u8 *loading_cook;              /* +360h the cook and the config resource (held) while the query loads */
    u8 *loading_config;
    u32 unused[2];
    struct modifier passive;       /* +370h passive/{regen,timeout,threshold}_{add,mul} */
    u32 duration_ms;               /* active/duration_s x 1000 (10 s) */
    u32 activation_timeout_ms;     /* activation_timeout_s x 1000 (3 s) */
    float throw_power;             /* active/throw_power (15) */
    struct splash splash;          /* +3B0h the template */
    struct node node;              /* +3F0h the passive's */
    u8 *linked;                    /* the regeneration list the node is in, 0 when out */
    u32 explode_time;              /* m_explode_time_ms, NEVER: not armed */
    u8 pin;                        /* m_is_pin_pulled */
    u8 owner;                      /* the thrower's place (GO_ID), taken when it is armed; FFh none */
    u8 released;                   /* the key let go before the pin was pulled (a tap): the pull arms it */
    u8 pad;
    void *body;                    /* the rigid body (made at set_world) */
    void *physics;                 /* the physics world while thrown */
    u8 *made_geometry;             /* the zone's collision geometry this sponge made (and frees) */
    matrix transform;              /* +430h m_transform */
    u8 *zone;                      /* the damage zone (data/active/damage_zone), a resource we hold */
    u32 sounds, effects;           /* zones moved with the explosion */
    matrix sound_local[MAX_ZONES];
    matrix effect_local[MAX_ZONES];
    struct inflictor inflictor;    /* +67Ch */
};
_Static_assert(__builtin_offsetof(struct sponge, inflictor) == 0x67C, "inflictor");
_Static_assert(__builtin_offsetof(struct sponge, passive) == 0x370, "passive");
_Static_assert(__builtin_offsetof(struct sponge, splash) == 0x3B0, "splash");
_Static_assert(__builtin_offsetof(struct sponge, node) == 0x3F0, "node");
_Static_assert(__builtin_offsetof(struct sponge, explode_time) == 0x41C, "explode_time");
_Static_assert(__builtin_offsetof(struct sponge, pin) == 0x420, "pin");
_Static_assert(__builtin_offsetof(struct sponge, body) == 0x424, "body");
_Static_assert(__builtin_offsetof(struct sponge, transform) == 0x430, "transform");
_Static_assert(__builtin_offsetof(struct sponge, zone) == 0x470, "zone");
_Static_assert(sizeof(struct sponge) == 0x698, "sponge");
_Static_assert(sizeof(struct splash) == 0x40 && sizeof(struct node) == 0x28, "splash, node");

/* the zone's hit hook (sponge_a_026g.asm sponge_zone_hit) reads these: the
 * arena's block sponge.state */
struct sp_globals {
    u8 *zone;                      /* the zone a sponge ticks now */
    void *instigator, *inflictor, *item;
};
struct sp_globals sp_g;
#define G (&sp_g)

/* ---- small things freestanding code needs ------------------------------ */

/* static: Research's are the global ones; gcc's own calls bind to these */
static void *memset(void *to, int value, unsigned long size) {
    u8 *at = to;
    while (size--) *at++ = (u8)value;
    return to;
}

static void *memcpy(void *to, const void *from, unsigned long size) {
    u8 *a = to;
    const u8 *b = from;
    while (size--) *a++ = *b++;
    return to;
}

/* ---- calls into the game ----------------------------------------------- */

typedef struct {
    u32 data, data_high, name, name_high, crc;
    u16 type, count;
} cfg;
_Static_assert(sizeof(cfg) == 24, "a config value is 24 bytes");
enum { CFG_FLOAT = 2, CFG_INDEXED = 4 };

static const cfg *child(const cfg *table, const char *key) {
    return (const cfg *)engine_cfg_child((const struct engine_cfg *)table, key);
}

static int has(const cfg *table, const char *key) {
    return (int)engine_cfg_has((const struct engine_cfg *)table, key);
}

static const cfg *item(const cfg *table, u32 index) { return (const cfg *)table->data + index; }

/* A number as the loaders read one: a float, or an int converted (0.28's
 * 0x543930 and 0.26g's 0x309B9D). */
static float number(const cfg *value) {
    union { u32 bits; float value; } as = {value->data};
    return value->type == CFG_FLOAT ? as.value : (float)(i32)value->data;
}

/* 0.28's `imul eax,[eax],3E8h`: an int's seconds in ms (a float's taken
 * whole, where 0.28 would read its bits). */
static u32 whole_ms(const cfg *value) {
    union { u32 bits; float value; } as = {value->data};
    return value->type == CFG_FLOAT ? (u32)(i32)as.value * 1000u : value->data * 1000u;
}

static void *allocate(u32 size, u32 info) {
    return engine_malloc(*(void **)ALLOCATOR, size, (const void *)info);
}

static void acquire(u8 *resource) {
    if (resource) __atomic_add_fetch(&AT(resource, REFCOUNT, i32), 1, __ATOMIC_SEQ_CST);
}

static void release(u8 *resource) {
    if (resource && __atomic_sub_fetch(&AT(resource, REFCOUNT, i32), 1, __ATOMIC_SEQ_CST) == 0)
        engine_resource_release(resource, resource + REFCOUNT);   /* eax as 0x25FB1D has it */
}

static u32 vt(const void *object, u32 offset) { return AT(*(const u8 *const *)object, offset, u32); }

/* The holder's interface (inventory+174h), slot `offset` of it: +0Ch the
 * player, +8 &the damage model; 0 out of an inventory. */
static void *from_holder(const struct sponge *s, u32 offset) {
    u8 *inventory = AT(s, I_INVENTORY, u8 *);
    u8 *owner = inventory ? AT(inventory, INV_OWNER, u8 *) : 0;
    return owner ? ((THISCALL void *(*)(void *))vt(owner, offset))(owner) : 0;
}

static u8 *holder(const struct sponge *s) { return from_holder(s, 0xC); }

static u8 *damage_model(const struct sponge *s) {
    u8 **at = from_holder(s, 0x8);
    return at ? *at : 0;
}

/* The holder's modifier `index` on b (0x2D5AE0, k 1). */
static float modified(const u8 *player, u32 index, float b) {
    u8 *mods = AT(player, P_PROFILE, u8 *) + 0x704 * AT(player, P_KIT, u8) + 0xF4;
    return engine_apply_modifier(mods, index, b, 1.0f);
}

static u8 *find_pool(u8 *pools, const char *name) { return engine_find_pool(pools, &name); }

static void hand_transform(struct sponge *s, matrix *out) { engine_hand_transform(s, out); }

/* out = b, then a (b applied first) */
static void multiply(const matrix *a, const matrix *b, matrix *out) { engine_matrix_multiply(a, b, out); }

static void translation(const float *position, matrix *out) { engine_matrix_translation(out, position); }

static void euler(const float *angles, matrix *out) { engine_matrix_rotation(angles, out); }

static const char SP_FILE[] = "artefact_sponge_core.cpp";
static const char SP_SERIALIZE[] = "survarium::artefact_sponge_core::serialize";

/* One field through the game's writer `write`; `line` 0.28's line of it. */
typedef void (*writer_fn)(const void *, void *, const char *, unsigned int, const char *, const char *);

static void field(u8 *writer, writer_fn write, const void *value, u32 line, const char *name) {
    write(value, writer, SP_FILE, line, SP_SERIALIZE, name);
}

/* ---- the rigid body ---------------------------------------------------- */

/* A sphere of 0.05 (dims 0.05, 0.1, 0.1, scale 1, type 3) with the body
 * description 0.28's 0x514400 and 0.26g's grenade 0x303890 both give:
 * {0.3, 0, 0, 0, 0.5, 0, 0.8, 1, 0, 0.005, 0.01 x 3, 1 x 6, 1}, damping
 * 0.03 and 0.01; 0.28 flags the shape (its +174h, 0.26g's resource flags
 * +FCh) with 2. */
static void *make_body(void) {
    static const float dims[3] = {0.05f, 0.1f, 0.1f}, scale[3] = {1.0f, 1.0f, 1.0f};
    struct {
        float a[4], b, c, d, e;
        u8 flag, pad[3];
        float f, g[3], h[6];
        u8 last, pad2[3];
    } desc = {{0.3f, 0, 0, 0}, 0.5f, 0, 0.8f, 1.0f, 0, {0}, 0.005f, {0.01f, 0.01f, 0.01f},
              {1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f}, 1, {0}};
    _Static_assert(sizeof desc == 0x50 && __builtin_offsetof(__typeof__(desc), last) == 0x4C, "desc");
    u8 *shape = engine_create_collision_shape(3, dims, scale, 0);
    AT(shape, 0xFC, u32) |= 2;
    u8 *body = engine_create_rigid_body(&desc, shape);
    u8 *bullet = AT(body, B_BULLET, u8 *);
    AT(bullet, 0x104, float) = 0.03f;
    AT(bullet, 0x100, float) = 0.01f;
    return body;
}

/* grenade_core's destructor 0x303770 (destroy_dynamic_rigid_body): the
 * shape, then the body through the physics allocator. */
static void destroy_body(u8 *body) {
    engine_destroy_shape(AT(body, B_SHAPE, u8 *));
    void *memory = engine_rt_cast_to_void(body);
    ((THISCALL void (*)(void *, u32))vt(body, 8))(body, 0);
    u8 *allocator = *(u8 **)PHYSICS_ALLOCATOR;
    ((THISCALL void (*)(void *, void *, const char *, const char *, u32))vt(allocator, 0x18))(
        allocator, memory, "vostok::physics::destroy_dynamic_rigid_body", ".\\dynamic_rigid_body.cpp", 0x4E);
}

static void physics_update(void *physics, void *body) {
    void *object = ((THISCALL void *(*)(void *))vt(body, BV_OBJECT))(body);
    ((THISCALL void (*)(void *, void *))vt(physics, PW_UPDATE))(physics, object);
}

/* The inflictor's position: where m_transform is (0.28's slot 2 read
 * the transform at each call; 0.26g's getter reads the field). */
static void place(struct sponge *s, const matrix *m) {
    s->inflictor.position[0] = m->m[12];
    s->inflictor.position[1] = m->m[13];
    s->inflictor.position[2] = m->m[14];
}

/* 0.28's 0x5146E0: into the physics world at `transform` with `linear`, no
 * spin; the item count 0, the inventory dirty. */
static void launch(struct sponge *s, const matrix *transform, const float linear[3]) {
    static const float zero[3] = {0, 0, 0};
    u8 *world = AT(s, I_WORLD, u8 *);
    s->physics = AT(world, W_PHYSICS, void *);
    ((THISCALL void (*)(void *, const matrix *))vt(s->body, BV_SET_TRANSFORM))(s->body, transform);
    place(s, transform);
    ((THISCALL void (*)(void *, void *, u32, u32))vt(s->physics, PW_ADD))(s->physics, s->body, 0x818, 8);
    engine_body_set_linear_velocity(linear, s->body);
    engine_body_set_angular_velocity(zero, s->body);
    if (AT(s, I_COUNT, u16)) {
        AT(s, I_COUNT, u16) = 0;
        u8 *inventory = AT(s, I_INVENTORY, u8 *);
        if (inventory) AT(inventory, INV_DIRTY, u8) = 1;
    }
}

static void leave_physics(struct sponge *s) {
    if (s->physics) ((THISCALL void (*)(void *, void *, u32))vt(s->physics, PW_REMOVE))(s->physics, s->body, 1);
    s->physics = 0;
}

/* 0.28's vt+80h: the hand while held, the body when thrown. */
static void current_transform(struct sponge *s, matrix *out) {
    if (!s->physics && AT(s, I_INVENTORY, u8 *)) hand_transform(s, out);
    else *out = s->transform;
}

/* ---- the damage zone --------------------------------------------------- */

/* 0.28's 0x51E520 (as 0.26g's anomaly does it, 0x31B9A0): the zone's
 * geometries' ghosts into the physics world, its timers reset, on. */
static void zone_on(struct sponge *s, u32 flag) {
    if (!s->zone) return;
    u8 *zone = s->zone, *physics = AT(AT(s, I_WORLD, u8 *), W_PHYSICS, u8 *);
    AT(zone, Z_WORLD, u8 *) = physics;
    engine_sensor_insert(zone + Z_BASE, physics);
    AT(zone, Z_LAST_HIT, u32) = NEVER;
    AT(zone, Z_HITTING, u8) = 0;
    ((THISCALL void (*)(void *, u32))vt(zone, ZV_ON_ACTIVATE))(zone, flag);
}

static void zone_off(struct sponge *s, u32 flag) {
    if (s->zone) engine_zone_deactivate(s->zone, flag);
}

static int zone_active(const struct sponge *s) { return s->zone && AT(s->zone, Z_ACTIVE, u8); }

static u8 *geometry(const struct sponge *s) {
    if (!s->zone) return 0;
    return AT(s->zone, Z_GEOMETRY_COUNT, u32) ? AT(AT(s->zone, Z_GEOMETRIES, u8 *), 0, u8 *) : 0;
}

/* 0.28's +A8h (0x4AE930): the zone's sound and effect zones where the
 * explosion is, their own offsets kept (local x transform). */
static void zone_place(struct sponge *s, const matrix *transform) {
    u8 *zone = s->zone;
    if (!zone) return;
    for (u32 i = 0; i < s->sounds && i < AT(zone, Z_SOUNDS + 4, u32); ++i)
        multiply(transform, &s->sound_local[i], &AT(zone, Z_SOUNDS, matrix *)[i]);
    for (u32 i = 0; i < s->effects && i < AT(zone, Z_EFFECTS + 4, u32); ++i)
        multiply(transform, &s->effect_local[i], &AT(zone, Z_EFFECTS, matrix *)[i]);
}

/* The thrower, from his place: its current kit's player, only while the
 * place is in the match (active_player_ptr 0x2D0AB0's test: below the
 * places' end and its bit in world+18FD8h, cleared at the tick of a leave
 * by 0x2D2040). A place is never given to another player in a match
 * (matchmaking.md: a joiner gets a new one), so the place names the thrower;
 * a thrower who left gets no credit, and one back in his place does. */
static u8 *owner_player(const struct sponge *s) {
    u8 *world = AT(s, I_WORLD, u8 *);
    if (!world || s->owner >= MAX_PLACES) return 0;
    u8 *places = AT(world, W_PLACES, u8 *), *end = AT(world, W_PLACES_END, u8 *);
    if (!places || end <= places || (u32)(end - places) / PLACE_SIZE <= s->owner) return 0;
    if (!(AT(world, W_ACTIVE_PLACES, u32) >> s->owner & 1)) return 0;
    u8 *place = places + PLACE_SIZE * s->owner;
    if (place[PLACE_KIT] >= KITS) return 0;
    u8 *player = AT(place, 4 * place[PLACE_KIT], u8 *);
    return player && player[P_GAME_OBJECT + GO_ID] == s->owner ? player : 0;
}

/* his game object (the killer a hit names) */
static void *owner_object(const struct sponge *s) {
    u8 *player = owner_player(s);
    return player ? player + P_GAME_OBJECT : 0;
}

/* The zone's tick (0x319A70), its hits the thrower's (the hook in its hit
 * loop reads G): the instigator found again from the thrower's place for
 * each tick (the inflictor's too), none kept after it. */
static void zone_tick(struct sponge *s, u32 dt, u32 time) {
    if (!s->zone) return;
    struct sp_globals *g = G;
    g->zone = s->zone;
    g->instigator = s->inflictor.instigator = owner_object(s);
    g->inflictor = &s->inflictor;
    g->item = s;
    engine_zone_tick(s->zone, dt, time, time % TICK == 0);
    g->zone = 0;
    g->instigator = g->inflictor = g->item = 0;
    s->inflictor.instigator = 0;
}

/* ---- the passive (0.28's 0x5139A0 / 0x513A10, on 0.26g's Lifebone list) */

THISCALL void sp_disable_passive(struct sponge *s);

/* Linked once: out of the list it may still be in first (0.28 links again
 * whatever was before, which would put the node in two lists). */
THISCALL void sp_enable_passive(struct sponge *s) {
    if (s->linked) sp_disable_passive(s);
    u8 *model = damage_model(s);
    u8 *pool = model ? find_pool(model + POOLS, "infection") : 0;
    if (!pool) return;
    u8 *list = pool + POOL_REGEN;
    s->node.next = 0;
    if (!AT(list, 4, struct node *)) AT(list, 4, struct node *) = &s->node;
    else AT(list, 8, struct node *)->next = &s->node;
    AT(list, 8, struct node *) = &s->node;
    s->linked = list;
}

/* Lifebone's 0x312380 for our node: out of the list it went into. */
THISCALL void sp_disable_passive(struct sponge *s) {
    u8 *list = s->linked;
    if (!list) return;
    s->linked = 0;
    struct node *before = 0, *at = AT(list, 4, struct node *);
    while (at && at != &s->node) before = at, at = at->next;
    if (!at) return;
    if (before) before->next = at->next;
    else AT(list, 4, struct node *) = at->next;
    if (!at->next) AT(list, 8, struct node *) = before;
}

/* ---- the slots --------------------------------------------------------- */

/* Armed at `time`: state 6, the explosion activation_timeout later rounded
 * up to the world's tick grid (as 0.26g's pull_pin 0x30A120), the thrower
 * the holder now. Once: only when not armed. */
static void arm(struct sponge *s, u32 time) {
    if (s->explode_time != NEVER) return;
    AT(s, A_STATE, u32) = ST_ARMED;
    u32 origin = AT(AT(s, I_WORLD, u8 *), W_TIME_BASE, u32), due = s->activation_timeout_ms + time;
    due = due > origin ? due - origin - 1 : 0;
    s->explode_time = (due / TICK + 1) * TICK + origin;
    u8 *player = holder(s);
    s->owner = player ? player[P_GAME_OBJECT + GO_ID] : 0xFF;
    s->released = 0;
}

/* 0.28's action 0x514580: a press (not armed) puts it in the grenade slot
 * (the throw state starts); a release takes it out and, with the pin pulled,
 * arms it. A release before the pull (a tap: both in one tick, or the
 * release first) is kept, and the pull arms it (0.28 would throw it unarmed:
 * it never explodes and is never thrown again). */
THISCALL void sp_action(struct sponge *s, u32 pressed, u32 time) {
    u8 *inventory = AT(s, I_INVENTORY, u8 *);
    if (!inventory) return;
    if ((u8)pressed && s->explode_time == NEVER) {
        AT(inventory, INV_GRENADE_SLOT, u32) = AT(s, I_SLOT, u32);
        s->released = 0;
        return;
    }
    AT(inventory, INV_GRENADE_SLOT, u32) = NO_SLOT;
    if (s->explode_time != NEVER) return;
    if (s->pin) arm(s, time);
    else s->released = 1;
}

/* The passive as the state says: in while it is in an inventory and held
 * (3), active (4) or armed in the hand or the air (6), out otherwise. */
static void sync_passive(struct sponge *s) {
    u32 state = AT(s, A_STATE, u32);
    if (AT(s, I_INVENTORY, u8 *) && (state == ST_PASSIVE || state == ST_ACTIVE || state == ST_ARMED))
        ((THISCALL void (*)(void *))vt(s, V_ENABLE_PASSIVE))(s);
    else
        ((THISCALL void (*)(void *))vt(s, V_DISABLE_PASSIVE))(s);
}

/* 0.28's removed_from_inventory 0x513200, its holes closed: the passive out
 * in every state (0.28 skipped state 6 and left the node in the old holder's
 * pool); armed in the hand, it falls from the hand (into the physics world
 * there, no speed; 0.28 left it where m_transform was, the identity), its
 * fuse and thrower kept; armed in the air or exploded, nothing else. */
THISCALL void sp_removed(struct sponge *s) {
    u32 state = AT(s, A_STATE, u32);
    ((THISCALL void (*)(void *))vt(s, V_DISABLE_PASSIVE))(s);
    if (state == ST_ARMED) {
        static const float still[3] = {0, 0, 0};
        if (!s->physics && s->body && holder(s)) {
            matrix hand;
            hand_transform(s, &hand);
            s->transform = hand;
            launch(s, &hand, still);
        }
        return;
    }
    if (state == ST_ZONE) return;
    if (state == ST_ACTIVE) ((THISCALL void (*)(void *))vt(s, V_DISABLE_ACTIVE))(s);
    AT(s, A_STATE, u32) = ST_NONE;
}

/* 0.28's 0x515190 (its +34h, 0.26g's +38h): the holder back, from state 5. */
THISCALL void sp_revived(struct sponge *s) {
    if (AT(s, A_STATE, u32) != ST_DEAD) return;
    ((THISCALL void (*)(void *))vt(s, V_ENABLE_PASSIVE))(s);
    AT(s, A_STATE, u32) = ST_PASSIVE;
}

/* 0.28's 0x513FD0: the splash's parameters, the thrower's radius (12) and
 * damage (11) modifiers on them, the inflictor this one's naming him (0.28
 * the holder: the same player while it is held; a thrower out of the match
 * neither). */
static void splash_of(struct sponge *s, struct splash *out) {
    *out = s->splash;
    out->inflictor = &s->inflictor;
    u8 *player = owner_player(s);
    s->inflictor.instigator = player ? player + P_GAME_OBJECT : 0;
    if (!player) return;
    out->radius = modified(player, M_RADIUS, out->radius);
    out->min = modified(player, M_DAMAGE, out->min);
    out->max = modified(player, M_DAMAGE, out->max);
    out->stamina = modified(player, M_DAMAGE, out->stamina);
    out->stopping = modified(player, M_DAMAGE, out->stopping);
}

/* 0.28's tick 0x514160: states 6 and 7 are the sponge's own. */
THISCALL void sp_tick(struct sponge *s, u32 dt, u32 time) {
    u32 state = AT(s, A_STATE, u32);
    if (state != ST_ARMED && state != ST_ZONE) engine_artefact_tick(s, dt, time);
    if (s->physics) {
        matrix at;
        s->transform = *((THISCALL matrix * (*)(void *, matrix *)) vt(s->body, BV_GET_TRANSFORM))(s->body, &at);
        place(s, &s->transform);
        physics_update(s->physics, s->body);
    }
    if (s->explode_time == NEVER) return;
    if (AT(s, A_STATE, u32) == ST_ARMED && time == s->explode_time) {
        matrix transform;
        struct splash hit;
        current_transform(s, &transform);
        float position[3] = {transform.m[12], transform.m[13], transform.m[14]};
        place(s, &transform);
        splash_of(s, &hit);
        u8 *world = AT(s, I_WORLD, u8 *);
        engine_splash_hit(&hit, AT(world, W_HITS, void *), position, time);
        s->inflictor.instigator = 0;                 /* the zone's ticks name him again each time */
        s->transform = transform;                    /* the explosion point: the zone's hits' position */
        zone_on(s, 1);
        u8 *shape = geometry(s);
        if (shape) ((THISCALL void (*)(void *, const matrix *))vt(shape, GV_SET_TRANSFORM))(shape, &transform);
        zone_place(s, &transform);
        leave_physics(s);
        s->pin = 0;
        AT(s, A_STATE, u32) = ST_ZONE;               /* before the slot: its removed_from_inventory sees it spent */
        u8 *inventory = AT(s, I_INVENTORY, u8 *);
        u32 slot = AT(s, I_SLOT, u32);
        if (inventory && slot < NO_SLOT && AT(inventory, INV_SLOTS + 4 * slot, struct sponge *) == s)
            engine_inventory_remove_item(inventory, slot);
        sp_disable_passive(s);                       /* 0.28 leaves it in the thrower's list (its +30h skips state 6) */
    }
    /* the zone runs on whatever the item's state (nothing can take an
     * exploded sponge: a container's can_use 0x31C6B0 wants state 2, which
     * only the anomaly's spawn of a state-0 artefact gives, 0x31B815) */
    if (zone_active(s)) zone_tick(s, dt, time);
    if (time == s->explode_time + s->duration_ms) {
        zone_off(s, 0);
        if (AT(s, A_STATE, u32) == ST_ZONE) AT(s, A_STATE, u32) = ST_NONE;
        s->explode_time = NEVER;
    }
}

/* ---- the throw (the throw-grenade state's hooks, sponge_a_026g.asm) ---- */

/* 0.28's grenade_set_base[6] 0x515150: ready to be thrown (not in the air). */
u32 sp_can_throw(struct sponge *s) { return !s->physics; }

/* [1] 0x514610: the pin; the "grenade index" 0. A release already seen
 * (a tap) arms it now. */
u32 sp_pull_pin(struct sponge *s, u32 time) {
    s->pin = 1;
    if (s->released) arm(s, time);
    return 0;
}

/* [4] 0x515140: done (the zone on). */
u32 sp_exploded(struct sponge *s) { return (u32)zone_active(s); }

/* [2] 0x514620: thrown along the aim's z axis at throw_power x the holder's
 * throw modifier (27), from the hand's transform. */
void sp_throw(struct sponge *s, const matrix *hand, const matrix *aim) {
    if (sp_exploded(s) || s->physics) return;
    u8 *player = holder(s);
    if (!player) return;
    float power = modified(player, M_THROW, s->throw_power);
    float v[3] = {aim->m[8] * power, aim->m[9] * power, aim->m[10] * power};
    launch(s, hand, v);
}

/* ---- serialization (0.28's 0x513A70 / 0x513C80) ------------------------- */

THISCALL void sp_serialize(struct sponge *s, u8 *writer, void *dumper) {
    engine_artefact_serialize(s, writer, dumper);
    field(writer, engine_write_u32, &s->explode_time, 0x3B, "m_explode_time_ms");
    field(writer, engine_write_u8, &s->pin, 0x3C, "m_is_pin_pulled");
    u8 thrown = s->physics != 0;
    field(writer, engine_write_u8, &thrown, 0x3D, "thrown()");
    if (thrown) {
        float v[3];
        field(writer, engine_write_matrix, &s->transform, 0x41, "m_transform");
        engine_body_linear_velocity(s->body, v);
        field(writer, engine_write_float3, v, 0x44, "linear_velocity");
        engine_body_angular_velocity(s->body, v);
        field(writer, engine_write_float3, v, 0x47, "angular_velocity");
    }
    /* ours: the tap waiting for the pull, the thrower's place (from the arming on) */
    field(writer, engine_write_u8, &s->released, 0x48, "m_released_before_pin");
    field(writer, engine_write_u8, &s->owner, 0x49, "m_zone_owner");
    if (AT(s, A_STATE, u32) == ST_ZONE && s->zone) {
        u8 *serial = s->zone + Z_SERIAL;
        ((THISCALL void (*)(void *, void *, void *))vt(serial, 0))(serial, writer, dumper);
        /* 0.28 wrote its geometry's transform, which is the explosion's:
         * m_transform, which the zone's hits' position (the inflictor's) is */
        field(writer, engine_write_matrix, &s->transform, 0x4F, "m_damage_zone->get_collision_geometry(0)->transform()");
    }
}

static u32 read32(u8 *reader) {
    u8 *at = AT(reader, 4, u8 *);
    AT(reader, 4, u8 *) = at + 4;
    return AT(at, 0, u32);
}

static u8 read8(u8 *reader) {
    u8 *at = AT(reader, 4, u8 *);
    AT(reader, 4, u8 *) = at + 1;
    return *at;
}

static void read_bytes(u8 *reader, void *out, u32 size) {
    u8 *at = AT(reader, 4, u8 *);
    memcpy(out, at, size);
    AT(reader, 4, u8 *) = at + size;
}

THISCALL void sp_deserialize(struct sponge *s, u8 *reader, void *a, void *b) {
    engine_artefact_deserialize(s, reader, a, b);
    s->explode_time = read32(reader);
    s->pin = read8(reader) != 0;
    if (read8(reader)) {
        float linear[3], angular[3];
        read_bytes(reader, &s->transform, sizeof s->transform);
        place(s, &s->transform);
        read_bytes(reader, linear, sizeof linear);
        read_bytes(reader, angular, sizeof angular);
        if (!s->physics) {
            launch(s, &s->transform, linear);
        } else {
            ((THISCALL void (*)(void *, const matrix *))vt(s->body, BV_SET_TRANSFORM))(s->body, &s->transform);
            physics_update(s->physics, s->body);
            engine_body_set_linear_velocity(linear, s->body);
        }
        engine_body_set_angular_velocity(angular, s->body);
    } else {
        leave_physics(s);
    }
    s->released = read8(reader) != 0;
    s->owner = read8(reader);
    s->inflictor.instigator = 0;
    if (AT(s, A_STATE, u32) == ST_ZONE && s->zone) {
        if (!zone_active(s)) zone_on(s, 1);
        u8 *serial = s->zone + Z_SERIAL;
        ((THISCALL void (*)(void *, void *, void *, void *))vt(serial, 4))(serial, reader, a, b);
        read_bytes(reader, &s->transform, sizeof s->transform);
        place(s, &s->transform);
        u8 *shape = geometry(s);
        if (shape) ((THISCALL void (*)(void *, const matrix *))vt(shape, GV_SET_TRANSFORM))(shape, &s->transform);
        zone_place(s, &s->transform);
        ((THISCALL void (*)(void *, void *))vt(serial, 8))(serial, AT(s, I_WORLD, void *));
    } else if (zone_active(s)) {
        zone_off(s, 1);
    }
    sync_passive(s);
}

/* ---- the world and the draw -------------------------------------------- */

/* Out of the scene's list (scene+0DCh {?, head, tail}, prev +4, next +8)
 * when it is in it. */
static void unlink_drawable(u8 *scene, u8 *drawable) {
    u8 *list = scene + 0xDC, *at = AT(list, 4, u8 *);
    while (at && at != drawable) at = AT(at, 8, u8 *);
    if (!at) return;
    u8 *prev = AT(drawable, 4, u8 *), *next = AT(drawable, 8, u8 *);
    if (prev) AT(prev, 8, u8 *) = next;
    else AT(list, 4, u8 *) = next;
    if (next) AT(next, 4, u8 *) = prev;
    else AT(list, 8, u8 *) = prev;
    AT(drawable, 4, u8 *) = AT(drawable, 8, u8 *) = 0;
}

/* At the list's end, as Rattle's set_world 0x25FB30 links its drawable. */
static void link_drawable(u8 *scene, u8 *drawable) {
    u8 *list = scene + 0xDC;
    AT(drawable, 8, u8 *) = 0;
    AT(drawable, 4, u8 *) = AT(list, 8, u8 *);
    if (!AT(list, 4, u8 *)) AT(list, 4, u8 *) = drawable;
    else AT(AT(list, 8, u8 *), 8, u8 *) = drawable;
    AT(list, 8, u8 *) = drawable;
}

/* 0.28's set_world 0x46A760 / 0x514400: the world, the rigid body, both
 * drawables (the artefact's, as Rattle's 0x25FB30, and the zone's). */
THISCALL void sp_set_world(struct sponge *s, u8 *world) {
    engine_rattle_set_world(s, world);
    if (!s->body) s->body = make_body();
    u8 *scene = AT(s, 0x340 + D_SCENE, u8 *);
    if (scene && s->zone) {
        unlink_drawable(scene, s->zone + Z_DRAWABLE);  /* once, however often the world is set */
        link_drawable(scene, s->zone + Z_DRAWABLE);
    }
}

static void free_memory(void *memory) {
    void *pointer = memory;
    engine_free(*(void **)ALLOCATOR, memory, &pointer);
}

/* Each thing out of what it went into, before it goes: the passive out of
 * its pool; the zone off (its ghosts out of the physics world, 0x319D30)
 * and its drawable out of the scene's list; the geometry this sponge made
 * out of the zone, destroyed (0x2F8BE0: its ghost) and freed to the cooks'
 * allocator it came from; the body out of the physics world, then
 * destroyed; the zone let go; the sponge's own drawable out of the list
 * (Rattle's destructor then takes its model out of the scene). */
THISCALL void *sp_dtor(struct sponge *s, u32 flags) {
    sp_disable_passive(s);
    u8 *scene = AT(s, 0x340 + D_SCENE, u8 *);
    if (s->zone) {
        if (G->zone == s->zone) G->zone = 0;
        if (zone_active(s)) zone_off(s, 1);
        if (scene) unlink_drawable(scene, s->zone + Z_DRAWABLE);
    }
    if (s->made_geometry) {
        u8 *made = s->made_geometry;
        s->made_geometry = 0;
        if (s->zone && AT(s->zone, Z_GEOMETRY_COUNT, u32) && AT(AT(s->zone, Z_GEOMETRIES, u8 **), 0, u8 *) == made)
            AT(AT(s->zone, Z_GEOMETRIES, u8 **), 0, u8 *) = 0;
        engine_geometry_destroy(made);
        free_memory(made);
    }
    if (s->body) {
        leave_physics(s);
        destroy_body(s->body);
    }
    s->body = 0;
    release(s->zone);
    s->zone = 0;
    if (scene) unlink_drawable(scene, (u8 *)s + 0x340);
    return engine_rattle_destroy(s, flags);
}

static u8 *renderer_of(u8 *scene) { return AT(AT(AT(scene, 0x9C, u8 *), 0x5360, u8 *), 0x200064, u8 *); }

/* The scene's model calls (0x25FB80's): the model's instance (+108h), the
 * renderer, the scene's +4, the matrix twice. */
typedef void (*model_fn)(void *, void *, void *, const void *, const void *);

static void model_call(model_fn add_or_update, u8 *drawable, const matrix *m) {
    u8 *scene = AT(drawable, D_SCENE, u8 *), *model = AT(drawable, D_MODEL, u8 *) + 0x108;
    if (add_or_update) add_or_update(model, renderer_of(scene), scene + 4, m, m);
    else engine_scene_remove_model(model, renderer_of(scene), scene + 4);
}

/* 0.28's presenter (0x4B9F20): in state 6 the model at the sponge's
 * transform (the hand, the body in the air), added when it comes, updated,
 * taken away when it goes; every other state the stock draw's. */
THISCALL void sp_draw(u8 *drawable) {
    struct sponge *s = (struct sponge *)(drawable - 0x340);
    u32 state = AT(s, A_STATE, u32), last = AT(drawable, D_LAST, u32);
    if (state == ST_ARMED) {
        matrix m;
        current_transform(s, &m);
        model_call(last == ST_ARMED ? engine_scene_update_model : engine_scene_add_model, drawable, &m);
        AT(drawable, D_LAST, u32) = ST_ARMED;
        return;
    }
    if (last == ST_ARMED) {
        model_call(0, drawable, 0);
        AT(drawable, D_LAST, u32) = ST_NONE;
    }
    engine_rattle_draw(drawable);
}

/* ---- the vtables ------------------------------------------------------- */

/* artefact<artefact_rattle_core>'s (0xA06124, its RTTI locator 0xA4C044
 * before it) with the sponge's slots; 0.28's sponge overrides there the
 * slots of the same meaning (0.28 offset in the comment). */
__attribute__((used)) const u32 sp_vtable[1 + 36] = {
    0xA4C044,
    (u32)sp_dtor, 0xF1900, 0xF1F00, 0xF1F40, 0xFDA90, 0xFDA90, 0xFDA90, 0x1AF330, 0x2FFC40, 0x2FFC80, 0x243590,
    (u32)sp_action,                                /* +2Ch (0.28 +28h 0x514580) */
    0x313470,                                      /* +30h added_to_inventory: passive, state 3 (0.28 0x5131E0) */
    (u32)sp_removed,                               /* +34h (0.28 +30h 0x513200) */
    (u32)sp_revived,                               /* +38h (0.28 +34h 0x515190) */
    0x1AF330, 0x2FFCE0, 0x2FFD50, 0x313880, 0x300040, 0x300040, 0x246380, 0x246380, 0x300040, 0x300040,
    0x300040,                                      /* +64h not a grenade set */
    (u32)sp_serialize,                             /* +68h (0.28 +70h 0x513A70) */
    (u32)sp_deserialize,                           /* +6Ch (0.28 +74h 0x513C80) */
    (u32)sp_set_world,                             /* +70h (0.28 +78h 0x46A760) */
    (u32)sp_tick,                                  /* +74h (0.28 +7Ch 0x514160) */
    0x15A4B0,                                      /* +78h the active's tick: none */
    (u32)sp_enable_passive,                        /* +7Ch (0.28 +94h 0x5139A0) */
    0x75AF0,                                       /* +80h enable_active: none (never state 4) */
    (u32)sp_disable_passive,                       /* +84h (0.28 +9Ch 0x513A10) */
    0x1AF330, 0x372420,
};
_Static_assert(sizeof sp_vtable == 37 * 4, "36 slots");
/* the drawable's (Rattle's 0xA061B8, locator 0xA4C030) */
__attribute__((used)) const u32 sp_drawable_vtable[1 + 1] = {0xA4C030, (u32)sp_draw};
/* hit_inflictor's three (grenade_core's at its +0, locator 0xA5291C):
 * not a bullet, not a grenade, where it is (0.28's 0x97B9A8); all three the
 * game's own functions (the position the field at +10h) */
__attribute__((used)) const u32 sp_inflictor_vtable[1 + 3] = {0xA5291C, RET_ZERO, RET_ZERO, FIELD_10H};

/* ---- the cook ---------------------------------------------------------- */

static const cfg *data_of(const cfg *root) { return child(root, "data"); }

int sp_is_sponge(u8 **resource) {
    const cfg *root = AT(*resource, 0x108, const cfg *);
    return child(data_of(root), "type")->data == TYPE_SPONGE;
}

u32 sp_size(void) { return sizeof(struct sponge); }

/* 0.28's reader 0x543930/0x5439E0, onto 0.26g's splash layout (the grenade
 * set's 0x309AE0: bodies -1 filled, the type by 0x304370). */
static void read_config(struct sponge *s, const cfg *root) {
    const cfg *data = data_of(root), *passive = child(data, "passive"), *active = child(data, "active");
    s->passive.regen_add = number(child(passive, "regen_add"));
    s->passive.regen_mul = number(child(passive, "regen_mul"));
    s->passive.timeout_add = number(child(passive, "timeout_add"));
    s->passive.timeout_mul = number(child(passive, "timeout_mul"));
    s->passive.threshold_add = number(child(passive, "threshold_add"));
    s->passive.threshold_mul = number(child(passive, "threshold_mul"));
    s->duration_ms = whole_ms(child(active, "duration_s"));
    s->activation_timeout_ms = whole_ms(child(data, "activation_timeout_s"));
    s->throw_power = number(child(active, "throw_power"));
    struct splash *h = &s->splash;
    h->radius = number(child(active, "splash_radius"));
    h->max = number(child(active, "splash_max_damage"));
    h->min = number(child(active, "splash_min_damage"));
    h->pierce = number(child(active, "splash_pierce"));
    h->stamina = number(child(active, "stamina_damage"));
    h->stopping = number(child(active, "stopping_power"));
    h->type = engine_damage_type_of((const char *)child(active, "splash_damage_type")->data);
    for (u32 i = 0; i < 8; ++i) h->bodies[i] = -1;
    const cfg *bodies = child(active, "splash_affected_bodies");
    for (u32 i = 0; i < bodies->count && i < 8; ++i) h->bodies[i] = (i32)item(bodies, i)->data;
}

/* construct (0x25E660's case for type 12): Rattle's object, then the
 * sponge's (0.28's ctor 0x5136E0). */
STDCALL struct sponge *sp_construct(struct sponge *s, u8 **resource, void *effect, void *scene) {
    if (!s) return 0;
    engine_rattle_construct(effect, AT(*resource, 0x108, const cfg *), s, scene);
    const cfg *root = AT(*resource, 0x108, const cfg *);
    memset((u8 *)s + 0x360, 0, sizeof *s - 0x360);
    AT(s, 0, const u32 *) = sp_vtable + 1;
    AT(s, 0x340, const u32 *) = sp_drawable_vtable + 1;
    s->inflictor.vtable = sp_inflictor_vtable + 1;
    s->inflictor.id = 0xFFFF;
    s->inflictor.item = s;
    read_config(s, root);
    s->node.vtable = REGEN_FUNCTION_VTABLE;
    s->node.function = REGEN_CALLBACK;
    s->node.self = s;
    s->node.modifier = &s->passive;
    s->explode_time = NEVER;
    s->owner = 0xFF;
    for (int i = 0; i < 16; ++i) s->transform.m[i] = i % 5 == 0 ? 1.0f : 0.0f;
    return s;
}

/* ---- the cook's queries ------------------------------------------------ */

/* The zone's creation data: a variant<32> holding the config value (as
 * 0x2F2C55 builds them for the victory items' containers). */
struct creation {
    u32 vtable, pad;
    cfg value;
    u32 pad2[2];
    struct creation *self;
    u32 type;
};
_Static_assert(sizeof(struct creation) == 0x30, "variant<32>");

/* The query's callback: a boost::function of a member function bound to its
 * object, in place and trivially copied (vtable BIND_VTABLE | 1), as the
 * late join binds network_client's (0x27D280): the game's invoker 0x202300
 * calls the function with ecx = the object + adj and the queries_result. */
struct function {
    u32 vtable;
    u32 pad;
    u32 buffer[4];                 /* the function, adj 0, the sponge */
};

THISCALL void sp_loaded(struct sponge *s, u8 *results);

static const char *string_of(const cfg *value) { return (const char *)value->data; }

static void local_matrix(const cfg *zone, int rotated, matrix *out) {
    matrix t, r;
    translation((const float *)child(zone, "position")->data, &t);
    if (!rotated) {
        *out = t;
        return;
    }
    euler((const float *)child(zone, "rotation")->data, &r);
    multiply(&t, &r, out);                       /* rotation, then translation (0x257DB7) */
}

/* The zone cooked: its collision geometry from the inline config (as the
 * collision sensors with inline geometries do it, 0x3046F0), the sound and
 * effect zones' own offsets kept for the explosion. */
static void zone_cooked(struct sponge *s, u8 *zone, const cfg *config) {
    s->zone = zone;
    acquire(zone);
    const cfg *geometries = child(config, "collision_geometries");
    if (AT(zone, Z_GEOMETRY_COUNT, u32) && geometries->count) {
        u8 *shape = allocate(0x24, GEOMETRY_INFO);
        memset(shape, 0, 0x24);
        AT(shape, 0, u32) = GEOMETRY_VTABLE;
        AT(shape, 4, u32) = GEOMETRY_VTABLE2;
        AT(AT(zone, Z_GEOMETRIES, u8 *), 0, u8 *) = shape;
        s->made_geometry = shape;
        engine_geometry_load(shape, item(geometries, 0));
    }
    if (has(config, "sound_zones")) {
        const cfg *zones = child(config, "sound_zones");
        for (u32 i = 0; i < zones->count && i < MAX_ZONES; ++i) local_matrix(item(zones, i), 0, &s->sound_local[i]);
        s->sounds = zones->count < MAX_ZONES ? zones->count : MAX_ZONES;
    }
    if (has(config, "effect_zones")) {
        const cfg *zones = child(config, "effect_zones");
        for (u32 i = 0; i < zones->count && i < MAX_ZONES; ++i)
            local_matrix(item(zones, i), 1, &s->effect_local[i]);
        s->effects = zones->count < MAX_ZONES ? zones->count : MAX_ZONES;
    }
}

/* The four resources come: Rattle's loader for the model, the particle and
 * the sound (entries 0..2), the zone (3); then the cook's finish, as the
 * stock callback 0x25E960 does for Rattle. */
THISCALL void sp_loaded(struct sponge *s, u8 *results) {
    u8 *cook = s->loading_cook;
    u8 **resource = &s->loading_config;
    engine_rattle_loaded(s, results);
    u8 *zone = AT(results, RESULT_STRIDE * 3 + RESULT_RESOURCE, u8 *);
    const cfg *root = AT(*resource, 0x108, const cfg *);
    if (zone) zone_cooked(s, zone, child(child(data_of(root), "active"), "damage_zone"));
    engine_cook_finish_query(cook, resource, s, AT(results, RESULT_QUERY, u8 *));
    release(s->loading_config);
    s->loading_cook = s->loading_config = 0;
}

/* 0.28's cook 0x46A8A0 on 0.26g's (0x25E800): one query of the model, the
 * spawn particle and sound and a damage zone (class 75h) made from
 * data/active/damage_zone. Returns 0 for anything not a sponge (the stock
 * code goes on). */
THISCALL u32 sp_cook_query(u8 *cook, u8 *parent, struct sponge *s, u8 **resource) {
    if (!sp_is_sponge(resource)) return 0;
    const cfg *root = AT(*resource, 0x108, const cfg *), *object = child(root, "object");
    const cfg *sound = child(object, "sound_on_spawn");
    struct { const char *name; u32 kind; } requests[4] = {
        {string_of(child(object, "model")), CLASS_MODEL},
        {string_of(child(object, "particle_on_spawn")), CLASS_PARTICLE},
        {string_of(item(sound, 1)), item(sound, 0)->data},
        {"damage_zone", CLASS_DAMAGE_ZONE},
    };
    struct creation zone = {CONFIG_VARIANT_VTABLE, 0, *child(child(data_of(root), "active"), "damage_zone"), {0}, 0, 0};
    zone.self = &zone;
    zone.type = engine_config_variant_type();
    void *creation[4] = {0, 0, 0, &zone};
    s->loading_cook = cook;
    s->loading_config = *resource;
    acquire(*resource);
    struct function done = {BIND_VTABLE | 1, 0, {(u32)sp_loaded, 0, (u32)s, 0}};
    engine_query_resources(requests, 4, &done, *(void **)ALLOCATOR, creation, parent, 1);
    return 1;
}
