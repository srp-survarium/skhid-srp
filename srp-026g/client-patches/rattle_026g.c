/* The Rattle (artefact_rattle, data type 8), exe patch 59: the
 * artefact_rattle_core 0.20e left as stubs, written for 0.26g build 2777.
 * docs/notes/rattle.md has the owner's rules and the proof of every address.
 *
 * The object is the game's artefact<artefact_rattle_core> (artefact_base and
 * the wrapper's drawable at +340h, 360h bytes, built by the game's own
 * constructor 0x25FA10), then the Rattle's fields (struct rattle). Its vtable
 * is Rattle's (0xA06124, RTTI locator 0xA4C044) with the active's slots of
 * its own, as Spring's (0xA05F44) has them: the duration from the config,
 * taken off at each tick, ended at 0, serialized in state 4, its seconds on
 * the hud.
 *
 * Its effect is on sound, presentation only: every game computes from the
 * carrier's artefacts (lockstep state) how loud the sounds he makes are for
 * others, and marks the sound's proxy (rattle_026g.asm). The marks are the
 * quiet q, u16 at proxy+3Ah (the second of the proxy's ext parameters, which
 * the constructor 0x5A6B0 zeroes and no code reads): gain 1 - q/65535.
 *
 * Every address is a VA of the original exe (image base 0x10000; never
 * rebased). The mutable state (the quiet of the steps being played) is on
 * the section's state page (-DRT_STATE_VA).
 */
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef int i32;

#ifndef RT_STATE_VA
#error "RT_STATE_VA: the section's state page (rattle_026g.py)"
#endif

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))
#define THISCALL __attribute__((thiscall))
#define STDCALL __attribute__((stdcall))
#define CLOBBERS "memory", "cc", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7"

/* ---- the game ---------------------------------------------------------- */

/* config look-ups (Research's, the Sponge's) */
#define CONFIG_CHILD 0xF6B10       /* eax = table, edi = key: the child */
#define CONFIG_HAS 0xF6C00         /* eax = table, esi = key: al */
/* artefact_base (client_patch.md 3.51, 3.52) */
#define BASE_SERIALIZE 0x313670    /* thiscall (writer, dumper); ret 8 */
#define BASE_DESERIALIZE 0x313760  /* thiscall (reader, a, b); ret 0Ch */
#define BASE_UI_INFO 0x313880      /* thiscall (out): +0 dict id, +4 count, +6 cooldown %; ret 4 */
#define I_INVENTORY 0x250
#define A_STATE 0x280
/* the inventory and the player */
#define INV_OWNER 0x174            /* the holder's interface: vt+0Ch the player */
#define INV_SLOTS 0x10C            /* the slots, 4 bytes each */
#define FIRST_ARTEFACT_SLOT 13     /* the artefact slots 13..18 (patch 54) */
#define LAST_ARTEFACT_SLOT 18
#define P_INVENTORY 0x10C          /* base_player's (process_quick_slots 0x2D8D6E) */
#define P_KIT 0x115
#define P_PROFILE 0x6B8
#define MODIFIERS 0x2D5AE0         /* eax = a kit's modifiers, esi = index; stdcall (B, k): xmm0 */
#define M_DURATION 39              /* artefact_duration (Spring's enable_active 0x311923) */
/* the weapon's sounds (play_sound_callback 0x297E20's effect) */
#define E_WEAPON 0x30              /* the weapon core */
#define W_USER 0x8                 /* its user, the player (set_user 0x2E2990) */
/* the field writer (Research's, the Sponge's): eax = &value; stdcall (writer, file, line, function, name) */
#define WRITE_U32 0x1F48A0
/* a sound proxy: the quiet (rattle_026g.asm reads it on the sound thread) */
#define PROXY_QUIET 0x3A
/* Rattle's (artefact<artefact_rattle_core>, client_patch.md 3.52) */
#define RATTLE_SIZE 0x360          /* 0x25E7C9 */

enum {
    ST_PASSIVE = 3, ST_ACTIVE = 4,
    QUIET_MAX = 0xFFFF,
};

/* The three fire handler states of a weapon core and where each holds its
 * weapon_sound_effect (initialize 0x251760, 0x2529D0, 0x2533C0). */
static const struct {
    u32 vtable, effect;
} FIRE_HANDLERS[] = {
    {0xA04EE4, 0x168},             /* weapon_core_fire_state */
    {0xA05150, 0x170},             /* double_barreled_weapon_core_fire_state */
    {0xA05248, 0x170},             /* pistol_weapon_core_fire_state */
};

/* ---- the object -------------------------------------------------------- */

struct rattle {
    u8 base[0x340];                /* artefact_base */
    u8 drawable[0x20];             /* +340h the wrapper's: Rattle's draw reads the artefact by these offsets */
    u32 duration_ms;               /* +360h data/active/duration x 1000 (10 s) */
    u32 left_ms;                   /* +364h the active's time left (Spring's m_time_left_to_deactivate) */
    float passive_steps;           /* +368h data/passive/steps_sound_volume_3rd_person (0.7) */
    float active_steps;            /* +36Ch data/active/steps_sound_volume_3rd_person (0) */
    float active_shots;            /* +370h data/active/shots_sound_volume_3rd_person (0.5) */
    u32 pad;
};
_Static_assert(__builtin_offsetof(struct rattle, duration_ms) == RATTLE_SIZE, "after Rattle's object");
_Static_assert(sizeof(struct rattle) == 0x378, "rattle");

/* the steps' quiet while on_player_sound_event or on_landing runs (the
 * point proxy's hook in 0x657C0 reads it; rattle_026g.asm) */
struct rt_globals {
    u32 pending;
};
#define G ((struct rt_globals *)RT_STATE_VA)

/* ---- small things freestanding code needs ------------------------------ */

void *memset(void *to, int value, unsigned long size) {
    u8 *at = to;
    while (size--) *at++ = (u8)value;
    return to;
}

typedef struct {
    u32 data, data_high, name, name_high, crc;
    u16 type, count;
} cfg;
_Static_assert(sizeof(cfg) == 24, "a config value is 24 bytes");
enum { CFG_FLOAT = 2 };

static const cfg *child(const cfg *table, const char *key) {
    const cfg *result;
    __asm__ volatile("call *%[fn]" : "=a"(result), "+D"(key) : "a"(table), [fn] "r"(CONFIG_CHILD)
                     : "ecx", "edx", CLOBBERS);
    return result;
}

static int has(const cfg *table, const char *key) {
    u32 result;
    __asm__ volatile("call *%[fn]" : "=a"(result), "+S"(key) : "a"(table), [fn] "r"(CONFIG_HAS)
                     : "ecx", "edx", "edi", CLOBBERS);
    return (u8)result;
}

/* A number as the loaders read one: a float, or an int converted (0x309B9D). */
static float number(const cfg *value) {
    union { u32 bits; float value; } as = {value->data};
    return value->type == CFG_FLOAT ? as.value : (float)(i32)value->data;
}

/* `key` of `table`, `otherwise` where the table or the key is not there. */
static float number_or(const cfg *table, const char *key, float otherwise) {
    return table && has(table, key) ? number(child(table, key)) : otherwise;
}

static u32 vt(const void *object, u32 offset) { return AT(*(const u8 *const *)object, offset, u32); }

/* The holder (the inventory's holder interface, vt+0Ch), 0 out of an inventory. */
static u8 *holder(const struct rattle *r) {
    u8 *inventory = AT(r, I_INVENTORY, u8 *);
    u8 *owner = inventory ? AT(inventory, INV_OWNER, u8 *) : 0;
    return owner ? ((THISCALL u8 *(*)(void *))vt(owner, 0xC))(owner) : 0;
}

/* The holder's modifier `index` on b (0x2D5AE0, k 1), as Spring's active. */
static float modified(const u8 *player, u32 index, float b) {
    u8 *mods = AT(player, P_PROFILE, u8 *) + 0x704 * AT(player, P_KIT, u8) + 0xF4;
    float result;
    union { float f; u32 u; } bb = {b}, kk = {1.0f};
    u32 fn = MODIFIERS;
    __asm__ volatile("push %[k]\n\tpush %[b]\n\tcall *%[fn]"
                     : "=Yz"(result), "+S"(index), "+a"(mods), [b] "+c"(bb.u), [k] "+d"(kk.u), [fn] "+D"(fn)
                     : : "memory", "cc", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7");
    return result;
}

static const char RT_FILE[] = ".\\artefact_rattle_core.cpp";
static const char RT_SERIALIZE[] = "survarium::artefact_rattle_core::serialize";

static void field_u32(u8 *writer, const u32 *value, u32 line, const char *name) {
    __asm__ volatile("push %[name]\n\tpush %[function]\n\tpush %[line]\n\tpush %[file]\n\tpush %[writer]\n\t"
                     "call *%[fn]"
                     : "+a"(value), [name] "+c"(name), [line] "+d"(line)
                     : [function] "i"(RT_SERIALIZE), [file] "i"(RT_FILE), [writer] "r"(writer), [fn] "r"(WRITE_U32)
                     : CLOBBERS);
}

static u32 read32(u8 *reader) {
    u8 *at = AT(reader, 4, u8 *);
    AT(reader, 4, u8 *) = at + 4;
    return AT(at, 0, u32);
}

/* ---- the vtable's slots ------------------------------------------------ */

/* +78h tick_impl (dt, time), state 4: Spring's 0x311890. */
THISCALL void rt_tick_impl(struct rattle *r, u32 dt, u32 time) {
    (void)time;
    r->left_ms = r->left_ms > dt ? r->left_ms - dt : 0;
}

/* +80h enable_active (time): Spring's 0x3118D0, the duration as the holder's
 * artefact_duration modifier makes it. */
THISCALL void rt_enable_active(struct rattle *r, u32 time) {
    (void)time;
    u8 *player = holder(r);
    float duration = (float)r->duration_ms;
    if (player) duration = modified(player, M_DURATION, duration);
    r->left_ms = duration > 0 ? (u32)duration : 0;          /* floor: positive */
}

/* +88h disable_active: from the active's end and a death (Lifebone's 0x312450). */
THISCALL void rt_disable_active(struct rattle *r) { r->left_ms = 0; }

/* +8Ch active_effect_ended: Spring's 0x311990 (0.20e's and 0.26g's Rattle: true at once). */
THISCALL u32 rt_active_effect_ended(struct rattle *r) { return r->left_ms == 0; }

/* +48h the hud's info: artefact_base's, then the active's seconds (Spring's 0x311A60). */
THISCALL void rt_ui_info(struct rattle *r, u8 *out) {
    ((THISCALL void (*)(void *, u8 *))BASE_UI_INFO)(r, out);
    AT(out, 2, u16) = (u16)((r->left_ms + 999) / 1000);
}

/* +68h serialize: artefact_base's, then the time left in state 4 (Spring's 0x3119A0). */
THISCALL void rt_serialize(struct rattle *r, u8 *writer, void *dumper) {
    ((THISCALL void (*)(void *, void *, void *))BASE_SERIALIZE)(r, writer, dumper);
    if (AT(r, A_STATE, u32) == ST_ACTIVE) field_u32(writer, &r->left_ms, 0x41, "m_time_left_to_deactivate");
}

/* +6Ch deserialize: its mirror (Spring's 0x3119E0); 0 out of state 4. */
THISCALL void rt_deserialize(struct rattle *r, u8 *reader, void *a, void *b) {
    ((THISCALL void (*)(void *, void *, void *, void *))BASE_DESERIALIZE)(r, reader, a, b);
    r->left_ms = AT(r, A_STATE, u32) == ST_ACTIVE ? read32(reader) : 0;
}

/* artefact<artefact_rattle_core>'s (0xA06124, its RTTI locator 0xA4C044
 * before it) with the Rattle's slots. */
__attribute__((used)) const u32 rt_vtable[1 + 36] = {
    0xA4C044,
    0x260860, 0xF1900, 0xF1F00, 0xF1F40, 0xFDA90, 0xFDA90, 0xFDA90, 0x1AF330, 0x2FFC40, 0x2FFC80, 0x243590,
    0x313300, 0x313470, 0x313490, 0x313470, 0x1AF330, 0x2FFCE0, 0x2FFD50,
    (u32)rt_ui_info,                               /* +48h (0x313880) */
    0x300040, 0x300040, 0x246380, 0x246380, 0x300040, 0x300040, 0x300040,
    (u32)rt_serialize,                             /* +68h (0x313670) */
    (u32)rt_deserialize,                           /* +6Ch (0x313760) */
    0x25FB30, 0x313200,
    (u32)rt_tick_impl,                             /* +78h (0x15A4B0 `ret 8`) */
    0x1AF330,                                      /* +7Ch passive on: nothing to link */
    (u32)rt_enable_active,                         /* +80h (0x75AF0 `ret 4`) */
    0x1AF330,                                      /* +84h passive off */
    (u32)rt_disable_active,                        /* +88h (0x1AF330 `ret`) */
    (u32)rt_active_effect_ended,                   /* +8Ch (0x372420 `mov al,1 / ret`) */
};
_Static_assert(sizeof rt_vtable == 37 * 4, "36 slots");

/* ---- the cook ---------------------------------------------------------- */

/* After Rattle's constructor (0x25E72C, the cook's case for type 8): the
 * vtable and the config's numbers. 0.26g's artefacts keep the duration in
 * data/active (0.20e's Rattle in data). */
STDCALL struct rattle *rt_construct(struct rattle *r, const cfg *root) {
    if (!r) return 0;
    memset((u8 *)r + RATTLE_SIZE, 0, sizeof *r - RATTLE_SIZE);
    AT(r, 0, const u32 *) = rt_vtable + 1;
    const cfg *data = has(root, "data") ? child(root, "data") : 0;
    const cfg *passive = data && has(data, "passive") ? child(data, "passive") : 0;
    const cfg *active = data && has(data, "active") ? child(data, "active") : 0;
    float duration = number_or(active, "duration", 0);
    r->duration_ms = duration > 0 ? (u32)(duration * 1000.0f + 0.5f) : 0;
    r->passive_steps = number_or(passive, "steps_sound_volume_3rd_person", 1);
    r->active_steps = number_or(active, "steps_sound_volume_3rd_person", 1);
    r->active_shots = number_or(active, "shots_sound_volume_3rd_person", 1);
    return r;
}

/* ---- the quiet --------------------------------------------------------- */

static float clamp01(float x) { return x < 0 ? 0 : x > 1 ? 1 : x; }

/* The gains of what `player` makes for others: his steps and movement noise
 * (each Rattle he owns, x its passive's factor or, active, its active's),
 * his shots (x each active Rattle's). */
static void gains(const u8 *player, float *steps, float *shots) {
    *steps = 1, *shots = 1;
    u8 *inventory = player ? AT(player, P_INVENTORY, u8 *) : 0;
    if (!inventory) return;
    for (u32 slot = FIRST_ARTEFACT_SLOT; slot <= LAST_ARTEFACT_SLOT; ++slot) {
        const struct rattle *r = AT(inventory, INV_SLOTS + 4 * slot, const struct rattle *);
        if (!r || AT(r, 0, const u32 *) != rt_vtable + 1) continue;
        u32 state = AT(r, A_STATE, u32);
        if (state == ST_PASSIVE) {
            *steps *= clamp01(r->passive_steps);
        } else if (state == ST_ACTIVE) {
            *steps *= clamp01(r->active_steps);
            *shots *= clamp01(r->active_shots);
        }
    }
}

/* gain -> quiet: 0 the stock, 65535 silent */
static u32 quiet(float gain) {
    float q = (1.0f - clamp01(gain)) * (float)QUIET_MAX + 0.5f;
    return q >= (float)QUIET_MAX ? QUIET_MAX : (u32)q;
}

/* on_player_sound_event's and on_landing's hooks: the quiet of `player`'s
 * steps, for the point proxies 0x657C0 makes until the hook clears it. */
STDCALL void rt_steps(const u8 *player) {
    float steps, shots;
    gains(player, &steps, &shots);
    G->pending = quiet(steps);
}

/* play_sound_callback's point proxy (0x297F79): `effect` a weapon's
 * weapon_sound_effect, `out` &the proxy's pointer. A fire handler's effect
 * gets its user's shot quiet. */
STDCALL void rt_shot(const u8 *effect, u8 *const *out) {
    u8 *proxy = out ? *out : 0;
    if (!proxy || !effect) return;
    int fire = 0;
    for (u32 i = 0; i < sizeof FIRE_HANDLERS / sizeof FIRE_HANDLERS[0]; ++i)
        fire |= AT(effect - FIRE_HANDLERS[i].effect, 0, u32) == FIRE_HANDLERS[i].vtable;
    if (!fire) return;
    const u8 *weapon = AT(effect, E_WEAPON, const u8 *);
    float steps, shots;
    gains(weapon ? AT(weapon, W_USER, const u8 *) : 0, &steps, &shots);
    u32 q = quiet(shots);
    if (q) AT(proxy, PROXY_QUIET, u16) = (u16)q;
}
