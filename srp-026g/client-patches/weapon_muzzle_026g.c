/* Patch 68, the muzzle device (build 2777): a weapon whose config names a
 * model at `data/srp_muzzle_model` (a muzzle device's modification adds it,
 * the weapon kit's slot 4) loads it with the weapon and draws it at the
 * weapon's barrel_point, moved `data/srp_muzzle_offset` metres forward along
 * the barrel (each weapon's own: the stock muzzle brakes are part of the weapon
 * models). README patch 68; client_patch.md section 3.56;
 * ../docs/notes/weapon-kit.md. weapon_muzzle_026g.asm holds the hooks; here
 * what they compute.
 *
 * The weapon object (weapon_cook 0x24A780) is 4710h bytes and a pointer for
 * each of its user animations, bullet shells and shot effects after them
 * (+0x4710; the shells' at [+0x820], the shots' at [+0x81C], their counts
 * the bytes +0x825 and +0x824); the patch makes it EXTENSION bytes more and
 * keeps its own after the last array (weapon_muzzle_extension). */
#include "engine_026g.h"

#define EXTENSION 0x50u

/* The weapon's own part (zeroed when the weapon is made). */
struct muzzle {
    void *model;            /* the device's model instance (a resource, +0xD0 its count), or 0 */
    float offset;           /* metres along the barrel from barrel_point */
    unsigned int added;     /* in the scene (as the weapon's model) */
    unsigned int pad;
    float matrix[16];       /* where it is drawn: barrel_point's, moved */
};

_Static_assert(sizeof(struct muzzle) == EXTENSION, "the extension");

/* A config node: +0 its value (a number, a string's address, its
 * children), +0x14 its type (2 a float, 5 a string), +0x16 its count. */
struct node {
    union {
        unsigned int u;
        int i;
        float f;
        const char *s;
    } value;
    unsigned char pad[0x10];
    unsigned short type;
    unsigned short count;
};

#define T_FLOAT 2
#define T_STRING 5

static const struct node *child(const void *table, const char *key)
{
    if (table == 0 || !engine_cfg_has(table, key))
        return 0;
    return (const struct node *)engine_cfg_child(table, key);
}

/* The weapon's own part: after its shot effects' pointers. */
struct muzzle *weapon_muzzle_extension(const unsigned char *weapon)
{
    const unsigned char *shots = *(const unsigned char *const *)(weapon + 0x81C);
    return (struct muzzle *)(shots + 4u * weapon[0x824]);
}

/* The device's model path of the weapon config `root` (its data/
 * srp_muzzle_model, a string), or 0. */
const char *weapon_muzzle_model(const void *root)
{
    const struct node *model = child(child(root, "data"), "srp_muzzle_model");
    return model != 0 && model->type == T_STRING ? model->value.s : 0;
}

static float offset_of(const void *root)
{
    const struct node *offset = child(child(root, "data"), "srp_muzzle_offset");
    if (offset == 0)
        return 0.0f;
    return offset->type == T_FLOAT ? offset->value.f : (float)offset->value.i;
}

/* The weapon was made (weapon_cook 0x24A780, its arrays zeroed): its part
 * zeroed too. */
void weapon_muzzle_made(unsigned char *weapon)
{
    unsigned char *part = (unsigned char *)weapon_muzzle_extension(weapon);
    for (unsigned int i = 0; i < EXTENSION; i++)
        part[i] = 0;
}

/* The weapon cook's sub-resources came (its callback 0x24ACF0: `results`
 * the queries' results, 2E0h each from +0x50, a resource at +0xDC;
 * `config` the weapon config's resource, its root at +0x108). The device's
 * is the last the cook asked for (weapon_muzzle_request: after the scope's
 * when the config names one, the model, each user animation of
 * user_animations_in_place/preview, the bullet shells' and the shot
 * effects'): kept, its count up. */
void weapon_muzzle_loaded(const unsigned char *results, const unsigned char *config, unsigned char *weapon)
{
    const void *root = *(const void *const *)(config + 0x108);
    struct muzzle *part = weapon_muzzle_extension(weapon);
    if (weapon_muzzle_model(root) == 0)
        return;
    unsigned int index = 1;
    if (child(child(root, "addons"), "rifle_scope_dict_id") != 0)
        index++;
    const struct node *preview = (const struct node *)engine_cfg_child(
        engine_cfg_child(root, "user_animations_in_place"), "preview");
    index += preview->count + weapon[0x825] + weapon[0x824];
    unsigned char *model = *(unsigned char *const *)(results + 0x50 + 0x2E0 * index + 0xDC);
    if (model == 0)
        return;
    __atomic_add_fetch((unsigned int *)(model + 0xD0), 1, __ATOMIC_SEQ_CST);
    part->model = model;
    part->offset = offset_of(root);
}

/* Where the device is drawn: barrel_point's matrix (+0x620, weapon::draw's
 * of this frame; rows, the fourth the point) with its point moved `offset`
 * forward, against its third row (a weapon's barrel and a device's model run
 * along -Z from their origins). */
void weapon_muzzle_place(unsigned char *weapon)
{
    struct muzzle *part = weapon_muzzle_extension(weapon);
    const float *barrel = (const float *)(weapon + 0x620);
    for (unsigned int i = 0; i < 16; i++)
        part->matrix[i] = barrel[i];
    for (unsigned int i = 0; i < 3; i++)
        part->matrix[12 + i] -= part->offset * barrel[8 + i];
}
