/* A smoke test of the built skhid_voice.dll on Windows or under Wine (never
 * the game): the exports resolve, and against a fake voice server on
 * 127.0.0.1 the DLL says HELLO, takes WELCOME, sends a HELLO at once for a
 * mute and for "Disable voice chat", hears a relayed speaker, and stops.
 * A missing microphone or output device only makes voice silent. The options'
 * microphone test starts with no match and in one, stops at a second press,
 * at sv_test_stop, at a push-to-talk and at the match's end; without a
 * microphone or a playback device it says so for a moment. With `full` after
 * the DLL's path a whole test runs too, 10 s recorded and played back: it
 * opens the default microphone and plays on the default device. The options'
 * microphone list is printed, and a choice not plugged in opens the default.
 *
 *   nix develop ./srp-026g -c bash srp-026g/client-voice/build.sh
 *   nix build ./srp-026g#skhid-voice-smoke
 *   (or: i686-w64-mingw32-gcc -O2 -o smoke_voice.exe smoke_voice.c -lws2_32)
 *   wine smoke_voice.exe path\to\skhid_voice.dll [full]     (exit status 0: passed)
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
typedef void(__cdecl *hear_fn)(int, float, float, int);
typedef unsigned(__cdecl *talking_fn)(void);
typedef int(__cdecl *slot_fn)(void);
typedef unsigned(__cdecl *voice_id_fn)(int);
typedef void(__cdecl *press_fn)(float, float);
/* voice-abi's Microphone: an endpoint ID and a name (UTF-8), each NUL-terminated in its room. */
struct microphone {
    char id[128];
    char name[256];
};
typedef unsigned(__cdecl *microphones_fn)(struct microphone *, unsigned);
typedef void(__cdecl *choose_fn)(const char *);

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

/* The microphone test's states (voice-abi's MicTest: the state in the low
 * byte of sv_test_state's word, the seconds left in the next). */
enum { TEST_IDLE, TEST_RECORDING, TEST_PLAYING, TEST_NO_MICROPHONE, TEST_NO_PLAYBACK };

/* Waits up to `ms` for the microphone test to be idle (`idle` 1) or not to
 * be (0); its word then. */
static unsigned test_within(talking_fn sv_test_state, int idle, DWORD ms)
{
    DWORD until = GetTickCount() + ms;
    unsigned state = sv_test_state();
    while ((state == TEST_IDLE) != (idle != 0) && (int)(until - GetTickCount()) > 0) {
        Sleep(5);
        state = sv_test_state();
    }
    return state;
}

/* A whole test: every change of its word within 25 s, to idle again. */
static void whole_test(press_fn sv_test_press, talking_fn sv_test_state)
{
    DWORD from = GetTickCount();
    unsigned last = TEST_IDLE, recorded = 0, played = 0, changes = 0;
    sv_test_press(1.0f, 0.5f);
    while (GetTickCount() - from < 25000) {
        unsigned state = sv_test_state();
        if (state != last) {
            printf("  %5lu ms: state %u, %u s left\n", (unsigned long)(GetTickCount() - from), state & 0xFF, state >> 8);
            fflush(stdout); /* each change as it comes, for whoever watches or listens */
            if ((state & 0xFF) == TEST_RECORDING)
                recorded++;
            if ((state & 0xFF) == TEST_PLAYING)
                played++;
            changes++;
            last = state;
            if (state == TEST_IDLE)
                break;
        }
        Sleep(5);
    }
    CHECK(last == TEST_IDLE && changes > 0, "a whole test ends idle");
    if (recorded) {
        CHECK(recorded == 10, "recording: each of its 10 seconds left is told");
        CHECK(played >= 9 && played <= 10, "playing: the seconds left of what was recorded");
    } else {
        printf("note: no whole test (a microphone and a playback device are needed)\n");
    }
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
    hear_fn sv_hear = (hear_fn)GetProcAddress(dll, "sv_hear");
    mask_fn sv_mute = (mask_fn)GetProcAddress(dll, "sv_mute");
    talking_fn sv_talking = (talking_fn)GetProcAddress(dll, "sv_talking");
    slot_fn sv_slot = (slot_fn)GetProcAddress(dll, "sv_slot");
    voice_id_fn sv_voice_id = (voice_id_fn)GetProcAddress(dll, "sv_voice_id");
    press_fn sv_test_press = (press_fn)GetProcAddress(dll, "sv_test_press");
    void_fn sv_test_stop = (void_fn)GetProcAddress(dll, "sv_test_stop");
    talking_fn sv_test_state = (talking_fn)GetProcAddress(dll, "sv_test_state");
    microphones_fn sv_microphones = (microphones_fn)GetProcAddress(dll, "sv_microphones");
    choose_fn sv_choose_microphone = (choose_fn)GetProcAddress(dll, "sv_choose_microphone");
    static struct microphone microphones[16];
    unsigned listed, i;
    unsigned test;
    CHECK(sv_version && sv_start && sv_stop && sv_ptt && sv_settings && sv_hear && sv_mute &&
              sv_talking && sv_slot && sv_voice_id && sv_test_press && sv_test_stop && sv_test_state &&
              sv_microphones && sv_choose_microphone,
          "every export resolves");
    if (failures)
        return 1;
    CHECK(sv_version() == 5, "sv_version is 5");
    sv_stop(); /* not started: nothing */
    CHECK(sv_talking() == 0 && sv_slot() == -1, "idle state");

    /* The options' Microphone list: the capture devices by their endpoint IDs and whole
     * names (none where there is no audio). A choice that is not plugged in opens the
     * default: the microphone test below runs with one. */
    listed = sv_microphones(microphones, 16);
    CHECK(listed <= 16, "the microphone list fits its room");
    for (i = 0; i < listed; i++)
        printf("note: microphone %u: %s (%s)\n", i, microphones[i].name, microphones[i].id);
    CHECK(sv_microphones(NULL, 16) == 0 && sv_microphones(microphones, 0) == 0, "no room: nothing listed");
    sv_choose_microphone("{0.0.1.00000000}.{00000000-0000-0000-0000-000000000000}");

    /* The microphone test with no match (the lobby): a worker of its own,
     * which leaves when the test is over. */
    CHECK(sv_test_state() == TEST_IDLE, "the microphone test is idle");
    sv_test_stop(); /* none runs: nothing */
    sv_test_press(1.0f, 0.5f);
    test = test_within(sv_test_state, 0, 3000);
    printf("note: the microphone test %s\n",
           (test & 0xFF) == TEST_RECORDING ? "records" : (test & 0xFF) == TEST_NO_MICROPHONE ? "finds no microphone"
           : (test & 0xFF) == TEST_NO_PLAYBACK ? "finds no playback device" : "does not start");
    if ((test & 0xFF) == TEST_RECORDING) {
        CHECK(test == (TEST_RECORDING | 10u << 8), "a press with no match records: 10 s left");
        sv_test_press(1.0f, 0.5f);
        CHECK(test_within(sv_test_state, 1, 1000) == TEST_IDLE, "a second press stops it");
        Sleep(300); /* its worker has left */
        sv_test_press(0.5f, 1.0f);
        CHECK((test_within(sv_test_state, 0, 3000) & 0xFF) == TEST_RECORDING, "the next press is a new test");
        sv_test_stop();
        CHECK(test_within(sv_test_state, 1, 1000) == TEST_IDLE, "sv_test_stop stops it");
    } else {
        CHECK(test == TEST_NO_MICROPHONE || test == TEST_NO_PLAYBACK, "without a device the test says which is missing");
        CHECK(test_within(sv_test_state, 1, 4000) == TEST_IDLE, "and is idle again after a moment");
        sv_test_press(1.0f, 0.5f); /* the worker of the first has left: another */
        CHECK(test_within(sv_test_state, 0, 3000) == test, "the next press says so again");
        sv_test_stop();
        CHECK(test_within(sv_test_state, 1, 1000) == TEST_IDLE, "sv_test_stop takes the notice away");
    }
    if (argc > 2 && !strcmp(argv[2], "full"))
        whole_test(sv_test_press, sv_test_state);
    /* A test with no match, running as a match starts: ended by the start. */
    sv_test_press(1.0f, 0.5f);
    test_within(sv_test_state, 0, 3000);

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
    CHECK(sv_test_state() == TEST_IDLE, "a match's start ends a microphone test");
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

    sv_hear(2, 0.0f, 0.0f, 1);   /* the radio */
    sv_hear(3, 0.9f, 0.2f, 0);   /* plain, to the left */
    sv_hear(4, 0.3f, 0.3f, 1);   /* both */
    sv_hear(99, 1.0f, 1.0f, 0);  /* past the 20: nothing */
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

    /* In the match, the first listed device chosen (the default without one). */
    sv_choose_microphone(listed ? microphones[0].id : NULL);
    /* The microphone test in a match, by the match's worker: talking wins,
     * and the match's end stops it. Nothing of it goes to the relay. */
    sv_test_press(1.0f, 0.5f);
    test = test_within(sv_test_state, 0, 3000);
    if ((test & 0xFF) == TEST_RECORDING) {
        Sleep(300);
        while (expect(0x01, buf, sizeof buf, 60) > 0) /* its HELLOs */
            ;
        CHECK(expect(0x02, buf, sizeof buf, 100) < 0 && expect(0x03, buf, sizeof buf, 100) < 0,
              "in a match the test sends no START and no AUDIO");
        sv_ptt(1);
        CHECK(test_within(sv_test_state, 1, 1000) == TEST_IDLE, "push-to-talk stops the test");
        CHECK(expect(0x02, buf, sizeof buf, 500) > 0, "and transmits as usual: START");
        sv_test_press(1.0f, 0.5f);
        Sleep(200);
        CHECK(sv_test_state() == TEST_IDLE, "held, push-to-talk keeps the test from starting");
        sv_ptt(0);
        Sleep(100);
        sv_test_press(1.0f, 0.5f);
        CHECK((test_within(sv_test_state, 0, 3000) & 0xFF) == TEST_RECORDING, "released, the test starts again");
    } else {
        CHECK(test == TEST_NO_MICROPHONE || test == TEST_NO_PLAYBACK, "in a match too the test says which device is missing");
    }
    sv_stop();
    CHECK(sv_test_state() == TEST_IDLE, "the match's end stops the test");

    printf(failures ? "%d FAILED\n" : "smoke test passed\n", failures);
    return failures != 0;
}
