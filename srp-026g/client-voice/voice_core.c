/* The portable core of skhid_voice.dll: see voice_core.h. */
#include "voice_core.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

#include <opus.h>

/* ---- The wire --------------------------------------------------------- */

static void put16(uint8_t *p, uint16_t v)
{
    p[0] = (uint8_t)v;
    p[1] = (uint8_t)(v >> 8);
}

static void put32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)v;
    p[1] = (uint8_t)(v >> 8);
    p[2] = (uint8_t)(v >> 16);
    p[3] = (uint8_t)(v >> 24);
}

static uint16_t get16(const uint8_t *p)
{
    return (uint16_t)(p[0] | (p[1] << 8));
}

static uint32_t get32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
           ((uint32_t)p[3] << 24);
}

size_t sv_write(const sv_msg *m, uint8_t *out, size_t cap)
{
    uint8_t buf[SV_MAX_DATAGRAM];
    size_t at = 3;
    unsigned k;
    buf[0] = SV_MAGIC;
    buf[1] = SV_VERSION;
    buf[2] = m->type;
    switch (m->type) {
    case SV_HELLO:
        put32(buf + at, m->session);
        buf[at + 4] = m->flags;
        put32(buf + at + 5, m->muted);
        at += 9;
        break;
    case SV_START:
        put32(buf + at, m->session);
        put16(buf + at + 4, m->utt);
        put32(buf + at + 6, m->ms);
        at += 10;
        break;
    case SV_END:
        put32(buf + at, m->session);
        put16(buf + at + 4, m->utt);
        put16(buf + at + 6, m->frames);
        at += 8;
        break;
    case SV_WELCOME:
        buf[at++] = m->slot;
        break;
    case SV_RELAY_START:
        buf[at] = m->slot;
        put16(buf + at + 1, m->utt);
        put32(buf + at + 3, m->ms);
        at += 7;
        break;
    case SV_RELAY_END:
        buf[at] = m->slot;
        put16(buf + at + 1, m->utt);
        buf[at + 3] = m->reason;
        at += 4;
        break;
    case SV_AUDIO:
    case SV_RELAY_AUDIO:
        if (m->type == SV_AUDIO) {
            put32(buf + at, m->session);
            at += 4;
        } else {
            buf[at++] = m->slot;
        }
        put16(buf + at, m->utt);
        put16(buf + at + 2, m->seq);
        if (m->n == 0 || m->n > SV_MAX_FRAMES)
            return 0;
        buf[at + 4] = m->n;
        at += 5;
        for (k = 0; k < m->n; k++) {
            if (m->len[k] == 0 || at + 1 + m->len[k] > sizeof buf)
                return 0;
            buf[at] = m->len[k];
            memcpy(buf + at + 1, m->frame[k], m->len[k]);
            at += 1 + m->len[k];
        }
        break;
    default:
        return 0;
    }
    if (at > cap)
        return 0;
    memcpy(out, buf, at);
    return at;
}

int sv_parse(const uint8_t *d, size_t len, sv_msg *m)
{
    size_t at = 3;
    unsigned k;
    memset(m, 0, sizeof *m);
    if (len < 3 || len > SV_MAX_DATAGRAM || d[0] != SV_MAGIC || d[1] != SV_VERSION)
        return 0;
    m->type = d[2];
    switch (m->type) {
    case SV_HELLO:
        /* The first form (no mask) is exactly 8 bytes. */
        if (len != at + 5 && len < at + 9)
            return 0;
        m->session = get32(d + at);
        m->flags = d[at + 4];
        if (len >= at + 9)
            m->muted = get32(d + at + 5);
        return 1;
    case SV_START:
        if (len < at + 10)
            return 0;
        m->session = get32(d + at);
        m->utt = get16(d + at + 4);
        m->ms = get32(d + at + 6);
        return 1;
    case SV_END:
        if (len < at + 8)
            return 0;
        m->session = get32(d + at);
        m->utt = get16(d + at + 4);
        m->frames = get16(d + at + 6);
        return 1;
    case SV_WELCOME:
        if (len < at + 1)
            return 0;
        m->slot = d[at];
        return 1;
    case SV_RELAY_START:
        if (len < at + 7)
            return 0;
        m->slot = d[at];
        m->utt = get16(d + at + 1);
        m->ms = get32(d + at + 3);
        return 1;
    case SV_RELAY_END:
        if (len < at + 4)
            return 0;
        m->slot = d[at];
        m->utt = get16(d + at + 1);
        m->reason = d[at + 3];
        return 1;
    case SV_AUDIO:
    case SV_RELAY_AUDIO:
        if (m->type == SV_AUDIO) {
            if (len < at + 4)
                return 0;
            m->session = get32(d + at);
            at += 4;
        } else {
            if (len < at + 1)
                return 0;
            m->slot = d[at++];
        }
        if (len < at + 5)
            return 0;
        m->utt = get16(d + at);
        m->seq = get16(d + at + 2);
        m->n = d[at + 4];
        at += 5;
        if (m->n == 0 || m->n > SV_MAX_FRAMES)
            return 0;
        for (k = 0; k < m->n; k++) {
            if (at >= len)
                return 0;
            m->len[k] = d[at];
            if (m->len[k] == 0 || at + 1 + m->len[k] > len)
                return 0;
            m->frame[k] = d + at + 1;
            at += 1 + m->len[k];
        }
        /* The frames are the datagram: nothing may follow them. */
        return at == len;
    default:
        return 0;
    }
}

/* ---- The send side ---------------------------------------------------- */

struct sv_encoder {
    OpusEncoder *opus;
};

sv_encoder *sv_encoder_new(void)
{
    int error = 0;
    sv_encoder *e = (sv_encoder *)calloc(1, sizeof *e);
    if (!e)
        return NULL;
    e->opus = opus_encoder_create(SV_RATE, 1, OPUS_APPLICATION_VOIP, &error);
    if (!e->opus || error != OPUS_OK) {
        free(e);
        return NULL;
    }
    opus_encoder_ctl(e->opus, OPUS_SET_BITRATE(SV_BITRATE));
    opus_encoder_ctl(e->opus, OPUS_SET_VBR(1));
    opus_encoder_ctl(e->opus, OPUS_SET_INBAND_FEC(0));
    opus_encoder_ctl(e->opus, OPUS_SET_DTX(0));
    opus_encoder_ctl(e->opus, OPUS_SET_SIGNAL(OPUS_SIGNAL_VOICE));
    opus_encoder_ctl(e->opus, OPUS_SET_COMPLEXITY(5));
    return e;
}

void sv_encoder_free(sv_encoder *e)
{
    if (!e)
        return;
    opus_encoder_destroy(e->opus);
    free(e);
}

size_t sv_encode(sv_encoder *e, const int16_t *pcm, uint8_t *out, size_t cap)
{
    opus_int32 got;
    if (cap > SV_MAX_FRAME_BYTES)
        cap = SV_MAX_FRAME_BYTES;
    got = opus_encode(e->opus, pcm, SV_FRAME, out, (opus_int32)cap);
    return got > 0 ? (size_t)got : 0;
}

void sv_sender_init(sv_sender *s, uint32_t session, uint16_t first_utt)
{
    memset(s, 0, sizeof *s);
    s->session = session;
    s->utt = (uint16_t)(first_utt - 1);
}

size_t sv_sender_start(sv_sender *s, uint32_t now_ms, uint8_t *out, size_t cap)
{
    sv_msg m;
    s->utt++;
    s->seq = 0;
    s->frames = 0;
    s->pending_len = 0;
    s->open = 1;
    memset(&m, 0, sizeof m);
    m.type = SV_START;
    m.session = s->session;
    m.utt = s->utt;
    m.ms = now_ms;
    return sv_write(&m, out, cap);
}

static size_t sender_audio(sv_sender *s, const uint8_t *second, size_t second_len,
                           uint8_t *out, size_t cap)
{
    sv_msg m;
    size_t written;
    memset(&m, 0, sizeof m);
    m.type = SV_AUDIO;
    m.session = s->session;
    m.utt = s->utt;
    m.seq = s->seq;
    m.n = 1;
    m.frame[0] = s->pending;
    m.len[0] = s->pending_len;
    if (second) {
        m.n = 2;
        m.frame[1] = second;
        m.len[1] = (uint8_t)second_len;
    }
    written = sv_write(&m, out, cap);
    s->seq++;
    s->frames = (uint16_t)(s->frames + m.n);
    s->pending_len = 0;
    return written;
}

size_t sv_sender_frame(sv_sender *s, const uint8_t *frame, size_t len, uint8_t *out,
                       size_t cap)
{
    if (!s->open || len == 0 || len > SV_MAX_FRAME_BYTES)
        return 0;
    if (s->pending_len == 0) {
        memcpy(s->pending, frame, len);
        s->pending_len = (uint8_t)len;
        return 0;
    }
    return sender_audio(s, frame, len, out, cap);
}

size_t sv_sender_flush(sv_sender *s, uint8_t *out, size_t cap)
{
    if (!s->open || s->pending_len == 0)
        return 0;
    return sender_audio(s, NULL, 0, out, cap);
}

size_t sv_sender_end(sv_sender *s, uint8_t *out, size_t cap)
{
    sv_msg m;
    if (!s->open)
        return 0;
    s->open = 0;
    s->pending_len = 0;
    memset(&m, 0, sizeof m);
    m.type = SV_END;
    m.session = s->session;
    m.utt = s->utt;
    m.frames = s->frames;
    return sv_write(&m, out, cap);
}

size_t sv_hello(uint32_t session, uint8_t flags, uint32_t muted, uint8_t *out, size_t cap)
{
    sv_msg m;
    memset(&m, 0, sizeof m);
    m.type = SV_HELLO;
    m.session = session;
    m.flags = flags;
    m.muted = muted;
    return sv_write(&m, out, cap);
}

/* ---- The radio filter and the mixer ------------------------------------ */

static void biquad_set(sv_biquad *q, int high, float f, float quality)
{
    const float pi = 3.14159265358979f;
    float w0 = 2.0f * pi * f / (float)SV_RATE;
    float c = cosf(w0), alpha = sinf(w0) / (2.0f * quality);
    float a0 = 1.0f + alpha;
    if (high) {
        q->b0 = (1.0f + c) / 2.0f / a0;
        q->b1 = -(1.0f + c) / a0;
    } else {
        q->b0 = (1.0f - c) / 2.0f / a0;
        q->b1 = (1.0f - c) / a0;
    }
    q->b2 = q->b0;
    q->a1 = -2.0f * c / a0;
    q->a2 = (1.0f - alpha) / a0;
    q->z1 = q->z2 = 0.0f;
}

static float biquad_run(sv_biquad *q, float x)
{
    /* Transposed direct form II. */
    float y = q->b0 * x + q->z1;
    q->z1 = q->b1 * x - q->a1 * y + q->z2;
    q->z2 = q->b2 * x - q->a2 * y;
    return y;
}

void sv_radio_init(sv_radio_state *r, uint32_t seed)
{
    /* Fourth-order Butterworth each way: two sections, Q 0.5412 and 1.3066. */
    biquad_set(&r->stage[0], 1, 300.0f, 0.5412f);
    biquad_set(&r->stage[1], 1, 300.0f, 1.3066f);
    biquad_set(&r->stage[2], 0, 3400.0f, 0.5412f);
    biquad_set(&r->stage[3], 0, 3400.0f, 1.3066f);
    r->noise = seed ? seed : 0x2545F491u;
}

/* tanh-like, without libm's tanh: exact enough below 3, 1 beyond. */
static float soft_sat(float v)
{
    float v2;
    if (v >= 3.0f)
        return 1.0f;
    if (v <= -3.0f)
        return -1.0f;
    v2 = v * v;
    return v * (27.0f + v2) / (27.0f + 9.0f * v2);
}

#define RADIO_DRIVE 2.0f
/* The band-pass takes the lows away: about as loud as the plain voice. */
#define RADIO_MAKEUP 1.25f

void sv_radio_process(sv_radio_state *r, float *x, int n, float noise)
{
    /* Uniform noise of rms `noise`: amplitude noise x sqrt(3). */
    float amplitude = noise * 1.7320508f;
    int i, k;
    for (i = 0; i < n; i++) {
        float v = x[i];
        for (k = 0; k < 4; k++)
            v = biquad_run(&r->stage[k], v);
        v = soft_sat(RADIO_DRIVE * v) / RADIO_DRIVE * RADIO_MAKEUP;
        if (amplitude > 0.0f) {
            r->noise = r->noise * 1664525u + 1013904223u;
            v += amplitude * ((float)(r->noise >> 8) / 8388608.0f - 1.0f);
        }
        x[i] = v;
    }
}

void sv_mix_clamp(const float *acc, int16_t *out, int n)
{
    int i;
    for (i = 0; i < n; i++) {
        float v = acc[i] * 32767.0f;
        if (v > 32767.0f)
            v = 32767.0f;
        else if (v < -32768.0f)
            v = -32768.0f;
        out[i] = (int16_t)lrintf(v);
    }
}

float sv_mic_gain(float sensitivity)
{
    if (!(sensitivity >= 0.0f))
        sensitivity = 0.0f; /* NaN too */
    if (sensitivity > 1.0f)
        sensitivity = 1.0f;
    return powf(2.0f, (sensitivity - 0.5f) * 4.0f);
}

/* ---- The receive side ------------------------------------------------- */

/* Noise under a far voice: about -40 dBFS. */
#define RADIO_NOISE 0.01f

typedef struct {
    int present;
    uint32_t index;
    uint8_t len;
    uint8_t data[SV_MAX_FRAME_BYTES];
} sv_cell;

typedef struct {
    int open;           /* an utterance being received or played */
    int playing;        /* its playout has started */
    int ended;          /* its END came */
    uint16_t utt;
    int have_closed;
    uint16_t closed_utt;
    uint32_t next;      /* the next frame index to play */
    uint32_t top;       /* the highest index received + 1 */
    int heard;          /* last_audio_ms is set */
    uint32_t last_audio_ms;
    sv_cell ring[SV_RING];
    OpusDecoder *dec;
    sv_radio_state radio;
} sv_speaker;

struct sv_core {
    sv_speaker speaker[SV_SLOTS];
    sv_stats stats;
    uint32_t muted;
    int last_speaker;
};

static int newer(uint16_t a, uint16_t b)
{
    return (int16_t)(a - b) > 0;
}

static int elapsed(uint32_t now, uint32_t then, uint32_t ms)
{
    return (int32_t)(now - then) >= (int32_t)ms;
}

static void speaker_open(sv_speaker *s, uint16_t utt, int slot)
{
    int k;
    s->open = 1;
    s->playing = 0;
    s->ended = 0;
    s->utt = utt;
    s->next = 0;
    s->top = 0;
    for (k = 0; k < SV_RING; k++)
        s->ring[k].present = 0;
    if (s->dec)
        opus_decoder_ctl(s->dec, OPUS_RESET_STATE);
    sv_radio_init(&s->radio, 0x9E3779B9u * (uint32_t)(slot + 1) + utt);
}

static void speaker_close(sv_core *c, sv_speaker *s)
{
    if (!s->open)
        return;
    s->open = 0;
    s->playing = 0;
    s->have_closed = 1;
    s->closed_utt = s->utt;
    c->stats.closed++;
}

/* The speaker's utterance `utt`, opened if new: 0 when the message is of an
 * older utterance (late). */
static int speaker_take(sv_core *c, sv_speaker *s, uint16_t utt, int slot)
{
    if (s->open) {
        if (utt == s->utt)
            return 1;
        if (!newer(utt, s->utt))
            return 0;
        speaker_close(c, s);
    } else if (s->have_closed && !newer(utt, s->closed_utt)) {
        return 0;
    }
    speaker_open(s, utt, slot);
    return 1;
}

static void speaker_frame(sv_core *c, sv_speaker *s, uint32_t index, const uint8_t *data,
                          uint8_t len)
{
    sv_cell *cell;
    if (s->top == 0 && !s->playing) {
        /* The utterance's first frame. */
        s->next = index;
        s->top = index;
    } else if (!s->playing && index < s->next && s->top - index < SV_RING) {
        /* Not playing yet: an earlier frame that came late still plays. */
        s->next = index;
    }
    if (index < s->next) {
        c->stats.late++;
        return;
    }
    if (index >= s->next + SV_RING) {
        /* Too far ahead: what is older than the ring holds is given up. */
        uint32_t keep = index - SV_RING + 1;
        uint32_t k;
        for (k = s->next; k < keep && k < s->top; k++) {
            sv_cell *old = &s->ring[k % SV_RING];
            if (old->present && old->index == k)
                old->present = 0;
        }
        s->next = keep;
    }
    cell = &s->ring[index % SV_RING];
    cell->present = 1;
    cell->index = index;
    cell->len = len;
    memcpy(cell->data, data, len);
    if (index + 1 > s->top)
        s->top = index + 1;
}

sv_core *sv_core_new(void)
{
    int k, error;
    sv_core *c = (sv_core *)calloc(1, sizeof *c);
    if (!c)
        return NULL;
    c->last_speaker = -1;
    for (k = 0; k < SV_SLOTS; k++) {
        error = 0;
        c->speaker[k].dec = opus_decoder_create(SV_RATE, 1, &error);
        if (!c->speaker[k].dec || error != OPUS_OK) {
            sv_core_free(c);
            return NULL;
        }
    }
    return c;
}

void sv_core_free(sv_core *c)
{
    int k;
    if (!c)
        return;
    for (k = 0; k < SV_SLOTS; k++)
        if (c->speaker[k].dec)
            opus_decoder_destroy(c->speaker[k].dec);
    free(c);
}

void sv_core_reset(sv_core *c)
{
    int k;
    for (k = 0; k < SV_SLOTS; k++) {
        OpusDecoder *dec = c->speaker[k].dec;
        memset(&c->speaker[k], 0, sizeof c->speaker[k]);
        c->speaker[k].dec = dec;
        if (dec)
            opus_decoder_ctl(dec, OPUS_RESET_STATE);
    }
    c->last_speaker = -1;
}

void sv_core_mute(sv_core *c, uint32_t mask)
{
    int k;
    c->muted = mask;
    for (k = 0; k < SV_SLOTS; k++) {
        sv_speaker *s = &c->speaker[k];
        if (!(mask & (1u << k)))
            continue;
        speaker_close(c, s);
        s->heard = 0;
    }
}

int sv_core_last_speaker(const sv_core *c)
{
    return c->last_speaker;
}

void sv_core_receive(sv_core *c, const sv_msg *m, uint32_t now_ms)
{
    sv_speaker *s;
    unsigned k;
    if (m->slot >= SV_SLOTS || (c->muted & (1u << m->slot)))
        return;
    s = &c->speaker[m->slot];
    switch (m->type) {
    case SV_RELAY_START:
        if (speaker_take(c, s, m->utt, m->slot)) {
            s->heard = 1;
            s->last_audio_ms = now_ms;
            c->last_speaker = m->slot;
        }
        break;
    case SV_RELAY_AUDIO:
        if (!speaker_take(c, s, m->utt, m->slot)) {
            c->stats.late += m->n;
            break;
        }
        s->heard = 1;
        s->last_audio_ms = now_ms;
        c->last_speaker = m->slot;
        /* Two frames a datagram: frame k of datagram seq is seq x 2 + k. */
        for (k = 0; k < m->n && k < SV_PER_DATAGRAM; k++)
            speaker_frame(c, s, (uint32_t)m->seq * SV_PER_DATAGRAM + k, m->frame[k],
                          m->len[k]);
        break;
    case SV_RELAY_END:
        if (s->open && m->utt == s->utt) {
            s->ended = 1;
            if (s->next >= s->top)
                speaker_close(c, s);
        }
        break;
    default:
        break;
    }
}

/* One frame of the speaker into pcm: 1, or 0 for nothing this tick. */
static int speaker_play(sv_core *c, sv_speaker *s, float *pcm, uint32_t now_ms)
{
    sv_cell *cell;
    int got;
    if (!s->open)
        return 0;
    if (!s->playing) {
        if (s->top - s->next >= SV_TARGET_FRAMES || (s->ended && s->top > s->next)) {
            s->playing = 1;
        } else {
            if (s->ended || elapsed(now_ms, s->last_audio_ms, SV_SILENT_CLOSE_MS)) {
                if (!s->ended)
                    c->stats.timeouts++;
                speaker_close(c, s);
            }
            return 0;
        }
    }
    if (s->next >= s->top) {
        /* Nothing to play: the end, a silence, or the network late. */
        if (s->ended || elapsed(now_ms, s->last_audio_ms, SV_SILENT_CLOSE_MS)) {
            if (!s->ended)
                c->stats.timeouts++;
            speaker_close(c, s);
        } else {
            c->stats.underruns++;
            s->playing = 0; /* buffer again */
        }
        return 0;
    }
    cell = &s->ring[s->next % SV_RING];
    if (cell->present && cell->index == s->next) {
        got = opus_decode_float(s->dec, cell->data, cell->len, pcm, SV_FRAME, 0);
        cell->present = 0;
        if (got == SV_FRAME)
            c->stats.decoded++;
    } else {
        /* Lost or still on its way while later ones are here: concealed. */
        got = opus_decode_float(s->dec, NULL, 0, pcm, SV_FRAME, 0);
        if (got == SV_FRAME)
            c->stats.concealed++;
    }
    s->next++;
    if (got != SV_FRAME)
        return 0;
    if (s->ended && s->next >= s->top)
        speaker_close(c, s);
    return 1;
}

void sv_core_mix(sv_core *c, int16_t *out, float volume, uint32_t far_mask,
                 uint32_t now_ms)
{
    float acc[SV_FRAME];
    float pcm[SV_FRAME];
    int k, i;
    if (!(volume >= 0.0f))
        volume = 0.0f;
    if (volume > 1.0f)
        volume = 1.0f;
    memset(acc, 0, sizeof acc);
    for (k = 0; k < SV_SLOTS; k++) {
        sv_speaker *s = &c->speaker[k];
        if (!speaker_play(c, s, pcm, now_ms))
            continue;
        if (far_mask & (1u << k))
            sv_radio_process(&s->radio, pcm, SV_FRAME, RADIO_NOISE);
        for (i = 0; i < SV_FRAME; i++)
            acc[i] += pcm[i] * volume;
    }
    sv_mix_clamp(acc, out, SV_FRAME);
}

uint32_t sv_core_talking(const sv_core *c, uint32_t now_ms)
{
    uint32_t mask = 0;
    int k;
    for (k = 0; k < SV_SLOTS; k++) {
        const sv_speaker *s = &c->speaker[k];
        if (s->open || (s->heard && !elapsed(now_ms, s->last_audio_ms, SV_HEARD_MS)))
            mask |= 1u << k;
    }
    return mask;
}

sv_stats sv_core_stats(const sv_core *c)
{
    return c->stats;
}
