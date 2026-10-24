/* Patch 64: the interface settings, saved variables of the gameplay options
 * that reach the match's movies (docs/notes/interface-level.md;
 * client_patch.md section 3.57). The entries are interface_026g.asm's; this
 * is their logic.
 *
 * A setting is a row of `interface_settings`: a console variable of the
 * game's own kind for a saved choice (cc_u32, as the hud's minimap_position:
 * kept in user.cfg, set by the console and the options), a row of the
 * gameplay options (an options_item_int, the arrow selector `< value >`, its
 * values' labels localization keys), and the method of the match's movies
 * that gets its value. One way for each:
 * - interface_registered: the hud's constructor (before the configs run)
 *   makes each row's variable, so user.cfg sets it and the options find it;
 * - interface_made: the gameplay options' tab (options_tab 0x262300, tab 0)
 *   has its 16 rows of the game and then one per setting (an
 *   options_item_int, index 16 + i); patch_026g.py makes the tab's count and
 *   fill_settings_data's label count 16 + INTERFACE_COUNT;
 * - interface_labelled: fill_settings_data's gameplay labels get each row's
 *   label key at 16 + i;
 * - interface_sync, every frame of a match after update_ui: each movie (the
 *   hud's, the player icons') gets root.<method>(value) when the value it
 *   was given last differs (a new movie: again). So the options' Accept, the
 *   console and user.cfg all reach a running match in the next frame.
 * A mode may force a least value (interface_force): the movies get the
 * larger of the setting and that (Blackout: the level Disabled). */
#include "engine_026g.h"

typedef unsigned char u8;
typedef unsigned int u32;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))

enum {
    GAME_ROWS = 16,                /* the gameplay options' own rows */
    COMMAND_SIZE = 0x4C,           /* cc_value<unsigned int> (0x1DCD0 fills up to +48h) */
    CC_U32 = 0x9E7CCC,             /* the vtable the hud's minimap_position gets (0x233911) */
    SAVED = 1,                     /* kept in user.cfg */
    KIND = 2,                      /* the kind user.cfg's lines may set (the configs' mask 2) */
    ITEM_SIZE = 0x20,              /* options_item_int */
    ITEM_WHERE = 0xAD8108,         /* its allocation's name (0x2624B4) */
    ALLOCATOR = 0x1F1D9C4,         /* the options' allocator (0x2624AE) */
    LABEL_SIZE = 0x4C,             /* a label of fill_settings_data (a buffer_string) */
    HUD_MOVIE = 0x08,              /* the hud's movie holders: [[+n]+108h]+4 */
    ICONS_MOVIE = 0x10,
    HOLDER_UI = 0x108,
    UI_MOVIE = 0x04,
    GFX_UINT = 4,                  /* a GFx value's type: an unsigned int at +8 */
    NONE = 0xFFFFFFFF,             /* a movie given nothing yet */
};

struct interface_setting {
    u8 command[COMMAND_SIZE];      /* the console variable */
    u32 value;                     /* its value, the index of the choice */
    u32 least;                     /* a mode's least (interface_force) */
    const char *name;              /* the variable's name, the option's */
    const char *label;             /* the option's label key */
    const char *const *values;     /* the choices' label keys */
    u32 count;
    const char *method;            /* root.<method>(value) of the match's movies */
};

/* The level of the interface in a match (docs/notes/interface-level.md):
 * 0 Full, 1 Minimal, 2 Disabled (as F11 hides it). */
static const char *const LEVELS[] = {"st_interface_full", "st_interface_minimal", "st_interface_disabled"};

struct interface_setting interface_settings[] = {
    {.name = "g_interface_level", .label = "st_interface_level_option", .values = LEVELS, .count = 3,
     .method = "root.set_interface_level"},
};

#define COUNT (sizeof(interface_settings) / sizeof(interface_settings[0]))

/* How many settings: patch_026g.py reads it from the link for the counts. */
const u32 interface_count = COUNT;

/* What each movie was given: the movie, then each setting's value. */
static void *given_movie[2];
static u32 given[2][COUNT];

/* The hud's constructor registered its show_quests_progress: each
 * setting's variable, once (the constructor's own guard). */
void interface_registered(void) {
    for (u32 i = 0; i < COUNT; i++) {
        struct interface_setting *s = &interface_settings[i];
        engine_cc_u32(s->command, s->name, &s->value, 0, s->count - 1, SAVED, KIND);
        AT(s->command, 0, u32) = CC_U32;
    }
}

/* The gameplay options' tab made its 16 rows: one per setting after them. */
void interface_made(u8 *tab) {
    void **items = AT(tab, 0, void **);
    for (u32 i = 0; i < COUNT; i++) {
        struct interface_setting *s = &interface_settings[i];
        void *item = engine_malloc(AT(ALLOCATOR, 0, void *), ITEM_SIZE, engine_allocation_name((void *)ITEM_WHERE));
        if (item)
            item = engine_options_item_int(GAME_ROWS + i, tab, item, s->name, s->values, s->count);
        items[GAME_ROWS + i] = item;
    }
}

/* fill_settings_data made the gameplay labels: each setting's after its 16. */
void interface_labelled(u8 *labels) {
    for (u32 i = 0; i < COUNT; i++)
        engine_string_assign(labels + (GAME_ROWS + i) * LABEL_SIZE, interface_settings[i].label);
}

/* A mode's least value of setting `i` (0: none). */
void interface_force(u32 i, u32 least) {
    if (i < COUNT)
        interface_settings[i].least = least;
}

static void *movie_of(u8 *hud, u32 holder) {
    u8 *at = AT(hud, holder, u8 *);
    if (!at)
        return 0;
    at = AT(at, HOLDER_UI, u8 *);
    return at ? AT(at, UI_MOVIE, void *) : 0;
}

/* After the hud's update_ui: each movie gets a setting's value when it changed. */
void interface_sync(u8 *hud) {
    void *movies[2] = {movie_of(hud, HUD_MOVIE), movie_of(hud, ICONS_MOVIE)};
    for (u32 m = 0; m < 2; m++) {
        if (movies[m] != given_movie[m]) {
            given_movie[m] = movies[m];
            for (u32 i = 0; i < COUNT; i++)
                given[m][i] = NONE;
        }
        if (!movies[m])
            continue;
        for (u32 i = 0; i < COUNT; i++) {
            struct interface_setting *s = &interface_settings[i];
            u32 value = s->value > s->least ? s->value : s->least;
            if (value >= s->count)
                value = s->count - 1;
            if (given[m][i] == value)
                continue;
            u32 argument[6] = {0, GFX_UINT, value, 0, 0, 0};
            engine_movie_invoke(movies[m], s->method, 0, argument, 1);
            given[m][i] = value;
        }
    }
}
