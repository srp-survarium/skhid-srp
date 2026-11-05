/* Patch 99: the game's sound follows the audio output device
 * (docs/notes/audio_device.md). The core, audio_device_026g.c, is plain C
 * over XAudio2 2.7's COM interfaces and the five platform functions below;
 * audio_device_engine_026g.c gives them in the game (the engine-function
 * library), test_audio_device_026g.c on the host (a made-up XAudio2).
 *
 * The game's XAudio2 engine and every voice it makes are ours (proxies that
 * forward each call and keep what the voice was given); a lost or changed
 * device makes a new engine on the default device and the same voices on
 * it, the game's pointers unchanged. */
#ifndef AUDIO_DEVICE_026G_H
#define AUDIO_DEVICE_026G_H

typedef unsigned char ad_u8;
typedef unsigned short ad_u16;
typedef unsigned int ad_u32;
typedef unsigned long long ad_u64;
typedef int ad_hresult;

#define AD_STDCALL __attribute__((stdcall))

/* -- the platform (the game's or the test's) -- */

/* Zeroed memory of `size` bytes, or 0. */
void *ad_alloc(ad_u32 size);
void ad_free(void *memory);
/* The game's XAudio2Create (0x5A640): CoCreateInstance and Initialize; the
 * engine in *engine on success only. */
ad_hresult ad_xaudio2_create(void **engine, ad_u32 flags);
/* CoCreateInstance of the MMDevice enumerator, on the thread that made the
 * engine (its COM is the game's). */
ad_hresult ad_mm_enumerator(void **enumerator);
/* The performance counter and its counts a second. */
ad_u64 ad_now(void);
ad_u64 ad_frequency(void);
/* A line `srp: audio_device: <what> <value>` to srp_patches.log. */
void ad_log(const char *what, ad_u32 value);

/* -- the core's entries -- */

/* The game's XAudio2Create calls (initialize_xaudio 0x59187, 0x591C8):
 * the game's engine is our proxy of the one made. cdecl, as the original. */
ad_hresult audio_device_create(void **engine, ad_u32 flags);
/* The sound thread's tick (before process_orders): a device event since the
 * last tick, or a retry due, rebuilds the engine. */
void ad_tick(void);

/* -- XAudio2 2.7's types (xaudio2.h of the June 2010 DirectX SDK; packed 1) -- */

#define AD_S_OK 0
#define AD_E_NOINTERFACE ((ad_hresult)0x80004002u)
#define AD_E_OUTOFMEMORY ((ad_hresult)0x8007000Eu)
#define AD_XAUDIO2_E_INVALID_CALL ((ad_hresult)0x88960001u)
#define AD_FAILED(hr) ((hr) < 0)
#define AD_DEFAULT_MULTIMEDIA_DEVICE 2      /* XAUDIO2_DEVICE_ROLE's bit the game picks */

#pragma pack(push, 1)
struct xa_voice_details { ad_u32 creation_flags, input_channels, input_sample_rate; };
struct xa_send { ad_u32 flags; void *voice; };
struct xa_sends { ad_u32 count; const struct xa_send *sends; };
struct xa_effect { void *effect; ad_u32 initial_state, output_channels; };
struct xa_chain { ad_u32 count; const struct xa_effect *effects; };
struct xa_filter { ad_u32 type; float frequency, one_over_q; };
struct xa_buffer {
    ad_u32 flags, audio_bytes;
    const void *audio;
    ad_u32 play_begin, play_length, loop_begin, loop_length, loop_count;
    void *context;
};
struct xa_buffer_wma { const ad_u32 *cumulative_bytes; ad_u32 packets; };
struct xa_voice_state { void *context; ad_u32 queued; ad_u64 samples; };
struct xa_device_details { ad_u16 id[256], name[256]; ad_u32 role; ad_u8 format[40]; };
#pragma pack(pop)
_Static_assert(sizeof(struct xa_device_details) == 1068, "XAUDIO2_DEVICE_DETAILS");
_Static_assert(sizeof(struct xa_buffer) == 36 || sizeof(void *) != 4, "XAUDIO2_BUFFER");

struct ad_guid { ad_u32 a; ad_u16 b, c; ad_u8 d[8]; };
struct ad_property_key { struct ad_guid format; ad_u32 id; };

/* IUnknown, as an effect (an XAPO) is held. */
struct ad_unknown_vtbl {
    ad_hresult (AD_STDCALL *query_interface)(void *self, const struct ad_guid *iid, void **out);
    ad_u32 (AD_STDCALL *add_ref)(void *self);
    ad_u32 (AD_STDCALL *release)(void *self);
};

/* IXAudio2 (2.7). */
struct xa_engine_vtbl {
    ad_hresult (AD_STDCALL *query_interface)(void *self, const struct ad_guid *iid, void **out);
    ad_u32 (AD_STDCALL *add_ref)(void *self);
    ad_u32 (AD_STDCALL *release)(void *self);
    ad_hresult (AD_STDCALL *get_device_count)(void *self, ad_u32 *count);
    ad_hresult (AD_STDCALL *get_device_details)(void *self, ad_u32 index, struct xa_device_details *details);
    ad_hresult (AD_STDCALL *initialize)(void *self, ad_u32 flags, ad_u32 processor);
    ad_hresult (AD_STDCALL *register_for_callbacks)(void *self, void *callback);
    void (AD_STDCALL *unregister_for_callbacks)(void *self, void *callback);
    ad_hresult (AD_STDCALL *create_source_voice)(void *self, void **voice, const void *format, ad_u32 flags,
                                                 float max_ratio, void *callback, const struct xa_sends *sends,
                                                 const struct xa_chain *chain);
    ad_hresult (AD_STDCALL *create_submix_voice)(void *self, void **voice, ad_u32 channels, ad_u32 rate,
                                                 ad_u32 flags, ad_u32 stage, const struct xa_sends *sends,
                                                 const struct xa_chain *chain);
    ad_hresult (AD_STDCALL *create_mastering_voice)(void *self, void **voice, ad_u32 channels, ad_u32 rate,
                                                    ad_u32 flags, ad_u32 device, const struct xa_chain *chain);
    ad_hresult (AD_STDCALL *start_engine)(void *self);
    void (AD_STDCALL *stop_engine)(void *self);
    ad_hresult (AD_STDCALL *commit_changes)(void *self, ad_u32 operation_set);
    void (AD_STDCALL *get_performance_data)(void *self, void *data);
    void (AD_STDCALL *set_debug_configuration)(void *self, const void *configuration, void *reserved);
};

/* IXAudio2Voice (2.7), then IXAudio2SourceVoice's own; IXAudio2SubmixVoice
 * and IXAudio2MasteringVoice add nothing. */
struct xa_voice_vtbl {
    void (AD_STDCALL *get_voice_details)(void *self, struct xa_voice_details *details);
    ad_hresult (AD_STDCALL *set_output_voices)(void *self, const struct xa_sends *sends);
    ad_hresult (AD_STDCALL *set_effect_chain)(void *self, const struct xa_chain *chain);
    ad_hresult (AD_STDCALL *enable_effect)(void *self, ad_u32 index, ad_u32 operation_set);
    ad_hresult (AD_STDCALL *disable_effect)(void *self, ad_u32 index, ad_u32 operation_set);
    void (AD_STDCALL *get_effect_state)(void *self, ad_u32 index, int *enabled);
    ad_hresult (AD_STDCALL *set_effect_parameters)(void *self, ad_u32 index, const void *parameters, ad_u32 size,
                                                   ad_u32 operation_set);
    ad_hresult (AD_STDCALL *get_effect_parameters)(void *self, ad_u32 index, void *parameters, ad_u32 size);
    ad_hresult (AD_STDCALL *set_filter_parameters)(void *self, const struct xa_filter *filter,
                                                   ad_u32 operation_set);
    void (AD_STDCALL *get_filter_parameters)(void *self, struct xa_filter *filter);
    ad_hresult (AD_STDCALL *set_output_filter_parameters)(void *self, void *destination,
                                                          const struct xa_filter *filter, ad_u32 operation_set);
    void (AD_STDCALL *get_output_filter_parameters)(void *self, void *destination, struct xa_filter *filter);
    ad_hresult (AD_STDCALL *set_volume)(void *self, float volume, ad_u32 operation_set);
    void (AD_STDCALL *get_volume)(void *self, float *volume);
    ad_hresult (AD_STDCALL *set_channel_volumes)(void *self, ad_u32 channels, const float *volumes,
                                                 ad_u32 operation_set);
    void (AD_STDCALL *get_channel_volumes)(void *self, ad_u32 channels, float *volumes);
    ad_hresult (AD_STDCALL *set_output_matrix)(void *self, void *destination, ad_u32 source_channels,
                                               ad_u32 destination_channels, const float *matrix,
                                               ad_u32 operation_set);
    void (AD_STDCALL *get_output_matrix)(void *self, void *destination, ad_u32 source_channels,
                                         ad_u32 destination_channels, float *matrix);
    void (AD_STDCALL *destroy_voice)(void *self);
    /* IXAudio2SourceVoice */
    ad_hresult (AD_STDCALL *start)(void *self, ad_u32 flags, ad_u32 operation_set);
    ad_hresult (AD_STDCALL *stop)(void *self, ad_u32 flags, ad_u32 operation_set);
    ad_hresult (AD_STDCALL *submit_source_buffer)(void *self, const struct xa_buffer *buffer,
                                                  const struct xa_buffer_wma *wma);
    ad_hresult (AD_STDCALL *flush_source_buffers)(void *self);
    ad_hresult (AD_STDCALL *discontinuity)(void *self);
    ad_hresult (AD_STDCALL *exit_loop)(void *self, ad_u32 operation_set);
    void (AD_STDCALL *get_state)(void *self, struct xa_voice_state *state);
    ad_hresult (AD_STDCALL *set_frequency_ratio)(void *self, float ratio, ad_u32 operation_set);
    void (AD_STDCALL *get_frequency_ratio)(void *self, float *ratio);
    ad_hresult (AD_STDCALL *set_source_sample_rate)(void *self, ad_u32 rate);
};

/* IXAudio2VoiceCallback (no IUnknown). */
struct xa_voice_callback_vtbl {
    void (AD_STDCALL *pass_start)(void *self, ad_u32 bytes_required);
    void (AD_STDCALL *pass_end)(void *self);
    void (AD_STDCALL *stream_end)(void *self);
    void (AD_STDCALL *buffer_start)(void *self, void *context);
    void (AD_STDCALL *buffer_end)(void *self, void *context);
    void (AD_STDCALL *loop_end)(void *self, void *context);
    void (AD_STDCALL *voice_error)(void *self, void *context, ad_hresult error);
};

/* IXAudio2EngineCallback (no IUnknown). */
struct xa_engine_callback_vtbl {
    void (AD_STDCALL *pass_start)(void *self);
    void (AD_STDCALL *pass_end)(void *self);
    void (AD_STDCALL *critical_error)(void *self, ad_hresult error);
};

/* IMMNotificationClient. */
struct mm_client_vtbl {
    ad_hresult (AD_STDCALL *query_interface)(void *self, const struct ad_guid *iid, void **out);
    ad_u32 (AD_STDCALL *add_ref)(void *self);
    ad_u32 (AD_STDCALL *release)(void *self);
    ad_hresult (AD_STDCALL *state_changed)(void *self, const ad_u16 *id, ad_u32 state);
    ad_hresult (AD_STDCALL *added)(void *self, const ad_u16 *id);
    ad_hresult (AD_STDCALL *removed)(void *self, const ad_u16 *id);
    ad_hresult (AD_STDCALL *default_changed)(void *self, ad_u32 flow, ad_u32 role, const ad_u16 *id);
    ad_hresult (AD_STDCALL *property_changed)(void *self, const ad_u16 *id, struct ad_property_key key);
};

/* IMMDeviceEnumerator. */
struct mm_enumerator_vtbl {
    ad_hresult (AD_STDCALL *query_interface)(void *self, const struct ad_guid *iid, void **out);
    ad_u32 (AD_STDCALL *add_ref)(void *self);
    ad_u32 (AD_STDCALL *release)(void *self);
    ad_hresult (AD_STDCALL *enum_endpoints)(void *self, ad_u32 flow, ad_u32 mask, void **devices);
    ad_hresult (AD_STDCALL *get_default_endpoint)(void *self, ad_u32 flow, ad_u32 role, void **device);
    ad_hresult (AD_STDCALL *get_device)(void *self, const ad_u16 *id, void **device);
    ad_hresult (AD_STDCALL *register_client)(void *self, void *client);
    ad_hresult (AD_STDCALL *unregister_client)(void *self, void *client);
};

#endif
