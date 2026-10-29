/* Lobby-only weight explanation. Read the same player config and profile as
 * the native weight display; never alter simulation or profile parameters. */
#include "engine_026g.h"
typedef unsigned int u32;
typedef unsigned char u8;
struct engine_cfg { union { float number; int integer; } value; u32 high, name, name_high, crc; unsigned short type, count; };
struct gfx_value { u32 owner, type; double number; u32 tail[2]; };
_Static_assert(sizeof(struct gfx_value) == 24, "GFx::Value ABI");
struct parameters { const void *menu; float minimum, movement, spending; };
enum readiness { UNAVAILABLE, LOADED };
struct configuration { enum readiness kind; union { struct parameters loaded; } as; };
static struct configuration configuration;

static const void *pointer(const void *p, u32 offset) { return *(const void *const *)((const u8 *)p + offset); }
static float number(const struct engine_cfg *p, const char *name)
{
    const struct engine_cfg *v = engine_cfg_child(p, name);
    return v->type == 2 ? v->value.number : (float)v->value.integer;
}
void weight_ui_load(const void *menu, const struct engine_cfg *root)
{
    const struct engine_cfg *player = engine_cfg_child(root, "player");
    const struct engine_cfg *stamina = engine_cfg_child(player, "stamina_params");
    configuration = (struct configuration){ LOADED, { .loaded = {
        menu, number(stamina, "min_carried_weight"),
        number(stamina, "max_carried_weight_movement_speed_modifier"),
        number(stamina, "max_carried_weight_spending_speed_modifier")
    } } };
}
static struct gfx_value value(double number)
{
    return (struct gfx_value){ 0, 5, number, {0, 0} };
}
/* The kit's own part of movement-speed modifier 4 (skills): the float the
 * native sum starts from (apply_modifier 0x2D5AE0: S = the kit's float plus
 * its item nodes; in the lobby the items are the movie's to add). */
static float kit_movement(const void *modifiers)
{
    return *(const float *)((const u8 *)modifiers + 4 * 4);
}
/* Sprint eligibility due to weight/Heavyweight only, not stamina or injury. */
enum sprint { WEIGHT_PERMITS_SPRINT, WEIGHT_BLOCKS_SPRINT, HEAVYWEIGHT_BLOCKS_SPRINT };
u32 weight_ui_invoke(const void *menu, void *movie, const struct gfx_value *original)
{
    const char *method = "root.update_player_items_weight";
    if (configuration.kind != LOADED || configuration.as.loaded.menu != menu)
        return engine_movie_invoke(movie, method, 0, original, 2);
    const struct parameters p = configuration.as.loaded;
    const void *profile = pointer(menu, 0x6a0);
    const u32 kit = *((const u8 *)profile + 0x4632);
    const void *modifiers = (const u8 *)profile + 0xf4 + kit * 0x704;
    const float current = (float)original[0].number, maximum = (float)original[1].number;
    /* Boundary parsing: a bad config never produces a fictional penalty. */
    if (!(maximum > p.minimum))
        return engine_movie_invoke(movie, method, 0, original, 2);
    float q = (current - p.minimum) / (maximum - p.minimum);
    if (q < 0) q = 0;
    if (q > 1) q = 1;
    /* Match player_stamina: these are player modifier indices, outside the
     * limited item-parameter domain accepted by profile::apply_kit_parameter. */
    const float movement = engine_apply_modifier(modifiers, 26, p.movement, 1);
    const float heavy = engine_apply_modifier(modifiers, 32, 0, 1);
    const float runner = engine_apply_modifier(modifiers, 31, 0, 1);
    /* Bear Arms (skill 112): additive modifier 62; above 0 the drawn weapon's
     * movement penalty is not added (0x2E2C5C..0x2E2C9C). */
    const float bear_arms = engine_apply_modifier(modifiers, 62, 0, 1);
    const enum sprint sprint = heavy > 0 ? HEAVYWEIGHT_BLOCKS_SPRINT :
        current >= maximum && runner <= 0 ? WEIGHT_BLOCKS_SPRINT : WEIGHT_PERMITS_SPRINT;
    struct gfx_value args[7] = {
        value(current), value(maximum),
        value(heavy > 0 ? 0 : (double)(movement * q)),
        value(p.spending * q), value(sprint),
        value(kit_movement(modifiers)), value(bear_arms > 0)
    };
    return engine_movie_invoke(movie, method, 0, args, 7);
}
