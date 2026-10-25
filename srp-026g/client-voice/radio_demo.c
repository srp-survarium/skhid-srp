/* radio_demo: a WAV through the voice's real path, for listening and for
 * measuring the radio (make_radio_sounds.py's sounds, voice_core.c's filter).
 *
 *   radio_demo <in.wav> <out.wav> near|far
 *
 * in.wav: 16-bit PCM, mono, 16 kHz. It is encoded as a talker's DLL does
 * (Opus, 20 ms frames, two a datagram), relayed in time (START, AUDIO every
 * 40 ms, END), received by a listener's core and mixed every 20 ms, near
 * (plain) or far (the radio: squelch-in, the filter over the static,
 * squelch-out). out.wav is what the listener hears, from the START on.
 * Built natively against voice_core.c and libopus (flake.nix's
 * voice-loopback check builds it too). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "voice_core.h"

static int16_t *read_wav(const char *path, size_t *count)
{
    FILE *f = fopen(path, "rb");
    uint8_t h[44];
    long size;
    int16_t *pcm;
    if (!f || fread(h, 1, 44, f) != 44 || memcmp(h, "RIFF", 4) || memcmp(h + 8, "WAVEfmt ", 8) ||
        h[20] != 1 || h[22] != 1 || h[24] + (h[25] << 8) != SV_RATE || h[34] != 16) {
        fprintf(stderr, "%s: not a 16-bit mono 16 kHz WAV\n", path);
        exit(2);
    }
    fseek(f, 0, SEEK_END);
    size = ftell(f) - 44;
    fseek(f, 44, SEEK_SET);
    *count = (size_t)size / 2;
    pcm = calloc(*count + SV_FRAME, 2);
    if (!pcm || fread(pcm, 2, *count, f) != *count)
        exit(2);
    fclose(f);
    return pcm;
}

static void put32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)v, p[1] = (uint8_t)(v >> 8), p[2] = (uint8_t)(v >> 16), p[3] = (uint8_t)(v >> 24);
}

static void write_wav(const char *path, const int16_t *pcm, size_t count)
{
    uint8_t h[44] = {'R', 'I', 'F', 'F', 0, 0, 0, 0, 'W', 'A', 'V', 'E', 'f', 'm', 't', ' ', 16, 0,
                     0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 16, 0, 'd', 'a', 't', 'a'};
    FILE *f = fopen(path, "wb");
    put32(h + 4, (uint32_t)(36 + count * 2));
    put32(h + 24, SV_RATE);
    put32(h + 28, SV_RATE * 2);
    put32(h + 40, (uint32_t)(count * 2));
    if (!f || fwrite(h, 1, 44, f) != 44 || fwrite(pcm, 2, count, f) != count) {
        fprintf(stderr, "%s: cannot write\n", path);
        exit(2);
    }
    fclose(f);
}

int main(int argc, char **argv)
{
    size_t count, frames, out_n = 0;
    int16_t *in, *out;
    uint32_t far;
    sv_encoder *e = sv_encoder_new();
    sv_core *c = sv_core_new();
    static uint8_t data[2][SV_MAX_FRAME_BYTES];
    if (argc != 4 || (strcmp(argv[3], "near") && strcmp(argv[3], "far"))) {
        fprintf(stderr, "usage: radio_demo <in.wav> <out.wav> near|far\n");
        return 2;
    }
    far = strcmp(argv[3], "far") ? 0 : 1u;
    in = read_wav(argv[1], &count);
    frames = (count + SV_FRAME - 1) / SV_FRAME;
    /* 2 s after the words for the buffer, the squelch-out and the close */
    out = calloc(frames * SV_FRAME + 2 * SV_RATE, 2);
    if (!e || !c || !out)
        return 2;
    {
        sv_msg m;
        memset(&m, 0, sizeof m);
        m.type = SV_RELAY_START, m.slot = 0, m.utt = 1;
        sv_core_receive(c, &m, 0);
    }
    for (uint32_t tick = 0; out_n + SV_FRAME <= frames * SV_FRAME + 2 * SV_RATE; tick++) {
        uint32_t now = tick * SV_FRAME_MS;
        /* AUDIO k (frames 2k, 2k+1) leaves when both are captured, 40 ms on. */
        if (tick % 2 == 1 && tick - 1 < frames) {
            sv_msg m;
            memset(&m, 0, sizeof m);
            m.type = SV_RELAY_AUDIO, m.slot = 0, m.utt = 1, m.seq = (uint16_t)(tick / 2);
            for (uint32_t k = 0; k < 2 && tick - 1 + k < frames; k++) {
                size_t len = sv_encode(e, in + (tick - 1 + k) * SV_FRAME, data[k], sizeof data[k]);
                m.frame[k] = data[k], m.len[k] = (uint8_t)len, m.n = (uint8_t)(k + 1);
            }
            if (m.n)
                sv_core_receive(c, &m, now);
        }
        if (tick == frames + 2) {
            sv_msg m;
            memset(&m, 0, sizeof m);
            m.type = SV_RELAY_END, m.slot = 0, m.utt = 1;
            sv_core_receive(c, &m, now);
        }
        sv_core_mix(c, out + out_n, 1.0f, far, now);
        out_n += SV_FRAME;
    }
    write_wav(argv[2], out, out_n);
    {
        sv_stats st = sv_core_stats(c);
        fprintf(stderr, "decoded %u, squelch in %u, out %u\n", st.decoded, st.squelch_in,
                st.squelch_out);
    }
    sv_core_free(c);
    sv_encoder_free(e);
    free(in);
    free(out);
    return 0;
}
