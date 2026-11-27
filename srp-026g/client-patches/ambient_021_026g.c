/* 0.21 ambient lights multiply existing indirect light, even above intensity 1.
 * Native 0.26 instead selects an additive pass for those lights. Preserve the
 * original linear RGB * intensity through the native multiplicative pass.
 * No authored light is selected by position, name or measured brightness. */
typedef unsigned char u8;
struct ambient_parameters { float red, green, blue, intensity; };
enum ambient_model { NATIVE_AMBIENT, ORIGINAL_021_AMBIENT };

static const void *pointer(const void *p, unsigned offset)
{
    return *(const void *const *)((const u8 *)p + offset);
}

static const char *after_prefix(const char *name, const char *prefix)
{
    while (*prefix) {
        if (*name++ != *prefix++) return 0;
    }
    return name;
}

static enum ambient_model model_for_game(const void *game)
{
    const char *name = pointer(game, 0x55e0);
    if (!name) return NATIVE_AMBIENT;
    const char prefix[] = "v021d_level_0";
    for (unsigned i = 0; i < sizeof(prefix) - 1; i++)
        if (name[i] != prefix[i]) return NATIVE_AMBIENT;
    name += sizeof(prefix) - 1;
    const char level = *name++;
    if (level < '1' || level > '6') return NATIVE_AMBIENT;
    const char *mode = after_prefix(name, "/day_");
    if (mode) {
        if ((*mode != '2' && *mode != '4' && *mode != '8') || mode[1])
            return NATIVE_AMBIENT;
    } else {
        mode = after_prefix(name, "/rain_");
        if (!mode) mode = after_prefix(name, "/evening_");
        if (!mode || (*mode != '2' && *mode != '4') || mode[1])
            return NATIVE_AMBIENT;
    }
    return ORIGINAL_021_AMBIENT;
}

static enum ambient_model model_for(const void *object)
{
    /* The object's actual owning scene, not the game's stale last-map name.
     * A lobby scene can share the game but is never its embedded game world.
     * query_players assigns map_name before loading/inserting the project;
     * this also works before the match wrapper exists and during replay load. */
    const void *scene = pointer(object, 0x108);
    if (!scene) return NATIVE_AMBIENT;
    const void *game = pointer(scene, 0x9c);
    if (!game || scene != (const u8 *)game + 0x18) return NATIVE_AMBIENT;
    return model_for_game(game);
}

int particle_021_original_intensity(const void *particle_engine, const void *engine_world)
{
    /* particle_engine+8 is its owning render::scene (get_scene 0x14F080).
     * base_game_scene+4 holds that same resource pointer: ambient insert
     * 0x290ABE -> scene_renderer 0x23AB4 -> engine::world 0x118C3D.
     * Match the actual scene before consulting the game name. Lobby and
     * other concurrently rendered scenes must retain their native gain. */
    if (!engine_world) return 0;
    const void *game = pointer(engine_world, 0x2d0);
    if (!game) return 0;
    const void *scene = pointer(particle_engine, 8);
    if (!scene || scene != pointer(game, 0x18 + 4)) return 0;
    return model_for_game(game) == ORIGINAL_021_AMBIENT;
}

void ambient_021_insert(const void *object, struct ambient_parameters *light)
{
    if (model_for(object) != ORIGINAL_021_AMBIENT || !(light->intensity > 1)) return;
    const float intensity = light->intensity;
    light->red *= intensity;
    light->green *= intensity;
    light->blue *= intensity;
    light->intensity = 1;
}
