/* Patch 99: the game's sound follows the audio output device
 * (docs/notes/audio_device.md; the types and the platform are
 * audio_device_026g.h's).
 *
 * 0.26g makes one XAudio2 2.7 engine and its mastering voice once, on the
 * default device of that moment (sound_world::initialize_xaudio 0x59080),
 * and never again. When that device goes (a headset unplugged, a USB DAC
 * removed) XAudio2 raises a critical error and stops: every sound is gone
 * until the game restarts. When another device becomes the default, the
 * game keeps playing on the old one. XAudio2's only way back is a new
 * engine, and a mastering voice can be destroyed only after every other
 * voice: so every voice is made again (as 0.69's stop_mastering_voice /
 * restart_mastering_voice do).
 *
 * The owner of that is this file. The game's engine is our proxy of the
 * one XAudio2Create made (audio_device_create), and each voice the game
 * makes through it is our proxy (struct ad_voice) of the real one: a call
 * is forwarded, and what it set is kept (the sends, the effect chain and
 * its parameters, the volumes and matrices, a source voice's format,
 * callback, frequency ratio, started state and the buffers it was given
 * whose OnBufferEnd has not come). On the sound thread's tick after a
 * device event (ad_tick), a rebuild makes a new engine and a mastering
 * voice on the default device (if that fails, nothing changes and it is
 * tried again later), then destroys the old voices and engine, and makes
 * every voice again from what was kept, the buffers queued again in order:
 * the game's pointers are the proxies, so it sees nothing but the sound
 * coming back.
 *
 * Threads: the game makes and uses its voices on the sound thread (orders
 * the logic thread posts run there, sound_world::tick 0x59A60), where the
 * rebuild runs too. XAudio2's thread calls the voice callbacks (a buffer's
 * end leaves the pending queue under its spin lock, never held across an
 * XAudio2 or game call) and the engine callback; the MMDevice's thread
 * calls the notification client. Those two only raise an event bit. */
#include "audio_device_026g.h"

#define AD_SENDS 4              /* sends kept a voice (the game gives 1) */
#define AD_EFFECTS 4            /* effects kept a chain (the game's reverb: 1) */
#define AD_PARAMETERS 128       /* bytes of an effect's parameters (XAUDIO2FX_REVERB_PARAMETERS: 52) */
#define AD_CHANNELS 8           /* channel volumes kept */
#define AD_MATRIX 64            /* an output matrix's coefficients kept (8 x 8) */
#define AD_FORMAT 64            /* bytes of a source voice's WAVEFORMATEX (PCM: 18) */
#define AD_PENDING 64           /* XAUDIO2_MAX_QUEUED_BUFFERS: a voice holds no more */
#define AD_CALLBACKS 4          /* engine callbacks of the game's kept */
#define AD_RETRY_SECONDS 2      /* a failed rebuild is tried again this much later */

#define AD_EVENT_CRITICAL 1u    /* the engine's critical error: its device is gone */
#define AD_EVENT_DEFAULT 2u     /* the default render device changed */

#define AD_FLOW_RENDER 0u       /* EDataFlow eRender */
#define AD_ROLE_COMMUNICATIONS 2u

static const struct ad_guid iid_unknown = {0x00000000, 0x0000, 0x0000, {0xC0, 0, 0, 0, 0, 0, 0, 0x46}};
static const struct ad_guid iid_xaudio2 = {0x8bcf1f58, 0x9fe7, 0x4583, {0x8a, 0xc6, 0xe2, 0xad, 0xc4, 0x65, 0xc8, 0xbb}};
static const struct ad_guid iid_notification_client =
    {0x7991eec9, 0x7e89, 0x4d85, {0x83, 0x90, 0x6c, 0x70, 0x3c, 0xec, 0x60, 0xc0}};

/* ---- small things ---- */

static void ad_copy(void *to, const void *from, ad_u32 size)
{
    ad_u8 *t = to;
    const ad_u8 *f = from;
    while (size--) *t++ = *f++;
}

static int ad_same_guid(const struct ad_guid *a, const struct ad_guid *b)
{
    const ad_u8 *x = (const ad_u8 *)a, *y = (const ad_u8 *)b;
    for (ad_u32 i = 0; i < sizeof *a; ++i)
        if (x[i] != y[i]) return 0;
    return 1;
}

static int ad_same_id(const ad_u16 *a, const ad_u16 *b)
{
    for (ad_u32 i = 0; i < 256; ++i) {
        if (a[i] != b[i]) return 0;
        if (!a[i]) return 1;
    }
    return 1;
}

struct ad_spin { ad_u32 held; };

static void ad_lock(struct ad_spin *spin)
{
    while (__atomic_exchange_n(&spin->held, 1, __ATOMIC_ACQUIRE))
        __builtin_ia32_pause();
}

static void ad_unlock(struct ad_spin *spin) { __atomic_store_n(&spin->held, 0, __ATOMIC_RELEASE); }

static const struct xa_engine_vtbl *engine_methods(void *engine) { return *(const struct xa_engine_vtbl **)engine; }
static const struct xa_voice_vtbl *voice_methods(void *voice) { return *(const struct xa_voice_vtbl **)voice; }
static const struct ad_unknown_vtbl *unknown_methods(void *object)
{
    return *(const struct ad_unknown_vtbl **)object;
}

/* ---- what a voice was given ---- */

enum ad_kind { AD_MASTER, AD_SUBMIX, AD_SOURCE };

/* A recorded value: not given, given (kept), or given and too big to keep
 * (forwarded, not made again: the rebuild logs it). */
enum ad_record { AD_UNSET, AD_SET, AD_TOO_BIG };

struct ad_voice;

struct ad_sends {
    enum { AD_SENDS_DEFAULT, AD_SENDS_LIST, AD_SENDS_TOO_BIG } kind;     /* default: the mastering voice */
    ad_u32 count;
    struct { ad_u32 flags; struct ad_voice *to; } send[AD_SENDS];
};

struct ad_effect {
    void *effect;                                   /* an IUnknown (XAPO), one reference ours */
    ad_u32 initial_state, output_channels;
    enum { AD_EFFECT_INITIAL, AD_EFFECT_ENABLED, AD_EFFECT_DISABLED } enabled;
    enum ad_record parameters;
    ad_u32 size;
    ad_u8 bytes[AD_PARAMETERS];
};

struct ad_chain {
    enum { AD_CHAIN_NONE, AD_CHAIN_SET, AD_CHAIN_TOO_BIG } kind;
    ad_u32 count;
    struct ad_effect effect[AD_EFFECTS];
};

struct ad_matrix { struct ad_voice *to; ad_u32 source_channels, destination_channels; float m[AD_MATRIX]; };
struct ad_output_filter { struct ad_voice *to; struct xa_filter filter; };

struct ad_pending_entry { struct xa_buffer buffer; enum { AD_PCM, AD_WMA } kind; struct xa_buffer_wma wma; };

struct ad_pending {
    struct ad_spin spin;
    ad_u32 first, count;                            /* a ring, in submission order */
    struct ad_pending_entry entry[AD_PENDING];
};

/* The callback object a real source voice is given: its OnBufferEnd takes
 * the buffer off the pending queue, then the game's callback runs. */
struct ad_callback { const struct xa_voice_callback_vtbl *vtbl; struct ad_voice *voice; };

struct ad_voice {
    const struct xa_voice_vtbl *vtbl;               /* first: the game's IXAudio2Voice */
    enum ad_kind kind;
    void *real;                                     /* 0: lost in a rebuild (every call refused) */
    struct ad_voice *previous, *next;               /* every voice of ours */
    struct ad_sends sends;
    struct ad_chain chain;
    struct { enum ad_record state; struct xa_filter value; } filter;
    struct { enum ad_record state; float value; } volume;
    struct { enum ad_record state; ad_u32 channels; float value[AD_CHANNELS]; } channel_volumes;
    struct { enum ad_record state; ad_u32 count; struct ad_matrix matrix[AD_SENDS]; } matrices;
    struct { enum ad_record state; ad_u32 count; struct ad_output_filter filter[AD_SENDS]; } output_filters;
    union {
        struct { ad_u32 channels, rate, flags, device; } master;
        struct { ad_u32 channels, rate, flags, stage; } submix;
        struct {
            enum ad_record format_kept;             /* AD_TOO_BIG: not made again */
            ad_u8 format[AD_FORMAT];
            ad_u32 flags;
            float max_ratio;
            void *game_callback;                    /* the game's IXAudio2VoiceCallback, or 0 */
            struct ad_callback callback;
            enum { AD_STOPPED, AD_STARTED } play;
            struct { enum ad_record state; float value; } ratio;
            struct { enum ad_record state; ad_u32 value; } rate;
            struct ad_pending *pending;
        } source;
    } as;
};

/* ---- the engine ---- */

/* Where the mastering voice is: none made yet (or the game destroyed it),
 * on the device `device`, or lost (its device gone or no longer the
 * default) with a rebuild due at `retry_at`. */
struct ad_binding {
    enum { AD_NO_MASTER, AD_BOUND, AD_LOST } kind;
    struct ad_voice *master;                        /* AD_BOUND, AD_LOST */
    ad_u16 device[256];                             /* AD_BOUND: XAUDIO2_DEVICE_DETAILS.DeviceID */
    ad_u64 retry_at;                                /* AD_LOST */
};

struct ad_engine {
    const struct xa_engine_vtbl *vtbl;              /* first: the game's IXAudio2 */
    ad_u32 references;
    void *real;
    ad_u32 create_flags;                            /* XAudio2Create's, for a new engine */
    enum { AD_RUNNING, AD_ENGINE_STOPPED } run;
    ad_u32 game_callback_count;
    void *game_callbacks[AD_CALLBACKS];
    struct ad_binding binding;
};

/* The engine callback each real engine is given, two so that a late event of
 * the engine a rebuild replaced is told from the new one's. */
struct ad_engine_callback { const struct xa_engine_callback_vtbl *vtbl; };

static struct ad_engine *ad_the_engine;            /* the game's one (sound_world+0C0h) */
static struct ad_voice *ad_voices;                  /* every voice of ours */
static struct ad_spin ad_voices_spin;
static ad_u32 ad_events;                            /* AD_EVENT_*, raised by any thread */
static struct ad_engine_callback ad_engine_callbacks[2];
static ad_u32 ad_current_callback;                  /* the live real engine's */
static void *ad_enumerator;                         /* the MMDevice enumerator our client is registered with */

static const struct xa_voice_vtbl ad_master_vtbl, ad_submix_vtbl, ad_source_vtbl;
static const struct xa_voice_callback_vtbl ad_callback_vtbl;

/* Ours, or 0 (a pointer the game had from elsewhere). */
static struct ad_voice *ad_ours(void *voice)
{
    if (!voice) return 0;
    const void *vtbl = *(const void **)voice;
    return vtbl == &ad_master_vtbl || vtbl == &ad_submix_vtbl || vtbl == &ad_source_vtbl ? voice : 0;
}

static void *ad_real_of(void *voice)
{
    struct ad_voice *ours = ad_ours(voice);
    return ours ? ours->real : voice;
}

static void ad_raise(ad_u32 event) { __atomic_or_fetch(&ad_events, event, __ATOMIC_RELEASE); }

/* ---- the records ---- */

static void ad_release_chain(struct ad_chain *chain)
{
    if (chain->kind == AD_CHAIN_SET)
        for (ad_u32 i = 0; i < chain->count; ++i)
            unknown_methods(chain->effect[i].effect)->release(chain->effect[i].effect);
    chain->kind = AD_CHAIN_NONE;
    chain->count = 0;
}

static void ad_keep_chain(struct ad_chain *chain, const struct xa_chain *given)
{
    ad_release_chain(chain);
    if (!given) return;
    if (given->count > AD_EFFECTS) {
        chain->kind = AD_CHAIN_TOO_BIG;
        return;
    }
    chain->kind = AD_CHAIN_SET;
    chain->count = given->count;
    for (ad_u32 i = 0; i < given->count; ++i) {
        struct ad_effect *e = &chain->effect[i];
        *e = (struct ad_effect){ given->effects[i].effect, given->effects[i].initial_state,
                                 given->effects[i].output_channels, AD_EFFECT_INITIAL, AD_UNSET, 0, {0} };
        unknown_methods(e->effect)->add_ref(e->effect);
    }
}

/* XAudio2's sends replace the voice's matrices and output filters. */
static void ad_keep_sends(struct ad_voice *voice, const struct xa_sends *given)
{
    voice->matrices.state = AD_UNSET;
    voice->matrices.count = 0;
    voice->output_filters.state = AD_UNSET;
    voice->output_filters.count = 0;
    if (!given) {
        voice->sends.kind = AD_SENDS_DEFAULT;
        voice->sends.count = 0;
        return;
    }
    if (given->count > AD_SENDS) {
        voice->sends.kind = AD_SENDS_TOO_BIG;
        return;
    }
    voice->sends.kind = AD_SENDS_LIST;
    voice->sends.count = given->count;
    for (ad_u32 i = 0; i < given->count; ++i) {
        voice->sends.send[i].flags = given->sends[i].flags;
        voice->sends.send[i].to = ad_ours(given->sends[i].voice);
        if (!voice->sends.send[i].to) voice->sends.kind = AD_SENDS_TOO_BIG;   /* not ours: not made again */
    }
}

/* The sends as the real voices: `room` for the list; 0 for the default. */
static const struct xa_sends *ad_real_sends(const struct ad_sends *sends, struct xa_sends *list,
                                            struct xa_send room[AD_SENDS])
{
    if (sends->kind != AD_SENDS_LIST) {
        if (sends->kind == AD_SENDS_TOO_BIG) ad_log("sends not kept, made to the master", sends->count);
        return 0;
    }
    list->count = 0;
    list->sends = room;
    for (ad_u32 i = 0; i < sends->count; ++i) {
        if (!sends->send[i].to->real) {
            ad_log("a send to a voice not made again, left out", i);
            continue;
        }
        room[list->count++] = (struct xa_send){ sends->send[i].flags, sends->send[i].to->real };
    }
    return list;
}

static const struct xa_sends *ad_translate_sends(const struct xa_sends *given, struct xa_sends *list,
                                                 struct xa_send room[AD_SENDS])
{
    if (!given || given->count > AD_SENDS) return given;    /* the default, or more than we translate */
    list->count = given->count;
    list->sends = room;
    for (ad_u32 i = 0; i < given->count; ++i)
        room[i] = (struct xa_send){ given->sends[i].flags, ad_real_of(given->sends[i].voice) };
    return list;
}

static const struct xa_chain *ad_real_chain(const struct ad_chain *chain, struct xa_chain *list,
                                            struct xa_effect room[AD_EFFECTS])
{
    if (chain->kind != AD_CHAIN_SET) {
        if (chain->kind == AD_CHAIN_TOO_BIG) ad_log("an effect chain not kept, made without", 0);
        return 0;
    }
    list->count = chain->count;
    list->effects = room;
    for (ad_u32 i = 0; i < chain->count; ++i)
        room[i] = (struct xa_effect){ chain->effect[i].effect, chain->effect[i].initial_state,
                                      chain->effect[i].output_channels };
    return list;
}

/* ---- the pending buffers of a source voice ---- */

static int ad_pending_push(struct ad_pending *p, const struct xa_buffer *buffer, const struct xa_buffer_wma *wma)
{
    ad_lock(&p->spin);
    int room = p->count < AD_PENDING;
    if (room) {
        ad_u32 at = (p->first + p->count++) % AD_PENDING;
        p->entry[at].buffer = *buffer;
        p->entry[at].kind = wma ? AD_WMA : AD_PCM;
        if (wma) p->entry[at].wma = *wma;
    }
    ad_unlock(&p->spin);
    return room;
}

static void ad_pending_drop_last(struct ad_pending *p)
{
    ad_lock(&p->spin);
    if (p->count) --p->count;
    ad_unlock(&p->spin);
}

/* A buffer's end: XAudio2 ends a voice's buffers in their order, so it is
 * the first; one that is not (never seen yet) is looked for. */
static void ad_pending_end(struct ad_pending *p, void *context)
{
    ad_lock(&p->spin);
    ad_u32 i = 0;
    while (i < p->count && p->entry[(p->first + i) % AD_PENDING].buffer.context != context) ++i;
    if (i < p->count) {
        for (; i > 0; --i)
            p->entry[(p->first + i) % AD_PENDING] = p->entry[(p->first + i - 1) % AD_PENDING];
        p->first = (p->first + 1) % AD_PENDING;
        --p->count;
    }
    ad_unlock(&p->spin);
}

/* ---- the registry ---- */

static void ad_link(struct ad_voice *voice)
{
    ad_lock(&ad_voices_spin);
    voice->next = ad_voices;
    if (ad_voices) ad_voices->previous = voice;
    ad_voices = voice;
    ad_unlock(&ad_voices_spin);
}

static void ad_unlink(struct ad_voice *voice)
{
    ad_lock(&ad_voices_spin);
    if (voice->previous) voice->previous->next = voice->next;
    else ad_voices = voice->next;
    if (voice->next) voice->next->previous = voice->previous;
    ad_unlock(&ad_voices_spin);
}

static struct ad_voice *ad_new_voice(enum ad_kind kind)
{
    struct ad_voice *voice = ad_alloc(sizeof *voice);
    if (!voice) return 0;
    voice->vtbl = kind == AD_MASTER ? &ad_master_vtbl : kind == AD_SUBMIX ? &ad_submix_vtbl : &ad_source_vtbl;
    voice->kind = kind;
    if (kind == AD_SOURCE) {
        voice->as.source.pending = ad_alloc(sizeof *voice->as.source.pending);
        if (!voice->as.source.pending) {
            ad_free(voice);
            return 0;
        }
        voice->as.source.callback = (struct ad_callback){ &ad_callback_vtbl, voice };
    }
    return voice;
}

static void ad_free_voice(struct ad_voice *voice)
{
    ad_release_chain(&voice->chain);
    if (voice->kind == AD_SOURCE) ad_free(voice->as.source.pending);
    ad_free(voice);
}

/* ---- the voices' methods (the game's calls) ---- */

#define VOICE(self) ((struct ad_voice *)(self))
#define REAL_OR(self, refused) do { if (!VOICE(self)->real) return (refused); } while (0)
#define REAL_OR_NOTHING(self) do { if (!VOICE(self)->real) return; } while (0)
#define REAL(self) voice_methods(VOICE(self)->real)

static void AD_STDCALL v_get_voice_details(void *self, struct xa_voice_details *details)
{
    if (!VOICE(self)->real) {
        *details = (struct xa_voice_details){ 0, 0, 0 };
        return;
    }
    REAL(self)->get_voice_details(VOICE(self)->real, details);
}

static ad_hresult AD_STDCALL v_set_output_voices(void *self, const struct xa_sends *sends)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    struct xa_sends list;
    struct xa_send room[AD_SENDS];
    ad_hresult hr = REAL(self)->set_output_voices(VOICE(self)->real, ad_translate_sends(sends, &list, room));
    if (!AD_FAILED(hr)) ad_keep_sends(VOICE(self), sends);
    return hr;
}

static ad_hresult AD_STDCALL v_set_effect_chain(void *self, const struct xa_chain *chain)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_effect_chain(VOICE(self)->real, chain);
    if (!AD_FAILED(hr)) ad_keep_chain(&VOICE(self)->chain, chain);
    return hr;
}

static void ad_keep_enabled(struct ad_voice *voice, ad_u32 index, int enabled)
{
    if (voice->chain.kind == AD_CHAIN_SET && index < voice->chain.count)
        voice->chain.effect[index].enabled = enabled ? AD_EFFECT_ENABLED : AD_EFFECT_DISABLED;
}

static ad_hresult AD_STDCALL v_enable_effect(void *self, ad_u32 index, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->enable_effect(VOICE(self)->real, index, operation_set);
    if (!AD_FAILED(hr)) ad_keep_enabled(VOICE(self), index, 1);
    return hr;
}

static ad_hresult AD_STDCALL v_disable_effect(void *self, ad_u32 index, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->disable_effect(VOICE(self)->real, index, operation_set);
    if (!AD_FAILED(hr)) ad_keep_enabled(VOICE(self), index, 0);
    return hr;
}

static void AD_STDCALL v_get_effect_state(void *self, ad_u32 index, int *enabled)
{
    *enabled = 0;
    REAL_OR_NOTHING(self);
    REAL(self)->get_effect_state(VOICE(self)->real, index, enabled);
}

static ad_hresult AD_STDCALL v_set_effect_parameters(void *self, ad_u32 index, const void *parameters,
                                                     ad_u32 size, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_effect_parameters(VOICE(self)->real, index, parameters, size, operation_set);
    struct ad_chain *chain = &VOICE(self)->chain;
    if (!AD_FAILED(hr) && chain->kind == AD_CHAIN_SET && index < chain->count) {
        struct ad_effect *e = &chain->effect[index];
        e->parameters = size <= AD_PARAMETERS ? AD_SET : AD_TOO_BIG;
        if (e->parameters == AD_SET) {
            ad_copy(e->bytes, parameters, size);
            e->size = size;
        }
    }
    return hr;
}

static ad_hresult AD_STDCALL v_get_effect_parameters(void *self, ad_u32 index, void *parameters, ad_u32 size)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    return REAL(self)->get_effect_parameters(VOICE(self)->real, index, parameters, size);
}

static ad_hresult AD_STDCALL v_set_filter_parameters(void *self, const struct xa_filter *filter,
                                                     ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_filter_parameters(VOICE(self)->real, filter, operation_set);
    if (!AD_FAILED(hr)) {
        VOICE(self)->filter.state = AD_SET;
        VOICE(self)->filter.value = *filter;
    }
    return hr;
}

static void AD_STDCALL v_get_filter_parameters(void *self, struct xa_filter *filter)
{
    REAL_OR_NOTHING(self);
    REAL(self)->get_filter_parameters(VOICE(self)->real, filter);
}

static ad_hresult AD_STDCALL v_set_output_filter_parameters(void *self, void *destination,
                                                            const struct xa_filter *filter, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_output_filter_parameters(VOICE(self)->real, ad_real_of(destination), filter,
                                                             operation_set);
    if (AD_FAILED(hr)) return hr;
    struct ad_voice *voice = VOICE(self);
    struct ad_voice *to = ad_ours(destination);
    if (destination && !to) {
        voice->output_filters.state = AD_TOO_BIG;
        return hr;
    }
    ad_u32 i = 0;
    while (i < voice->output_filters.count && voice->output_filters.filter[i].to != to) ++i;
    if (i == AD_SENDS) {
        voice->output_filters.state = AD_TOO_BIG;
        return hr;
    }
    if (voice->output_filters.state == AD_UNSET) voice->output_filters.state = AD_SET;
    if (i == voice->output_filters.count) ++voice->output_filters.count;
    voice->output_filters.filter[i] = (struct ad_output_filter){ to, *filter };
    return hr;
}

static void AD_STDCALL v_get_output_filter_parameters(void *self, void *destination, struct xa_filter *filter)
{
    REAL_OR_NOTHING(self);
    REAL(self)->get_output_filter_parameters(VOICE(self)->real, ad_real_of(destination), filter);
}

static ad_hresult AD_STDCALL v_set_volume(void *self, float volume, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_volume(VOICE(self)->real, volume, operation_set);
    if (!AD_FAILED(hr)) {
        VOICE(self)->volume.state = AD_SET;
        VOICE(self)->volume.value = volume;
    }
    return hr;
}

static void AD_STDCALL v_get_volume(void *self, float *volume)
{
    *volume = 0;
    REAL_OR_NOTHING(self);
    REAL(self)->get_volume(VOICE(self)->real, volume);
}

static ad_hresult AD_STDCALL v_set_channel_volumes(void *self, ad_u32 channels, const float *volumes,
                                                   ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_channel_volumes(VOICE(self)->real, channels, volumes, operation_set);
    if (!AD_FAILED(hr)) {
        struct ad_voice *voice = VOICE(self);
        voice->channel_volumes.state = channels <= AD_CHANNELS ? AD_SET : AD_TOO_BIG;
        if (voice->channel_volumes.state == AD_SET) {
            voice->channel_volumes.channels = channels;
            ad_copy(voice->channel_volumes.value, volumes, channels * sizeof *volumes);
        }
    }
    return hr;
}

static void AD_STDCALL v_get_channel_volumes(void *self, ad_u32 channels, float *volumes)
{
    REAL_OR_NOTHING(self);
    REAL(self)->get_channel_volumes(VOICE(self)->real, channels, volumes);
}

static ad_hresult AD_STDCALL v_set_output_matrix(void *self, void *destination, ad_u32 source_channels,
                                                 ad_u32 destination_channels, const float *matrix,
                                                 ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_output_matrix(VOICE(self)->real, ad_real_of(destination), source_channels,
                                                  destination_channels, matrix, operation_set);
    if (AD_FAILED(hr)) return hr;
    struct ad_voice *voice = VOICE(self);
    struct ad_voice *to = ad_ours(destination);
    ad_u32 size = source_channels * destination_channels;
    if ((destination && !to) || size > AD_MATRIX || source_channels > AD_MATRIX) {
        voice->matrices.state = AD_TOO_BIG;
        return hr;
    }
    ad_u32 i = 0;
    while (i < voice->matrices.count && voice->matrices.matrix[i].to != to) ++i;
    if (i == AD_SENDS) {
        voice->matrices.state = AD_TOO_BIG;
        return hr;
    }
    if (voice->matrices.state == AD_UNSET) voice->matrices.state = AD_SET;
    if (i == voice->matrices.count) ++voice->matrices.count;
    struct ad_matrix *m = &voice->matrices.matrix[i];
    m->to = to;
    m->source_channels = source_channels;
    m->destination_channels = destination_channels;
    ad_copy(m->m, matrix, size * sizeof *matrix);
    return hr;
}

static void AD_STDCALL v_get_output_matrix(void *self, void *destination, ad_u32 source_channels,
                                           ad_u32 destination_channels, float *matrix)
{
    REAL_OR_NOTHING(self);
    REAL(self)->get_output_matrix(VOICE(self)->real, ad_real_of(destination), source_channels,
                                  destination_channels, matrix);
}

static void AD_STDCALL v_destroy_voice(void *self)
{
    struct ad_voice *voice = VOICE(self);
    if (voice->real) voice_methods(voice->real)->destroy_voice(voice->real);
    ad_unlink(voice);
    struct ad_engine *engine = ad_the_engine;
    if (engine && engine->binding.kind != AD_NO_MASTER && engine->binding.master == voice)
        engine->binding.kind = AD_NO_MASTER;
    ad_free_voice(voice);
}

static ad_hresult AD_STDCALL v_start(void *self, ad_u32 flags, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->start(VOICE(self)->real, flags, operation_set);
    if (!AD_FAILED(hr)) VOICE(self)->as.source.play = AD_STARTED;
    return hr;
}

static ad_hresult AD_STDCALL v_stop(void *self, ad_u32 flags, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->stop(VOICE(self)->real, flags, operation_set);
    if (!AD_FAILED(hr)) VOICE(self)->as.source.play = AD_STOPPED;
    return hr;
}

static ad_hresult AD_STDCALL v_submit_source_buffer(void *self, const struct xa_buffer *buffer,
                                                    const struct xa_buffer_wma *wma)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    struct ad_pending *pending = VOICE(self)->as.source.pending;
    if (!ad_pending_push(pending, buffer, wma)) return AD_XAUDIO2_E_INVALID_CALL;   /* XAudio2's own limit */
    ad_hresult hr = REAL(self)->submit_source_buffer(VOICE(self)->real, buffer, wma);
    if (AD_FAILED(hr)) ad_pending_drop_last(pending);
    return hr;
}

static ad_hresult AD_STDCALL v_flush_source_buffers(void *self)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    return REAL(self)->flush_source_buffers(VOICE(self)->real);     /* their ends come as callbacks */
}

static ad_hresult AD_STDCALL v_discontinuity(void *self)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    return REAL(self)->discontinuity(VOICE(self)->real);
}

static ad_hresult AD_STDCALL v_exit_loop(void *self, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    return REAL(self)->exit_loop(VOICE(self)->real, operation_set);
}

static void AD_STDCALL v_get_state(void *self, struct xa_voice_state *state)
{
    if (!VOICE(self)->real) {
        *state = (struct xa_voice_state){ 0, 0, 0 };
        return;
    }
    REAL(self)->get_state(VOICE(self)->real, state);
}

static ad_hresult AD_STDCALL v_set_frequency_ratio(void *self, float ratio, ad_u32 operation_set)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_frequency_ratio(VOICE(self)->real, ratio, operation_set);
    if (!AD_FAILED(hr)) {
        VOICE(self)->as.source.ratio.state = AD_SET;
        VOICE(self)->as.source.ratio.value = ratio;
    }
    return hr;
}

static void AD_STDCALL v_get_frequency_ratio(void *self, float *ratio)
{
    *ratio = 1;
    REAL_OR_NOTHING(self);
    REAL(self)->get_frequency_ratio(VOICE(self)->real, ratio);
}

static ad_hresult AD_STDCALL v_set_source_sample_rate(void *self, ad_u32 rate)
{
    REAL_OR(self, AD_XAUDIO2_E_INVALID_CALL);
    ad_hresult hr = REAL(self)->set_source_sample_rate(VOICE(self)->real, rate);
    if (!AD_FAILED(hr)) {
        VOICE(self)->as.source.rate.state = AD_SET;
        VOICE(self)->as.source.rate.value = rate;
    }
    return hr;
}

#define AD_VOICE_METHODS \
    v_get_voice_details, v_set_output_voices, v_set_effect_chain, v_enable_effect, v_disable_effect, \
    v_get_effect_state, v_set_effect_parameters, v_get_effect_parameters, v_set_filter_parameters, \
    v_get_filter_parameters, v_set_output_filter_parameters, v_get_output_filter_parameters, v_set_volume, \
    v_get_volume, v_set_channel_volumes, v_get_channel_volumes, v_set_output_matrix, v_get_output_matrix, \
    v_destroy_voice
/* A submix or mastering voice has no source methods: its table ends there. */
static const struct xa_voice_vtbl ad_master_vtbl = { AD_VOICE_METHODS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
static const struct xa_voice_vtbl ad_submix_vtbl = { AD_VOICE_METHODS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
static const struct xa_voice_vtbl ad_source_vtbl = {
    AD_VOICE_METHODS, v_start, v_stop, v_submit_source_buffer, v_flush_source_buffers, v_discontinuity,
    v_exit_loop, v_get_state, v_set_frequency_ratio, v_get_frequency_ratio, v_set_source_sample_rate,
};

/* ---- a source voice's callback (XAudio2's thread) ---- */

#define GAME_CALLBACK(self) (((struct ad_callback *)(self))->voice->as.source.game_callback)
#define GAME_CALLBACK_METHODS(callback) (*(const struct xa_voice_callback_vtbl **)(callback))

static void AD_STDCALL cb_pass_start(void *self, ad_u32 bytes)
{
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->pass_start(game, bytes);
}

static void AD_STDCALL cb_pass_end(void *self)
{
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->pass_end(game);
}

static void AD_STDCALL cb_stream_end(void *self)
{
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->stream_end(game);
}

static void AD_STDCALL cb_buffer_start(void *self, void *context)
{
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->buffer_start(game, context);
}

static void AD_STDCALL cb_buffer_end(void *self, void *context)
{
    ad_pending_end(((struct ad_callback *)self)->voice->as.source.pending, context);
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->buffer_end(game, context);
}

static void AD_STDCALL cb_loop_end(void *self, void *context)
{
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->loop_end(game, context);
}

static void AD_STDCALL cb_voice_error(void *self, void *context, ad_hresult error)
{
    void *game = GAME_CALLBACK(self);
    if (game) GAME_CALLBACK_METHODS(game)->voice_error(game, context, error);
}

static const struct xa_voice_callback_vtbl ad_callback_vtbl = {
    cb_pass_start, cb_pass_end, cb_stream_end, cb_buffer_start, cb_buffer_end, cb_loop_end, cb_voice_error,
};

/* ---- the engine callback (XAudio2's thread) and the device client (MMDevice's) ---- */

static void AD_STDCALL ec_pass(void *self) { (void)self; }

static void AD_STDCALL ec_critical_error(void *self, ad_hresult error)
{
    (void)error;
    if (self == &ad_engine_callbacks[__atomic_load_n(&ad_current_callback, __ATOMIC_ACQUIRE)])
        ad_raise(AD_EVENT_CRITICAL);
}

static const struct xa_engine_callback_vtbl ad_engine_callback_vtbl = { ec_pass, ec_pass, ec_critical_error };
static struct ad_engine_callback ad_engine_callbacks[2] = { { &ad_engine_callback_vtbl },
                                                            { &ad_engine_callback_vtbl } };

static struct { const struct mm_client_vtbl *vtbl; } ad_client;

static ad_hresult AD_STDCALL mm_query_interface(void *self, const struct ad_guid *iid, void **out)
{
    if (ad_same_guid(iid, &iid_unknown) || ad_same_guid(iid, &iid_notification_client)) {
        *out = self;
        return AD_S_OK;
    }
    *out = 0;
    return AD_E_NOINTERFACE;
}

static ad_u32 AD_STDCALL mm_reference(void *self) { (void)self; return 1; }   /* static: never freed */
static ad_hresult AD_STDCALL mm_state_changed(void *self, const ad_u16 *id, ad_u32 state)
{
    (void)self, (void)id, (void)state;
    return AD_S_OK;
}
static ad_hresult AD_STDCALL mm_device(void *self, const ad_u16 *id) { (void)self, (void)id; return AD_S_OK; }
static ad_hresult AD_STDCALL mm_property_changed(void *self, const ad_u16 *id, struct ad_property_key key)
{
    (void)self, (void)id, (void)key;
    return AD_S_OK;
}

static ad_hresult AD_STDCALL mm_default_changed(void *self, ad_u32 flow, ad_u32 role, const ad_u16 *id)
{
    (void)self, (void)id;
    if (flow == AD_FLOW_RENDER && role != AD_ROLE_COMMUNICATIONS) ad_raise(AD_EVENT_DEFAULT);
    return AD_S_OK;
}

static const struct mm_client_vtbl ad_client_vtbl = {
    mm_query_interface, mm_reference, mm_reference, mm_state_changed, mm_device, mm_device, mm_default_changed,
    mm_property_changed,
};

static void ad_watch_devices(void)
{
    ad_client.vtbl = &ad_client_vtbl;
    void *enumerator = 0;
    ad_hresult hr = ad_mm_enumerator(&enumerator);
    if (AD_FAILED(hr) || !enumerator) {
        ad_log("no MMDevice enumerator: a new default device is not followed", (ad_u32)hr);
        return;
    }
    const struct mm_enumerator_vtbl *methods = *(const struct mm_enumerator_vtbl **)enumerator;
    hr = methods->register_client(enumerator, &ad_client);
    if (AD_FAILED(hr)) {
        ad_log("the device client not registered: a new default device is not followed", (ad_u32)hr);
        methods->release(enumerator);
        return;
    }
    ad_enumerator = enumerator;
}

static void ad_unwatch_devices(void)
{
    if (!ad_enumerator) return;
    const struct mm_enumerator_vtbl *methods = *(const struct mm_enumerator_vtbl **)ad_enumerator;
    methods->unregister_client(ad_enumerator, &ad_client);
    methods->release(ad_enumerator);
    ad_enumerator = 0;
}

/* ---- the engine's methods (the game's calls) ---- */

#define ENGINE(self) ((struct ad_engine *)(self))
#define REAL_ENGINE(self) engine_methods(ENGINE(self)->real)

static ad_hresult AD_STDCALL e_query_interface(void *self, const struct ad_guid *iid, void **out)
{
    if (ad_same_guid(iid, &iid_unknown) || ad_same_guid(iid, &iid_xaudio2)) {
        __atomic_add_fetch(&ENGINE(self)->references, 1, __ATOMIC_RELAXED);
        *out = self;
        return AD_S_OK;
    }
    *out = 0;
    return AD_E_NOINTERFACE;
}

static ad_u32 AD_STDCALL e_add_ref(void *self) { return __atomic_add_fetch(&ENGINE(self)->references, 1, __ATOMIC_RELAXED); }

static ad_u32 AD_STDCALL e_release(void *self)
{
    struct ad_engine *engine = ENGINE(self);
    ad_u32 left = __atomic_sub_fetch(&engine->references, 1, __ATOMIC_ACQ_REL);
    if (left) return left;
    ad_unwatch_devices();
    engine_methods(engine->real)->unregister_for_callbacks(engine->real, &ad_engine_callbacks[ad_current_callback]);
    engine_methods(engine->real)->release(engine->real);
    __atomic_store_n(&ad_the_engine, 0, __ATOMIC_RELEASE);
    ad_free(engine);
    return 0;
}

static ad_hresult AD_STDCALL e_get_device_count(void *self, ad_u32 *count)
{
    return REAL_ENGINE(self)->get_device_count(ENGINE(self)->real, count);
}

static ad_hresult AD_STDCALL e_get_device_details(void *self, ad_u32 index, struct xa_device_details *details)
{
    return REAL_ENGINE(self)->get_device_details(ENGINE(self)->real, index, details);
}

static ad_hresult AD_STDCALL e_initialize(void *self, ad_u32 flags, ad_u32 processor)
{
    return REAL_ENGINE(self)->initialize(ENGINE(self)->real, flags, processor);
}

static ad_hresult AD_STDCALL e_register_for_callbacks(void *self, void *callback)
{
    struct ad_engine *engine = ENGINE(self);
    ad_hresult hr = REAL_ENGINE(self)->register_for_callbacks(engine->real, callback);
    if (AD_FAILED(hr)) return hr;
    for (ad_u32 i = 0; i < engine->game_callback_count; ++i)
        if (engine->game_callbacks[i] == callback) return hr;
    if (engine->game_callback_count < AD_CALLBACKS) engine->game_callbacks[engine->game_callback_count++] = callback;
    else ad_log("an engine callback of the game's not kept", engine->game_callback_count);
    return hr;
}

static void AD_STDCALL e_unregister_for_callbacks(void *self, void *callback)
{
    struct ad_engine *engine = ENGINE(self);
    REAL_ENGINE(self)->unregister_for_callbacks(engine->real, callback);
    for (ad_u32 i = 0; i < engine->game_callback_count; ++i)
        if (engine->game_callbacks[i] == callback) {
            engine->game_callbacks[i] = engine->game_callbacks[--engine->game_callback_count];
            return;
        }
}

/* A WAVEFORMATEX's bytes: PCM's 16 (its cbSize is not read: XAudio2
 * ignores it), another's 18 and its cbSize. */
static ad_u32 ad_format_size(const void *format)
{
    ad_u16 tag, extra;
    ad_copy(&tag, format, 2);                               /* wFormatTag */
    ad_copy(&extra, (const ad_u8 *)format + 16, 2);         /* cbSize */
    return tag == 1 ? 16u : 18u + extra;
}

static ad_hresult AD_STDCALL e_create_source_voice(void *self, void **out, const void *format, ad_u32 flags,
                                                   float max_ratio, void *callback, const struct xa_sends *sends,
                                                   const struct xa_chain *chain)
{
    struct ad_engine *engine = ENGINE(self);
    ad_u32 format_size = ad_format_size(format);
    struct ad_voice *voice = ad_new_voice(AD_SOURCE);
    if (!voice) return AD_E_OUTOFMEMORY;
    struct xa_sends list;
    struct xa_send room[AD_SENDS];
    void *real = 0;
    ad_hresult hr = REAL_ENGINE(self)->create_source_voice(engine->real, &real, format, flags, max_ratio,
                                                           &voice->as.source.callback,
                                                           ad_translate_sends(sends, &list, room), chain);
    if (AD_FAILED(hr)) {
        ad_free_voice(voice);
        return hr;
    }
    voice->real = real;
    voice->as.source.format_kept = format_size <= AD_FORMAT ? AD_SET : AD_TOO_BIG;
    if (voice->as.source.format_kept == AD_SET) ad_copy(voice->as.source.format, format, format_size);
    voice->as.source.flags = flags;
    voice->as.source.max_ratio = max_ratio;
    voice->as.source.game_callback = callback;
    ad_keep_sends(voice, sends);
    ad_keep_chain(&voice->chain, chain);
    ad_link(voice);
    *out = voice;
    return hr;
}

static ad_hresult AD_STDCALL e_create_submix_voice(void *self, void **out, ad_u32 channels, ad_u32 rate,
                                                   ad_u32 flags, ad_u32 stage, const struct xa_sends *sends,
                                                   const struct xa_chain *chain)
{
    struct ad_voice *voice = ad_new_voice(AD_SUBMIX);
    if (!voice) return AD_E_OUTOFMEMORY;
    struct xa_sends list;
    struct xa_send room[AD_SENDS];
    void *real = 0;
    ad_hresult hr = REAL_ENGINE(self)->create_submix_voice(ENGINE(self)->real, &real, channels, rate, flags, stage,
                                                           ad_translate_sends(sends, &list, room), chain);
    if (AD_FAILED(hr)) {
        ad_free_voice(voice);
        return hr;
    }
    voice->real = real;
    voice->as.submix.channels = channels;
    voice->as.submix.rate = rate;
    voice->as.submix.flags = flags;
    voice->as.submix.stage = stage;
    ad_keep_sends(voice, sends);
    ad_keep_chain(&voice->chain, chain);
    ad_link(voice);
    *out = voice;
    return hr;
}

/* The device the game's rule picks (initialize_xaudio 0x594A0..0x59698):
 * the last of the DefaultMultimediaDevice role, else device 0; its id. */
static int ad_default_device(void *engine, ad_u32 *index, ad_u16 id[256])
{
    static struct xa_device_details details;      /* 1068 bytes: the sound thread's alone */
    ad_u32 count = 0;
    if (AD_FAILED(engine_methods(engine)->get_device_count(engine, &count)) || !count) return 0;
    ad_u32 chosen = 0;
    for (ad_u32 i = 0; i < count; ++i)
        if (!AD_FAILED(engine_methods(engine)->get_device_details(engine, i, &details))
            && (details.role & AD_DEFAULT_MULTIMEDIA_DEVICE))
            chosen = i;
    if (AD_FAILED(engine_methods(engine)->get_device_details(engine, chosen, &details))) return 0;
    *index = chosen;
    ad_copy(id, details.id, sizeof details.id);
    return 1;
}

static ad_hresult AD_STDCALL e_create_mastering_voice(void *self, void **out, ad_u32 channels, ad_u32 rate,
                                                      ad_u32 flags, ad_u32 device, const struct xa_chain *chain)
{
    struct ad_engine *engine = ENGINE(self);
    struct ad_voice *voice = ad_new_voice(AD_MASTER);
    if (!voice) return AD_E_OUTOFMEMORY;
    void *real = 0;
    ad_hresult hr = REAL_ENGINE(self)->create_mastering_voice(engine->real, &real, channels, rate, flags, device,
                                                              chain);
    if (AD_FAILED(hr)) {
        ad_free_voice(voice);
        return hr;
    }
    voice->real = real;
    voice->as.master.channels = channels;
    voice->as.master.rate = rate;
    voice->as.master.flags = flags;
    voice->as.master.device = device;
    voice->sends.kind = AD_SENDS_LIST;                  /* a master sends nowhere */
    ad_keep_chain(&voice->chain, chain);
    ad_link(voice);
    static struct xa_device_details details;
    engine->binding.kind = AD_BOUND;
    engine->binding.master = voice;
    if (AD_FAILED(REAL_ENGINE(self)->get_device_details(engine->real, device, &details))) details.id[0] = 0;
    ad_copy(engine->binding.device, details.id, sizeof details.id);
    *out = voice;
    return hr;
}

static ad_hresult AD_STDCALL e_start_engine(void *self)
{
    ad_hresult hr = REAL_ENGINE(self)->start_engine(ENGINE(self)->real);
    if (!AD_FAILED(hr)) ENGINE(self)->run = AD_RUNNING;
    return hr;
}

static void AD_STDCALL e_stop_engine(void *self)
{
    REAL_ENGINE(self)->stop_engine(ENGINE(self)->real);
    ENGINE(self)->run = AD_ENGINE_STOPPED;
}

static ad_hresult AD_STDCALL e_commit_changes(void *self, ad_u32 operation_set)
{
    return REAL_ENGINE(self)->commit_changes(ENGINE(self)->real, operation_set);
}

static void AD_STDCALL e_get_performance_data(void *self, void *data)
{
    REAL_ENGINE(self)->get_performance_data(ENGINE(self)->real, data);
}

static void AD_STDCALL e_set_debug_configuration(void *self, const void *configuration, void *reserved)
{
    REAL_ENGINE(self)->set_debug_configuration(ENGINE(self)->real, configuration, reserved);
}

static const struct xa_engine_vtbl ad_engine_vtbl = {
    e_query_interface, e_add_ref, e_release, e_get_device_count, e_get_device_details, e_initialize,
    e_register_for_callbacks, e_unregister_for_callbacks, e_create_source_voice, e_create_submix_voice,
    e_create_mastering_voice, e_start_engine, e_stop_engine, e_commit_changes, e_get_performance_data,
    e_set_debug_configuration,
};

ad_hresult audio_device_create(void **out, ad_u32 flags)
{
    void *real = 0;
    ad_hresult hr = ad_xaudio2_create(&real, flags);
    if (AD_FAILED(hr)) return hr;
    struct ad_engine *engine = __atomic_load_n(&ad_the_engine, __ATOMIC_ACQUIRE) ? 0 : ad_alloc(sizeof *engine);
    if (!engine) {
        ad_log("an engine not ours: its device is not followed", 0);
        *out = real;
        return hr;
    }
    engine->vtbl = &ad_engine_vtbl;
    engine->references = 1;
    engine->real = real;
    engine->create_flags = flags;
    engine->run = AD_RUNNING;
    engine->binding.kind = AD_NO_MASTER;
    if (AD_FAILED(engine_methods(real)->register_for_callbacks(real, &ad_engine_callbacks[ad_current_callback])))
        ad_log("the engine callback not registered: a lost device is not seen", 0);
    ad_watch_devices();
    __atomic_store_n(&ad_the_engine, engine, __ATOMIC_RELEASE);
    *out = engine;
    return hr;
}

/* ---- the rebuild (the sound thread) ---- */

static void ad_replay(struct ad_voice *voice)
{
    void *real = voice->real;
    const struct xa_voice_vtbl *m = voice_methods(real);
    if (voice->chain.kind == AD_CHAIN_SET)
        for (ad_u32 i = 0; i < voice->chain.count; ++i) {
            const struct ad_effect *e = &voice->chain.effect[i];
            if (e->enabled == AD_EFFECT_ENABLED) m->enable_effect(real, i, 0);
            if (e->enabled == AD_EFFECT_DISABLED) m->disable_effect(real, i, 0);
            if (e->parameters == AD_SET) m->set_effect_parameters(real, i, e->bytes, e->size, 0);
            if (e->parameters == AD_TOO_BIG) ad_log("an effect's parameters not kept", e->size);
        }
    if (voice->filter.state == AD_SET) m->set_filter_parameters(real, &voice->filter.value, 0);
    if (voice->volume.state == AD_SET) m->set_volume(real, voice->volume.value, 0);
    if (voice->channel_volumes.state == AD_SET)
        m->set_channel_volumes(real, voice->channel_volumes.channels, voice->channel_volumes.value, 0);
    if (voice->channel_volumes.state == AD_TOO_BIG) ad_log("channel volumes not kept", 0);
    for (ad_u32 i = 0; i < voice->matrices.count; ++i) {
        const struct ad_matrix *x = &voice->matrices.matrix[i];
        if (x->to && !x->to->real) continue;
        if (AD_FAILED(m->set_output_matrix(real, x->to ? x->to->real : 0, x->source_channels,
                                           x->destination_channels, x->m, 0)))
            ad_log("an output matrix not made again (the default kept)", x->destination_channels);
    }
    if (voice->matrices.state == AD_TOO_BIG) ad_log("an output matrix not kept", 0);
    for (ad_u32 i = 0; i < voice->output_filters.count; ++i) {
        const struct ad_output_filter *f = &voice->output_filters.filter[i];
        if (f->to && !f->to->real) continue;
        m->set_output_filter_parameters(real, f->to ? f->to->real : 0, &f->filter, 0);
    }
    if (voice->output_filters.state == AD_TOO_BIG) ad_log("an output filter not kept", 0);
    if (voice->kind != AD_SOURCE) return;
    if (voice->as.source.ratio.state == AD_SET) m->set_frequency_ratio(real, voice->as.source.ratio.value, 0);
    if (voice->as.source.rate.state == AD_SET) m->set_source_sample_rate(real, voice->as.source.rate.value);
}

/* A source voice's buffers again, in order (their copy taken under the
 * lock, the submissions outside it: the voice is not started yet). */
static void ad_resubmit(struct ad_voice *voice)
{
    static struct ad_pending copy;                  /* the sound thread's alone */
    struct ad_pending *pending = voice->as.source.pending;
    ad_lock(&pending->spin);
    copy.first = pending->first;
    copy.count = pending->count;
    ad_copy(copy.entry, pending->entry, sizeof copy.entry);
    ad_unlock(&pending->spin);
    for (ad_u32 i = 0; i < copy.count; ++i) {
        const struct ad_pending_entry *e = &copy.entry[(copy.first + i) % AD_PENDING];
        if (AD_FAILED(voice_methods(voice->real)->submit_source_buffer(voice->real, &e->buffer,
                                                                       e->kind == AD_WMA ? &e->wma : 0)))
            ad_log("a pending buffer not queued again", i);
    }
}

/* The voices of a kind, the submixes by their processing stage (rising:
 * a voice before the stages it sends to), into `out` (room for every voice). */
static ad_u32 ad_collect(enum ad_kind kind, struct ad_voice **out)
{
    ad_u32 n = 0;
    for (struct ad_voice *v = ad_voices; v; v = v->next) {
        if (v->kind != kind) continue;
        ad_u32 at = n++;
        while (at > 0 && kind == AD_SUBMIX && out[at - 1]->as.submix.stage > v->as.submix.stage) {
            out[at] = out[at - 1];
            --at;
        }
        out[at] = v;
    }
    return n;
}

static ad_u32 ad_count_voices(void)
{
    ad_u32 n = 0;
    for (struct ad_voice *v = ad_voices; v; v = v->next) ++n;
    return n;
}

static void ad_make_again(struct ad_engine *engine, struct ad_voice *voice)
{
    struct xa_sends sends_list;
    struct xa_send sends_room[AD_SENDS];
    struct xa_chain chain_list;
    struct xa_effect chain_room[AD_EFFECTS];
    const struct xa_sends *sends = ad_real_sends(&voice->sends, &sends_list, sends_room);
    const struct xa_chain *chain = ad_real_chain(&voice->chain, &chain_list, chain_room);
    const struct xa_engine_vtbl *e = engine_methods(engine->real);
    if (voice->kind == AD_SOURCE && voice->as.source.format_kept != AD_SET) {
        ad_log("a source voice's format not kept: it stays silent", 0);
        return;
    }
    void *real = 0;
    ad_hresult hr = voice->kind == AD_SUBMIX
        ? e->create_submix_voice(engine->real, &real, voice->as.submix.channels, voice->as.submix.rate,
                                 voice->as.submix.flags, voice->as.submix.stage, sends, chain)
        : e->create_source_voice(engine->real, &real, voice->as.source.format, voice->as.source.flags,
                                 voice->as.source.max_ratio, &voice->as.source.callback, sends, chain);
    if (AD_FAILED(hr)) {
        ad_log("a voice not made again: it stays silent", (ad_u32)hr);
        return;
    }
    voice->real = real;
    ad_replay(voice);
    if (voice->kind == AD_SOURCE) {
        ad_resubmit(voice);
        if (voice->as.source.play == AD_STARTED) voice_methods(real)->start(real, 0, 0);
    }
}

/* The new engine and its mastering voice first: when either fails, nothing
 * changed (the old graph kept, the rebuild tried again later). Then the old
 * graph goes (the sources, the submixes up their stages, the master, the
 * engine) and every voice is made again on the new one, the submixes down
 * their stages (a voice after the ones it sends to), then the sources. */
static int ad_rebuild(struct ad_engine *engine)
{
    void *fresh = 0;
    ad_hresult hr = ad_xaudio2_create(&fresh, engine->create_flags);
    if (AD_FAILED(hr) || !fresh) {
        ad_log("rebuild: no new engine", (ad_u32)hr);
        return 0;
    }
    const struct xa_engine_vtbl *e = engine_methods(fresh);
    ad_u32 index;
    ad_u16 id[256];
    if (!ad_default_device(fresh, &index, id)) {
        ad_log("rebuild: no audio device", 0);
        e->release(fresh);
        return 0;
    }
    struct ad_voice *master = engine->binding.master;
    void *fresh_master = 0;
    hr = e->create_mastering_voice(fresh, &fresh_master, master->as.master.channels, master->as.master.rate,
                                   master->as.master.flags, index, 0);
    if (AD_FAILED(hr)) {
        ad_log("rebuild: no mastering voice on the default device", (ad_u32)hr);
        e->release(fresh);
        return 0;
    }
    ad_u32 next_callback = 1 - ad_current_callback;
    if (AD_FAILED(e->register_for_callbacks(fresh, &ad_engine_callbacks[next_callback])))
        ad_log("rebuild: the engine callback not registered", 0);

    /* commit: the old graph goes */
    ad_lock(&ad_voices_spin);
    ad_u32 total = ad_count_voices();
    struct ad_voice **order = total ? ad_alloc(total * sizeof *order) : 0;
    if (total && !order) {
        ad_unlock(&ad_voices_spin);
        ad_log("rebuild: no memory", total);
        voice_methods(fresh_master)->destroy_voice(fresh_master);
        e->unregister_for_callbacks(fresh, &ad_engine_callbacks[next_callback]);
        e->release(fresh);
        return 0;
    }
    void *old = engine->real;
    engine_methods(old)->stop_engine(old);
    ad_u32 sources = ad_collect(AD_SOURCE, order);
    for (ad_u32 i = 0; i < sources; ++i)
        if (order[i]->real) {
            voice_methods(order[i]->real)->destroy_voice(order[i]->real);
            order[i]->real = 0;
        }
    ad_u32 submixes = ad_collect(AD_SUBMIX, order);
    for (ad_u32 i = 0; i < submixes; ++i)
        if (order[i]->real) {
            voice_methods(order[i]->real)->destroy_voice(order[i]->real);
            order[i]->real = 0;
        }
    voice_methods(master->real)->destroy_voice(master->real);
    engine_methods(old)->unregister_for_callbacks(old, &ad_engine_callbacks[ad_current_callback]);
    engine_methods(old)->release(old);
    __atomic_store_n(&ad_current_callback, next_callback, __ATOMIC_RELEASE);

    /* the new graph */
    engine->real = fresh;
    master->real = fresh_master;
    master->as.master.device = index;
    if (master->chain.kind != AD_CHAIN_NONE) {
        struct xa_chain chain_list;
        struct xa_effect chain_room[AD_EFFECTS];
        voice_methods(fresh_master)->set_effect_chain(fresh_master,
                                                      ad_real_chain(&master->chain, &chain_list, chain_room));
    }
    ad_replay(master);
    for (ad_u32 i = submixes; i-- > 0;) ad_make_again(engine, order[i]);
    sources = ad_collect(AD_SOURCE, order);
    for (ad_u32 i = 0; i < sources; ++i) ad_make_again(engine, order[i]);
    ad_unlock(&ad_voices_spin);
    ad_free(order);
    for (ad_u32 i = 0; i < engine->game_callback_count; ++i)
        e->register_for_callbacks(fresh, engine->game_callbacks[i]);
    if (engine->run == AD_ENGINE_STOPPED) e->stop_engine(fresh);
    engine->binding.kind = AD_BOUND;
    ad_copy(engine->binding.device, id, sizeof id);
    ad_log("rebuilt on audio device", index);
    return 1;
}

/* The transition: (the binding, the events, the time) -> a rebuild or not. */
static void ad_step(struct ad_engine *engine, ad_u32 events, ad_u64 now)
{
    struct ad_binding *b = &engine->binding;
    switch (b->kind) {
    case AD_NO_MASTER:
        return;
    case AD_BOUND:
        if (!(events & AD_EVENT_CRITICAL)) {
            if (!(events & AD_EVENT_DEFAULT)) return;
            ad_u32 index;
            static ad_u16 id[256];
            if (ad_default_device(engine->real, &index, id) && ad_same_id(id, b->device)) return;
        }
        ad_log(events & AD_EVENT_CRITICAL ? "the audio device was lost" : "the default audio device changed",
               events);
        break;
    case AD_LOST:
        if (!events && now < b->retry_at) return;
        break;
    }
    if (!ad_rebuild(engine)) {
        b->kind = AD_LOST;
        b->retry_at = now + AD_RETRY_SECONDS * ad_frequency();
    }
}

void ad_tick(void)
{
    struct ad_engine *engine = __atomic_load_n(&ad_the_engine, __ATOMIC_ACQUIRE);
    if (!engine) return;
    ad_u32 events = __atomic_exchange_n(&ad_events, 0, __ATOMIC_ACQ_REL);
    if (!events && engine->binding.kind != AD_LOST) return;
    ad_step(engine, events, ad_now());
}
