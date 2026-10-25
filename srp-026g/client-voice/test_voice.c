/* The loopback test of skhid_voice.dll's core (voice_core.c), run on Linux
 * with the system's libopus: the wire, the send side's datagrams, loss and
 * reordering through the jitter buffer, Opus and its concealment, the
 * radio filter, the mixer, mutes. Exit status 0 when everything passed.
 *
 *   cc -O2 -o test_voice test_voice.c voice_core.c $(pkg-config --cflags --libs opus) -lm
 *   ./test_voice
 *
 * (The flake's checks.x86_64-linux.voice-loopback builds and runs it.) */
#include "voice_core.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures;

#define CHECK(cond, ...)                                                                   \
    do {                                                                                   \
        if (!(cond)) {                                                                     \
            failures++;                                                                    \
            printf("FAIL %s:%d: ", __FILE__, __LINE__);                                    \
            printf(__VA_ARGS__);                                                           \
            printf("\n");                                                                  \
        }                                                                                  \
    } while (0)

static const double PI = 3.14159265358979;

/* ---- The wire ---------------------------------------------------------- */

static void roundtrip(const sv_msg *m, size_t expect_len)
{
    uint8_t buf[SV_MAX_DATAGRAM];
    sv_msg back;
    size_t len = sv_write(m, buf, sizeof buf);
    CHECK(len == expect_len, "type %02x: written %zu bytes, expected %zu", m->type, len,
          expect_len);
    CHECK(sv_parse(buf, len, &back), "type %02x: does not parse back", m->type);
    CHECK(back.type == m->type && back.session == m->session && back.slot == m->slot &&
              back.flags == m->flags && back.muted == m->muted && back.utt == m->utt &&
              back.ms == m->ms && back.seq == m->seq && back.frames == m->frames &&
              back.reason == m->reason && back.n == m->n,
          "type %02x: fields differ after the round trip", m->type);
    for (unsigned k = 0; k < m->n && k < back.n; k++)
        CHECK(back.len[k] == m->len[k] && !memcmp(back.frame[k], m->frame[k], m->len[k]),
              "type %02x: frame %u differs", m->type, k);
    /* Every shorter prefix is refused (AUDIO), or parses only with all its
     * fixed fields (the fixed types take no prefix either). */
    for (size_t cut = 0; cut < len; cut++)
        CHECK(!sv_parse(buf, cut, &back) ||
                  (m->type == SV_HELLO && cut == 8 /* the first form */),
              "type %02x: a %zu-byte prefix parses", m->type, cut);
}

static void test_wire(void)
{
    uint8_t a[40], b[7];
    sv_msg m;
    memset(a, 0xA5, sizeof a);
    memset(b, 0x3C, sizeof b);

    memset(&m, 0, sizeof m);
    m.type = SV_HELLO, m.session = 0x89ABCDEF, m.flags = 1, m.muted = 0x80001;
    roundtrip(&m, 12);
    memset(&m, 0, sizeof m);
    m.type = SV_START, m.session = 7, m.utt = 0xFFFE, m.ms = 0x12345678;
    roundtrip(&m, 13);
    memset(&m, 0, sizeof m);
    m.type = SV_AUDIO, m.session = 9, m.utt = 3, m.seq = 400, m.n = 2;
    m.frame[0] = a, m.len[0] = sizeof a, m.frame[1] = b, m.len[1] = sizeof b;
    roundtrip(&m, 3 + 4 + 2 + 2 + 1 + 1 + 40 + 1 + 7);
    memset(&m, 0, sizeof m);
    m.type = SV_END, m.session = 9, m.utt = 3, m.frames = 801;
    roundtrip(&m, 11);
    memset(&m, 0, sizeof m);
    m.type = SV_WELCOME, m.slot = 19;
    roundtrip(&m, 4);
    memset(&m, 0, sizeof m);
    m.type = SV_RELAY_START, m.slot = 4, m.utt = 77, m.ms = 1000;
    roundtrip(&m, 10);
    memset(&m, 0, sizeof m);
    m.type = SV_RELAY_AUDIO, m.slot = 4, m.utt = 77, m.seq = 1, m.n = 1;
    m.frame[0] = a, m.len[0] = 10;
    roundtrip(&m, 3 + 1 + 2 + 2 + 1 + 1 + 10);
    memset(&m, 0, sizeof m);
    m.type = SV_RELAY_END, m.slot = 4, m.utt = 77, m.reason = SV_END_TOO_LONG;
    roundtrip(&m, 7);

    /* ROSTER: each slot's voice id; a count past SV_SLOTS or entries past
     * the end are refused. */
    {
        uint8_t buf[SV_MAX_DATAGRAM];
        sv_msg back;
        memset(&m, 0, sizeof m);
        m.type = SV_ROSTER, m.count = 3;
        m.roster_slot[0] = 0, m.roster_id[0] = 0xDEADBEEF;
        m.roster_slot[1] = 2, m.roster_id[1] = 1;
        m.roster_slot[2] = 19, m.roster_id[2] = 0x80000000;
        size_t len = sv_write(&m, buf, sizeof buf);
        CHECK(len == 4 + 15 && sv_parse(buf, len, &back) && back.count == 3 &&
                  back.roster_slot[2] == 19 && back.roster_id[0] == 0xDEADBEEF &&
                  back.roster_id[2] == 0x80000000,
              "ROSTER round trip");
        CHECK(!sv_parse(buf, len - 1, &back), "a ROSTER cut short parses");
        buf[3] = SV_SLOTS + 1;
        CHECK(!sv_parse(buf, len, &back), "a ROSTER of 21 parses");
        memset(&m, 0, sizeof m);
        m.type = SV_ROSTER;
        CHECK(sv_write(&m, buf, sizeof buf) == 4 && sv_parse(buf, 4, &back) && back.count == 0,
              "an empty ROSTER");
    }

    /* HELLO's first form (no mask) parses, the mask 0. */
    {
        uint8_t hello[8] = {0x56, 1, 1, 1, 2, 3, 4, 1};
        CHECK(sv_parse(hello, sizeof hello, &m) && m.type == SV_HELLO &&
                  m.session == 0x04030201 && m.flags == 1 && m.muted == 0,
              "HELLO without a mask");
    }
    /* sv_hello makes the 12-byte form. */
    {
        uint8_t out[32];
        CHECK(sv_hello(5, 0, 0x3, out, sizeof out) == 12 && sv_parse(out, 12, &m) &&
                  m.muted == 3 && m.flags == 0,
              "sv_hello");
    }

    /* Malformed. */
    {
        uint8_t bad[64];
        uint8_t ok[] = {0x56, 1, 0x81, 2};
        CHECK(sv_parse(ok, sizeof ok, &m), "WELCOME");
        memcpy(bad, ok, sizeof ok);
        bad[0] = 0x57;
        CHECK(!sv_parse(bad, 4, &m), "a wrong magic parses");
        bad[0] = 0x56, bad[1] = 2;
        CHECK(!sv_parse(bad, 4, &m), "a wrong version parses");
        bad[1] = 1, bad[2] = 0x85;
        CHECK(!sv_parse(bad, 4, &m), "a reserved type (85, TEXT) parses");
        bad[2] = 0x7F;
        CHECK(!sv_parse(bad, 4, &m), "an unknown type parses");
        /* AUDIO with n 0, n too big, an empty frame, a frame past the end,
         * bytes after the frames. */
        uint8_t audio[] = {0x56, 1, 0x83, 0, 1, 0, 2, 0, 1, 3, 9, 9, 9};
        CHECK(sv_parse(audio, sizeof audio, &m) && m.n == 1 && m.len[0] == 3, "AUDIO");
        memcpy(bad, audio, sizeof audio);
        bad[8] = 0;
        CHECK(!sv_parse(bad, sizeof audio, &m), "AUDIO with no frames parses");
        bad[8] = SV_MAX_FRAMES + 1;
        CHECK(!sv_parse(bad, sizeof audio, &m), "AUDIO with too many frames parses");
        bad[8] = 1, bad[9] = 0;
        CHECK(!sv_parse(bad, sizeof audio, &m), "an empty frame parses");
        bad[9] = 4;
        CHECK(!sv_parse(bad, sizeof audio, &m), "a frame past the end parses");
        bad[9] = 3, bad[13] = 0;
        CHECK(!sv_parse(bad, sizeof audio + 1, &m), "bytes after the frames parse");
        bad[8] = 2;
        CHECK(!sv_parse(bad, sizeof audio, &m), "a missing second frame parses");
        CHECK(!sv_parse(audio, 0, &m) && !sv_parse(audio, 2, &m), "a stub parses");
        {
            uint8_t big[SV_MAX_DATAGRAM + 1];
            memset(big, 0, sizeof big);
            big[0] = 0x56, big[1] = 1, big[2] = 0x81;
            CHECK(!sv_parse(big, sizeof big, &m), "an oversized datagram parses");
        }
        /* Random and damaged datagrams: parsed only when well-formed, and
         * never a crash. */
        uint32_t r = 1;
        int parsed = 0;
        sv_core *fuzz = sv_core_new();
        int16_t fuzz_out[SV_FRAME];
        for (int i = 0; i < 200000; i++) {
            size_t len = (size_t)(r % 64);
            for (size_t k = 0; k < len; k++) {
                r = r * 1103515245u + 12345u;
                bad[k] = (uint8_t)(r >> 16);
            }
            if (i % 2 == 0) {
                /* Half of them: a relayed AUDIO of two short frames, two
                 * of its bytes changed and its end cut anywhere. */
                static const uint8_t audio2[] = {0x56, 1,    0x83, 4, 1, 0, 2, 0, 2,
                                                 3,    0xAA, 0xBB, 0xCC, 2, 0xDD, 0xEE};
                memcpy(bad, audio2, sizeof audio2);
                bad[(r >> 4) % sizeof audio2] ^= (uint8_t)(r >> 12);
                bad[(r >> 20) % sizeof audio2] ^= (uint8_t)(1u << ((r >> 9) % 8));
                len = 3 + (r >> 24) % (sizeof audio2 - 2);
            } else if (len >= 3) {
                bad[0] = 0x56, bad[1] = 1, bad[2] = (uint8_t)(0x81 + (r >> 8) % 4);
            }
            if (sv_parse(bad, len, &m)) {
                parsed++;
                /* And into a listener: garbage Opus, any order, never a crash. */
                sv_core_receive(fuzz, &m, (uint32_t)i);
                if (i % 50 == 0)
                    sv_core_mix(fuzz, fuzz_out, 1.0f, (uint32_t)i, (uint32_t)i);
                for (unsigned k = 0; k < m.n; k++)
                    CHECK(m.len[k] > 0 && m.frame[k] > bad &&
                              m.frame[k] + m.len[k] <= bad + len,
                          "a parsed frame outside its datagram");
            }
            r = r * 1103515245u + 12345u;
        }
        sv_core_free(fuzz);
        printf("random datagrams: %d of 200000 parse (all well-formed), played\n", parsed);
    }
    /* sv_write refuses what does not fit. */
    {
        uint8_t out[8];
        memset(&m, 0, sizeof m);
        m.type = SV_START;
        CHECK(sv_write(&m, out, sizeof out) == 0, "START written into 8 bytes");
        m.type = 0x7F;
        CHECK(sv_write(&m, out, sizeof out) == 0, "an unknown type written");
    }
}

/* ---- Speech through the codec, the network and the jitter buffer ------- */

#define SECONDS 2
#define SAMPLES (SV_RATE * SECONDS)
#define FRAMES (SAMPLES / SV_FRAME)

/* Something speech-like: a voiced tone of harmonics on a moving pitch, and
 * syllables (4 Hz) as its envelope. */
static void speech(int16_t *pcm, int n)
{
    double phase = 0.0;
    for (int i = 0; i < n; i++) {
        double t = (double)i / SV_RATE;
        double f0 = 140.0 + 30.0 * sin(2 * PI * 1.5 * t);
        double env = 0.5 - 0.5 * cos(2 * PI * 4.0 * t);
        double v = 0.0;
        phase += 2 * PI * f0 / SV_RATE;
        for (int h = 1; h <= 20 && h * f0 < 3800.0; h++)
            v += sin(h * phase) / h;
        pcm[i] = (int16_t)(v * env * 7000.0);
    }
}

typedef struct {
    uint8_t data[SV_MAX_DATAGRAM];
    size_t len;
} datagram;

/* The talker: START, the AUDIO datagrams, END. Returns their count. */
static int talk(const int16_t *pcm, datagram *out, int cap)
{
    sv_encoder *e = sv_encoder_new();
    sv_sender s;
    int count = 0;
    uint8_t frame[SV_MAX_FRAME_BYTES];
    size_t bytes = 0;
    CHECK(e != NULL, "no encoder");
    if (!e)
        return 0;
    sv_sender_init(&s, 0x1234, 500);
    out[count].len = sv_sender_start(&s, 0, out[count].data, SV_MAX_DATAGRAM);
    count++;
    for (int f = 0; f < FRAMES; f++) {
        size_t len = sv_encode(e, pcm + f * SV_FRAME, frame, sizeof frame);
        CHECK(len > 0 && len <= SV_MAX_FRAME_BYTES, "frame %d: %zu bytes", f, len);
        bytes += len;
        size_t d = sv_sender_frame(&s, frame, len, out[count].data, SV_MAX_DATAGRAM);
        if (d) {
            CHECK(d <= SV_MAX_DATAGRAM, "a datagram of %zu bytes", d);
            out[count++].len = d;
        }
        if (count >= cap - 2)
            break;
    }
    out[count].len = sv_sender_flush(&s, out[count].data, SV_MAX_DATAGRAM);
    if (out[count].len)
        count++;
    out[count].len = sv_sender_end(&s, out[count].data, SV_MAX_DATAGRAM);
    count++;
    CHECK(s.frames == FRAMES, "the sender counted %u frames", s.frames);
    printf("talker: %d frames, %zu bytes of Opus (%.1f kbit/s), %d datagrams\n", FRAMES,
           bytes, bytes * 8.0 / SECONDS / 1000.0, count);
    sv_encoder_free(e);
    return count;
}

/* The server's relay of a client's datagram (C->S into S->C, slot 3). */
static int relay(const datagram *in, sv_msg *out, int slot)
{
    sv_msg m;
    if (!sv_parse(in->data, in->len, &m))
        return 0;
    *out = m;
    out->slot = (uint8_t)slot;
    switch (m.type) {
    case SV_START:
        out->type = SV_RELAY_START;
        return 1;
    case SV_AUDIO:
        out->type = SV_RELAY_AUDIO;
        return 1;
    case SV_END:
        out->type = SV_RELAY_END;
        out->reason = SV_END_RELEASED;
        return 1;
    default:
        return 0;
    }
}

/* The best normalized correlation of b against a over lags 0..max. */
static double best_correlation(const float *a, int na, const float *b, int nb, int max,
                               int *lag_out)
{
    double best = -1.0;
    for (int lag = 0; lag <= max; lag++) {
        double ab = 0, aa = 0, bb = 0;
        for (int i = 0; i < na && i + lag < nb; i++) {
            ab += a[i] * b[i + lag];
            aa += a[i] * a[i];
            bb += b[i + lag] * b[i + lag];
        }
        if (aa > 0 && bb > 0 && ab / sqrt(aa * bb) > best) {
            best = ab / sqrt(aa * bb);
            *lag_out = lag;
        }
    }
    return best;
}

/* Plays the datagrams as a listener gets them: datagram i arrives at
 * `arrive[i]` ms (or never, < 0); the mixer runs every 20 ms. The output
 * as floats; its length in samples. */
static int listen(const datagram *d, int count, const int *arrive, float *out, int max_ticks,
                  sv_stats *stats, uint32_t far_mask)
{
    sv_core *c = sv_core_new();
    int16_t pcm[SV_FRAME];
    int n = 0, closed_at = -1;
    CHECK(c != NULL, "no core");
    if (!c)
        return 0;
    for (int tick = 0; tick < max_ticks; tick++) {
        uint32_t now = (uint32_t)tick * SV_FRAME_MS;
        for (int i = 0; i < count; i++) {
            sv_msg m;
            if (arrive[i] >= 0 && (uint32_t)arrive[i] >= now &&
                (uint32_t)arrive[i] < now + SV_FRAME_MS && relay(&d[i], &m, 3))
                sv_core_receive(c, &m, now);
        }
        sv_core_mix(c, pcm, 1.0f, far_mask, now);
        for (int i = 0; i < SV_FRAME; i++)
            out[n++] = pcm[i] / 32768.0f;
        if (closed_at < 0 && tick > 10 && !(sv_core_talking(c, now) & (1u << 3)))
            closed_at = tick;
    }
    *stats = sv_core_stats(c);
    CHECK(closed_at >= 0, "the speaker never stopped talking");
    CHECK(sv_core_last_speaker(c) == 3, "the last speaker is %d, not 3",
          sv_core_last_speaker(c));
    sv_core_free(c);
    return n;
}

static double energy(const float *x, int n)
{
    double e = 0;
    for (int i = 0; i < n; i++)
        e += (double)x[i] * x[i];
    return e;
}

static void test_loopback(void)
{
    static int16_t pcm[SAMPLES];
    static float input[SAMPLES];
    static datagram d[FRAMES];
    static int arrive[FRAMES];
    enum { TICKS = SECONDS * 50 + 150 };
    static float clean[TICKS * SV_FRAME], lossy[TICKS * SV_FRAME];
    sv_stats st;
    int lag = 0;

    speech(pcm, SAMPLES);
    for (int i = 0; i < SAMPLES; i++)
        input[i] = pcm[i] / 32768.0f;
    int count = talk(pcm, d, FRAMES);
    CHECK(count == 2 + FRAMES / 2, "%d datagrams", count);

    /* In time and in order: datagram i (START first) every 40 ms, a little
     * jitter, 50 ms on the way. */
    for (int i = 0; i < count; i++)
        arrive[i] = 50 + 40 * (i > 0 ? i - 1 : 0) + (i * 7) % 15;
    int n = listen(d, count, arrive, clean, TICKS, &st, 0);
    printf("clean: decoded %u, concealed %u, late %u, underruns %u, closed %u, timeouts %u\n",
           st.decoded, st.concealed, st.late, st.underruns, st.closed, st.timeouts);
    CHECK(st.decoded == FRAMES, "clean: %u frames decoded, not %d", st.decoded, FRAMES);
    CHECK(st.concealed == 0 && st.late == 0, "clean: concealed or late frames");
    CHECK(st.closed == 1 && st.timeouts == 0, "clean: closed %u, timeouts %u", st.closed,
          st.timeouts);
    double corr = best_correlation(input, SAMPLES, clean, n, SV_RATE / 4, &lag);
    double gain = 10 * log10(energy(clean, n) / energy(input, SAMPLES));
    printf("clean: correlation %.3f at %d ms, level %+.1f dB\n", corr, lag * 1000 / SV_RATE,
           gain);
    CHECK(corr > 0.75, "clean: correlation %.3f", corr);
    CHECK(fabs(gain) < 3.0, "clean: level %+.1f dB", gain);
    /* Latency: 50 ms on the way, 60 ms buffered, the codec's look-ahead. */
    CHECK(lag * 1000 / SV_RATE >= 60 && lag * 1000 / SV_RATE <= 200, "clean: lag %d ms",
          lag * 1000 / SV_RATE);
    int clean_lag = lag;

    /* 10 % lost (never the START or END: the relay's resends of END are
     * the talker's), and every seventh pair swapped. */
    uint32_t r = 99;
    int lost = 0;
    for (int i = 0; i < count; i++) {
        arrive[i] = 50 + 40 * (i > 0 ? i - 1 : 0) + (i * 7) % 15;
        r = r * 1103515245u + 12345u;
        if (i > 1 && i < count - 2 && (r >> 16) % 10 == 0) {
            arrive[i] = -1;
            lost++;
        }
    }
    for (int i = 3; i + 1 < count - 1; i += 7)
        if (arrive[i] >= 0 && arrive[i + 1] >= 0)
            arrive[i] = arrive[i + 1] + 5;
    n = listen(d, count, arrive, lossy, TICKS, &st, 0);
    printf("lossy (%d datagrams lost, %d frames): decoded %u, concealed %u, late %u, "
           "underruns %u\n",
           lost, lost * 2, st.decoded, st.concealed, st.late, st.underruns);
    CHECK(st.decoded == (uint32_t)(FRAMES - lost * 2), "lossy: %u decoded", st.decoded);
    CHECK(st.concealed == (uint32_t)lost * 2, "lossy: %u concealed, %d lost", st.concealed,
          lost * 2);
    CHECK(st.late == 0, "lossy: %u late (a swap within the buffer is not late)", st.late);
    /* The same playout times: compared where they are. */
    corr = best_correlation(clean + clean_lag, SAMPLES, lossy + clean_lag, n - clean_lag, 0,
                            &lag);
    gain = 10 * log10(energy(lossy, n) / energy(clean, n));
    printf("lossy vs clean: correlation %.3f at %d samples, level %+.1f dB\n", corr, lag,
           gain);
    CHECK(corr > 0.6, "lossy: correlation %.3f with the clean output", corr);
    CHECK(fabs(gain) < 3.0, "lossy: level %+.1f dB", gain);

    /* A swap past the buffer: a datagram 200 ms late is dropped as late. */
    for (int i = 0; i < count; i++)
        arrive[i] = 50 + 40 * (i > 0 ? i - 1 : 0);
    arrive[10] += 200;
    listen(d, count, arrive, lossy, TICKS, &st, 0);
    CHECK(st.late == 2 && st.concealed == 2, "very late: %u late, %u concealed", st.late,
          st.concealed);

    /* No END (it was lost three times): closed by the silence. */
    for (int i = 0; i < count; i++)
        arrive[i] = 50 + 40 * (i > 0 ? i - 1 : 0);
    arrive[count - 1] = -1;
    listen(d, count, arrive, lossy, TICKS, &st, 0);
    CHECK(st.closed == 1 && st.timeouts == 1, "no END: closed %u, timeouts %u", st.closed,
          st.timeouts);
    CHECK(st.decoded == FRAMES, "no END: %u decoded", st.decoded);

    /* Through the radio filter: still the voice. Its level against the
     * plain voice is measured on retail speech (radio_demo: -0.2 dB); this
     * synthetic voice has its fundamental below the radio's band. */
    for (int i = 0; i < count; i++)
        arrive[i] = 50 + 40 * (i > 0 ? i - 1 : 0);
    n = listen(d, count, arrive, lossy, TICKS, &st, 1u << 3);
    gain = 10 * log10(energy(lossy, n) / energy(clean, n));
    printf("radio: level %+.1f dB against the plain voice; squelch in %u, out %u\n", gain,
           st.squelch_in, st.squelch_out);
    CHECK(fabs(gain) < 8.0, "radio: level %+.1f dB", gain);
    CHECK(st.squelch_in == 1 && st.squelch_out == 1, "radio: squelch in %u, out %u",
          st.squelch_in, st.squelch_out);
    CHECK(st.decoded == FRAMES, "radio: %u decoded (the squelch-in holds the words back, "
          "drops none)", st.decoded);
    n = listen(d, count, arrive, lossy, TICKS, &st, 0);
    CHECK(st.squelch_in == 0 && st.squelch_out == 0, "near: squelch in %u, out %u",
          st.squelch_in, st.squelch_out);
}

/* ---- Utterances, timeouts, mutes ---------------------------------------- */

static void feed(sv_core *c, int type, int slot, uint16_t utt, uint16_t seq,
                 const uint8_t *frame, size_t len, uint32_t now)
{
    sv_msg m;
    memset(&m, 0, sizeof m);
    m.type = (uint8_t)type;
    m.slot = (uint8_t)slot;
    m.utt = utt;
    m.seq = seq;
    if (type == SV_RELAY_AUDIO) {
        m.n = 2;
        m.frame[0] = m.frame[1] = frame;
        m.len[0] = m.len[1] = (uint8_t)len;
    }
    sv_core_receive(c, &m, now);
}

static void test_utterances(void)
{
    sv_encoder *enc = sv_encoder_new();
    sv_core *c = sv_core_new();
    int16_t pcm[SV_FRAME], out[SV_FRAME];
    uint8_t frame[SV_MAX_FRAME_BYTES];
    size_t len;
    sv_stats st;
    for (int i = 0; i < SV_FRAME; i++)
        pcm[i] = (int16_t)(8000 * sin(2 * PI * 440.0 * i / SV_RATE));
    len = sv_encode(enc, pcm, frame, sizeof frame);
    CHECK(len > 0, "encode");

    CHECK(sv_core_last_speaker(c) == -1, "a last speaker before any");
    /* A slot out of range is ignored. */
    feed(c, SV_RELAY_START, SV_SLOTS, 1, 0, NULL, 0, 0);
    CHECK(sv_core_talking(c, 0) == 0, "slot 20 talks");

    /* Slot 2 talks, utterance 10; then 11 starts: 10's late audio is late. */
    feed(c, SV_RELAY_START, 2, 10, 0, NULL, 0, 0);
    feed(c, SV_RELAY_AUDIO, 2, 10, 0, frame, len, 0);
    CHECK(sv_core_talking(c, 0) == (1u << 2), "slot 2 not talking");
    CHECK(sv_core_last_speaker(c) == 2, "last speaker %d", sv_core_last_speaker(c));
    feed(c, SV_RELAY_START, 2, 11, 0, NULL, 0, 20);
    feed(c, SV_RELAY_AUDIO, 2, 10, 1, frame, len, 20);
    st = sv_core_stats(c);
    CHECK(st.late == 2 && st.closed == 1, "old utterance: late %u closed %u", st.late,
          st.closed);
    /* An END of another utterance does nothing; its own closes it. */
    feed(c, SV_RELAY_END, 2, 10, 0, NULL, 0, 40);
    CHECK(sv_core_talking(c, 40) & (1u << 2), "END of the old utterance closed the new");
    feed(c, SV_RELAY_END, 2, 11, 0, NULL, 0, 40);
    CHECK(sv_core_talking(c, 40) & (1u << 2), "not heard just after its END");
    CHECK(!(sv_core_talking(c, 40 + SV_HEARD_MS) & (1u << 2)), "heard 250 ms after its END");
    /* The closed utterance does not come back with a late datagram. */
    feed(c, SV_RELAY_AUDIO, 2, 11, 3, frame, len, 300);
    CHECK(!(sv_core_talking(c, 300 + SV_HEARD_MS) & (1u << 2)), "a closed utterance reopened");

    /* Wrap of utt: 0xFFFF then 0 is newer. */
    uint32_t closed = sv_core_stats(c).closed;
    feed(c, SV_RELAY_START, 5, 0xFFFF, 0, NULL, 0, 1000);
    feed(c, SV_RELAY_START, 5, 0, 0, NULL, 0, 1000);
    CHECK(sv_core_stats(c).closed == closed + 1, "the wrap: 0 is not newer than 0xFFFF");
    feed(c, SV_RELAY_START, 5, 0xFFFF, 0, NULL, 0, 1000);
    CHECK(sv_core_stats(c).closed == closed + 1, "the wrap: 0xFFFF came back after 0");

    /* A START and nothing else: closed by silence after 1 s. */
    sv_core_reset(c);
    CHECK(sv_core_last_speaker(c) == -1, "the reset kept the last speaker");
    feed(c, SV_RELAY_START, 7, 1, 0, NULL, 0, 0);
    for (uint32_t t = 0; t <= 1100; t += 20)
        sv_core_mix(c, out, 1.0f, 0, t);
    CHECK(!(sv_core_talking(c, 1100) & (1u << 7)), "silent speaker still talking");
    CHECK(sv_core_stats(c).timeouts == 1, "silent speaker: timeouts %u",
          sv_core_stats(c).timeouts);

    /* Mutes: slot 4 muted is never played, never talking, never last. */
    sv_core_reset(c);
    sv_core_mute(c, 1u << 4);
    feed(c, SV_RELAY_START, 4, 1, 0, NULL, 0, 0);
    for (uint16_t s = 0; s < 4; s++)
        feed(c, SV_RELAY_AUDIO, 4, 1, s, frame, len, 0);
    double e = 0;
    for (uint32_t t = 0; t < 200; t += 20) {
        sv_core_mix(c, out, 1.0f, 0, t);
        for (int i = 0; i < SV_FRAME; i++)
            e += (double)out[i] * out[i];
    }
    CHECK(e == 0.0, "a muted slot was played");
    CHECK(sv_core_talking(c, 0) == 0, "a muted slot talks");
    CHECK(sv_core_last_speaker(c) == -1, "a muted slot is the last speaker");
    /* Slot 6 talks and is muted while it talks: cut at once. */
    sv_core_mute(c, 0);
    feed(c, SV_RELAY_START, 6, 1, 0, NULL, 0, 0);
    for (uint16_t s = 0; s < 4; s++)
        feed(c, SV_RELAY_AUDIO, 6, 1, s, frame, len, 0);
    sv_core_mix(c, out, 1.0f, 0, 0);
    e = 0;
    for (int i = 0; i < SV_FRAME; i++)
        e += (double)out[i] * out[i];
    CHECK(e > 0.0, "slot 6 not played");
    CHECK(sv_core_last_speaker(c) == 6, "last speaker %d", sv_core_last_speaker(c));
    sv_core_mute(c, 1u << 6);
    CHECK(sv_core_talking(c, 0) == 0, "muted slot 6 still talks");
    sv_core_mix(c, out, 1.0f, 0, 20);
    e = 0;
    for (int i = 0; i < SV_FRAME; i++)
        e += (double)out[i] * out[i];
    CHECK(e == 0.0, "muted slot 6 still played");
    /* Volume 0 plays nothing. */
    sv_core_mute(c, 0);
    feed(c, SV_RELAY_START, 8, 1, 0, NULL, 0, 0);
    for (uint16_t s = 0; s < 4; s++)
        feed(c, SV_RELAY_AUDIO, 8, 1, s, frame, len, 0);
    sv_core_mix(c, out, 0.0f, 0, 0);
    e = 0;
    for (int i = 0; i < SV_FRAME; i++)
        e += (double)out[i] * out[i];
    CHECK(e == 0.0, "volume 0 played");

    sv_core_free(c);
    sv_encoder_free(enc);
}

/* ---- The radio filter, the mixer, the gain ------------------------------ */

static double sine_gain_db(double f)
{
    enum { N = SV_RATE };
    static float x[N];
    sv_radio_state r;
    double in = 0, out = 0;
    sv_radio_init(&r, 1);
    for (int i = 0; i < N; i++)
        x[i] = (float)(0.1 * sin(2 * PI * f * i / SV_RATE));
    for (int i = N / 2; i < N; i++)
        in += (double)x[i] * x[i];
    sv_radio_process(&r, x, N, 0.0f);
    for (int i = N / 2; i < N; i++) /* after the filters settled */
        out += (double)x[i] * x[i];
    return 10 * log10(out / in);
}

static void test_radio(void)
{
    /* The filter's shape against retail's (the _radio takes against the
     * plain ones, 0.69d0, median by third octave; voice_core.c), each
     * relative to the 500..3150 Hz average. Our filter's sections are fitted
     * to it; a sine at a band's centre is within 4.5 dB of retail's band. */
    static const double centre[] = {400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150,
                                    4000, 5000, 6300};
    static const double retail[] = {-1.7, 7.2, 6.0, 1.1, -6.5, -2.1, 0.0, 3.0, -1.8, -6.6,
                                    7.7, 9.7, 3.7};
    enum { BANDS = sizeof centre / sizeof *centre };
    double ours[BANDS], mid = 0;
    int in_mid = 0;
    for (int b = 0; b < BANDS; b++) {
        ours[b] = sine_gain_db(centre[b]);
        if (centre[b] >= 500 && centre[b] <= 3150)
            mid += ours[b], in_mid++;
    }
    mid /= in_mid;
    printf("radio vs retail (dB, 0 = the 500..3150 Hz average):");
    for (int b = 0; b < BANDS; b++) {
        printf(" %.0f:%+.1f/%+.1f", centre[b], ours[b] - mid, retail[b]);
        CHECK(fabs(ours[b] - mid - retail[b]) < 4.5, "radio at %.0f Hz: %+.1f dB, retail %+.1f",
              centre[b], ours[b] - mid, retail[b]);
    }
    printf("\n");
    double g125 = sine_gain_db(125) - mid, g250 = sine_gain_db(250) - mid;
    printf("radio: 125 Hz %+.1f dB, 250 Hz %+.1f (retail -47, -33)\n", g125, g250);
    CHECK(g125 < -40 && g250 < -25, "the lows pass: 125 Hz %+.1f, 250 Hz %+.1f dB", g125, g250);

    /* Clipping: a loud tone stays within -1..1 and is squashed, not cut. */
    {
        float x[SV_RATE / 10];
        sv_radio_state r;
        float peak = 0;
        sv_radio_init(&r, 1);
        for (int i = 0; i < SV_RATE / 10; i++)
            x[i] = (float)(0.99 * sin(2 * PI * 1000.0 * i / SV_RATE));
        sv_radio_process(&r, x, SV_RATE / 10, 0.0f);
        for (int i = 0; i < SV_RATE / 10; i++)
            if (fabsf(x[i]) > peak)
                peak = fabsf(x[i]);
        printf("radio: a full-scale 1 kHz peaks at %.2f\n", peak);
        CHECK(peak < 0.75f && peak > 0.4f, "clipping: the peak is %.2f", peak);
    }
    /* The static: the retail bed, -44 dBFS rms on silence, looped. */
    {
        float x[SV_RATE];
        sv_radio_state r;
        double e = 0;
        sv_radio_init(&r, 1);
        memset(x, 0, sizeof x);
        sv_radio_process(&r, x, SV_RATE, 1.0f);
        for (int i = 0; i < SV_RATE; i++)
            e += (double)x[i] * x[i];
        double db = 10 * log10(e / SV_RATE);
        printf("radio: static %.1f dBFS (a %u-sample loop)\n", db, sv_static_bed_len);
        CHECK(db > -46 && db < -42, "static %.1f dBFS", db);
        CHECK(sv_static_bed_len > SV_RATE / 2, "the static's loop is %u samples",
              sv_static_bed_len);
    }
    /* The mixer saturates. */
    {
        float acc[4] = {2.0f, -2.0f, 0.5f, 0.0f};
        int16_t out[4];
        sv_mix_clamp(acc, out, 4);
        CHECK(out[0] == 32767 && out[1] == -32768 && out[2] == 16384 && out[3] == 0,
              "clamp: %d %d %d %d", out[0], out[1], out[2], out[3]);
    }
    /* Two loud speakers mixed: clamped, not wrapped. */
    {
        sv_encoder *e = sv_encoder_new();
        sv_core *c = sv_core_new();
        int16_t pcm[SV_FRAME], out[SV_FRAME];
        uint8_t frame[SV_MAX_FRAME_BYTES];
        int wrapped = 0, at_max = 0;
        for (int f = 0; f < 6; f++) {
            for (int i = 0; i < SV_FRAME; i++)
                pcm[i] = (int16_t)(32000 * sin(2 * PI * 500.0 * (f * SV_FRAME + i) / SV_RATE));
            size_t len = sv_encode(e, pcm, frame, sizeof frame);
            if (f % 2 == 0)
                continue;
            for (int slot = 0; slot < 2; slot++)
                feed(c, SV_RELAY_AUDIO, slot, 1, (uint16_t)(f / 2), frame, len, 0);
        }
        for (uint32_t t = 0; t < 120; t += 20) {
            int prev = 0;
            sv_core_mix(c, out, 1.0f, 0, t);
            for (int i = 0; i < SV_FRAME; i++) {
                if (out[i] == 32767 || out[i] == -32768)
                    at_max++;
                /* A wrap would jump from one end to the other. */
                if (i && abs(out[i] - prev) > 40000)
                    wrapped++;
                prev = out[i];
            }
        }
        CHECK(at_max > 0 && wrapped == 0, "two loud speakers: %d at the limits, %d wraps",
              at_max, wrapped);
        sv_core_free(c);
        sv_encoder_free(e);
    }
    /* The microphone's gain. */
    CHECK(fabsf(sv_mic_gain(0.0f) - 0.25f) < 1e-4f && fabsf(sv_mic_gain(0.5f) - 1.0f) < 1e-4f &&
              fabsf(sv_mic_gain(1.0f) - 4.0f) < 1e-4f && fabsf(sv_mic_gain(7.0f) - 4.0f) < 1e-4f &&
              fabsf(sv_mic_gain(NAN) - 0.25f) < 1e-4f,
          "the microphone's gain");
}

/* ---- The squelch -------------------------------------------------------- */

/* Slot 3 talks: START and 12 frames at t 0, END at `end_ms` (none: < 0),
 * muted at `mute_at` (never: < 0); the mixer runs to 1.5 s with `far_at`
 * giving the far mask by time. The stats; the frames from the begin to the
 * words' first; the output's first SV_SQUELCH_FRAMES ticks from the begin. */
static sv_stats squelch_case(int end_ms, uint32_t (*far_at)(uint32_t t), int mute_at,
                             int *words_tick, int16_t *head)
{
    sv_encoder *enc = sv_encoder_new();
    sv_core *c = sv_core_new();
    int16_t pcm[SV_FRAME], out[SV_FRAME];
    uint8_t frame[SV_MAX_FRAME_BYTES];
    size_t len;
    int begun = -1;
    for (int i = 0; i < SV_FRAME; i++)
        pcm[i] = (int16_t)(8000 * sin(2 * PI * 1000.0 * i / SV_RATE));
    len = sv_encode(enc, pcm, frame, sizeof frame);
    feed(c, SV_RELAY_START, 3, 1, 0, NULL, 0, 0);
    for (uint16_t s = 0; s < 6; s++)
        feed(c, SV_RELAY_AUDIO, 3, 1, s, frame, len, 0);
    *words_tick = -1;
    for (uint32_t t = 0, tick = 0; t < 1500; t += 20, tick++) {
        uint32_t decoded = sv_core_stats(c).decoded;
        if (end_ms >= 0 && t == (uint32_t)end_ms)
            feed(c, SV_RELAY_END, 3, 1, 0, NULL, 0, t);
        if (mute_at >= 0 && t == (uint32_t)mute_at)
            sv_core_mute(c, 1u << 3);
        sv_core_mix(c, out, 1.0f, far_at(t), t);
        if (begun < 0 && (sv_core_stats(c).squelch_in || sv_core_stats(c).decoded))
            begun = (int)tick;
        if (begun >= 0 && tick - (uint32_t)begun < SV_SQUELCH_FRAMES && head)
            memcpy(head + (tick - (uint32_t)begun) * SV_FRAME, out, sizeof out);
        if (*words_tick < 0 && sv_core_stats(c).decoded > decoded)
            *words_tick = (int)tick - begun;
    }
    sv_stats st = sv_core_stats(c);
    sv_core_free(c);
    sv_encoder_free(enc);
    return st;
}

static uint32_t always_far(uint32_t t) { (void)t; return 1u << 3; }
static uint32_t never_far(uint32_t t) { (void)t; return 0; }
static uint32_t far_later(uint32_t t) { return t >= 100 ? 1u << 3 : 0; }
static uint32_t near_later(uint32_t t) { return t >= 100 ? 0 : 1u << 3; }

static void test_squelch(void)
{
    static int16_t head[SV_SQUELCH_FRAMES * SV_FRAME];
    int words;
    sv_stats st;
    CHECK(sv_squelch_in_len == SV_SQUELCH_FRAMES * SV_FRAME,
          "the squelch-in is %u samples, the words wait %d frames", sv_squelch_in_len,
          SV_SQUELCH_FRAMES);
    CHECK(sv_squelch_out_len > SV_RATE / 20 && sv_squelch_out_len < SV_RATE / 2,
          "the squelch-out is %u samples", sv_squelch_out_len);

    /* Far: the squelch-in alone, then the words; the squelch-out at the END. */
    st = squelch_case(400, always_far, -1, &words, head);
    printf("squelch: far: in %u, out %u, the words after %d frames\n", st.squelch_in,
           st.squelch_out, words);
    CHECK(st.squelch_in == 1 && st.squelch_out == 1, "far: in %u, out %u", st.squelch_in,
          st.squelch_out);
    CHECK(words == SV_SQUELCH_FRAMES, "far: the words after %d frames, not %d", words,
          SV_SQUELCH_FRAMES);
    CHECK(!memcmp(head, sv_squelch_in, sizeof head),
          "far: the first %d frames are not the retail squelch-in", SV_SQUELCH_FRAMES);
    /* Far, its END lost: the squelch-out when the silence closes it. */
    st = squelch_case(-1, always_far, -1, &words, NULL);
    CHECK(st.squelch_in == 1 && st.squelch_out == 1 && st.timeouts == 1,
          "far, no END: in %u, out %u, timeouts %u", st.squelch_in, st.squelch_out,
          st.timeouts);
    /* Near: none, and the words at once. */
    st = squelch_case(400, never_far, -1, &words, NULL);
    CHECK(st.squelch_in == 0 && st.squelch_out == 0 && words == 0,
          "near: in %u, out %u, the words after %d", st.squelch_in, st.squelch_out, words);
    /* A switch mid-utterance adds none: in for far at the begin, out for far
     * at the end. */
    st = squelch_case(400, far_later, -1, &words, NULL);
    CHECK(st.squelch_in == 0 && st.squelch_out == 1, "near then far: in %u, out %u",
          st.squelch_in, st.squelch_out);
    st = squelch_case(400, near_later, -1, &words, NULL);
    CHECK(st.squelch_in == 1 && st.squelch_out == 0, "far then near: in %u, out %u",
          st.squelch_in, st.squelch_out);
    /* Muted while it talks: cut, no squelch-out. */
    st = squelch_case(400, always_far, 200, &words, NULL);
    CHECK(st.squelch_in == 1 && st.squelch_out == 0, "muted: in %u, out %u", st.squelch_in,
          st.squelch_out);
}

int main(void)
{
    test_wire();
    test_loopback();
    test_utterances();
    test_radio();
    test_squelch();
    if (failures) {
        printf("%d checks FAILED\n", failures);
        return 1;
    }
    printf("all voice checks passed\n");
    return 0;
}
