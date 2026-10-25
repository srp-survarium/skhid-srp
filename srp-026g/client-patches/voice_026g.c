/* Voice chat (exe patch 66): team push-to-talk in a match through
 * skhid_voice.dll (srp-026g/client-voice, beside the exe) and srp's voice
 * relay on the match port + 1. The rules: ../docs/notes/voice-chat.md; the
 * proof of every address: docs/notes/client_patch.md section 3.55 and
 * tools/re-kit/v026g. voice_026g.asm hands the game's sites to the vc_*
 * functions here; check_voice_026g.py runs them.
 *
 * - the options: s_chat_volume (0..100), s_use_microphone and s_mic_sens
 *   (0..100), the sound tab's dead voice controls, and g_hide_spam, the
 *   gameplay tab's dead option 5, now "Disable voice chat"; the mutes in
 *   srp_voice_mutes (the muted voice ids, eight hex digits and `|` each). All kept in
 *   user.cfg;
 * - a match: the DLL loaded at the first connect, started with the match
 *   server's host, its port + 1 and the session; stopped when the world ends
 *   or is left;
 * - every frame: the options to the DLL, the muted slots, which teammates are
 *   far (beyond 25 m, or either of the two dead: the radio filter), and the
 *   hud's list of who talks (root.srp_voice_talking) on a change; push-to-
 *   talk let go when its key is no longer held or the game lost the focus;
 * - the keys: the stock `ptt` action (47h, P) held to talk; "mute the last
 *   speaker" (37h, the action table's dead serialize_player_state, renamed);
 * - the Esc menu of a match: its voice panel (root.srp_voice_panel) with the
 *   teammates and their mutes; its clicks (srp_voice_mute, srp_voice_mute_all).
 *
 * A mute is kept by the player's voice id: srp's ROSTER gives one per place
 * of the team (a hash of the account with a server key: stable, opaque, and
 * not the name, which an account can change). The game knows no account ids
 * (the 0xE4 profiles carry the team, the name, the clan and the kits). The
 * panel and the hud show the names. Every address is a VA of the original exe
 * (image base 0x10000, never rebased). */
#include "engine_026g.h"

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))
#define THISCALL __attribute__((thiscall))
#define STDCALL __attribute__((stdcall))

/* ---- the game ----------------------------------------------------------- */

#define IAT_GET_MODULE_HANDLE 0x87003C /* [import] kernel32 GetModuleHandleA */
#define IAT_LOAD_LIBRARY 0x870080      /* [import] kernel32 LoadLibraryA */
#define IAT_GET_PROC_ADDRESS 0x870084  /* [import] kernel32 GetProcAddress */
#define IAT_FREE_LIBRARY 0x870088      /* [import] kernel32 FreeLibrary */

/* the game (game::game's object) */
#define GAME_WORLD 0x18            /* the game world, the state a match's menu opens over */
#define GAME_TRANSLATOR 0x52A8     /* text_translator (translate_text 0x1FF0F0) */
#define GAME_BINDER 0x5344         /* -> the key binder (get_binded_action 0x207D50) */
#define GAME_NETWORK 0x53AC        /* -> network_client */
#define GAME_OPTIONS 0x57F4        /* game_options: the main (Esc) menu */
/* network_client */
#define NETWORK_LOCAL_PLACE 0x1F1C44 /* u8: the game's own roster place; 0xFF none */
#define NETWORK_MATCH 0x1F1C48     /* -> the match client (vtable 0xA08864) */
/* the match client: +0Ch the static info (0xE4) */
#define MATCH_STATIC 0xC
#define STATIC_COUNT 0x577B5       /* u8: the places (the late join raises it) */
#define STATIC_PROFILE 0x45FC      /* a place's profile: its size */
#define PROFILE_NAME 0x8           /* char[64], UTF-8 */
#define PROFILE_TEAM 0x45EC        /* u32; 3 none */
/* the game world (its vtable 0x9FC4F4) */
#define WORLD_GAME 0x9C            /* -> the game */
#define WORLD_CORE 0x4D4C          /* -> +1F8h: game_world_core */
#define WORLD_HUD_MOVIE 0x204      /* -> the hud movie's holder: [+108h]+4 the movie */
#define CORE_OFFSET 0x1F8
#define CORE_PLACES 0x18458        /* -> places, 30h each: [3] players, +Ch the current kit */
#define PLACE_SIZE 0x30
#define PLACE_KIT 0xC
#define PLAYER_ALIVE 0x33C         /* m_is_alive */
#define PLAYER_IN_WORLD 0x33F      /* in the world's tick lists */
#define PLAYER_POSITION 0x43C      /* the feet: x, y, z */
/* game_options */
#define OPTIONS_MOVIE 0xC          /* -> the main menu movie's holder */
#define OPTIONS_GAME 0x34          /* -> the game */
#define HOLDER_MOVIE 0x108         /* a holder's -> +4 the movie */
/* console variables: cc_value<bool> 0x1DDB0 (48h, vtable cc_bool 0x9E7C4C),
 * cc_value<float> 0x1DD40 (4Ch, cc_float 0x9E7C6C: voice_cc_float),
 * cc_string 0xEC860 (48h); kind 2 and saved: user.cfg, as patch 58's */
#define CC_BOOL_VTABLE 0x9E7C4C
#define CC_KIND_USER 2
/* the actions (the binder's ids; the chat keys' kind 8) */
#define ACTION_PTT 0x47
#define ACTION_MUTE_LAST 0x37
#define KINDS_CHAT 8
#define KEY_DOWN 1
#define KEY_UP 3
/* GFx values (Scaleform::GFx::Value, 18h bytes) */
enum { GFX_BOOL = 2, GFX_INT = 3, GFX_UINT = 4, GFX_NUMBER = 5, GFX_STRING = 6, GFX_MANAGED = 0x40 };

typedef struct {
    void *object;
    u32 type;
    union {
        double number;
        u32 word;
        u8 flag;
        const char *text;
        const char **managed;
    } value;
    u32 extra[2];
} gfx_value;
_Static_assert(sizeof(gfx_value) == 0x18, "a GFx::Value is 18h bytes");

void voice_cc_float(void *object, const char *name, float *value, float max, u32 saved, u32 kind);

/* ---- the DLL ------------------------------------------------------------ */

typedef int (*sv_version_t)(void);
typedef int (*sv_start_t)(const char *host, unsigned port, unsigned session);
typedef void (*sv_stop_t)(void);
typedef void (*sv_ptt_t)(int down);
typedef void (*sv_settings_t)(float volume, int mic_on, float mic_sens);
typedef void (*sv_mask_t)(unsigned mask);
typedef unsigned (*sv_talking_t)(void);
typedef int (*sv_slot_t)(void);
typedef unsigned (*sv_voice_id_t)(int slot);

struct voice_dll {
    sv_version_t version;
    sv_start_t start;
    sv_stop_t stop;
    sv_ptt_t ptt;
    sv_settings_t settings;
    sv_mask_t radio;
    sv_mask_t mute;
    sv_talking_t talking;
    sv_slot_t slot;
    sv_slot_t last_speaker;
    sv_voice_id_t voice_id;
};

#define VOICE_DLL_VERSION 1
#define MAX_PLACES 20
#define MUTES_SIZE 512
#define NEAR_SQUARED (25.0f * 25.0f)
#define SELF_BEFORE_WELCOME 31     /* sv_talking's bit for the player before its slot is known */
#define FLAG_SELF 1                /* HUD.srp_voice_talking's flags */
#define FLAG_RADIO 2

/* The state (voice.state). */
struct voice_state {
    u8 cc_volume[0x4C], cc_microphone[0x48], cc_sensitivity[0x4C], cc_off[0x48], cc_mutes[0x48];
    float volume;                  /* s_chat_volume, 0..100 */
    float sensitivity;             /* s_mic_sens, 0..100 */
    u8 microphone;                 /* s_use_microphone */
    u8 off;                        /* g_hide_spam: "Disable voice chat" */
    u8 registered;
    u8 loaded;                     /* 0 not tried, 1 loaded, 2 missing */
    char mutes[MUTES_SIZE];        /* srp_voice_mutes: "name|name|" */
    struct voice_dll dll;
    void *module;
    u8 started;                    /* sv_start'ed for this match */
    u8 ptt;                        /* push-to-talk held (sent to the DLL) */
    u8 hud_shown;                  /* the hud's list is not empty */
    u8 settings_sent;
    u32 ptt_key;                   /* the DIK of the key held */
    void *input_world;             /* the input world the key came from */
    void *game;                    /* the game of the match */
    float sent_volume, sent_sensitivity;
    int sent_microphone;
    u32 sent_mute, sent_radio;
    u32 shown_talking, shown_radio; /* the hud's list as last sent */
    u32 mute_version, mute_seen;   /* the mutes changed / as last applied */
    void *(STDCALL *foreground)(void);
    u32 (STDCALL *window_process)(void *window, u32 *process);
    u32 (STDCALL *current_process)(void);
};
struct voice_state voice;

/* ---- small helpers (no C library in the arena) -------------------------- */

static u32 length(const char *text) {
    u32 n = 0;
    while (text[n]) n++;
    return n;
}

static int same(const char *a, const char *b) {
    while (*a && *a == *b) a++, b++;
    return *a == *b;
}

static void *proc(void *module, const char *name) {
    void *(STDCALL *get)(void *, const char *) = *(void *(STDCALL **)(void *, const char *))IAT_GET_PROC_ADDRESS;
    return module ? get(module, name) : 0;
}

static void *movie_of(void *holder) {
    if (!holder) return 0;
    void *inner = AT(holder, HOLDER_MOVIE, void *);
    return inner ? AT(inner, 4, void *) : 0;
}

static void gfx_string(gfx_value *value, const char *text) {
    value->object = 0;
    value->type = GFX_STRING;
    value->value.number = 0;
    value->value.text = text;
    value->extra[0] = value->extra[1] = 0;
}

static void gfx_word(gfx_value *value, u32 type, u32 word) {
    value->object = 0;
    value->type = type;
    value->value.number = 0;
    value->value.word = word;
    value->extra[0] = value->extra[1] = 0;
}

/* An argument of a movie's call as a number (int, uint, Number, bool). */
static int gfx_number(const gfx_value *value, u32 *out) {
    switch (value->type & 0x3F) {
    case GFX_BOOL: *out = value->value.flag != 0; return 1;
    case GFX_INT: case GFX_UINT: *out = value->value.word; return 1;
    case GFX_NUMBER:
        if (!(value->value.number >= 0 && value->value.number < 4294967296.0)) return 0;
        *out = (u32)value->value.number;
        return 1;
    default: return 0;
    }
}

/* ---- the console variables ---------------------------------------------- */

/* game::game's world made (voice_register): before user.cfg runs and the
 * options look their variables up. */
void vc_register(void) {
    if (voice.registered) return;
    voice.registered = 1;
    voice.volume = 100.0f;
    voice.sensitivity = 50.0f;
    voice.microphone = 1;
    voice.off = 0;
    voice.mutes[0] = 0;
    voice_cc_float(voice.cc_volume, "s_chat_volume", &voice.volume, 100.0f, 1, CC_KIND_USER);
    voice_cc_float(voice.cc_sensitivity, "s_mic_sens", &voice.sensitivity, 100.0f, 1, CC_KIND_USER);
    engine_cc_bool(voice.cc_microphone, "s_use_microphone", &voice.microphone, 1, CC_KIND_USER);
    AT(voice.cc_microphone, 0, u32) = CC_BOOL_VTABLE;
    engine_cc_bool(voice.cc_off, "g_hide_spam", &voice.off, 1, CC_KIND_USER);
    AT(voice.cc_off, 0, u32) = CC_BOOL_VTABLE;
    engine_cc_string(voice.cc_mutes, "srp_voice_mutes", voice.mutes, MUTES_SIZE, 1);
}

/* ---- the mutes ---------------------------------------------------------- */

/* The mutes are voice ids (srp's ROSTER: a stable, opaque id per account;
 * a name can change hands): eight hex digits and `|` each, the oldest first. */
#define MUTE_RECORD 9

static const char HEX[] = "0123456789abcdef";

static int mute_at(u32 id) {
    char want[MUTE_RECORD];
    for (u32 i = 0; i < 8; i++) want[i] = HEX[(id >> (28 - 4 * i)) & 0xF];
    want[8] = '|';
    u32 used = length(voice.mutes);
    for (u32 at = 0; at + MUTE_RECORD <= used; at += MUTE_RECORD) {
        u32 i = 0;
        while (i < MUTE_RECORD && voice.mutes[at + i] == want[i]) i++;
        if (i == MUTE_RECORD) return (int)at;
    }
    return -1;
}

static int is_muted(u32 id) {
    return id && mute_at(id) >= 0;
}

/* Mutes or unmutes a voice id: at the end when added; the oldest go when the
 * variable is full. 0 (not known) is never kept. */
static void set_mute(u32 id, int muted) {
    int at = mute_at(id);
    if (!id) return;
    u32 used = length(voice.mutes);
    used -= used % MUTE_RECORD;    /* whatever user.cfg had past the last whole record */
    voice.mutes[used] = 0;
    if (muted && at < 0) {
        if (used + MUTE_RECORD + 1 > MUTES_SIZE) {
            for (u32 i = 0; i + MUTE_RECORD <= used; i++) voice.mutes[i] = voice.mutes[i + MUTE_RECORD];
            used -= MUTE_RECORD;
        }
        for (u32 i = 0; i < 8; i++) voice.mutes[used + i] = HEX[(id >> (28 - 4 * i)) & 0xF];
        voice.mutes[used + 8] = '|';
        voice.mutes[used + MUTE_RECORD] = 0;
    } else if (!muted && at >= 0) {
        for (u32 i = (u32)at; i + MUTE_RECORD <= used; i++) voice.mutes[i] = voice.mutes[i + MUTE_RECORD];
    } else {
        return;
    }
    voice.mute_version++;
}

/* A slot's voice id (0 while not known). */
static u32 id_of(int slot) {
    return voice.loaded == 1 && voice.started && slot >= 0 && slot < MAX_PLACES ? voice.dll.voice_id(slot) : 0;
}

/* ---- the match ---------------------------------------------------------- */

static u8 *static_info(void *game) {
    if (!game) return 0;
    void *network = AT(game, GAME_NETWORK, void *);
    if (!network) return 0;
    u8 *match = AT(network, NETWORK_MATCH, u8 *);
    return match ? match + MATCH_STATIC : 0;
}

static u32 places(const u8 *info) {
    u32 count = info ? AT(info, STATIC_COUNT, u8) : 0;
    return count > MAX_PLACES ? MAX_PLACES : count;
}

static const char *name_of(const u8 *info, u32 place) {
    return (const char *)(info + STATIC_PROFILE * place + PROFILE_NAME);
}

static u32 team_of(const u8 *info, u32 place) {
    return AT(info + STATIC_PROFILE * place, PROFILE_TEAM, u32);
}

static int local_place(void *game) {
    void *network = game ? AT(game, GAME_NETWORK, void *) : 0;
    u32 place = network ? AT(network, NETWORK_LOCAL_PLACE, u8) : 0xFF;
    return place < MAX_PLACES ? (int)place : -1;
}

/* A place's player, alive and in the world, or null. */
static const u8 *living(void *world, u32 place) {
    u8 *core = AT(world, WORLD_CORE, u8 *);
    if (!core) return 0;
    u8 *list = AT(core + CORE_OFFSET, CORE_PLACES, u8 *);
    if (!list) return 0;
    u8 *entry = list + PLACE_SIZE * place;
    u32 kit = AT(entry, PLACE_KIT, u8);
    if (kit >= 3) return 0;
    const u8 *player = AT(entry, 4 * kit, const u8 *);
    if (!player || !AT(player, PLAYER_ALIVE, u8) || !AT(player, PLAYER_IN_WORLD, u8)) return 0;
    return player;
}

/* The slots heard through the radio: a teammate farther than 25 m, or either
 * of the two not alive (a dead player's position is where it died). */
static u32 far_slots(void *world, const u8 *info, int self) {
    u32 count = places(info), mask = 0;
    const u8 *me = self >= 0 && (u32)self < count ? living(world, (u32)self) : 0;
    for (u32 place = 0; place < count; place++) {
        if ((int)place == self) continue;
        const u8 *them = living(world, place);
        if (!me || !them) {
            mask |= 1u << place;
            continue;
        }
        const float *a = (const float *)(me + PLAYER_POSITION), *b = (const float *)(them + PLAYER_POSITION);
        float dx = a[0] - b[0], dy = a[1] - b[1], dz = a[2] - b[2];
        if (dx * dx + dy * dy + dz * dz > NEAR_SQUARED) mask |= 1u << place;
    }
    return mask;
}

static u32 muted_slots(const u8 *info) {
    u32 count = places(info), mask = 0;
    for (u32 place = 0; place < count; place++)
        if (is_muted(id_of((int)place))) mask |= 1u << place;
    return mask;
}

/* ---- the DLL ------------------------------------------------------------ */

static int load(void) {
    if (voice.loaded) return voice.loaded == 1;
    voice.loaded = 2;
    void *(STDCALL *load_library)(const char *) = *(void *(STDCALL **)(const char *))IAT_LOAD_LIBRARY;
    void *module = load_library("skhid_voice.dll");
    if (!module) return 0;
    struct voice_dll dll = {
        (sv_version_t)proc(module, "sv_version"), (sv_start_t)proc(module, "sv_start"),
        (sv_stop_t)proc(module, "sv_stop"), (sv_ptt_t)proc(module, "sv_ptt"),
        (sv_settings_t)proc(module, "sv_settings"), (sv_mask_t)proc(module, "sv_radio"),
        (sv_mask_t)proc(module, "sv_mute"), (sv_talking_t)proc(module, "sv_talking"),
        (sv_slot_t)proc(module, "sv_slot"), (sv_slot_t)proc(module, "sv_last_speaker"),
        (sv_voice_id_t)proc(module, "sv_voice_id"),
    };
    void **each = (void **)&dll;
    for (u32 i = 0; i < sizeof dll / sizeof *each; i++)
        if (!each[i]) goto refused;
    if (dll.version() != VOICE_DLL_VERSION) goto refused;
    voice.dll = dll;
    voice.module = module;
    void *user32 = (*(void *(STDCALL **)(const char *))IAT_GET_MODULE_HANDLE)("user32.dll");
    void *kernel32 = (*(void *(STDCALL **)(const char *))IAT_GET_MODULE_HANDLE)("kernel32.dll");
    voice.foreground = (void *(STDCALL *)(void))proc(user32, "GetForegroundWindow");
    voice.window_process = (u32(STDCALL *)(void *, u32 *))proc(user32, "GetWindowThreadProcessId");
    voice.current_process = (u32(STDCALL *)(void))proc(kernel32, "GetCurrentProcessId");
    voice.loaded = 1;
    return 1;
refused:
    (*(int(STDCALL **)(void *))IAT_FREE_LIBRARY)(module);
    return 0;
}

static void hud_list(void *world, const u8 *info, u32 talking, u32 radio, int self);

/* match_client::connect (voice_connect): the match server's host, its port
 * (the low 16 bits) and the session of its 0xD8. */
void vc_connect(const char *host, u32 port, u32 session) {
    if (!host || !load()) return;
    voice.started = voice.dll.start(host, (port & 0xFFFF) + 1, session) == 0;
    voice.ptt = 0;
    voice.settings_sent = 0;
    voice.sent_mute = voice.sent_radio = 0;
    voice.mute_seen = voice.mute_version - 1;
    voice.shown_talking = voice.shown_radio = 0;
    voice.input_world = 0;
}

/* The world ended or left (voice_finish, voice_deactivate): nothing more of
 * this match. Twice is nothing. */
void vc_stop(void) {
    if (voice.loaded == 1 && voice.started) voice.dll.stop();
    voice.started = 0;
    voice.ptt = 0;
    voice.game = 0;
}

static int focused(void) {
    if (!voice.foreground || !voice.window_process || !voice.current_process) return 1;
    void *window = voice.foreground();
    u32 process = 0;
    if (!window) return 0;
    voice.window_process(window, &process);
    return process == voice.current_process();
}

/* Whether the key is held now, asked as on_keyboard_action asks for Tab
 * (0x25CD0E..0x25CD24): the input world's vtable +24h gives the keyboard,
 * the keyboard's +0 (thiscall, the DIK code; `ret 4`) answers in al. */
typedef void *(THISCALL *keyboard_of_t)(void *input_world);
typedef u8(THISCALL *pressed_t)(void *keyboard, u32 key);

static int key_held(void *input_world, u32 key) {
    if (!input_world) return 0;
    keyboard_of_t keyboard_of = AT(AT(input_world, 0, u8 *), 0x24, keyboard_of_t);
    void *keyboard = keyboard_of(input_world);
    if (!keyboard) return 0;
    pressed_t pressed = AT(AT(keyboard, 0, u8 *), 0, pressed_t);
    return pressed(keyboard, key) != 0;
}

/* Every frame of a world (voice_tick). */
void vc_frame(void *world) {
    if (!voice.started || !world) return;
    void *game = AT(world, WORLD_GAME, void *);
    voice.game = game;
    const u8 *info = static_info(game);
    float volume = voice.off ? 0.0f : voice.volume / 100.0f;
    int microphone = !voice.off && voice.microphone;
    float sensitivity = voice.sensitivity / 100.0f;
    if (!voice.settings_sent || volume != voice.sent_volume || microphone != voice.sent_microphone
        || sensitivity != voice.sent_sensitivity) {
        voice.dll.settings(volume, microphone, sensitivity);
        voice.sent_volume = volume;
        voice.sent_microphone = microphone;
        voice.sent_sensitivity = sensitivity;
        voice.settings_sent = 1;
    }
    if (voice.ptt && (!microphone || !key_held(voice.input_world, voice.ptt_key) || !focused())) {
        voice.dll.ptt(0);
        voice.ptt = 0;
    }
    int self = local_place(game);
    u32 mute = muted_slots(info);
    if (mute != voice.sent_mute || voice.mute_seen != voice.mute_version) {
        voice.dll.mute(mute);
        voice.sent_mute = mute;
        voice.mute_seen = voice.mute_version;
    }
    u32 radio = far_slots(world, info, self);
    if (radio != voice.sent_radio) {
        voice.dll.radio(radio);
        voice.sent_radio = radio;
    }
    u32 talking = voice.off ? 0 : voice.dll.talking();
    if (talking & (1u << SELF_BEFORE_WELCOME)) {
        talking &= ~(1u << SELF_BEFORE_WELCOME);
        if (self >= 0) talking |= 1u << self;
    }
    talking &= ~mute;
    talking &= places(info) >= 32 ? ~0u : (1u << places(info)) - 1;
    if (talking != voice.shown_talking || (radio & talking) != (voice.shown_radio & voice.shown_talking))
        hud_list(world, info, talking, radio, self);
}

/* The hud's list of who talks: root.srp_voice_talking(name, flags, ...), the
 * player's own first. */
static void hud_list(void *world, const u8 *info, u32 talking, u32 radio, int self) {
    void *movie = movie_of(AT(world, WORLD_HUD_MOVIE, void *));
    voice.shown_talking = talking;
    voice.shown_radio = radio;
    if (!movie) return;
    gfx_value values[2 * MAX_PLACES];
    u32 n = 0;
    if (self >= 0 && (talking & (1u << self))) {
        gfx_string(&values[n++], name_of(info, (u32)self));
        gfx_word(&values[n++], GFX_UINT, FLAG_SELF);
    }
    for (u32 place = 0; place < places(info); place++) {
        if ((int)place == self || !(talking & (1u << place))) continue;
        gfx_string(&values[n++], name_of(info, place));
        gfx_word(&values[n++], GFX_UINT, (radio & (1u << place)) ? FLAG_RADIO : 0);
    }
    voice.hud_shown = n != 0;
    engine_movie_invoke(movie, "root.srp_voice_talking", 0, values, n);
}

/* ---- the keys ----------------------------------------------------------- */

/* A short line under the list (root.srp_voice_notice): `key`'s text in the
 * game's language and the name. */
static void notice(void *world, void *game, const char *key, const char *name) {
    void *movie = world ? movie_of(AT(world, WORLD_HUD_MOVIE, void *)) : 0;
    if (!movie) return;
    char text[0x200];
    engine_translate((u8 *)game + GAME_TRANSLATOR, key, text);
    gfx_value values[2];
    gfx_string(&values[0], text);
    gfx_string(&values[1], name);
    engine_movie_invoke(movie, "root.srp_voice_notice", 0, values, 2);
}

/* Every key of a match (voice_key): the input world's key going down (1),
 * held (2) or up (3). Not taken: the game goes on with it. */
void vc_key(void *game, u32 key, u32 kind, void *input_world) {
    if (!voice.started || !game || (kind != KEY_DOWN && kind != KEY_UP)) return;
    void *binder = AT(game, GAME_BINDER, void *);
    if (!binder) return;
    u32 toggle = 0;
    u32 action = engine_binded_action(binder, KINDS_CHAT, key, &toggle);
    if (action == ACTION_PTT) {
        if (kind == KEY_DOWN && !voice.ptt && !voice.off && voice.microphone) {
            voice.dll.ptt(1);
            voice.ptt = 1;
            voice.ptt_key = key;
            voice.input_world = input_world;
        } else if (kind == KEY_UP && voice.ptt && key == voice.ptt_key) {
            voice.dll.ptt(0);
            voice.ptt = 0;
        }
    } else if (action == ACTION_MUTE_LAST && kind == KEY_DOWN && !voice.off) {
        int slot = voice.dll.last_speaker();
        const u8 *info = static_info(game);
        u32 id = id_of(slot);
        if (slot < 0 || (u32)slot >= places(info) || slot == local_place(game) || !id) return;
        int muted = !is_muted(id);
        set_mute(id, muted);
        notice((u8 *)game + GAME_WORLD, game, muted ? "st_voice_muted" : "st_voice_unmuted",
               name_of(info, (u32)slot));
    }
}

/* ---- the Esc menu ------------------------------------------------------- */

/* The menu shown over `state` (voice_menu): over a match's world, its voice
 * panel lists the teammates and their mutes; anywhere else it is empty. */
void vc_menu(void *options, void *state) {
    if (!options) return;
    void *movie = movie_of(AT(options, OPTIONS_MOVIE, void *));
    void *game = AT(options, OPTIONS_GAME, void *);
    if (!movie || !game) return;
    static char title[0x200], all[0x200];
    gfx_value values[2 + 3 * MAX_PLACES];
    u32 n = 2;
    engine_translate((u8 *)game + GAME_TRANSLATOR, "st_voice_chat_volume_option", title);
    engine_translate((u8 *)game + GAME_TRANSLATOR, "st_voice_mute_all", all);
    gfx_string(&values[0], title);
    gfx_string(&values[1], all);
    const u8 *info = static_info(game);
    int self = local_place(game);
    if (voice.started && !voice.off && state == (u8 *)game + GAME_WORLD && self >= 0 && (u32)self < places(info)) {
        u32 team = team_of(info, (u32)self);
        for (u32 place = 0; place < places(info); place++) {
            const char *name = name_of(info, place);
            u32 id = id_of((int)place);
            if ((int)place == self || team_of(info, place) != team || !name[0] || !id) continue;
            gfx_word(&values[n++], GFX_UINT, place);
            gfx_string(&values[n++], name);
            gfx_word(&values[n++], GFX_BOOL, (u32)is_muted(id));
        }
    }
    engine_movie_invoke(movie, "root.srp_voice_panel", 0, values, n);
}

/* A call of the main menu movie (voice_menu_call): 1 for the voice panel's
 * (srp_voice_mute(slot, muted), srp_voice_mute_all(muted)), done here; 0 for
 * any other, the game's. */
u32 vc_menu_call(void *handler, const char *name, const gfx_value *args, u32 count) {
    (void)handler;
    if (!name) return 0;
    int one = same(name, "srp_voice_mute"), all = same(name, "srp_voice_mute_all");
    if (!one && !all) return 0;
    void *game = voice.game;
    const u8 *info = static_info(game);
    int self = local_place(game);
    u32 slot = 0, muted = 0;
    if (!voice.started || !info || self < 0 || (u32)self >= places(info)) return 1;
    if (one) {
        if (count < 2 || !gfx_number(&args[0], &slot) || !gfx_number(&args[1], &muted)) return 1;
        if (slot >= places(info) || (int)slot == self || team_of(info, slot) != team_of(info, (u32)self)) return 1;
        set_mute(id_of((int)slot), muted != 0);
        return 1;
    }
    if (count < 1 || !gfx_number(&args[0], &muted)) return 1;
    for (u32 place = 0; place < places(info); place++)
        if ((int)place != self && team_of(info, place) == team_of(info, (u32)self))
            set_mute(id_of((int)place), muted != 0);
    return 1;
}
