/* A smoke test of the built skhid_voice.dll on Windows or under Wine (never
 * the game): the exports resolve, and against a fake voice server on
 * 127.0.0.1 the DLL says HELLO, takes WELCOME, sends a HELLO at once for a
 * mute and for "Disable voice chat", hears a relayed speaker, and stops.
 * A missing microphone or output device only makes voice silent.
 *
 *   nix develop ./srp-026g -c bash srp-026g/client-voice/build.sh
 *   nix build ./srp-026g#skhid-voice-smoke
 *   (or: i686-w64-mingw32-gcc -O2 -o smoke_voice.exe smoke_voice.c -lws2_32)
 *   wine smoke_voice.exe path\to\skhid_voice.dll     (exit status 0: passed)
 */
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <windows.h>

#include <stdio.h>
#include <string.h>

typedef int(__cdecl *version_fn)(void);
typedef int(__cdecl *start_fn)(const char *, unsigned, unsigned);
typedef void(__cdecl *void_fn)(void);
typedef void(__cdecl *int_fn)(int);
typedef void(__cdecl *settings_fn)(float, int, float);
typedef void(__cdecl *mask_fn)(unsigned);
typedef unsigned(__cdecl *talking_fn)(void);
typedef int(__cdecl *slot_fn)(void);
typedef unsigned(__cdecl *voice_id_fn)(int);

static int failures;
#define CHECK(cond, what)                                                                  \
    do {                                                                                   \
        if (!(cond)) {                                                                     \
            failures++;                                                                    \
            printf("FAIL: %s\n", what);                                                    \
        } else {                                                                           \
            printf("ok: %s\n", what);                                                      \
        }                                                                                  \
    } while (0)

static SOCKET server;
static struct sockaddr_in client;

/* The next datagram of `type` within `ms`: its length, or -1. */
static int expect(unsigned char type, unsigned char *buf, int cap, DWORD ms)
{
    DWORD until = GetTickCount() + ms;
    while ((int)(until - GetTickCount()) > 0) {
        int len = sizeof client;
        int got = recvfrom(server, (char *)buf, cap, 0, (struct sockaddr *)&client, &len);
        if (got >= 3 && buf[0] == 0x56 && buf[1] == 1 && buf[2] == type)
            return got;
    }
    return -1;
}

static void reply(const unsigned char *data, int len)
{
    sendto(server, (const char *)data, len, 0, (struct sockaddr *)&client, sizeof client);
}

int main(int argc, char **argv)
{
    WSADATA wsa;
    HMODULE dll;
    struct sockaddr_in at;
    int len = sizeof at;
    DWORD timeout = 50;
    unsigned char buf[512];
    int got;

    dll = LoadLibraryA(argc > 1 ? argv[1] : "skhid_voice.dll");
    CHECK(dll != NULL, "the DLL loads");
    if (!dll)
        return 1;
    version_fn sv_version = (version_fn)GetProcAddress(dll, "sv_version");
    start_fn sv_start = (start_fn)GetProcAddress(dll, "sv_start");
    void_fn sv_stop = (void_fn)GetProcAddress(dll, "sv_stop");
    int_fn sv_ptt = (int_fn)GetProcAddress(dll, "sv_ptt");
    settings_fn sv_settings = (settings_fn)GetProcAddress(dll, "sv_settings");
    mask_fn sv_radio = (mask_fn)GetProcAddress(dll, "sv_radio");
    mask_fn sv_mute = (mask_fn)GetProcAddress(dll, "sv_mute");
    talking_fn sv_talking = (talking_fn)GetProcAddress(dll, "sv_talking");
    slot_fn sv_slot = (slot_fn)GetProcAddress(dll, "sv_slot");
    slot_fn sv_last_speaker = (slot_fn)GetProcAddress(dll, "sv_last_speaker");
    voice_id_fn sv_voice_id = (voice_id_fn)GetProcAddress(dll, "sv_voice_id");
    CHECK(sv_version && sv_start && sv_stop && sv_ptt && sv_settings && sv_radio && sv_mute &&
              sv_talking && sv_slot && sv_last_speaker && sv_voice_id,
          "every export resolves");
    if (failures)
        return 1;
    CHECK(sv_version() == 1, "sv_version is 1");
    sv_stop(); /* not started: nothing */
    CHECK(sv_talking() == 0 && sv_slot() == -1 && sv_last_speaker() == -1, "idle state");

    WSAStartup(MAKEWORD(2, 2), &wsa);
    server = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    memset(&at, 0, sizeof at);
    at.sin_family = AF_INET;
    at.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    bind(server, (struct sockaddr *)&at, sizeof at);
    getsockname(server, (struct sockaddr *)&at, &len);
    setsockopt(server, SOL_SOCKET, SO_RCVTIMEO, (const char *)&timeout, sizeof timeout);

    CHECK(sv_start(NULL, 1, 1) != 0 && sv_start("127.0.0.1", 0, 1) != 0, "bad arguments refused");
    CHECK(sv_start("127.0.0.1", ntohs(at.sin_port), 0xCAFE0001u) == 0, "sv_start");
    CHECK(sv_start("127.0.0.1", ntohs(at.sin_port), 0xCAFE0001u) == 0, "sv_start again, same");
    got = expect(0x01, buf, sizeof buf, 1000);
    CHECK(got == 12 && buf[3] == 0x01 && buf[4] == 0x00 && buf[5] == 0xFE && buf[6] == 0xCA &&
              buf[7] == 1 && buf[8] == 0 && buf[9] == 0 && buf[10] == 0 && buf[11] == 0,
          "HELLO: session, hears, no mutes");

    {
        unsigned char welcome[] = {0x56, 1, 0x81, 5};
        int i;
        reply(welcome, sizeof welcome);
        for (i = 0; i < 50 && sv_slot() != 5; i++)
            Sleep(10);
        CHECK(sv_slot() == 5, "WELCOME: own slot 5");
    }

    {
        unsigned char roster[] = {0x56, 1, 0x90, 2, 2, 0x11, 0, 0, 0, 5, 0x22, 0, 0, 0};
        reply(roster, sizeof roster);
        for (int i = 0; i < 50 && sv_voice_id(2) != 0x11; i++) Sleep(10);
        CHECK(sv_voice_id(2) == 0x11 && sv_voice_id(5) == 0x22, "ROSTER: stable account ids");
    }

    sv_mute(1u << 3);
    got = expect(0x01, buf, sizeof buf, 300);
    CHECK(got == 12 && buf[8] == 0x08, "a mute sends a HELLO at once, the mask in it");

    /* A teammate (slot 2) talks: START and an AUDIO of two (garbage) frames. */
    {
        unsigned char start[] = {0x56, 1, 0x82, 2, 7, 0, 0, 0, 0, 0};
        unsigned char audio[] = {0x56, 1, 0x83, 2, 7, 0, 0, 0, 2, 3, 0x08, 1, 2, 3, 0x08, 1, 2};
        unsigned char muted[] = {0x56, 1, 0x82, 3, 9, 0, 0, 0, 0, 0};
        int i;
        reply(muted, sizeof muted);
        reply(start, sizeof start);
        reply(audio, sizeof audio);
        for (i = 0; i < 50 && !(sv_talking() & (1u << 2)); i++)
            Sleep(10);
        CHECK(sv_talking() & (1u << 2), "slot 2 is heard");
        CHECK(!(sv_talking() & (1u << 3)), "muted slot 3 is not");
        CHECK(sv_last_speaker() == 2, "the last speaker is 2");
    }

    /* A new account reuses slot 2: its low first utterance id must not be
     * rejected by the old account's closed/open utterance history. */
    {
        unsigned char roster[] = {0x56, 1, 0x90, 2, 2, 0x12, 0, 0, 0, 5, 0x22, 0, 0, 0};
        unsigned char start[] = {0x56, 1, 0x82, 2, 1, 0, 0, 0, 0, 0};
        reply(roster, sizeof roster);
        for (int i = 0; i < 50 && (sv_voice_id(2) != 0x12 || (sv_talking() & (1u << 2))); i++) Sleep(10);
        CHECK(sv_voice_id(2) == 0x12 && !(sv_talking() & (1u << 2)), "changed roster clears old speaker");
        reply(start, sizeof start);
        for (int i = 0; i < 50 && !(sv_talking() & (1u << 2)); i++) Sleep(10);
        CHECK(sv_talking() & (1u << 2), "late joiner can start with a lower utterance id");
    }

    sv_radio(1u << 2);
    sv_ptt(1);
    Sleep(100);
    got = expect(0x02, buf, sizeof buf, 300);
    CHECK(((sv_talking() & (1u << 5)) != 0) == (got > 0),
          "own talking bit means capture actually started");
    printf("note: START %s (it needs a microphone)\n", got > 0 ? "sent" : "not sent");
    sv_ptt(0);
    if (got > 0) {
        got = expect(0x04, buf, sizeof buf, 300);
        CHECK(got == 11, "END after the release");
    }

    /* "Disable voice chat": hears 0 at once, nothing talking, PTT silent. */
    sv_settings(0.0f, 0, 0.5f);
    got = expect(0x01, buf, sizeof buf, 300);
    CHECK(got == 12 && buf[7] == 0, "disabled: a HELLO at once, hears 0");
    sv_ptt(1);
    Sleep(100);
    CHECK(sv_talking() == 0, "disabled: nothing talking, not even oneself");
    CHECK(expect(0x02, buf, sizeof buf, 200) < 0, "disabled: push-to-talk sends no START");
    sv_ptt(0);
    sv_settings(1.0f, 1, 0.5f);
    got = expect(0x01, buf, sizeof buf, 300);
    CHECK(got == 12 && buf[7] == 1, "enabled again: a HELLO at once, hears 1");

    /* Push-to-talk held when the match ends, its release never given: the
     * next session starts released (no microphone, no START). */
    sv_ptt(1);
    Sleep(50);
    sv_stop();
    CHECK(sv_slot() == -1 && sv_talking() == 0, "stopped");
    sv_stop();
    CHECK(sv_start("127.0.0.1", ntohs(at.sin_port), 0xCAFE0002u) == 0, "started again");
    got = expect(0x01, buf, sizeof buf, 1000);
    CHECK(got == 12 && buf[3] == 0x02, "HELLO of the new session");
    {
        unsigned char welcome[] = {0x56, 1, 0x81, 5};
        reply(welcome, sizeof welcome);
    }
    Sleep(200);
    CHECK(!(sv_talking() & ((1u << 5) | (1u << 31))), "a key held across the stop is released: not talking");
    CHECK(expect(0x02, buf, sizeof buf, 400) < 0, "a key held across the stop is released: no START");
    sv_stop();

    printf(failures ? "%d FAILED\n" : "smoke test passed\n", failures);
    return failures != 0;
}
