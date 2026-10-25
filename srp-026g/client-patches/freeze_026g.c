/* Patch 71: the freeze detector (freeze_026g.asm has the hooks; client_patch.md
 * section 3.58; the rules: ../docs/notes/freeze-detector.md).
 *
 * The hooks count what a frame does (freeze_026g.asm's frz_totals: how many
 * times each hooked function ran and, for the timed ones, the microseconds it
 * took, both ever growing). frz_frame runs on the main thread once a frame,
 * after the swap chain's present: it measures the frame with
 * QueryPerformanceCounter, takes what each total grew by since the last frame
 * (the frame's own counters), and when the frame took at least srp_freeze_ms
 * and three times the median of the last 64 frames, writes one line of the
 * game's log with the frame, the match's state and those counters. From a
 * match's loading or start until the lobby again it keeps the frames in the
 * world (a histogram) and the three longest, and at the lobby writes the
 * summary. frz_logic_done runs on the logic thread after every logic tick and
 * names the state (the game's own fields, read where the game writes them).
 *
 * Nothing here is read by the game: what a match computes is the same with or
 * without it. Every address is a VA of the original exe (image base 0x10000;
 * never rebased); tools/re-kit/v026g has a row for each.
 */
typedef unsigned char u8;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef long long s64;

#define AT(base, offset, type) (*(type *)((u8 *)(base) + (offset)))
#define STDCALL __attribute__((stdcall))

/* ---- the game ---------------------------------------------------------- */

#define QUERY_COUNTER 0x870158     /* [import] QueryPerformanceCounter */
#define QUERY_FREQUENCY 0x870150   /* [import] QueryPerformanceFrequency */
#define ENGINE_GAME 0x2D0          /* engine_world: the game (logic_tick's [ebx+2D0h], is_loading its vtable +18h) */
#define GAME_WORLD 0x18            /* game: its game_world, in place (game::game 0x1E909F) */
#define GAME_LOADING 0x4E38        /* game: u8, what game::is_loading (0x1ED870) answers */
#define GAME_LOBBY 0x5390          /* game: the lobby menu (switch_to_lobby 0x1ED222) */
#define GAME_SCENE 0x539C          /* game: the scene shown: the lobby, the login or game+18h (0x1ED1C2) */
#define GAME_CLIENT 0x53AC         /* game: the network client (set_network_client 0x1ED6EA) */
#define CLIENT_VTABLE 0x9FFB14     /* network_client (0x2199C6); a replay's client is another */
#define CLIENT_TIME_STATE 0x1F1C60 /* network_client: the time rule's state the camera keeps (0x267AFB):
                                      1 waiting, 2 countdown, 3 play; 0 out of a match */

/* ---- freeze_026g.asm --------------------------------------------------- */

enum {
    F_LOGIC, F_PRESENT, F_SIM, F_TICKS, F_ROLLBACKS, F_MESSAGES, F_DATAGRAMS, F_BAKES, F_LOADS, F_WAITS,
    F_COOKS, F_CALLBACKS, F_PARTICLES, F_INVOKES, F_ADVANCE, F_FLASH, F_SHOTS, F_HITS, F_DECALS, F_IMPACTS,
    F_SOUNDS, F_RESIM_TICKS, F_RESIM_SHOTS, F_KILLS, F_KILL_ROWS, F_COUNT
};
struct frz_total {
    u32 count;                     /* calls, ever */
    u32 us;                        /* their time, ever (the timed functions') */
};
extern volatile struct frz_total frz_totals[F_COUNT];
extern volatile u32 frz_state;     /* the state, frz_logic_done's */
extern volatile u32 frz_world;     /* the engine world, logic_tick's argument */
extern volatile u32 frz_tsc_mult;  /* 2^32 / TSC ticks per microsecond: the hooks' clock */
extern volatile u32 frz_limit_ms;  /* srp_freeze_ms */
extern volatile u32 frz_sim_time;  /* the latest time a world ticked to (freeze_tick's) */
void frz_log(const char *text);    /* one line of the game's log, channel "game", info */

enum { S_MENU, S_LOBBY, S_LOADING, S_WAITING, S_COUNTDOWN, S_PLAY, S_REPLAY, S_COUNT };

/* ---- the state (logic thread) ------------------------------------------ */

void frz_logic_done(void)
{
    u8 *world = (u8 *)frz_world;
    u8 *game = world ? AT(world, ENGINE_GAME, u8 *) : 0;
    u32 state = S_MENU;
    if (game) {
        u8 *scene = AT(game, GAME_SCENE, u8 *);
        if (AT(game, GAME_LOADING, u8)) {
            state = S_LOADING;
        } else if (scene == game + GAME_WORLD) {
            u8 *client = AT(game, GAME_CLIENT, u8 *);
            if (!client || AT(client, 0, u32) != CLIENT_VTABLE) {
                state = S_REPLAY;
            } else {
                u32 time = AT(client, CLIENT_TIME_STATE, u32);
                state = time == 3 ? S_PLAY : time == 2 ? S_COUNTDOWN : S_WAITING;
            }
        } else if (scene && scene == AT(game, GAME_LOBBY, u8 *)) {
            state = S_LOBBY;
        }
    }
    /* Out of a world, the next one's ticks start again from 0 (the logic
     * thread alone ticks a world, so no tick runs while this is written). */
    if (state < S_WAITING)
        frz_sim_time = 0;
    frz_state = state;
}

/* ---- the frames (main thread) ------------------------------------------ */

enum {
    FINE = 1000,                   /* 0.1 ms buckets up to 100 ms */
    COARSE = 900,                  /* 1 ms buckets up to 1 s */
    BUCKETS = FINE + COARSE + 1,   /* and one for longer */
    RING = 64,                     /* the frames the median is of */
    WORST = 3,
    LINES = 100,                   /* long-frame lines from a match's start to the lobby, or in the lobby */
    LINES_LOADING = 20,            /* of them, while loading */
    LINE = 1024,
};

struct frame {
    u32 us;                        /* the frame, by QueryPerformanceCounter */
    u32 state;
    u32 limit;                     /* the limit it was over */
    u32 count[F_COUNT];            /* what each total grew by in the frame */
    u32 time[F_COUNT];
};

static struct {
    u32 freq;                      /* QueryPerformanceFrequency, 0 before the first frame */
    u64 qpc0, tsc0, last;
    double us_per_qpc;
    struct frz_total seen[F_COUNT];
    u32 ring[RING], at, median;
    u32 session;                   /* 1 from a match's loading or start to the lobby */
    u32 lines, lines_loading, told;
    u32 frames, ms, carry, max;    /* frames in the world, their time */
    u32 bucket[BUCKETS];
    u32 longs, by_state[S_COUNT];
    struct frame worst[WORST];
    u32 worsts;
    char line[LINE];
} frz;

static const char *state_name(u32 state)
{
    switch (state) {
    case S_LOBBY: return "lobby";
    case S_LOADING: return "loading";
    case S_WAITING: return "waiting";
    case S_COUNTDOWN: return "countdown";
    case S_PLAY: return "play";
    case S_REPLAY: return "replay";
    default: return "menu";
    }
}

static char *put(char *p, const char *s)
{
    while (*s)
        *p++ = *s++;
    return p;
}

static char *put_u(char *p, u32 v)
{
    char digits[10];
    int n = 0;
    do {
        digits[n++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    while (n)
        *p++ = digits[--n];
    return p;
}

static char *put_ms(char *p, u32 us)
{
    p = put_u(p, us / 1000);
    *p++ = '.';
    *p++ = (char)('0' + us / 100 % 10);
    return p;
}

static char *put_timed(char *p, const char *name, const struct frame *f, u32 i)
{
    p = put(p, name);
    p = put_u(p, f->count[i]);
    p = put(p, " in ");
    p = put_ms(p, f->time[i]);
    return put(p, " ms");
}

static char *put_count(char *p, const char *name, const struct frame *f, u32 i)
{
    return put_u(put(p, name), f->count[i]);
}

static char *put_time(char *p, const char *name, const struct frame *f, u32 i)
{
    return put(put_ms(put(p, name), f->time[i]), " ms");
}

/* "83.4 ms in play (limit 40.0 ms): logic 1 in 71.2 ms, ..." The
 * simulation first: how long it took, its ticks and how many of them were
 * played again after a rollback, the bullets fired (again) in them and what
 * they hit and left; then the rest. */
static char *put_frame(char *p, const struct frame *f)
{
    p = put_ms(p, f->us);
    p = put(p, " ms in ");
    p = put(p, state_name(f->state));
    p = put(p, " (limit ");
    p = put_ms(p, f->limit);
    p = put(p, " ms): ");
    p = put_timed(p, "logic ", f, F_LOGIC);
    p = put_time(p, ", present ", f, F_PRESENT);
    p = put_timed(p, ", sim ", f, F_SIM);
    p = put_count(p, ", ticks ", f, F_TICKS);
    p = put_count(p, " (resimulated ", f, F_RESIM_TICKS);
    p = put_count(p, "), rollbacks ", f, F_ROLLBACKS);
    p = put_count(p, ", shots ", f, F_SHOTS);
    p = put_count(p, " (replayed ", f, F_RESIM_SHOTS);
    p = put_count(p, "), hits ", f, F_HITS);
    p = put_count(p, ", decals ", f, F_DECALS);
    p = put_count(p, ", impacts ", f, F_IMPACTS);
    p = put_count(p, ", sounds ", f, F_SOUNDS);
    p = put_count(p, ", kills ", f, F_KILLS);
    p = put_count(p, ", feed rows ", f, F_KILL_ROWS);
    p = put_count(p, ", messages ", f, F_MESSAGES);
    p = put_count(p, ", datagrams ", f, F_DATAGRAMS);
    p = put_timed(p, ", bakes ", f, F_BAKES);
    p = put_timed(p, ", baked loads ", f, F_LOADS);
    p = put_timed(p, ", sync loads ", f, F_WAITS);
    p = put_count(p, ", cooks ", f, F_COOKS);
    p = put_timed(p, ", callbacks ", f, F_CALLBACKS);
    p = put_count(p, ", particles ", f, F_PARTICLES);
    p = put_count(p, ", invokes ", f, F_INVOKES);
    p = put_timed(p, ", advance ", f, F_ADVANCE);
    return put_time(p, ", flash render ", f, F_FLASH);
}

static void log_line(char *end)
{
    *end = 0;
    frz_log(frz.line);
}

static int in_world(u32 state)
{
    return state >= S_WAITING;
}

static u32 bucket_of(u32 us)
{
    if (us < FINE * 100)
        return us / 100;
    if (us < 1000000)
        return FINE + (us - FINE * 100) / 1000;
    return BUCKETS - 1;
}

/* The bucket's upper edge, at most the longest frame. */
static u32 bucket_us(u32 b)
{
    u32 us = b < FINE ? (b + 1) * 100 : b < BUCKETS - 1 ? FINE * 100 + (b - FINE + 1) * 1000 : frz.max;
    return us < frz.max ? us : frz.max;
}

/* The frame time p per mille of the world's frames are at or below. */
static u32 percentile(u32 p)
{
    u32 want = frz.frames / 1000 * p + (frz.frames % 1000 * p + 999) / 1000, seen = 0;
    for (u32 b = 0; b < BUCKETS; b++) {
        seen += frz.bucket[b];
        if (seen >= want && seen)
            return bucket_us(b);
    }
    return frz.max;
}

static void start_session(void)
{
    volatile u32 *bucket = frz.bucket;
    for (u32 b = 0; b < BUCKETS; b++)
        bucket[b] = 0;
    for (u32 s = 0; s < S_COUNT; s++)
        frz.by_state[s] = 0;
    frz.frames = frz.ms = frz.carry = frz.max = frz.longs = frz.worsts = 0;
    frz.lines = frz.lines_loading = frz.told = 0;
    frz.session = 1;
}

static void summary(void)
{
    char *p = put(frz.line, "srp freeze summary: ");
    p = put_u(p, frz.ms / 1000);
    *p++ = '.';
    *p++ = (char)('0' + frz.ms / 100 % 10);
    p = put(p, " s in the world, ");
    p = put_u(p, frz.frames);
    p = put(p, " frames: p50 ");
    p = put_ms(p, percentile(500));
    p = put(p, " ms, p95 ");
    p = put_ms(p, percentile(950));
    p = put(p, " ms, p99 ");
    p = put_ms(p, percentile(990));
    p = put(p, " ms, max ");
    p = put_ms(p, frz.max);
    log_line(put(p, " ms"));
    p = put(frz.line, "srp freeze summary: ");
    p = put_u(p, frz.longs);
    p = put(p, " long frames (srp_freeze_ms ");
    p = put_u(p, frz_limit_ms);
    p = put(p, "): loading ");
    p = put_u(p, frz.by_state[S_LOADING]);
    p = put(p, ", waiting ");
    p = put_u(p, frz.by_state[S_WAITING]);
    p = put(p, ", countdown ");
    p = put_u(p, frz.by_state[S_COUNTDOWN]);
    p = put(p, ", play ");
    p = put_u(p, frz.by_state[S_PLAY]);
    p = put(p, ", replay ");
    log_line(put_u(p, frz.by_state[S_REPLAY]));
    for (u32 i = 0; i < frz.worsts; i++) {
        p = put(frz.line, "srp freeze summary: worst ");
        p = put_u(p, i + 1);
        p = put(p, ": ");
        log_line(put_frame(p, &frz.worst[i]));
    }
    frz.session = 0;
    frz.lines = frz.lines_loading = frz.told = 0;
}

static void keep_worst(const struct frame *f)
{
    u32 at = frz.worsts < WORST ? frz.worsts++ : WORST;
    while (at > 0 && frz.worst[at - 1].us < f->us) {
        if (at < WORST)
            frz.worst[at] = frz.worst[at - 1];
        at--;
    }
    if (at < WORST)
        frz.worst[at] = *f;
}

static void long_frame(const struct frame *f)
{
    frz.longs++;
    frz.by_state[f->state]++;
    if (in_world(f->state))
        keep_worst(f);
    u32 *lines = f->state == S_LOADING ? &frz.lines_loading : &frz.lines;
    u32 most = f->state == S_LOADING ? LINES_LOADING : LINES;
    if (*lines < most) {
        ++*lines;
        log_line(put_frame(put(frz.line, "srp freeze: "), f));
    } else if (!frz.told) {
        frz.told = 1;
        frz_log("srp freeze: more long frames than are logged; the summary counts them all");
    }
}

static void median_of_ring(void)
{
    u32 sorted[RING];
    for (u32 i = 0; i < RING; i++) {
        u32 v = frz.ring[i], j = i;
        for (; j > 0 && sorted[j - 1] > v; j--)
            sorted[j] = sorted[j - 1];
        sorted[j] = v;
    }
    frz.median = sorted[RING / 2];
}

/* After the swap chain's present, on the main thread. */
void frz_frame(void)
{
    u64 now, frequency;
    u64 tsc = __builtin_ia32_rdtsc();
    ((int (STDCALL *)(u64 *)) * (u32 *)QUERY_COUNTER)(&now);
    if (!frz.freq) {
        ((int (STDCALL *)(u64 *)) * (u32 *)QUERY_FREQUENCY)(&frequency);
        if (!frequency || frequency > 0xFFFFFFFFu)
            return;
        frz.freq = (u32)frequency;
        frz.us_per_qpc = 1000000.0 / (double)(s64)frequency;
        frz.qpc0 = frz.last = now;
        frz.tsc0 = tsc;
        for (u32 i = 0; i < F_COUNT; i++) {
            frz.seen[i].count = frz_totals[i].count;
            frz.seen[i].us = frz_totals[i].us;
        }
        return;
    }
    /* The hooks' clock: TSC ticks per microsecond, over all the time so far. */
    if (now - frz.qpc0 >= frz.freq) {
        double us = (double)(s64)(now - frz.qpc0) * frz.us_per_qpc;
        double ticks = (double)(s64)(tsc - frz.tsc0);
        if (ticks > us * 16)
            frz_tsc_mult = (u32)(int)(4294967296.0 * us / ticks + 0.5);
    }
    struct frame f;
    double span = (double)(s64)(now - frz.last) * frz.us_per_qpc;
    f.us = span < 2000000000.0 ? (u32)(int)span : 2000000000u;
    frz.last = now;
    f.state = frz_state;
    for (u32 i = 0; i < F_COUNT; i++) {
        u32 count = frz_totals[i].count, us = frz_totals[i].us;
        f.count[i] = count - frz.seen[i].count;
        f.time[i] = us - frz.seen[i].us;
        frz.seen[i].count = count;
        frz.seen[i].us = us;
    }
    /* A match from its loading or start to the lobby. */
    if (f.state <= S_LOBBY) {
        if (frz.session) {
            if (frz.frames)
                summary();
            else
                frz.session = 0;
            frz.lines = frz.lines_loading = frz.told = 0;
        }
    } else if (!frz.session) {
        start_session();
    }
    if (in_world(f.state) && frz.session) {
        frz.frames++;
        frz.carry += f.us;
        frz.ms += frz.carry / 1000;
        frz.carry %= 1000;
        if (f.us > frz.max)
            frz.max = f.us;
        frz.bucket[bucket_of(f.us)]++;
    }
    /* Long: at least srp_freeze_ms (0: never) and three times the median. */
    u32 limit = frz_limit_ms * 1000;
    if (limit < frz.median * 3)
        limit = frz.median * 3;
    f.limit = limit;
    if (frz_limit_ms && f.us >= limit)
        long_frame(&f);
    frz.ring[frz.at++ % RING] = f.us;
    if (frz.at % RING == 0)
        median_of_ring();
}
