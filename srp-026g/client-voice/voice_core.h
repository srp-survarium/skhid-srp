/* The portable core of skhid_voice.dll (voice_win.c is the Windows part):
 * the voice port's wire, the Opus encoder and decoders, the per-speaker
 * jitter buffers, the radio filter and the mixer. No Windows headers, no
 * threads, no I/O: test_voice.c runs it on Linux.
 *
 * The design: srp-026g/docs/notes/voice-chat.md. 16 kHz mono, 20 ms Opus
 * frames (320 samples), 16 kbit/s, two frames a datagram. */
#ifndef SKHID_VOICE_CORE_H
#define SKHID_VOICE_CORE_H

#include <stddef.h>
#include <stdint.h>

#define SV_RATE 16000
#define SV_FRAME 320            /* samples in 20 ms */
#define SV_FRAME_MS 20
#define SV_BITRATE 16000
#define SV_SLOTS 20             /* a match's places: 0..19 */
#define SV_MAX_DATAGRAM 400     /* srp drops anything longer */
#define SV_MAX_FRAMES 8         /* frames a datagram may hold (we send 2) */
#define SV_MAX_FRAME_BYTES 255  /* a frame's length is a u8 */
#define SV_PER_DATAGRAM 2       /* frames we send in one AUDIO */

#define SV_MAGIC 0x56
#define SV_VERSION 1

/* Message types. 0x0X client to server, 0x8X server to client. */
enum {
    SV_HELLO = 0x01,
    SV_START = 0x02,
    SV_AUDIO = 0x03,
    SV_END = 0x04,
    SV_WELCOME = 0x81,
    SV_RELAY_START = 0x82,
    SV_RELAY_AUDIO = 0x83,
    SV_RELAY_END = 0x84,
    /* S->C, with each WELCOME: the listener's team, each place's voice id
     * (`u8 count, count x (u8 slot, u32 id)`): a stable, opaque id per
     * account (srp's keyed hash of it), what a mute is kept by. */
    SV_ROSTER = 0x90,
};

/* END reasons (S->C). */
enum { SV_END_RELEASED = 0, SV_END_TIMEOUT = 1, SV_END_TOO_LONG = 2 };

/* HELLO flags. HELLO's body is `u32 session, u8 flags, u32 muted`; the
 * parser also takes the first form without `muted` (then 0). */
#define SV_HELLO_HEARS 0x01

/* One message, any type. Only the fields of its type are meaningful. For
 * AUDIO the frames point into the parsed datagram (or the caller's buffers
 * when writing). */
typedef struct {
    uint8_t type;
    uint32_t session;   /* C->S */
    uint8_t slot;       /* S->C: WELCOME's own slot, the relays' speaker */
    uint8_t flags;      /* HELLO */
    uint32_t muted;     /* HELLO: bit n, the listener mutes slot n */
    uint16_t utt;       /* START, AUDIO, END */
    uint32_t ms;        /* START: client_ms or server_ms */
    uint16_t seq;       /* AUDIO */
    uint16_t frames;    /* C->S END: frames sent */
    uint8_t reason;     /* S->C END */
    uint8_t n;          /* AUDIO: frames in it */
    uint8_t count;      /* ROSTER: places in it */
    uint8_t roster_slot[SV_SLOTS];
    uint32_t roster_id[SV_SLOTS];
    const uint8_t *frame[SV_MAX_FRAMES];
    uint8_t len[SV_MAX_FRAMES];
} sv_msg;

/* Writes `m` to `out` (at most `cap` bytes): the datagram's length, or 0
 * when the type is unknown, a frame is empty, or it does not fit. */
size_t sv_write(const sv_msg *m, uint8_t *out, size_t cap);

/* Parses a datagram: 1 and `m` filled, or 0 for anything that is not a
 * well-formed message of a known type (a wrong magic or version, too short,
 * frames that run past the end, an empty frame, more than SV_MAX_FRAMES).
 * Bytes after a fixed-size message are ignored (later fields). */
int sv_parse(const uint8_t *data, size_t len, sv_msg *m);

/* ---- The send side --------------------------------------------------- */

typedef struct sv_encoder sv_encoder;
sv_encoder *sv_encoder_new(void);
void sv_encoder_free(sv_encoder *e);
/* Encodes one frame of SV_FRAME samples: its length (1..cap), or 0. */
size_t sv_encode(sv_encoder *e, const int16_t *pcm, uint8_t *out, size_t cap);

/* The talker's utterance: datagrams made from encoded frames. */
typedef struct {
    uint32_t session;
    uint16_t utt;       /* the current (or last) utterance */
    uint16_t seq;       /* the next AUDIO's */
    uint16_t frames;    /* frames sent in this utterance */
    int open;
    uint8_t pending_len;
    uint8_t pending[SV_MAX_FRAME_BYTES];
} sv_sender;

void sv_sender_init(sv_sender *s, uint32_t session, uint16_t first_utt);
/* A new utterance (the previous one is taken as ended): its START. */
size_t sv_sender_start(sv_sender *s, uint32_t now_ms, uint8_t *out, size_t cap);
/* One encoded frame: an AUDIO datagram once SV_PER_DATAGRAM are held,
 * else 0. Nothing when no utterance is open. */
size_t sv_sender_frame(sv_sender *s, const uint8_t *frame, size_t len, uint8_t *out,
                       size_t cap);
/* A frame still held (an AUDIO of one frame), else 0. */
size_t sv_sender_flush(sv_sender *s, uint8_t *out, size_t cap);
/* The END of the open utterance (closes it; the caller resends the same
 * bytes). 0 when none was open. */
size_t sv_sender_end(sv_sender *s, uint8_t *out, size_t cap);
size_t sv_hello(uint32_t session, uint8_t flags, uint32_t muted, uint8_t *out, size_t cap);

/* ---- The receive side ------------------------------------------------ */

/* Frames a speaker's jitter buffer holds: 320 ms. */
#define SV_RING 16
/* Frames buffered before an utterance starts playing: 60 ms. */
#define SV_TARGET_FRAMES 3
/* An utterance with no audio for this long is closed. */
#define SV_SILENT_CLOSE_MS 1000
/* A speaker counts as heard for this long after its last audio. */
#define SV_HEARD_MS 250

typedef struct {
    uint32_t decoded;   /* frames decoded */
    uint32_t concealed; /* frames made by packet-loss concealment */
    uint32_t late;      /* frames dropped: already played, or of an old utterance */
    uint32_t underruns; /* ticks a playing speaker had nothing */
    uint32_t closed;    /* utterances closed */
    uint32_t timeouts;  /* of them, by silence */
} sv_stats;

typedef struct sv_core sv_core;
sv_core *sv_core_new(void);
void sv_core_free(sv_core *c);
/* A relayed message (WELCOME is the caller's; others are ignored). */
void sv_core_receive(sv_core *c, const sv_msg *m, uint32_t now_ms);
/* The next 20 ms of every speaker mixed: SV_FRAME samples into `out`.
 * `volume` 0..1; a speaker whose bit is in `far_mask` goes through the
 * radio filter. */
void sv_core_mix(sv_core *c, int16_t *out, float volume, uint32_t far_mask,
                 uint32_t now_ms);
/* Bit n: slot n has an open utterance or was heard within SV_HEARD_MS. */
uint32_t sv_core_talking(const sv_core *c, uint32_t now_ms);
sv_stats sv_core_stats(const sv_core *c);
/* The slots muted (bit n): never played, their messages ignored, never
 * talking; a speaker muted while it talks is cut at once. */
void sv_core_mute(sv_core *c, uint32_t mask);
/* The slot last heard (a START or AUDIO of a slot not muted), -1 for none
 * since the core was made or reset. */
int sv_core_last_speaker(const sv_core *c);
/* Forget every speaker (a new match). */
void sv_core_reset(sv_core *c);

/* ---- Pieces, public for the tests ------------------------------------ */

typedef struct {
    float b0, b1, b2, a1, a2, z1, z2;
} sv_biquad;

typedef struct {
    sv_biquad stage[4]; /* high-pass 300 Hz twice, low-pass 3400 Hz twice */
    uint32_t noise;     /* the noise's generator */
} sv_radio_state;

void sv_radio_init(sv_radio_state *r, uint32_t seed);
/* In place, samples in -1..1: band-pass, light clipping, and noise of
 * `noise` amplitude (rms) added. */
void sv_radio_process(sv_radio_state *r, float *x, int n, float noise);
/* Sums to int16 with saturation. */
void sv_mix_clamp(const float *acc, int16_t *out, int n);
/* The capture gain of the options' sensitivity 0..1: 0.25..4. */
float sv_mic_gain(float sensitivity);

#endif
