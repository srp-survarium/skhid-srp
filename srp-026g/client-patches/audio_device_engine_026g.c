/* Patch 100 in the game: audio_device_026g.c's platform through the
 * engine-function library (engine_026g.tsv), and the sound thread's hook.
 * patch_026g.py's AUDIO_DEVICE_HOOKS call audio_device_create (from
 * initialize_xaudio's two XAudio2Create calls) and audio_device_orders
 * (sound_world::tick's process_orders call). */
#include "engine_026g.h"
#include "audio_device_026g.h"

#define HEAP_ZERO_MEMORY 8u
#define CLSCTX_INPROC_SERVER 1u

static const struct ad_guid clsid_mm_enumerator =
    {0xbcde0395, 0xe52f, 0x467c, {0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e}};
static const struct ad_guid iid_mm_enumerator =
    {0xa95664d2, 0x9614, 0x4f35, {0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6}};

void *ad_alloc(ad_u32 size) { return engine_heap_alloc(engine_get_process_heap(), HEAP_ZERO_MEMORY, size); }

void ad_free(void *memory)
{
    if (memory) engine_heap_free(engine_get_process_heap(), 0, memory);
}

ad_hresult ad_xaudio2_create(void **engine, ad_u32 flags) { return (ad_hresult)engine_xaudio2_create(engine, flags); }

ad_hresult ad_mm_enumerator(void **enumerator)
{
    return (ad_hresult)engine_co_create_instance(&clsid_mm_enumerator, 0, CLSCTX_INPROC_SERVER,
                                                 &iid_mm_enumerator, enumerator);
}

ad_u64 ad_now(void)
{
    ad_u64 count = 0;
    engine_query_performance_counter((unsigned int *)&count);
    return count;
}

ad_u64 ad_frequency(void)
{
    ad_u64 frequency = 0;
    engine_query_performance_frequency((unsigned int *)&frequency);
    return frequency;
}

void ad_log(const char *what, ad_u32 value)
{
    char line[200];
    ad_u32 n = 0;
    for (const char *p = "srp: audio_device: "; *p; ++p) line[n++] = *p;
    for (const char *p = what; *p && n < sizeof line - 16; ++p) line[n++] = *p;
    line[n++] = ' ';
    line[n++] = '0';
    line[n++] = 'x';
    for (int shift = 28; shift >= 0; shift -= 4) line[n++] = "0123456789abcdef"[(value >> shift) & 0xF];
    line[n++] = '\r';
    line[n++] = '\n';
    /* FILE_APPEND_DATA, share read and write, OPEN_ALWAYS, normal: vostok::log's */
    void *file = engine_create_file_a("srp_patches.log", 4, 3, 0, 4, 0x80, 0);
    if (file == (void *)-1) return;
    unsigned int written = 0;
    engine_write_file(file, line, n, &written, 0);
    engine_close_handle(file);
}

/* sound_world::tick's `push edi / call process_orders` (0x59AAB): the
 * sound thread, every tick, before the orders run. */
void __attribute__((stdcall)) audio_device_orders(struct sound_world *world)
{
    ad_tick();
    engine_process_sound_orders(world);
}
