/* Patch 70 (build 2777): the camouflage/decal baker keeps its textures. The
 * arena's block bake_cache.code with bake_cache_026g.asm (the hooks and the
 * game's call sequences); the tables are bake_cache.state. client_patch.md
 * section 3.57; the rules: ../docs/notes/baked-textures.md.
 *
 * The baker (camouflage_decal_texture_type_baker_cook, a static at
 * 0xA264E80) bakes an item's camouflage and decals into a texture of each
 * type (d, n, f, s) and saves it as user_data/streamed_textures/<name>.dds;
 * its list of saved textures (buffer_vector<baked_texture_data> at +230h,
 * 1024 records of 124h) lets a later query load the file instead. Stock, the
 * game deletes the whole folder when it starts (game::game 0x1E9455) and when
 * it exits (game::exit 0x1EC832), so every game bakes again every item it
 * sees, in a match on the spot; and a texture is put in the list only when
 * its bake is done, so the player objects of one player (one a kit) that ask
 * for the same texture together bake it once each.
 *
 * Here:
 * - bc_open (the game's start): the folder is kept while its stamp file says
 *   this revision; its .dds files of a whole DDS go back into the list (CRC 0:
 *   the first load takes the CRC of what it reads, bake_cache_adopt), at most
 *   SEED_FILES and SEED_BYTES of them, the rest deleted. Another revision, or
 *   no stamp: the stock delete, then bc_stamp writes the stamp.
 * - bc_lookup / bc_take: a texture whose bake or load is under way (its
 *   owner, the first query of its path) is not baked or loaded again: a later
 *   query of the path waits, and gets the owner's texture when it is made
 *   (bake_cache_baked, bake_cache_loaded); if the owner ends otherwise, the
 *   waiting queries are made again (bake_cache_finish).
 * - keep: a texture loaded from the folder is held for the game (KEPT,
 *   KEPT_BYTES, the least used let go), and a later query of its path (a
 *   respawn builds the player's model again) gets it at once.
 * Textures only: nothing a match computes.
 */
typedef unsigned int u32;
typedef unsigned char u8;
typedef void *handle;
#define STDCALL __attribute__((stdcall))

/* The game's imports (KERNEL32, their IAT slots). */
typedef handle (STDCALL *find_first_t)(const char *, void *);
typedef int (STDCALL *find_next_t)(handle, void *);
typedef int (STDCALL *find_close_t)(handle);
typedef handle (STDCALL *create_file_t)(const char *, u32, u32, void *, u32, u32, handle);
typedef int (STDCALL *read_file_t)(handle, void *, u32, u32 *, void *);
typedef int (STDCALL *write_file_t)(handle, const void *, u32, u32 *, void *);
typedef int (STDCALL *close_handle_t)(handle);
typedef int (STDCALL *delete_file_t)(const char *);
typedef int (STDCALL *create_directory_t)(const char *, void *);
typedef u32 (STDCALL *tick_count_t)(void);
#define IMPORT(type, va) (*(type volatile *)(va))
#define FIND_FIRST IMPORT(find_first_t, 0x870238)          /* FindFirstFileA */
#define FIND_NEXT IMPORT(find_next_t, 0x87023C)            /* FindNextFileA */
#define FIND_CLOSE IMPORT(find_close_t, 0x870228)          /* FindClose */
#define CREATE_FILE IMPORT(create_file_t, 0x870048)        /* CreateFileA */
#define READ_FILE IMPORT(read_file_t, 0x870064)            /* ReadFile */
#define WRITE_FILE IMPORT(write_file_t, 0x870108)          /* WriteFile */
#define CLOSE_HANDLE IMPORT(close_handle_t, 0x870040)      /* CloseHandle */
#define DELETE_FILE IMPORT(delete_file_t, 0x87021C)        /* DeleteFileA */
#define CREATE_DIRECTORY IMPORT(create_directory_t, 0x870220) /* CreateDirectoryA */
#define TICK_COUNT IMPORT(tick_count_t, 0x870090)          /* GetTickCount */
#define INVALID ((handle)-1)
#define GENERIC_READ 0x80000000u
#define GENERIC_WRITE 0x40000000u
#define SHARE_READ 1u
#define CREATE_ALWAYS 2u
#define OPEN_EXISTING 3u
#define ATTRIBUTE_NORMAL 0x80u
#define ATTRIBUTE_DIRECTORY 0x10u

/* The baker's list (the static 0xA264E80): begin, end, capacity end at +230h,
 * the bytes of all records at +4923Ch; the ages' counter. */
#define LIST_BEGIN 0xA2650B0u
#define LIST_END 0xA2650B4u
#define LIST_CAPACITY 0xA2650B8u
#define LIST_BYTES 0xA2AE0BCu
#define AGES 0x2260920u
#define RECORD 0x124u              /* path {begin, end, capacity end, text[104h], '/'}, CRC, size, age, loading */
#define TEXT 0xCu
#define TEXT_SIZE 0x104u
#define SEPARATOR 0x110u
#define CRC 0x114u
#define SIZE 0x118u
#define AGE 0x11Cu
#define LOADING 0x120u
#define REVISION 0x380866u         /* the revision byte (patch 7's `mov byte [esp+27h],REVISION`) */
#define TYPE 0x7A0u                /* a bake's parameters: the type (0 d, 1 n, 2 f, 3 s) */
#define MODEL 0x790u               /* ... the model instance whose surface it bakes */
#define REFS 0xD0u                 /* a model instance's references */

#define SEED_FILES 512u            /* half the list: the game's own bakes have the rest */
#define SEED_BYTES (512u << 20)    /* half of what remove_unwanted_files keeps (1 GiB) */
#define OWNERS 64
#define WAITERS 192
#define PER_OWNER 16               /* bake_cache_026g.asm's MAX_WAITERS */
#define STALE_MS 10000u            /* an owner this old is taken over by the next query of its path */
#define KEPT 256                   /* textures held for the game, the least used let go first ... */
#define KEPT_BYTES (256u << 20)    /* ... and at most this many bytes of their files */
#define TEXTURE_REFS 4u            /* a texture's references (0x12C6D0's inc [eax+4]) */

static const char PREFIX[] = "user_data/streamed_textures/";
static const char STAMP_NAME[] = "srp_baked_textures.txt";
static const char STAMP_TEXT[] = "Skhid baked textures, revision ";

struct owner {
    void *query;                   /* 0: free */
    void *surface;
    u32 type;
    u32 since;                     /* GetTickCount at its look-up */
    char path[TEXT_SIZE];
};

struct waiter {                    /* the asm's WAITER, 20 bytes */
    void *query;                   /* 0: free */
    void *surface;
    void *model;
    u32 type;
    void *cook;
};

struct kept {                     /* a loaded texture held for the game: a respawn's query gets it */
    void *texture;                 /* 0: free */
    u32 bytes;
    u32 used;
    char path[TEXT_SIZE];
};

static struct owner owners[OWNERS];
static struct kept kept[KEPT];
static u32 kept_bytes, kept_clock;
static struct waiter waiters[WAITERS];
static u8 waiter_of[WAITERS];      /* its owner's index */
static volatile int lock;
volatile int bc_owners;            /* the owners in the table: bake_cache_finish's quick test */
void bc_release_texture(void *texture);   /* bake_cache_026g.asm: a reference let go, as 0x12C6D0 does */

static void acquire(void)
{
    while (__atomic_exchange_n(&lock, 1, __ATOMIC_ACQUIRE))
        __asm__ volatile("pause");
}

static void release(void)
{
    __atomic_store_n(&lock, 0, __ATOMIC_RELEASE);
}

static u32 length(const char *s)
{
    u32 n = 0;
    while (s[n])
        n++;
    return n;
}

static int same(const char *a, const char *b)
{
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

/* out = a + b, NUL ended; 0 when it does not fit in `size`. */
static int join(char *out, u32 size, const char *a, u32 a_length, const char *b)
{
    u32 n = 0;
    for (u32 i = 0; i < a_length; i++) {
        if (n + 1 >= size)
            return 0;
        out[n++] = a[i];
    }
    for (; *b; b++) {
        if (n + 1 >= size)
            return 0;
        out[n++] = *b;
    }
    out[n] = 0;
    return 1;
}

/* The folder's path with a separator after it; 0 when it does not fit. */
static u32 folder(char *out, u32 size, const char *begin, const char *end)
{
    u32 n = (u32)(end - begin);
    if (!begin || end < begin || n + 2 > size)
        return 0;
    for (u32 i = 0; i < n; i++)
        out[i] = begin[i];
    if (n && out[n - 1] != '\\' && out[n - 1] != '/')
        out[n++] = '\\';
    out[n] = 0;
    return n;
}

static u32 stamp_text(char *out)
{
    u32 n = 0, revision = *(volatile u8 *)REVISION;
    for (const char *s = STAMP_TEXT; *s; s++)
        out[n++] = *s;
    char digits[3];
    u32 count = 0;
    do {
        digits[count++] = (char)('0' + revision % 10);
        revision /= 10;
    } while (revision);
    while (count)
        out[n++] = digits[--count];
    out[n++] = '\r';
    out[n++] = '\n';
    out[n] = 0;
    return n;
}

/* A whole DDS the baker writes: "DDS ", a header of 124, DXT1 or DXT5, and
 * the file 128 bytes of header and the data the header's linear size says
 * (a file cut short by a game that stopped while it wrote is not). */
static int whole(const char *path, u32 size)
{
    u8 header[128];
    u32 got = 0;
    handle file = CREATE_FILE(path, GENERIC_READ, SHARE_READ, 0, OPEN_EXISTING, ATTRIBUTE_NORMAL, 0);
    if (file == INVALID)
        return 0;
    int read = READ_FILE(file, header, sizeof header, &got, 0);
    CLOSE_HANDLE(file);
    if (!read || got != sizeof header)
        return 0;
    const u32 *h = (const u32 *)header;
    u32 width = h[4], height = h[3], linear = h[5], fourcc = h[21];
    return h[0] == 0x20534444u && h[1] == 124 && (fourcc == 0x31545844u || fourcc == 0x35545844u)
        && width >= 4 && width <= 4096 && height >= 4 && height <= 4096 && linear && size == 128 + linear;
}

/* A record of a file kept from an earlier game, at the list's end. */
static int seed(const char *name, u32 size)
{
    u8 *record = *(u8 **)LIST_END;
    if (record + RECORD > *(u8 **)LIST_CAPACITY)
        return 0;
    char *text = (char *)record + TEXT;
    if (!join(text, TEXT_SIZE, PREFIX, sizeof PREFIX - 1, name))
        return 0;
    *(char **)record = text;
    *(char **)(record + 4) = text + length(text);
    *(char **)(record + 8) = text + TEXT_SIZE;
    record[SEPARATOR] = '/';
    *(u32 *)(record + CRC) = 0;
    *(u32 *)(record + SIZE) = size;
    *(u32 *)(record + AGE) = ++*(u32 *)AGES;
    record[LOADING] = 0;
    *(u8 **)LIST_END = record + RECORD;
    *(u32 *)LIST_BYTES += size;
    return 1;
}

/* The game's start (bake_cache_start, before the stock delete): 1 to delete
 * the folder (no stamp, or another revision's), else the folder's textures go
 * back into the list and 0. begin..end the folder's physical path. */
int bc_open(const char *begin, const char *end)
{
    char path[300], expected[64], found[64];
    u8 data[320];                  /* WIN32_FIND_DATAA */
    u32 base = folder(path, sizeof path - 32, begin, end), got = 0;
    if (!base || !join(path + base, sizeof path - base, STAMP_NAME, sizeof STAMP_NAME - 1, ""))
        return 1;
    u32 want = stamp_text(expected);
    handle file = CREATE_FILE(path, GENERIC_READ, SHARE_READ, 0, OPEN_EXISTING, ATTRIBUTE_NORMAL, 0);
    if (file == INVALID)
        return 1;
    int read = READ_FILE(file, found, sizeof found - 1, &got, 0);
    CLOSE_HANDLE(file);
    if (!read || got != want)
        return 1;
    found[got] = 0;
    if (!same(found, expected))
        return 1;

    u32 files = 0, bytes = 0;
    path[base] = 0;
    join(path + base, sizeof path - base, "*.dds", 5, "");
    handle find = FIND_FIRST(path, data);
    if (find == INVALID)
        return 0;
    do {
        u32 attributes = *(u32 *)data, high = *(u32 *)(data + 28), size = *(u32 *)(data + 32);
        const char *name = (const char *)data + 44;
        if (attributes & ATTRIBUTE_DIRECTORY)
            continue;
        if (!join(path + base, sizeof path - base, name, length(name), ""))
            continue;
        if (!high && whole(path, size) && files < SEED_FILES && size <= SEED_BYTES - bytes && seed(name, size)) {
            files++;
            bytes += size;
        } else {
            DELETE_FILE(path);
        }
    } while (FIND_NEXT(find, data));
    FIND_CLOSE(find);
    return 0;
}

/* After the stock delete: the folder made again and its stamp written, but
 * only when no .dds is left in it (the delete can fail, a file held open by
 * another game: each left is deleted, and if one stays the folder is not
 * stamped, so the next start deletes it again). */
void bc_stamp(const char *begin, const char *end)
{
    char path[300], text[64];
    u8 data[320];                  /* WIN32_FIND_DATAA */
    u32 base = folder(path, sizeof path - 32, begin, end), wrote = 0;
    int left = 0;
    if (!base)
        return;
    path[base - 1] = 0;
    CREATE_DIRECTORY(path, 0);
    path[base - 1] = '\\';
    join(path + base, sizeof path - base, "*.dds", 5, "");
    handle find = FIND_FIRST(path, data);
    if (find != INVALID) {
        do {
            const char *name = (const char *)data + 44;
            if (*(u32 *)data & ATTRIBUTE_DIRECTORY)
                continue;
            if (!join(path + base, sizeof path - base, name, length(name), "") || !DELETE_FILE(path))
                left = 1;
        } while (FIND_NEXT(find, data));
        FIND_CLOSE(find);
    }
    if (left || !join(path + base, sizeof path - base, STAMP_NAME, sizeof STAMP_NAME - 1, ""))
        return;
    u32 n = stamp_text(text);
    handle file = CREATE_FILE(path, GENERIC_WRITE, SHARE_READ, 0, CREATE_ALWAYS, ATTRIBUTE_NORMAL, 0);
    if (file == INVALID)
        return;
    WRITE_FILE(file, text, n, &wrote, 0);
    CLOSE_HANDLE(file);
}

static int owner_of_path(const char *path)
{
    for (int i = 0; i < OWNERS; i++)
        if (owners[i].query && same(owners[i].path, path))
            return i;
    return -1;
}

/* The size of the list's record of a path (its file's), 0 for none. */
static u32 record_size(const char *path)
{
    for (u8 *record = *(u8 **)LIST_BEGIN; record < *(u8 **)LIST_END; record += RECORD)
        if (same((const char *)record + TEXT, path))
            return *(u32 *)(record + SIZE);
    return 0;
}

/* A loaded texture held for the game (bc_take, a load's texture shared): one
 * reference more, the least used let go past KEPT or KEPT_BYTES (into gone,
 * released by the caller with the lock let go). Returns how many went. */
static int keep(const char *path, void *texture, void **gone)
{
    u32 bytes = record_size(path);
    int count = 0;
    if (!bytes || bytes > KEPT_BYTES)
        return 0;
    for (int i = 0; i < KEPT; i++)
        if (kept[i].texture && same(kept[i].path, path))
            return 0;
    for (;;) {
        int free = -1, oldest = -1, entries = 0;
        for (int i = 0; i < KEPT; i++) {
            if (!kept[i].texture) {
                if (free < 0)
                    free = i;
                continue;
            }
            entries++;
            if (oldest < 0 || kept[i].used < kept[oldest].used)
                oldest = i;
        }
        if (free >= 0 && kept_bytes + bytes <= KEPT_BYTES) {
            if (!join(kept[free].path, TEXT_SIZE, path, length(path), ""))
                return count;
            ++*(u32 *)((u8 *)texture + TEXTURE_REFS);
            kept[free].texture = texture;
            kept[free].bytes = bytes;
            kept[free].used = ++kept_clock;
            kept_bytes += bytes;
            return count;
        }
        if (oldest < 0 || !entries)
            return count;
        gone[count++] = kept[oldest].texture;
        kept_bytes -= kept[oldest].bytes;
        kept[oldest].texture = 0;
    }
}

/* translate_query's look-up (bake_cache_lookup): 2 when a texture of the path
 * is held (*texture: the query gets it at once), 1 when a query of the same
 * path is under way (this query then waits: a reference to its model instance
 * is kept), else 0 (the query bakes or loads, the path's owner when the table
 * has room). An owner listed for STALE_MS (one that never ended: none is
 * known) is taken over by this query, its waiters with it. */
int bc_lookup(const char *path, void *query, void *surface, void *parameters, void *cook, void **texture)
{
    if (!path || !query || !parameters)
        return 0;
    u32 type = *(u32 *)((u8 *)parameters + TYPE), now = TICK_COUNT();
    int result = 0;
    acquire();
    for (int i = 0; i < KEPT && type < 4; i++) {
        if (kept[i].texture && same(kept[i].path, path)) {
            kept[i].used = ++kept_clock;
            *texture = kept[i].texture;
            release();
            return 2;
        }
    }
    int owner = owner_of_path(path);
    if (owner >= 0 && now - owners[owner].since > STALE_MS) {
        owners[owner].query = query;
        owners[owner].surface = surface;
        owners[owner].type = type;
        owners[owner].since = now;
    } else if (owner >= 0) {
        int count = 0, free = -1;
        for (int i = 0; i < WAITERS; i++) {
            if (waiters[i].query && waiter_of[i] == owner)
                count++;
            else if (!waiters[i].query && free < 0)
                free = i;
        }
        if (owners[owner].type == type && type < 4 && count < PER_OWNER && free >= 0) {
            u8 *model = *(u8 **)((u8 *)parameters + MODEL);
            if (model)
                __atomic_add_fetch((volatile int *)(model + REFS), 1, __ATOMIC_SEQ_CST);
            waiters[free].query = query;
            waiters[free].surface = surface;
            waiters[free].model = model;
            waiters[free].type = type;
            waiters[free].cook = cook;
            waiter_of[free] = (u8)owner;
            result = 1;
        }
    } else {
        for (int i = 0; i < OWNERS; i++) {
            if (owners[i].query)
                continue;
            if (join(owners[i].path, TEXT_SIZE, path, length(path), "")) {
                owners[i].query = query;
                owners[i].surface = surface;
                owners[i].type = type;
                owners[i].since = now;
                bc_owners++;
            }
            break;
        }
    }
    release();
    return result;
}

/* The end of an owner's query: its waiting queries into out (at most max) and
 * their count, the owner out of the table; -1 when the query owns nothing.
 * check (a texture made, bake_cache_baked 1 and bake_cache_loaded 2): -1 also,
 * the owner kept, unless the texture is in the owner's surface's slot of its
 * type (holder + 4 x type), as the bake and the load put it. A loaded texture
 * is held for the game (keep). */
int bc_take(void *query, void *texture, void *holder, u32 type, struct waiter *out, int max, int check)
{
    void *gone[KEPT];
    int count = 0, went = 0;
    acquire();
    int owner = -1;
    for (int i = 0; i < OWNERS && query; i++)
        if (owners[i].query == query)
            owner = i;
    if (owner < 0 || (check && (!texture || holder != owners[owner].surface || type != owners[owner].type
                                || type >= 4 || ((void **)holder)[type] != texture))) {
        release();
        return -1;
    }
    for (int i = 0; i < WAITERS; i++) {
        if (!waiters[i].query || waiter_of[i] != owner || count >= max)
            continue;
        out[count].query = waiters[i].query;
        out[count].surface = waiters[i].surface;
        out[count].model = waiters[i].model;
        out[count].type = waiters[i].type;
        out[count].cook = waiters[i].cook;
        count++;
        waiters[i].query = 0;
    }
    if (check == 2)
        went = keep(owners[owner].path, texture, gone);
    owners[owner].query = 0;
    bc_owners--;
    release();
    for (int i = 0; i < went; i++)
        bc_release_texture(gone[i]);
    return count;
}

/* The game's CRC-32 step (0x42050, bake_cache_crc32 in its place): every byte
 * crc = T[(crc ^ byte) & 0xFF] ^ (crc >> 8), T the table 0x1C210 makes at
 * 0x225FD68 (the polynomial 04C11DB7h reflected: EDB88320h; the callers set
 * the start and the end). Here slice by 8 (bake_cache_crc32, its loop in
 * assembly): eight bytes a step through eight tables made from T, the same
 * value for every input. The stock loop took most of a bake burst's time:
 * over every baked DDS (process_compression's 0x12F340) and every one loaded
 * (check_loaded's 0x3C0FF0), 1.4 to 5.6 MB. */
u32 bc_crc_tables[8][256];
volatile int bc_crc_ready;

/* The eight tables (cdecl; bake_cache_crc32 calls it while bc_crc_ready is
 * 0). Two threads that find them unmade make them both, with the same
 * values. */
void bc_crc_build(void)
{
    for (u32 i = 0; i < 256; i++) {
        u32 c = i;
        for (int k = 0; k < 8; k++)
            c = c & 1 ? (c >> 1) ^ 0xEDB88320u : c >> 1;
        bc_crc_tables[0][i] = c;
    }
    for (int t = 1; t < 8; t++)
        for (u32 i = 0; i < 256; i++)
            bc_crc_tables[t][i] = (bc_crc_tables[t - 1][i] >> 8) ^ bc_crc_tables[0][bc_crc_tables[t - 1][i] & 0xFF];
    __atomic_store_n(&bc_crc_ready, 1, __ATOMIC_RELEASE);
}
