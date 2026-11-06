/* Host test of patch 100 (audio_device_026g.c) against a made-up XAudio2 2.7
 * that keeps XAudio2's rules: one mastering voice an engine; a voice is
 * made only after the master, sends only to voices of its engine and of a
 * later stage; a voice that is another's destination, and the master
 * while any voice lives, are not destroyed; an effect is in one voice at a
 * time; a voice's buffers end in their order, each with one OnBufferEnd.
 *
 * i686 (stdcall), under the sanitizers:
 *   CC=$(nix-build '<nixpkgs>' -A pkgsi686Linux.gcc --no-out-link)/bin/gcc
 *   $CC -std=gnu2x -g -O1 -msse2 -mfpmath=sse -Wall -Wextra -Werror \
 *     -fsanitize=address,undefined audio_device_026g.c test_audio_device_026g.c -o /tmp/ad-test
 *   ASAN_OPTIONS=detect_leaks=0 /tmp/ad-test
 * (the leak check is the made-up engine's own: every voice, engine and
 * effect reference given back, counted below). */
#include "audio_device_026g.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define S AD_STDCALL
#define MAX_VOICES 64
#define MAX_QUEUE 64

/* ---- the made-up devices ---- */

static struct { ad_u32 count, default_index; ad_u16 id[4][8]; } devices;
static void set_devices(ad_u32 count, ad_u32 default_index, const char *names)
{
    devices.count = count;
    devices.default_index = default_index;
    memset(devices.id, 0, sizeof devices.id);
    for (ad_u32 i = 0; i < count; ++i) devices.id[i][0] = (ad_u16)names[i];
}

/* ---- the made-up effect (an XAPO) ---- */

struct m_effect { const struct ad_unknown_vtbl *vtbl; int references; int locked_by; };
static ad_hresult S fx_qi(void *self, const struct ad_guid *iid, void **out) { (void)self, (void)iid; *out = 0; return AD_E_NOINTERFACE; }
static ad_u32 S fx_add_ref(void *self) { return ++((struct m_effect *)self)->references; }
static ad_u32 S fx_release(void *self) { return --((struct m_effect *)self)->references; }
static const struct ad_unknown_vtbl fx_vtbl = { fx_qi, fx_add_ref, fx_release };

/* ---- the made-up engine and voices ---- */

enum m_kind { M_MASTER, M_SUBMIX, M_SOURCE };
struct m_engine;
struct m_voice {
    const struct xa_voice_vtbl *vtbl;
    struct m_engine *engine;
    enum m_kind kind;
    int serial;
    ad_u32 channels, rate, stage, device;
    ad_u32 send_count;
    struct m_voice *to[4];                  /* default (count 0xFFFFFFFF): the master */
    ad_u32 effect_count;
    struct m_effect *effect[4];
    ad_u8 parameters[64];
    ad_u32 parameters_size;
    float volume, ratio;
    float matrix[16];
    ad_u32 matrix_source, matrix_destination;
    struct m_voice *matrix_to;
    void *callback;
    int started;
    ad_u32 queued;
    struct xa_buffer queue[MAX_QUEUE];
    int flushed[MAX_QUEUE];                 /* ended on the next pass of a live engine */
    ad_u8 format[18];
};
struct m_engine {
    const struct xa_engine_vtbl *vtbl;
    int references, serial, dead;
    struct m_voice *voice[MAX_VOICES];
    ad_u32 voices;
    struct m_voice *master;
    void *callback[4];
    ad_u32 callbacks;
};
static int engines_made, engines_alive, voices_alive, rule_broken, next_serial, create_fails;
static struct m_engine *last_engine;

#define DEFAULT_SENDS 0xFFFFFFFFu

static void rule(const char *what)
{
    fprintf(stderr, "XAudio2 rule broken: %s\n", what);
    ++rule_broken;
}

static int sends_to(const struct m_voice *from, const struct m_voice *to)
{
    if (from->send_count == DEFAULT_SENDS) return to->kind == M_MASTER;
    for (ad_u32 i = 0; i < from->send_count; ++i)
        if (from->to[i] == to) return 1;
    return 0;
}

static int set_sends(struct m_voice *v, const struct xa_sends *sends)
{
    if (!sends) {
        v->send_count = DEFAULT_SENDS;
        return 1;
    }
    for (ad_u32 i = 0; i < sends->count; ++i) {
        struct m_voice *to = sends->sends[i].voice;
        int found = 0;
        for (ad_u32 j = 0; j < v->engine->voices; ++j) found |= v->engine->voice[j] == to;
        if (!found || to->kind == M_SOURCE || (to->kind == M_SUBMIX && v->kind == M_SUBMIX && to->stage <= v->stage)) {
            rule("a send to a voice not of the engine or not of a later stage");
            return 0;
        }
    }
    v->send_count = sends->count;
    for (ad_u32 i = 0; i < sends->count; ++i) v->to[i] = sends->sends[i].voice;
    v->matrix_to = 0;
    return 1;
}

static void drop_effects(struct m_voice *v)
{
    for (ad_u32 i = 0; i < v->effect_count; ++i) {
        v->effect[i]->locked_by = 0;
        fx_release(v->effect[i]);
    }
    v->effect_count = 0;
    v->parameters_size = 0;
}

static int set_chain(struct m_voice *v, const struct xa_chain *chain)
{
    if (chain)
        for (ad_u32 i = 0; i < chain->count; ++i)
            if (((struct m_effect *)chain->effects[i].effect)->locked_by) {
                rule("an effect in two voices");
                return 0;
            }
    drop_effects(v);
    if (!chain) return 1;
    for (ad_u32 i = 0; i < chain->count; ++i) {
        v->effect[i] = chain->effects[i].effect;
        v->effect[i]->locked_by = v->serial;
        fx_add_ref(v->effect[i]);
    }
    v->effect_count = chain->count;
    return 1;
}

#define MV(self) ((struct m_voice *)(self))
static void S mv_details(void *self, struct xa_voice_details *d) { *d = (struct xa_voice_details){ 0, MV(self)->channels, MV(self)->rate }; }
static ad_hresult S mv_set_output_voices(void *self, const struct xa_sends *s) { return set_sends(MV(self), s) ? 0 : AD_XAUDIO2_E_INVALID_CALL; }
static ad_hresult S mv_set_effect_chain(void *self, const struct xa_chain *c) { return set_chain(MV(self), c) ? 0 : AD_XAUDIO2_E_INVALID_CALL; }
static ad_hresult S mv_enable(void *self, ad_u32 i, ad_u32 o) { (void)self, (void)i, (void)o; return 0; }
static void S mv_effect_state(void *self, ad_u32 i, int *e) { (void)self, (void)i; *e = 1; }
static ad_hresult S mv_set_parameters(void *self, ad_u32 i, const void *p, ad_u32 size, ad_u32 o)
{
    (void)i, (void)o;
    memcpy(MV(self)->parameters, p, size);
    MV(self)->parameters_size = size;
    return 0;
}
static ad_hresult S mv_get_parameters(void *self, ad_u32 i, void *p, ad_u32 size) { (void)i; memcpy(p, MV(self)->parameters, size); return 0; }
static ad_hresult S mv_set_filter(void *self, const struct xa_filter *f, ad_u32 o) { (void)self, (void)f, (void)o; return 0; }
static void S mv_get_filter(void *self, struct xa_filter *f) { (void)self; *f = (struct xa_filter){ 0, 0, 0 }; }
static ad_hresult S mv_set_output_filter(void *self, void *d, const struct xa_filter *f, ad_u32 o) { (void)self, (void)d, (void)f, (void)o; return 0; }
static void S mv_get_output_filter(void *self, void *d, struct xa_filter *f) { (void)self, (void)d; *f = (struct xa_filter){ 0, 0, 0 }; }
static ad_hresult S mv_set_volume(void *self, float v, ad_u32 o) { (void)o; MV(self)->volume = v; return 0; }
static void S mv_get_volume(void *self, float *v) { *v = MV(self)->volume; }
static ad_hresult S mv_set_channel_volumes(void *self, ad_u32 n, const float *v, ad_u32 o) { (void)self, (void)n, (void)v, (void)o; return 0; }
static void S mv_get_channel_volumes(void *self, ad_u32 n, float *v) { (void)self; memset(v, 0, n * sizeof *v); }
static ad_hresult S mv_set_matrix(void *self, void *to, ad_u32 s, ad_u32 d, const float *m, ad_u32 o)
{
    (void)o;
    struct m_voice *v = MV(self);
    if (to && !sends_to(v, to)) {
        rule("a matrix to a voice it does not send to");
        return AD_XAUDIO2_E_INVALID_CALL;
    }
    v->matrix_to = to;
    v->matrix_source = s;
    v->matrix_destination = d;
    memcpy(v->matrix, m, s * d * sizeof *m);
    return 0;
}
static void S mv_get_matrix(void *self, void *to, ad_u32 s, ad_u32 d, float *m) { (void)to; memcpy(m, MV(self)->matrix, s * d * sizeof *m); }
static void S mv_destroy(void *self)
{
    struct m_voice *v = MV(self);
    struct m_engine *e = v->engine;
    for (ad_u32 i = 0; i < e->voices; ++i)
        if (e->voice[i] != v && sends_to(e->voice[i], v)) {
            rule("a voice destroyed while another sends to it");
            return;
        }
    if (v->kind == M_MASTER && e->voices > 1) {
        rule("the master destroyed while voices live");
        return;
    }
    for (ad_u32 i = 0; i < e->voices; ++i)
        if (e->voice[i] == v) e->voice[i] = e->voice[--e->voices];
    if (e->master == v) e->master = 0;
    drop_effects(v);
    --voices_alive;
    free(v);
}
static ad_hresult S mv_start(void *self, ad_u32 f, ad_u32 o) { (void)f, (void)o; MV(self)->started = 1; return 0; }
static ad_hresult S mv_stop(void *self, ad_u32 f, ad_u32 o) { (void)f, (void)o; MV(self)->started = 0; return 0; }
static ad_hresult S mv_submit(void *self, const struct xa_buffer *b, const struct xa_buffer_wma *w)
{
    (void)w;
    struct m_voice *v = MV(self);
    if (v->queued == MAX_QUEUE) return AD_XAUDIO2_E_INVALID_CALL;
    v->flushed[v->queued] = 0;
    v->queue[v->queued++] = *b;
    return 0;
}
/* XAudio2's: every buffer of a stopped voice, all but the playing one of a
 * started one; their ends on a later pass (play), never on a dead engine. */
static ad_hresult S mv_flush(void *self)
{
    struct m_voice *v = MV(self);
    for (ad_u32 i = v->started ? 1 : 0; i < v->queued; ++i) v->flushed[i] = 1;
    return 0;
}
static ad_hresult S mv_nothing(void *self) { (void)self; return 0; }
static ad_hresult S mv_exit_loop(void *self, ad_u32 o) { (void)self, (void)o; return 0; }
static void S mv_get_state(void *self, struct xa_voice_state *s) { *s = (struct xa_voice_state){ 0, MV(self)->queued, 0 }; }
static ad_hresult S mv_set_ratio(void *self, float r, ad_u32 o) { (void)o; MV(self)->ratio = r; return 0; }
static void S mv_get_ratio(void *self, float *r) { *r = MV(self)->ratio; }
static ad_hresult S mv_set_rate(void *self, ad_u32 r) { MV(self)->rate = r; return 0; }
static const struct xa_voice_vtbl mv_vtbl = {
    mv_details, mv_set_output_voices, mv_set_effect_chain, mv_enable, mv_enable, mv_effect_state, mv_set_parameters,
    mv_get_parameters, mv_set_filter, mv_get_filter, mv_set_output_filter, mv_get_output_filter, mv_set_volume,
    mv_get_volume, mv_set_channel_volumes, mv_get_channel_volumes, mv_set_matrix, mv_get_matrix, mv_destroy,
    mv_start, mv_stop, mv_submit, mv_flush, mv_nothing, mv_exit_loop, mv_get_state, mv_set_ratio, mv_get_ratio,
    mv_set_rate,
};

static struct m_voice *new_voice(struct m_engine *e, enum m_kind kind)
{
    struct m_voice *v = calloc(1, sizeof *v);
    v->vtbl = &mv_vtbl;
    v->engine = e;
    v->kind = kind;
    v->serial = ++next_serial;
    v->volume = v->ratio = 1;
    e->voice[e->voices++] = v;
    ++voices_alive;
    return v;
}

static void forget_voice(struct m_voice *v)
{
    struct m_engine *e = v->engine;
    for (ad_u32 i = 0; i < e->voices; ++i)
        if (e->voice[i] == v) e->voice[i] = e->voice[--e->voices];
    drop_effects(v);
    --voices_alive;
    free(v);
}

#define ME(self) ((struct m_engine *)(self))
static ad_hresult S me_qi(void *self, const struct ad_guid *iid, void **out) { (void)self, (void)iid; *out = 0; return AD_E_NOINTERFACE; }
static ad_u32 S me_add_ref(void *self) { return ++ME(self)->references; }
static ad_u32 S me_release(void *self)
{
    struct m_engine *e = ME(self);
    if (--e->references) return e->references;
    if (e->voices) rule("an engine released with voices alive");
    --engines_alive;
    free(e);
    return 0;
}
static ad_hresult S me_device_count(void *self, ad_u32 *count) { (void)self; *count = devices.count; return 0; }
static ad_hresult S me_device_details(void *self, ad_u32 i, struct xa_device_details *d)
{
    (void)self;
    if (i >= devices.count) return AD_XAUDIO2_E_INVALID_CALL;
    memset(d, 0, sizeof *d);
    memcpy(d->id, devices.id[i], sizeof devices.id[i]);
    d->role = i == devices.default_index ? 0xF : 0;
    return 0;
}
static ad_hresult S me_initialize(void *self, ad_u32 f, ad_u32 p) { (void)self, (void)f, (void)p; return 0; }
static ad_hresult S me_register(void *self, void *c) { ME(self)->callback[ME(self)->callbacks++] = c; return 0; }
static void S me_unregister(void *self, void *c)
{
    struct m_engine *e = ME(self);
    for (ad_u32 i = 0; i < e->callbacks; ++i)
        if (e->callback[i] == c) e->callback[i] = e->callback[--e->callbacks];
}
static ad_hresult S me_source(void *self, void **out, const void *format, ad_u32 flags, float max_ratio, void *callback,
                              const struct xa_sends *sends, const struct xa_chain *chain)
{
    (void)flags, (void)max_ratio;
    struct m_engine *e = ME(self);
    if (!e->master || e->dead) return AD_XAUDIO2_E_INVALID_CALL;
    struct m_voice *v = new_voice(e, M_SOURCE);
    memcpy(v->format, format, sizeof v->format);
    v->callback = callback;
    if (!set_sends(v, sends) || !set_chain(v, chain)) {
        forget_voice(v);
        return AD_XAUDIO2_E_INVALID_CALL;
    }
    *out = v;
    return 0;
}
static ad_hresult S me_submix(void *self, void **out, ad_u32 channels, ad_u32 rate, ad_u32 flags, ad_u32 stage,
                              const struct xa_sends *sends, const struct xa_chain *chain)
{
    (void)flags;
    struct m_engine *e = ME(self);
    if (!e->master || e->dead) return AD_XAUDIO2_E_INVALID_CALL;
    struct m_voice *v = new_voice(e, M_SUBMIX);
    v->channels = channels;
    v->rate = rate;
    v->stage = stage;
    if (!set_sends(v, sends) || !set_chain(v, chain)) {
        forget_voice(v);
        return AD_XAUDIO2_E_INVALID_CALL;
    }
    *out = v;
    return 0;
}
static ad_hresult S me_master(void *self, void **out, ad_u32 channels, ad_u32 rate, ad_u32 flags, ad_u32 device,
                              const struct xa_chain *chain)
{
    (void)flags;
    struct m_engine *e = ME(self);
    if (e->master || device >= devices.count) return AD_XAUDIO2_E_INVALID_CALL;
    struct m_voice *v = new_voice(e, M_MASTER);
    v->channels = channels ? channels : 2;
    v->rate = rate;
    v->device = device;
    v->send_count = 0;
    set_chain(v, chain);
    e->master = v;
    *out = v;
    return 0;
}
static ad_hresult S me_start(void *self) { (void)self; return 0; }
static void S me_stop(void *self) { (void)self; }
static ad_hresult S me_commit(void *self, ad_u32 o) { (void)self, (void)o; return 0; }
static void S me_performance(void *self, void *d) { (void)self, (void)d; }
static void S me_debug(void *self, const void *c, void *r) { (void)self, (void)c, (void)r; }
static const struct xa_engine_vtbl me_vtbl = {
    me_qi, me_add_ref, me_release, me_device_count, me_device_details, me_initialize, me_register, me_unregister,
    me_source, me_submix, me_master, me_start, me_stop, me_commit, me_performance, me_debug,
};

static void end_buffer(struct m_voice *v, ad_u32 at)
{
    void *context = v->queue[at].context;
    --v->queued;
    memmove(v->queue + at, v->queue + at + 1, (v->queued - at) * sizeof v->queue[0]);
    memmove(v->flushed + at, v->flushed + at + 1, (v->queued - at) * sizeof v->flushed[0]);
    (*(const struct xa_voice_callback_vtbl **)v->callback)->buffer_end(v->callback, context);
}

/* XAudio2's thread, a pass of a live engine: the flushed buffers end, then
 * the first buffer of every started source. */
static void play(struct m_engine *e)
{
    if (e->dead) return;
    for (ad_u32 i = 0; i < e->voices; ++i) {
        struct m_voice *v = e->voice[i];
        if (v->kind != M_SOURCE) continue;
        for (ad_u32 b = 0; b < v->queued;)
            if (v->flushed[b]) end_buffer(v, b);
            else ++b;
        if (v->started && v->queued) end_buffer(v, 0);
    }
}

/* The device under the engine goes: its callbacks' OnCriticalError. */
static void lose_device(struct m_engine *e)
{
    e->dead = 1;
    for (ad_u32 i = 0; i < e->callbacks; ++i)
        (*(const struct xa_engine_callback_vtbl **)e->callback[i])->critical_error(e->callback[i], (ad_hresult)0x88960004u);
}

/* ---- the made-up MMDevice enumerator ---- */

static void *client;
static int enumerator_references;
static ad_hresult S mm_qi(void *self, const struct ad_guid *iid, void **out) { (void)self, (void)iid; *out = 0; return AD_E_NOINTERFACE; }
static ad_u32 S mm_add_ref(void *self) { (void)self; return ++enumerator_references; }
static ad_u32 S mm_release(void *self) { (void)self; return --enumerator_references; }
static ad_hresult S mm_unused(void *self, ad_u32 a, ad_u32 b, void **c) { (void)self, (void)a, (void)b, (void)c; return -1; }
static ad_hresult S mm_get_device(void *self, const ad_u16 *id, void **d) { (void)self, (void)id, (void)d; return -1; }
static ad_hresult S mm_register(void *self, void *c) { (void)self; client = c; return 0; }
static ad_hresult S mm_unregister(void *self, void *c) { (void)self; assert(client == c); client = 0; return 0; }
static const struct mm_enumerator_vtbl mm_vtbl = { mm_qi, mm_add_ref, mm_release, mm_unused, mm_unused, mm_get_device, mm_register, mm_unregister };
static struct { const struct mm_enumerator_vtbl *vtbl; } enumerator = { &mm_vtbl };

static void default_changed(ad_u32 role)
{
    assert(client);
    static const ad_u16 id[2] = { 'x', 0 };
    (*(const struct mm_client_vtbl **)client)->default_changed(client, 0, role, id);
}

/* ---- the platform ---- */

static ad_u64 now;
static int logs;
void *ad_alloc(ad_u32 size) { return calloc(1, size); }
void ad_free(void *memory) { free(memory); }
ad_hresult ad_xaudio2_create(void **out, ad_u32 flags)
{
    (void)flags;
    if (create_fails) return AD_E_OUTOFMEMORY;
    struct m_engine *e = calloc(1, sizeof *e);
    e->vtbl = &me_vtbl;
    e->references = 1;
    e->serial = ++engines_made;
    ++engines_alive;
    last_engine = e;
    *out = e;
    return 0;
}
ad_hresult ad_mm_enumerator(void **out)
{
    ++enumerator_references;
    *out = &enumerator;
    return 0;
}
ad_u64 ad_now(void) { return now; }
ad_u64 ad_frequency(void) { return 1000; }
void ad_log(const char *what, ad_u32 value)
{
    ++logs;
    fprintf(stderr, "  log: %s 0x%08x\n", what, value);
}

/* ---- the game ---- */

struct game_callback { const struct xa_voice_callback_vtbl *vtbl; int ends; void *last; };
static void S g_none(void *self) { (void)self; }
static void S g_pass(void *self, ad_u32 b) { (void)self, (void)b; }
static void S g_context(void *self, void *c) { (void)self, (void)c; }
static void S g_end(void *self, void *c)
{
    struct game_callback *g = self;
    ++g->ends;
    g->last = c;
}
static void S g_error(void *self, void *c, ad_hresult e) { (void)self, (void)c, (void)e; }
static const struct xa_voice_callback_vtbl g_vtbl = { g_pass, g_none, g_none, g_context, g_end, g_context, g_error };

#define SOURCES 4
static void *xaudio, *master, *scene, *environment, *source[SOURCES];
static struct game_callback callbacks[SOURCES];
static struct m_effect reverb = { &fx_vtbl, 1, 0 };     /* the game's reference: XAudio2CreateReverb's */
static int contexts[SOURCES][8];
static const ad_u8 reverb_parameters[52] = { 1, 2, 3, 4, 5 };

#define ENGINE_CALL(name, ...) (*(const struct xa_engine_vtbl **)xaudio)->name(xaudio, __VA_ARGS__)
#define VOICE_CALL(voice, name, ...) (*(const struct xa_voice_vtbl **)(voice))->name((voice) __VA_OPT__(,) __VA_ARGS__)

static void game_start(void)
{
    assert(audio_device_create(&xaudio, 0) == 0);
    ad_u32 count;
    assert(ENGINE_CALL(get_device_count, &count) == 0 && count == devices.count);
    assert(ENGINE_CALL(create_mastering_voice, &master, 0, 44100, 0, devices.default_index, 0) == 0);
    assert(ENGINE_CALL(create_submix_voice, &scene, 2, 44100, 0, 2, 0, 0) == 0);
    assert(VOICE_CALL(scene, set_output_voices, 0) == 0);
    struct xa_effect effect = { &reverb, 1, 1 };
    struct xa_chain chain = { 1, &effect };
    assert(ENGINE_CALL(create_submix_voice, &environment, 1, 44100, 0, 1, 0, &chain) == 0);
    reverb.references--;                                /* the game lets its own go, as sound_scene does */
    struct xa_send to_scene = { 0, scene };
    struct xa_sends sends = { 1, &to_scene };
    assert(VOICE_CALL(environment, set_output_voices, &sends) == 0);
    assert(VOICE_CALL(environment, set_effect_parameters, 0, reverb_parameters, sizeof reverb_parameters, 0) == 0);
    ad_u8 format[18] = { 1, 0, 1, 0, 0x44, 0xAC, 0, 0, 0x88, 0x58, 1, 0, 2, 0, 16, 0, 0x77, 0x77 };
    for (int i = 0; i < SOURCES; ++i) {
        callbacks[i].vtbl = &g_vtbl;
        assert(ENGINE_CALL(create_source_voice, &source[i], format, 0, 2.0f, &callbacks[i], 0, 0) == 0);
        assert(VOICE_CALL(source[i], set_output_voices, &sends) == 0);
        float matrix[2] = { 0.25f * (float)i, 0.5f };
        assert(VOICE_CALL(source[i], set_output_matrix, 0, 1, 2, matrix, 0) == 0);
        assert(VOICE_CALL(source[i], set_volume, 0.1f * (float)(i + 1), 0) == 0);
        assert(VOICE_CALL(source[i], set_frequency_ratio, 1.5f, 0) == 0);
        for (int b = 0; b < 3; ++b) {
            struct xa_buffer buffer = { 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[i][b] };
            assert(VOICE_CALL(source[i], submit_source_buffer, &buffer, 0) == 0);
        }
        if (i % 2 == 0) assert(VOICE_CALL(source[i], start, 0, 0) == 0);
    }
}

static void game_end(void)
{
    for (int i = 0; i < SOURCES; ++i) VOICE_CALL(source[i], destroy_voice);
    VOICE_CALL(environment, destroy_voice);
    VOICE_CALL(scene, destroy_voice);
    VOICE_CALL(master, destroy_voice);
    assert((*(const struct xa_engine_vtbl **)xaudio)->release(xaudio) == 0);
}

/* The live engine's graph is the game's: every voice of ours on it, sending
 * where the game said, with what it set. */
static void check_graph(struct m_engine *e, ad_u32 device, int all_started)
{
    assert(e->voices == 3 + SOURCES);
    assert(e->master && e->master->device == device && e->master->rate == 44100);
    struct m_voice *s = 0, *env = 0;
    for (ad_u32 i = 0; i < e->voices; ++i) {
        if (e->voice[i]->kind == M_SUBMIX && e->voice[i]->stage == 2) s = e->voice[i];
        if (e->voice[i]->kind == M_SUBMIX && e->voice[i]->stage == 1) env = e->voice[i];
    }
    assert(s && env && s->channels == 2 && sends_to(s, e->master));
    assert(env->send_count == 1 && env->to[0] == s);
    assert(env->effect_count == 1 && env->effect[0] == &reverb && reverb.locked_by == env->serial);
    assert(env->parameters_size == sizeof reverb_parameters && !memcmp(env->parameters, reverb_parameters, 52));
    for (int i = 0; i < SOURCES; ++i) {
        struct m_voice *v = 0;
        for (ad_u32 j = 0; j < e->voices; ++j)
            if (e->voice[j]->kind == M_SOURCE && e->voice[j]->callback
                && *(void **)((ad_u8 *)e->voice[j]->callback + sizeof(void *)) == source[i])
                v = e->voice[j];
        assert(v && v->send_count == 1 && v->to[0] == s);
        assert(v->matrix_source == 1 && v->matrix_destination == 2 && v->matrix[0] == 0.25f * (float)i);
        assert(v->volume == 0.1f * (float)(i + 1) && v->ratio == 1.5f);
        assert(v->started == (all_started || i % 2 == 0));
        assert(v->format[0] == 1 && (e->serial == 1 || v->format[16] == 0));   /* made again: PCM without its cbSize */
    }
}

static int pending_contexts(int i, void **out)
{
    struct m_voice *v = 0;
    struct m_engine *e = last_engine;
    for (ad_u32 j = 0; j < e->voices; ++j)
        if (e->voice[j]->kind == M_SOURCE && *(void **)((ad_u8 *)e->voice[j]->callback + sizeof(void *)) == source[i])
            v = e->voice[j];
    assert(v);
    for (ad_u32 b = 0; b < v->queued; ++b) out[b] = v->queue[b].context;
    return (int)v->queued;
}

int main(void)
{
    set_devices(2, 1, "AB");
    game_start();
    struct m_engine *first = last_engine;
    assert(engines_alive == 1 && client && enumerator_references == 1);
    check_graph(first, 1, 0);

    puts("a played buffer leaves the pending queue");
    play(first);
    for (int i = 0; i < SOURCES; ++i) assert(callbacks[i].ends == (i % 2 == 0));

    puts("the default changes to the same device: nothing");
    default_changed(1);
    ad_tick();
    assert(last_engine == first && engines_alive == 1 && !rule_broken);

    puts("the device is lost: a new engine on the new default, the same graph");
    set_devices(1, 0, "C");
    void *old_callback = first->callback[0];
    lose_device(first);
    ad_tick();
    struct m_engine *second = last_engine;
    assert(second != first && engines_alive == 1 && !rule_broken);
    check_graph(second, 0, 0);
    for (int i = 0; i < SOURCES; ++i) {
        void *queued[8];
        int n = pending_contexts(i, queued);
        int played = i % 2 == 0;
        assert(n == 3 - played);
        for (int b = 0; b < n; ++b) assert(queued[b] == &contexts[i][b + played]);
    }
    puts("the game's calls reach the new voices; every buffer ends once");
    assert(VOICE_CALL(source[1], start, 0, 0) == 0);
    assert(VOICE_CALL(source[3], start, 0, 0) == 0);
    for (int round = 0; round < 4; ++round) play(second);
    for (int i = 0; i < SOURCES; ++i) assert(callbacks[i].ends == 3 && callbacks[i].last == &contexts[i][2]);
    for (int i = 0; i < SOURCES; ++i) {
        struct xa_buffer buffer = { 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[i][3] };
        assert(VOICE_CALL(source[i], submit_source_buffer, &buffer, 0) == 0);
    }

    puts("a late critical error of the old engine is not seen");
    (*(const struct xa_engine_callback_vtbl **)old_callback)->critical_error(old_callback, 1);
    ad_tick();
    assert(last_engine == second && engines_alive == 1);

    puts("no device: the old graph kept, tried again two seconds later");
    set_devices(0, 0, "");
    lose_device(second);
    int made = engines_made;
    ad_tick();
    assert(engines_made == made + 1 && engines_alive == 1 && !rule_broken);   /* the new one let go */
    now = 1500;
    set_devices(1, 0, "D");
    ad_tick();
    assert(engines_made == made + 1);
    now = 2001;
    ad_tick();
    struct m_engine *third = last_engine;
    assert(engines_made == made + 2 && engines_alive == 1 && !rule_broken);
    check_graph(third, 0, 1);

    puts("a new default device: rebuilt on it");
    set_devices(2, 1, "DE");
    default_changed(0);
    ad_tick();
    struct m_engine *fourth = last_engine;
    assert(fourth != third && engines_alive == 1 && !rule_broken);
    check_graph(fourth, 1, 1);
    puts("a communications default is not followed");
    set_devices(2, 0, "DE");
    default_changed(2);
    ad_tick();
    assert(last_engine == fourth);

    puts("a rebuild that cannot make an engine keeps the graph");
    create_fails = 1;
    lose_device(fourth);
    ad_tick();
    assert(last_engine == fourth && engines_alive == 1);
    create_fails = 0;
    now += 3000;
    ad_tick();
    assert(last_engine != fourth && engines_alive == 1 && !rule_broken);
    check_graph(last_engine, 0, 1);
    for (int round = 0; round < 8; ++round) play(last_engine);
    for (int i = 0; i < SOURCES; ++i) assert(callbacks[i].ends == 4 && callbacks[i].last == &contexts[i][3]);

    puts("a flushed buffer ends on a live engine's pass, once");
    {
        struct m_engine *e = last_engine;
        struct xa_buffer b4 = { 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[3][4] };
        struct xa_buffer b5 = { 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[3][5] };
        assert(VOICE_CALL(source[3], submit_source_buffer, &b4, 0) == 0);
        assert(VOICE_CALL(source[3], submit_source_buffer, &b5, 0) == 0);
        assert(VOICE_CALL(source[3], stop, 0, 0) == 0);
        assert(VOICE_CALL(source[3], flush_source_buffers) == 0);
        assert(callbacks[3].ends == 4);
        play(e);
        assert(callbacks[3].ends == 6 && callbacks[3].last == &contexts[3][5]);
    }

    puts("flushed buffers on a dead engine: ended by the rebuild, never played again");
    {
        struct xa_buffer b[3];
        for (int k = 0; k < 3; ++k) b[k] = (struct xa_buffer){ 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[0][4 + k] };
        for (int k = 0; k < 3; ++k) assert(VOICE_CALL(source[0], submit_source_buffer, &b[k], 0) == 0);
        lose_device(last_engine);                           /* dead: no pass ends anything now */
        assert(VOICE_CALL(source[0], flush_source_buffers) == 0);   /* started: [0][4] stays */
        struct xa_buffer c4 = { 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[2][4] };
        assert(VOICE_CALL(source[2], submit_source_buffer, &c4, 0) == 0);
        assert(VOICE_CALL(source[2], stop, 0, 0) == 0);    /* detach_voice: stop, flush */
        assert(VOICE_CALL(source[2], flush_source_buffers) == 0);
        for (int k = 5; k < 7; ++k) {                      /* the voice's next owner */
            struct xa_buffer c = { 0, 4, "abcd", 0, 0, 0, 0, 0, &contexts[2][k] };
            assert(VOICE_CALL(source[2], submit_source_buffer, &c, 0) == 0);
        }
        assert(VOICE_CALL(source[2], start, 0, 0) == 0);
        set_devices(0, 0, "");
        ad_tick();                                          /* no device: lost, the flushed kept */
        assert(callbacks[0].ends == 4 && callbacks[2].ends == 4);
        set_devices(1, 0, "F");
        now += 3000;
        ad_tick();
        assert(!rule_broken && engines_alive == 1);
        assert(callbacks[0].ends == 6 && callbacks[0].last == &contexts[0][6]);
        assert(callbacks[2].ends == 5 && callbacks[2].last == &contexts[2][4]);
        void *queued[8];
        assert(pending_contexts(0, queued) == 1 && queued[0] == &contexts[0][4]);
        assert(pending_contexts(2, queued) == 2 && queued[0] == &contexts[2][5] && queued[1] == &contexts[2][6]);
        assert(pending_contexts(3, queued) == 0);
        for (int round = 0; round < 4; ++round) play(last_engine);
        assert(callbacks[0].ends == 7 && callbacks[0].last == &contexts[0][4]);
        assert(callbacks[2].ends == 7 && callbacks[2].last == &contexts[2][6]);
        assert(callbacks[3].ends == 6);
    }

    puts("the game's end: everything given back");
    game_end();
    assert(engines_alive == 0 && voices_alive == 0 && !client && enumerator_references == 0);
    assert(reverb.references == 0 && reverb.locked_by == 0 && !rule_broken);
    ad_tick();                                          /* no engine: nothing */
    puts("all audio_device checks passed");
    return 0;
}
