/* Build-2777 profile validation before the native, unchecked reader runs.
 * Freestanding x86: no globals, runtime imports, or absolute code addresses.
 * dictionary is a live native items_dictionary, never packet-controlled.
 */
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
#define INLINE static __attribute__((always_inline)) inline

struct reader { const u8 *p; u32 left; int ok; };

INLINE u32 take(struct reader *r, u32 n) {
    if (n > r->left) { r->ok = 0; return 0; }
    r->p += n;
    r->left -= n;
    return 1;
}

INLINE u32 number(struct reader *r, u32 n) {
    const u8 *p = r->p;
    if (!take(r, n)) return 0;
    u32 value = 0;
    for (u32 i = 0; i < n; ++i) value |= (u32)p[i] << (8 * i);
    return value;
}

INLINE int string(struct reader *r, u32 maximum) {
    u32 length = number(r, 1);
    return r->ok && length <= maximum && take(r, length);
}

INLINE const u8 *item(const u8 *dictionary, u32 id) {
    const u8 *rows = *(const u8 *const *)(dictionary + 0x130);
    u32 count = *(const u32 *)(dictionary + 0x134);
    for (u32 i = 0; i < count; ++i, rows += 0x3a0)
        if (*(const u16 *)rows == id) return rows;
    return (const u8 *)0;
}

INLINE int modification(const u8 *dictionary, u32 id, u32 defect) {
    u32 offset = defect ? 0x148 : 0x140;
    const u8 *rows = *(const u8 *const *)(dictionary + offset);
    u32 count = *(const u32 *)(dictionary + offset + 4);
    for (u32 i = 0; i < count; ++i, rows += 0x2c)
        if (*(const u32 *)rows == id) return 1;
    return 0;
}

/* Must be first in the linked code: the NASM wrapper calls this entry. */
__attribute__((section(".text.entry")))
int late_join_validate_profile(const u8 *bytes, u32 length, const u8 *dictionary) {
    if (!bytes || !dictionary || length > 32768) return 0;
    struct reader r = {bytes, length, 1};
    /* 0/1 are the two-team modes; 4 + place is an FFA identity.
     * 2 (neutral), 3 (none), and identities outside the 20-place engine
     * roster are never player affiliations. Admission checks the place. */
    u32 side = number(&r, 1);
    if ((side > 1 && (side < 4 || side >= 24)) || !string(&r, 63) || !string(&r, 14)) return 0;
    number(&r, 4); /* protection */
    u32 kits = number(&r, 1), items = number(&r, 1);
    if (!r.ok || !kits || kits > 3 || items > 69) return 0;
    u16 references[3 * 23];
    u32 referenced = 0, usable = 0;
    for (u32 kit = 0; kit < kits; ++kit) {
        usable |= number(&r, 1) == 0;
        number(&r, 4); /* kit id */
        if (!string(&r, 63)) return 0;
        u32 mask = number(&r, 4);
        if (mask >> 23) return 0;
        for (u32 slot = 0; slot < 23; ++slot) {
            if (!(mask & (1u << slot))) continue;
            u32 index = number(&r, 2);
            if (index >= items) return 0;
            references[referenced++] = (u16)index;
            number(&r, 2); /* stack count */
        }
        /* Native 128-bit modifier mask: only its low 68 bits are valid.
         * Wire dwords are [bits64..95, bits96..127, bits0..31, bits32..63]. */
        u32 masks[4];
        for (u32 i = 0; i < 4; ++i) masks[i] = number(&r, 4);
        if (masks[0] >> 4 || masks[1]) return 0;
        for (u32 i = 0; i < 4; ++i) {
            for (u32 mask = masks[i]; mask; mask &= mask - 1) {
                u32 value = number(&r, 4);
                if ((value & 0x7f800000) == 0x7f800000) return 0;
            }
        }
    }
    if (!usable) return 0;
    for (u32 index = 0; index < items; ++index) {
        number(&r, 4); /* instance id */
        u32 id = number(&r, 2);
        if (!id) {
            for (u32 i = 0; i < referenced; ++i)
                if (references[i] == index) return 0;
            continue;
        }
        const u8 *row = item(dictionary, id);
        if (!row) return 0;
        if (!take(&r, row[0x22a] ? 4 : 12)) return 0;
        u32 modifications = number(&r, 1);
        if (modifications > 8) return 0;
        for (u32 i = 0; i < modifications; ++i) {
            u32 id = number(&r, 4), defect = number(&r, 1);
            if (defect > 1 || !modification(dictionary, id, defect)) return 0;
        }
        u32 upgrades = number(&r, 1);
        if (upgrades > 3 || !take(&r, upgrades * 17)) return 0;
    }
    return r.ok && !r.left;
}
