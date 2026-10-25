/* skhid_voice.dll: in-match team voice chat for the Skhid 0.26g client.
 *
 * The game has no capture path and no codec (srp-026g/docs/notes/
 * voice-chat.md), so this DLL, beside survarium.exe, does all of it: the
 * microphone (waveIn, 16 kHz mono, opened only while push-to-talk is held
 * and for a second after), Opus at 16 kbit/s, a UDP socket to srp's voice
 * port, a jitter buffer and an Opus decoder per speaker, the radio filter
 * for far teammates, the mix, and playback (waveOut, 16 kHz mono). The exe
 * patch loads it with LoadLibraryA and calls the exports below; the wire,
 * the codec, the buffers and the filter are voice_core.c, which
 * test_voice.c checks on Linux.
 *
 * Build (32-bit Windows, libopus and libgcc linked in, nothing but system
 * DLLs imported):
 *     nix build ./srp-026g#skhid-voice          -> result/skhid_voice.dll
 *     nix build ./srp-026g#checks.x86_64-linux.voice-loopback
 *
 * Exports, all __cdecl, undecorated (skhid_voice.def), all called from the
 * game's thread and cheap: none blocks (but sv_start's name lookup, and
 * sv_stop's wait for the threads, at most a few tens of ms), none allocates
 * but sv_start.
 *   int      sv_version(void)            1.
 *   int      sv_start(const char *host, unsigned port, unsigned session)
 *            The voice port (the match port + 1) of the match server and
 *            the session of the game's 0xD8. Starts the socket and the
 *            threads and sends a HELLO; again with the same arguments it
 *            does nothing, with others it stops and starts again. 0 on
 *            success.
 *   void     sv_stop(void)               Everything stopped and closed;
 *            safe when not started, and twice.
 *   void     sv_ptt(int down)            Push-to-talk held or not.
 *   void     sv_settings(float volume, int mic_on, float mic_sens)
 *            volume 0..1 (0: nothing played, nothing asked of the server);
 *            mic_on 0: push-to-talk sends nothing; mic_sens 0..1, the
 *            microphone's gain (0.25x..4x). "Disable voice chat" is
 *            (0, 0, any): it takes effect at once.
 *   void     sv_radio(unsigned far_mask) Bit n: slot n is far, through the
 *            radio filter.
 *   void     sv_mute(unsigned mask)      Bit n: slot n muted: never
 *            played, and the server is told (a HELLO at once).
 *   unsigned sv_talking(void)            Bit n: slot n is heard now; the
 *            own slot's bit while transmitting (bit 31 before WELCOME).
 *            0 while voice is disabled.
 *   int      sv_slot(void)               The own slot (WELCOME), -1 before.
 *   int      sv_last_speaker(void)       The slot last heard (not muted),
 *            -1 for none since sv_start.
 *   unsigned sv_voice_id(int slot)       The slot's voice id (ROSTER: srp's
 *            stable, opaque id of its account, which a mute is kept by), 0
 *            while not known.
 *
 * Nothing of the audio is kept or written anywhere. Errors (no microphone,
 * no output device) go to OutputDebugString and leave voice silent.
 */
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <mmsystem.h>

#include <string.h>

#include "voice_core.h"

#ifndef SIO_UDP_CONNRESET
#define SIO_UDP_CONNRESET _WSAIOW(IOC_VENDOR, 12)
#endif

/* Exported by skhid_voice.def (only these names: libopus's are not). */
#define SV_EXPORT __cdecl

#define HELLO_EVERY_MS 2000
#define END_RESENDS 2          /* after the first: three ENDs in all */
#define END_RESEND_MS 30
#define MIC_CLOSE_AFTER_MS 1000
#define TALK_LIMIT_MS 30000    /* the server cuts an utterance here too */
#define OUT_BUFFERS 4
#define OUT_PREROLL 3          /* buffers of silence queued at the start */
#define IN_BUFFERS 4
#define OUTPUT_RETRY_MS 5000

/* ---- What the game's thread sets and reads, lock-free ------------------- */

static volatile LONG g_ptt;
static volatile LONG g_mic_on = 1;
static volatile LONG g_volume_bits = 0x3F800000;  /* 1.0f */
static volatile LONG g_sens_bits = 0x3F000000;    /* 0.5f */
static volatile LONG g_far;
static volatile LONG g_muted;
static volatile LONG g_slot = -1;
static volatile LONG g_heard;         /* the mixer's talking mask */
static volatile LONG g_last_speaker = -1;
static volatile LONG g_ids[SV_SLOTS];  /* ROSTER: each slot's voice id, 0 unknown */
static volatile LONG g_started;
static volatile LONG g_capped;        /* talked past the limit: until released */
static volatile LONG g_hello_now;     /* a HELLO at once (mute or hears changed) */

static float bits_float(LONG bits)
{
    float f;
    memcpy(&f, &bits, sizeof f);
    return f;
}

static LONG float_bits(float f)
{
    LONG bits;
    memcpy(&bits, &f, sizeof bits);
    return bits;
}

static float volume_now(void)
{
    return bits_float(g_volume_bits);
}

/* ---- The running session -------------------------------------------- */

typedef struct {
    char host[256];
    unsigned port;
    uint32_t session;
    SOCKET sock;
    HANDLE stop;
    HANDLE net_thread;
    HANDLE audio_thread;
    SRWLOCK core_lock;
    sv_core *core;
    int wsa;
} session_t;

static SRWLOCK g_api = SRWLOCK_INIT;  /* sv_start / sv_stop */
static session_t g_s;
static int g_running;
static int g_broken;  /* a thread never stopped: no new start */

static void say(const char *text)
{
    OutputDebugStringA("skhid_voice: ");
    OutputDebugStringA(text);
    OutputDebugStringA("\n");
}

static void send_bytes(const uint8_t *data, size_t len)
{
    if (len && g_s.sock != INVALID_SOCKET)
        send(g_s.sock, (const char *)data, (int)len, 0);
}

static void send_hello(void)
{
    uint8_t buf[32];
    uint8_t flags = volume_now() > 0.0f ? SV_HELLO_HEARS : 0;
    send_bytes(buf, sv_hello(g_s.session, flags, (uint32_t)g_muted, buf, sizeof buf));
}

/* ---- The network thread: what the server sends, and the HELLOs --------- */

static DWORD WINAPI net_main(LPVOID unused)
{
    uint8_t buf[SV_MAX_DATAGRAM + 64];
    DWORD last_hello = GetTickCount();
    (void)unused;
    send_hello();
    while (WaitForSingleObject(g_s.stop, 0) != WAIT_OBJECT_0) {
        int got = recv(g_s.sock, (char *)buf, sizeof buf, 0);
        DWORD now = GetTickCount();
        if (got > 0) {
            sv_msg m;
            if (sv_parse(buf, (size_t)got, &m)) {
                if (m.type == SV_WELCOME) {
                    if (m.slot < SV_SLOTS)
                        InterlockedExchange(&g_slot, m.slot);
                } else if (m.type == SV_ROSTER) {
                    for (unsigned k = 0; k < m.count; k++)
                        if (m.roster_slot[k] < SV_SLOTS)
                            InterlockedExchange(&g_ids[m.roster_slot[k]], (LONG)m.roster_id[k]);
                } else if (m.type == SV_RELAY_START || m.type == SV_RELAY_AUDIO ||
                           m.type == SV_RELAY_END) {
                    if (m.slot < SV_SLOTS && !((uint32_t)g_muted & (1u << m.slot)) &&
                        volume_now() > 0.0f) {
                        AcquireSRWLockExclusive(&g_s.core_lock);
                        sv_core_receive(g_s.core, &m, now);
                        ReleaseSRWLockExclusive(&g_s.core_lock);
                    }
                }
            }
        } else if (got == SOCKET_ERROR) {
            int error = WSAGetLastError();
            if (error != WSAETIMEDOUT && error != WSAEWOULDBLOCK && error != WSAECONNRESET &&
                error != WSAEMSGSIZE)
                Sleep(20); /* something else: do not spin */
        }
        if (InterlockedExchange(&g_hello_now, 0) || now - last_hello >= HELLO_EVERY_MS) {
            send_hello();
            last_hello = now;
        }
    }
    return 0;
}

/* ---- The audio thread: playback, capture, the talker's datagrams ------- */

typedef struct {
    HWAVEOUT out;
    WAVEHDR out_hdr[OUT_BUFFERS];
    int16_t out_pcm[OUT_BUFFERS][SV_FRAME];
    int out_queued[OUT_BUFFERS];
    HANDLE out_event;
    DWORD out_failed_at;
    int out_tried;
    DWORD next_mix;            /* without an output device: the mixer's clock */

    HWAVEIN in;
    WAVEHDR in_hdr[IN_BUFFERS];
    int16_t in_pcm[IN_BUFFERS][SV_FRAME];
    HANDLE in_event;
    int in_failed;             /* this press: no microphone */

    sv_encoder *enc;
    sv_sender snd;
    int transmitting;
    DWORD talk_started;
    DWORD released_at;
    uint8_t end[32];
    size_t end_len;
    int end_left;
    DWORD end_next;
    uint32_t muted_applied;
} audio_t;

static WAVEFORMATEX wave_format(void)
{
    WAVEFORMATEX f;
    memset(&f, 0, sizeof f);
    f.wFormatTag = WAVE_FORMAT_PCM;
    f.nChannels = 1;
    f.nSamplesPerSec = SV_RATE;
    f.wBitsPerSample = 16;
    f.nBlockAlign = 2;
    f.nAvgBytesPerSec = SV_RATE * 2;
    return f;
}

static void mix_into(int16_t *pcm, DWORD now)
{
    uint32_t heard;
    float volume = volume_now();
    AcquireSRWLockExclusive(&g_s.core_lock);
    sv_core_mix(g_s.core, pcm, volume, (uint32_t)g_far, now);
    heard = sv_core_talking(g_s.core, now);
    InterlockedExchange(&g_last_speaker, sv_core_last_speaker(g_s.core));
    ReleaseSRWLockExclusive(&g_s.core_lock);
    InterlockedExchange(&g_heard, (LONG)heard);
}

static void output_open(audio_t *a, DWORD now)
{
    WAVEFORMATEX f = wave_format();
    int k;
    a->out_tried = 1;
    if (waveOutOpen(&a->out, WAVE_MAPPER, &f, (DWORD_PTR)a->out_event, 0, CALLBACK_EVENT) !=
        MMSYSERR_NOERROR) {
        a->out = NULL;
        a->out_failed_at = now;
        say("no output device; voice is not played (tried again in 5 s)");
        return;
    }
    for (k = 0; k < OUT_BUFFERS; k++) {
        memset(&a->out_hdr[k], 0, sizeof a->out_hdr[k]);
        a->out_hdr[k].lpData = (LPSTR)a->out_pcm[k];
        a->out_hdr[k].dwBufferLength = sizeof a->out_pcm[k];
        waveOutPrepareHeader(a->out, &a->out_hdr[k], sizeof a->out_hdr[k]);
        a->out_queued[k] = 0;
    }
    /* A little silence first: the buffers that are played while the next
     * ones are mixed. */
    for (k = 0; k < OUT_PREROLL; k++) {
        memset(a->out_pcm[k], 0, sizeof a->out_pcm[k]);
        if (waveOutWrite(a->out, &a->out_hdr[k], sizeof a->out_hdr[k]) == MMSYSERR_NOERROR)
            a->out_queued[k] = 1;
    }
}

static void output_close(audio_t *a)
{
    int k;
    if (!a->out)
        return;
    waveOutReset(a->out);
    for (k = 0; k < OUT_BUFFERS; k++)
        waveOutUnprepareHeader(a->out, &a->out_hdr[k], sizeof a->out_hdr[k]);
    waveOutClose(a->out);
    a->out = NULL;
}

static void output_pump(audio_t *a, DWORD now)
{
    int k;
    if (!a->out) {
        if (!a->out_tried || now - a->out_failed_at >= OUTPUT_RETRY_MS)
            output_open(a, now);
    }
    if (!a->out) {
        /* No device: the speakers are mixed all the same (timeouts, the
         * talking mask), into nothing. */
        int16_t scratch[SV_FRAME];
        int catch_up = 0;
        if ((int32_t)(now - a->next_mix) > 200)
            a->next_mix = now;
        while ((int32_t)(now - a->next_mix) >= 0 && catch_up++ < 10) {
            mix_into(scratch, a->next_mix);
            a->next_mix += SV_FRAME_MS;
        }
        return;
    }
    for (k = 0; k < OUT_BUFFERS; k++) {
        if (a->out_queued[k] && !(a->out_hdr[k].dwFlags & WHDR_DONE))
            continue;
        a->out_queued[k] = 0;
        mix_into(a->out_pcm[k], now);
        a->out_hdr[k].dwFlags &= ~WHDR_DONE;
        if (waveOutWrite(a->out, &a->out_hdr[k], sizeof a->out_hdr[k]) == MMSYSERR_NOERROR)
            a->out_queued[k] = 1;
    }
    a->next_mix = now;
}

static int input_open(audio_t *a)
{
    WAVEFORMATEX f = wave_format();
    int k;
    if (waveInOpen(&a->in, WAVE_MAPPER, &f, (DWORD_PTR)a->in_event, 0, CALLBACK_EVENT) !=
        MMSYSERR_NOERROR) {
        a->in = NULL;
        say("no microphone; push-to-talk sends nothing");
        return 0;
    }
    for (k = 0; k < IN_BUFFERS; k++) {
        memset(&a->in_hdr[k], 0, sizeof a->in_hdr[k]);
        a->in_hdr[k].lpData = (LPSTR)a->in_pcm[k];
        a->in_hdr[k].dwBufferLength = sizeof a->in_pcm[k];
        waveInPrepareHeader(a->in, &a->in_hdr[k], sizeof a->in_hdr[k]);
        waveInAddBuffer(a->in, &a->in_hdr[k], sizeof a->in_hdr[k]);
    }
    if (waveInStart(a->in) != MMSYSERR_NOERROR) {
        say("the microphone does not start");
        waveInReset(a->in);
        for (k = 0; k < IN_BUFFERS; k++)
            waveInUnprepareHeader(a->in, &a->in_hdr[k], sizeof a->in_hdr[k]);
        waveInClose(a->in);
        a->in = NULL;
        return 0;
    }
    return 1;
}

static void input_close(audio_t *a)
{
    int k;
    if (!a->in)
        return;
    waveInReset(a->in);
    for (k = 0; k < IN_BUFFERS; k++)
        waveInUnprepareHeader(a->in, &a->in_hdr[k], sizeof a->in_hdr[k]);
    waveInClose(a->in);
    a->in = NULL;
}

static void talk_end(audio_t *a, DWORD now)
{
    uint8_t buf[SV_MAX_DATAGRAM];
    send_bytes(buf, sv_sender_flush(&a->snd, buf, sizeof buf));
    a->end_len = sv_sender_end(&a->snd, a->end, sizeof a->end);
    send_bytes(a->end, a->end_len);
    a->end_left = a->end_len ? END_RESENDS : 0;
    a->end_next = now + END_RESEND_MS;
    a->transmitting = 0;
    a->released_at = now;
}

static void input_pump(audio_t *a, DWORD now)
{
    int ptt = g_ptt != 0;
    int want;
    int k;
    if (!ptt) {
        InterlockedExchange(&g_capped, 0);
        a->in_failed = 0;
    }
    want = ptt && g_mic_on && !g_capped;
    if (want && !a->transmitting) {
        uint8_t buf[64];
        if (!a->in && !a->in_failed && !input_open(a))
            a->in_failed = 1;
        if (a->in) {
            a->transmitting = 1;
            a->talk_started = now;
            a->end_left = 0;
            send_bytes(buf, sv_sender_start(&a->snd, now, buf, sizeof buf));
        }
    }
    if (a->transmitting && now - a->talk_started >= TALK_LIMIT_MS) {
        InterlockedExchange(&g_capped, 1);
        want = 0;
    }
    if (a->transmitting && !want)
        talk_end(a, now);
    if (a->in) {
        for (k = 0; k < IN_BUFFERS; k++) {
            WAVEHDR *h = &a->in_hdr[k];
            if (!(h->dwFlags & WHDR_DONE))
                continue;
            if (a->transmitting && h->dwBytesRecorded == sizeof a->in_pcm[k]) {
                uint8_t frame[SV_MAX_FRAME_BYTES], buf[SV_MAX_DATAGRAM];
                float gain = sv_mic_gain(bits_float(g_sens_bits));
                size_t len;
                int i;
                for (i = 0; i < SV_FRAME; i++) {
                    float v = a->in_pcm[k][i] * gain;
                    a->in_pcm[k][i] = (int16_t)(v > 32767.0f    ? 32767
                                                : v < -32768.0f ? -32768
                                                                : (int)v);
                }
                len = sv_encode(a->enc, a->in_pcm[k], frame, sizeof frame);
                if (len)
                    send_bytes(buf, sv_sender_frame(&a->snd, frame, len, buf, sizeof buf));
            }
            h->dwFlags &= ~WHDR_DONE;
            h->dwBytesRecorded = 0;
            waveInAddBuffer(a->in, h, sizeof *h);
        }
        if (!a->transmitting && now - a->released_at >= MIC_CLOSE_AFTER_MS)
            input_close(a);
    }
    if (a->end_left > 0 && (int32_t)(now - a->end_next) >= 0) {
        send_bytes(a->end, a->end_len);
        a->end_left--;
        a->end_next = now + END_RESEND_MS;
    }
}

static DWORD WINAPI audio_main(LPVOID param)
{
    audio_t *a = (audio_t *)param;
    HANDLE wait[3];
    SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    wait[0] = g_s.stop;
    wait[1] = a->out_event;
    wait[2] = a->in_event;
    a->next_mix = GetTickCount();
    for (;;) {
        DWORD now;
        uint32_t muted;
        if (WaitForMultipleObjects(3, wait, FALSE, 10) == WAIT_OBJECT_0)
            break;
        now = GetTickCount();
        muted = (uint32_t)g_muted;
        if (muted != a->muted_applied || volume_now() <= 0.0f) {
            AcquireSRWLockExclusive(&g_s.core_lock);
            if (volume_now() <= 0.0f)
                sv_core_mute(g_s.core, 0xFFFFFFFFu); /* disabled: drop everyone */
            sv_core_mute(g_s.core, muted);
            ReleaseSRWLockExclusive(&g_s.core_lock);
            a->muted_applied = muted;
        }
        output_pump(a, now);
        input_pump(a, now);
    }
    if (a->transmitting)
        talk_end(a, GetTickCount());
    input_close(a);
    output_close(a);
    return 0;
}

static audio_t *g_audio;

/* ---- Start and stop ---------------------------------------------------- */

static void stop_locked(void)
{
    HANDLE threads[2];
    DWORD count = 0;
    int stuck = 0;
    if (!g_running)
        return;
    InterlockedExchange(&g_started, 0);
    SetEvent(g_s.stop);
    if (g_s.net_thread)
        threads[count++] = g_s.net_thread;
    if (g_s.audio_thread)
        threads[count++] = g_s.audio_thread;
    if (count && WaitForMultipleObjects(count, threads, TRUE, 3000) == WAIT_TIMEOUT) {
        /* A thread that does not end (a stuck audio driver): its memory is
         * left to it rather than freed under it. */
        stuck = 1;
        say("a thread did not stop; its memory is left");
    }
    InterlockedExchange(&g_slot, -1);
    InterlockedExchange(&g_heard, 0);
    InterlockedExchange(&g_last_speaker, -1);
    InterlockedExchange(&g_capped, 0);
    for (unsigned k = 0; k < SV_SLOTS; k++)
        InterlockedExchange(&g_ids[k], 0);
    g_running = 0;
    if (stuck) {
        /* The threads read g_s: it stays as it is, and voice stays off
         * until the game restarts (sv_start refuses). */
        g_broken = 1;
        return;
    }
    if (g_s.net_thread)
        CloseHandle(g_s.net_thread);
    if (g_s.audio_thread)
        CloseHandle(g_s.audio_thread);
    if (g_s.sock != INVALID_SOCKET)
        closesocket(g_s.sock);
    if (g_audio) {
        sv_encoder_free(g_audio->enc);
        if (g_audio->out_event)
            CloseHandle(g_audio->out_event);
        if (g_audio->in_event)
            CloseHandle(g_audio->in_event);
        HeapFree(GetProcessHeap(), 0, g_audio);
    }
    sv_core_free(g_s.core);
    if (g_s.stop)
        CloseHandle(g_s.stop);
    if (g_s.wsa)
        WSACleanup();
    g_audio = NULL;
    memset(&g_s, 0, sizeof g_s);
    g_s.sock = INVALID_SOCKET;
    InterlockedExchange(&g_slot, -1);
    InterlockedExchange(&g_heard, 0);
    InterlockedExchange(&g_last_speaker, -1);
    InterlockedExchange(&g_capped, 0);
    for (unsigned k = 0; k < SV_SLOTS; k++)
        InterlockedExchange(&g_ids[k], 0);
}

static int start_locked(const char *host, unsigned port, uint32_t session)
{
    WSADATA wsa;
    struct addrinfo hints, *found = NULL;
    char service[16];
    DWORD timeout = 20;
    BOOL no_reset = FALSE;
    DWORD bytes = 0;
    HMODULE self;

    memset(&g_s, 0, sizeof g_s);
    g_s.sock = INVALID_SOCKET;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0)
        return 3;
    g_s.wsa = 1;
    lstrcpynA(g_s.host, host, sizeof g_s.host);
    g_s.port = port;
    g_s.session = session;

    memset(&hints, 0, sizeof hints);
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_DGRAM;
    hints.ai_protocol = IPPROTO_UDP;
    {
        /* The port in decimal (no user32 for wsprintf). */
        char digits[8];
        int n = 0, k = 0;
        unsigned v = port;
        do {
            digits[n++] = (char)('0' + v % 10);
            v /= 10;
        } while (v && n < 7);
        while (n)
            service[k++] = digits[--n];
        service[k] = 0;
    }
    if (getaddrinfo(host, service, &hints, &found) != 0 || !found) {
        say("the voice server's address does not resolve");
        WSACleanup();
        return 4;
    }
    g_s.sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (g_s.sock == INVALID_SOCKET ||
        connect(g_s.sock, found->ai_addr, (int)found->ai_addrlen) != 0) {
        freeaddrinfo(found);
        if (g_s.sock != INVALID_SOCKET)
            closesocket(g_s.sock);
        WSACleanup();
        return 5;
    }
    freeaddrinfo(found);
    setsockopt(g_s.sock, SOL_SOCKET, SO_RCVTIMEO, (const char *)&timeout, sizeof timeout);
    /* A port that answered "unreachable" once must not fail every recv. */
    WSAIoctl(g_s.sock, SIO_UDP_CONNRESET, &no_reset, sizeof no_reset, NULL, 0, &bytes, NULL,
             NULL);

    InitializeSRWLock(&g_s.core_lock);
    g_s.core = sv_core_new();
    g_audio = (audio_t *)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, sizeof *g_audio);
    g_s.stop = CreateEventA(NULL, TRUE, FALSE, NULL);
    if (g_audio) {
        g_audio->enc = sv_encoder_new();
        g_audio->out_event = CreateEventA(NULL, FALSE, FALSE, NULL);
        g_audio->in_event = CreateEventA(NULL, FALSE, FALSE, NULL);
        sv_sender_init(&g_audio->snd, session,
                       (uint16_t)(GetTickCount() ^ (GetCurrentProcessId() << 7) ^ session));
        g_audio->muted_applied = 0;
    }
    if (!g_s.core || !g_audio || !g_audio->enc || !g_s.stop || !g_audio->out_event ||
        !g_audio->in_event) {
        g_running = 1;
        stop_locked();
        return 6;
    }
    g_running = 1;
    /* Never unloaded while its threads run. */
    GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                       (LPCSTR)&start_locked, &self);
    InterlockedExchange(&g_started, 1);
    g_s.net_thread = CreateThread(NULL, 0, net_main, NULL, 0, NULL);
    g_s.audio_thread = CreateThread(NULL, 0, audio_main, g_audio, 0, NULL);
    if (!g_s.net_thread || !g_s.audio_thread) {
        stop_locked();
        return 7;
    }
    return 0;
}

/* ---- The exports ------------------------------------------------------- */

int SV_EXPORT sv_version(void)
{
    return 1;
}

int SV_EXPORT sv_start(const char *host, unsigned port, unsigned session)
{
    int result = 0;
    if (!host || !*host || port == 0 || port > 65535)
        return 1;
    AcquireSRWLockExclusive(&g_api);
    if (g_broken) {
        result = 8;
    } else if (!(g_running && g_s.port == port && g_s.session == session &&
                 lstrcmpA(g_s.host, host) == 0)) {
        stop_locked();
        result = start_locked(host, port, session);
    }
    ReleaseSRWLockExclusive(&g_api);
    return result;
}

void SV_EXPORT sv_stop(void)
{
    AcquireSRWLockExclusive(&g_api);
    stop_locked();
    ReleaseSRWLockExclusive(&g_api);
}

void SV_EXPORT sv_ptt(int down)
{
    InterlockedExchange(&g_ptt, down != 0);
}

void SV_EXPORT sv_settings(float volume, int mic_on, float mic_sens)
{
    int heard_before = volume_now() > 0.0f;
    if (!(volume >= 0.0f))
        volume = 0.0f; /* NaN too */
    if (volume > 1.0f)
        volume = 1.0f;
    if (!(mic_sens >= 0.0f))
        mic_sens = 0.0f;
    if (mic_sens > 1.0f)
        mic_sens = 1.0f;
    InterlockedExchange(&g_volume_bits, float_bits(volume));
    InterlockedExchange(&g_sens_bits, float_bits(mic_sens));
    InterlockedExchange(&g_mic_on, mic_on != 0);
    if (heard_before != (volume > 0.0f))
        InterlockedExchange(&g_hello_now, 1);
}

void SV_EXPORT sv_radio(unsigned far_mask)
{
    InterlockedExchange(&g_far, (LONG)far_mask);
}

void SV_EXPORT sv_mute(unsigned mask)
{
    if ((LONG)mask != g_muted) {
        InterlockedExchange(&g_muted, (LONG)mask);
        InterlockedExchange(&g_hello_now, 1);
    }
}

unsigned SV_EXPORT sv_talking(void)
{
    unsigned mask = 0;
    if (!g_started)
        return 0;
    if (volume_now() > 0.0f)
        mask = (unsigned)g_heard & ~(unsigned)g_muted;
    if (g_ptt && g_mic_on && !g_capped) {
        LONG slot = g_slot;
        mask |= slot >= 0 && slot < SV_SLOTS ? 1u << slot : 1u << 31;
    }
    return mask;
}

int SV_EXPORT sv_slot(void)
{
    return (int)g_slot;
}

unsigned SV_EXPORT sv_voice_id(int slot)
{
    return slot >= 0 && slot < SV_SLOTS ? (unsigned)g_ids[slot] : 0;
}

int SV_EXPORT sv_last_speaker(void)
{
    /* Recorded only from slots that were not muted when heard; a slot muted
     * since stays the answer (so a key may unmute it again). */
    return (int)g_last_speaker;
}
