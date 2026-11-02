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
static float kit_float(const void *modifiers, u32 index)
{
    return *(const float *)((const u8 *)modifiers + index * 4);
}
/* The selected kit's 68 player modifier floats: the lobby profile (+6A0h),
 * its kit index (+4632h), the kit's block (+0F4h + kit x 704h). */
enum { KIT_MODIFIERS = 68 };
static const void *selected_kit_modifiers(const void *menu)
{
    const void *profile = pointer(menu, 0x6a0);
    const u32 kit = *((const u8 *)profile + 0x4632);
    return (const u8 *)profile + 0xf4 + kit * 0x704;
}
/* The weapon card's in-match view (docs/notes/alt-weapon-stats.md): every one
 * of the kit's own floats, by index (srp sends the skills' sums there,
 * session wire.rs); the kit's items are the movie's to add, as for the weight.
 * Sent with every weight callback (update_player_items_weight 0x2221E0, from
 * on_profile_changed 0x222CDD and an inventory action 0x2759EB). */
static void send_kit_modifiers(const void *menu, void *movie)
{
    const void *modifiers = selected_kit_modifiers(menu);
    struct gfx_value args[KIT_MODIFIERS];
    for (u32 index = 0; index < KIT_MODIFIERS; index++)
        args[index] = value(kit_float(modifiers, index));
    engine_movie_invoke(movie, "root.set_kit_modifiers", 0, args, KIT_MODIFIERS);
}
/* Sprint eligibility due to weight/Heavyweight only, not stamina or injury. */
enum sprint { WEIGHT_PERMITS_SPRINT, WEIGHT_BLOCKS_SPRINT, HEAVYWEIGHT_BLOCKS_SPRINT };
u32 weight_ui_invoke(const void *menu, void *movie, const struct gfx_value *original)
{
    const char *method = "root.update_player_items_weight";
    send_kit_modifiers(menu, movie);
    if (configuration.kind != LOADED || configuration.as.loaded.menu != menu)
        return engine_movie_invoke(movie, method, 0, original, 2);
    const struct parameters p = configuration.as.loaded;
    const void *modifiers = selected_kit_modifiers(menu);
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
    /* The carry limit's parts: the menu's base limit (+3118h, the B the lobby's
     * apply_kit_parameter(5) 0x2F7660 starts from at 0x2221F3) and the kit's own
     * floats of modifiers 5 (skills: Athlete, +2 kg a rank) and 26 (Packrat, the
     * weight penalty's scale, -5 % a rank); the items' are the movie's. */
    const float base = *(const float *)((const u8 *)menu + 0x3118);
    /* Sprint stamina use (player_stamina::sprint 0x329A70): the drain through
     * apply_modifier 19 (stamina_spending_speed: items, the weight's node) then
     * 67 (stamina_spending_speed_skill), both multiplicative; the kit's own floats
     * of the two, the items' are the movie's. */
    struct gfx_value args[12] = {
        value(current), value(maximum),
        value(heavy > 0 ? 0 : (double)(movement * q)),
        value(p.spending * q), value(sprint),
        value(kit_float(modifiers, 4)), value(bear_arms > 0),
        value(base), value(kit_float(modifiers, 5)), value(kit_float(modifiers, 26)),
        value(kit_float(modifiers, 19)), value(kit_float(modifiers, 67))
    };
    return engine_movie_invoke(movie, method, 0, args, 12);
}
