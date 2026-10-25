/* Patch 68 (build 2777): a weapon's attachments in at most three upgrade
 * records, the weapon kit (README patch 68; client_patch.md section 3.56;
 * the layout: ../docs/notes/weapon-kit.md).
 *
 * The client keeps at most three upgrade records an item (the item record's
 * inline buffer_vector, 0xB4-byte record +0x60..+0xB4; a fourth is the fatal
 * assert at 0x1F1655), and the config builder (0x2F14C0) keeps their values
 * in a stack array of three. A weapon (stock 0.26g gave weapons none) holds
 * up to six things: two decals (slots 0 and 1), its camouflage (2), its
 * scope (3), a muzzle device (4) and a grip (5). The server sends them as
 * at most three records: the decals as they are, and one "kit" record in
 * slot 3 for the rest. The kit record is the first of scope, camouflage,
 * muzzle, grip the weapon wears (its item id and record dict id, as that
 * one's own record would be), its value names the others by dict id:
 *
 *   bits  0..15  the camouflage's dict id (0: none, or the kit's own)
 *   bits 16..31  the muzzle device's
 *   bits 32..47  the grip's
 *   bits 48..55  the kit's own logical slot (2, 3, 4 or 5)
 *   bits 56..63  KIT_MAGIC
 *
 * Every copy of an item record (the inventory, the kits, the profiles of a
 * match's static info, a late join's, a replay's) carries the records as
 * they are, so nothing else needs to know. Here:
 *
 *   weapon_kit_expand          item_config_cook::translate_query (0x2F1750)
 *                              gets, in place of the item's own list, a list
 *                              of the records and one for each dict id a kit
 *                              names (its own stack, per call): each record's
 *                              item gives its modification, as the stock
 *                              does for any record
 *   weapon_kit_preview_apply   patch 48's queue of changes applied to the
 *                              window's weapon as the server applies them,
 *                              on the logical slots, packed again
 */
#include "engine_026g.h"

#define KIT_MAGIC 0xA7u
#define KIT_SLOT 3u
#define MAX_RECORDS 3u          /* the item record's inline capacity */
#define MAX_LOGICAL 8u          /* six slots, two spare for what no slot names */
#define TAKE_OFF 0xFFu          /* patch 48's "take off" */

/* An upgrade record of the item record: {u32 id, u32 dict id, u64 value,
 * u8 slot}, 0x18 bytes. */
struct record {
    unsigned int id;
    unsigned int dict;
    unsigned int low;
    unsigned int high;
    unsigned char slot;
    unsigned char pad[7];
};

_Static_assert(sizeof(struct record) == 0x18, "an upgrade record is 18h bytes");

/* The item record's buffer_vector of records at +0x60: begin, end,
 * capacity end. */
struct records {
    struct record *begin;
    struct record *end;
    struct record *capacity;
};

/* Patch 48's queued change: {u32 upgrade, u32 the record's dict id, u32
 * slot or TAKE_OFF}. */
struct change {
    unsigned int upgrade;
    unsigned int dict;
    unsigned int slot;
};

/* The fields of the kit's value: the logical slot each names and the bit
 * it begins at. */
static const unsigned char field_slot[3] = {2, 4, 5};
static const unsigned char field_shift[3] = {0, 16, 32};

static int is_kit(const struct record *r)
{
    unsigned int own = (r->high >> 16) & 0xFF;
    return r->slot == KIT_SLOT && (r->high >> 24) == KIT_MAGIC && own >= 2 && own <= 5;
}

static unsigned int field(const struct record *r, unsigned int i)
{
    return i < 2 ? (r->low >> field_shift[i]) & 0xFFFF : r->high & 0xFFFF;
}

static void clear(struct record *r)
{
    unsigned char *p = (unsigned char *)r;
    for (unsigned int i = 0; i < sizeof *r; i++)
        p[i] = 0;
}

/* The item's records (`in`) hold a kit: what makes translate_query's list
 * (weapon_kit_expand) worth making; else nothing changes. */
unsigned int weapon_kit_holds(const struct records *in)
{
    if (in == 0 || in->begin == 0 || in->end < in->begin
        || (unsigned int)(in->end - in->begin) > MAX_RECORDS)
        return 0;
    for (const struct record *r = in->begin; r < in->end; r++)
        if (is_kit(r))
            return 1;
    return 0;
}

/* translate_query's list (`in`, the item's records) with every dict id a
 * kit names as a record of its own ({0, dict id, 0, its slot}) after the
 * records, in `out` (6 records) and `list`: the number of records, or 0
 * when `in` has no kit (the item's own list is used, as the stock). A dict
 * id is left out when the items dictionary lacks it (an older game data:
 * the record would be read from a null entry, 0x2F18E3), when a record of
 * the list holds it already (its modification would add a second
 * `camouflage`: the config's duplicate-name assert) or when a record that is
 * no kit is in its slot. */
unsigned int weapon_kit_expand(const struct records *in, struct record *out, struct records *list,
                               void *dictionary)
{
    unsigned int count = 0;
    int kits = 0;
    if (in == 0 || in->begin == 0 || in->end < in->begin
        || (unsigned int)(in->end - in->begin) > MAX_RECORDS)
        return 0;
    for (const struct record *r = in->begin; r < in->end; r++) {
        kits += is_kit(r);
        out[count++] = *r;
    }
    if (!kits)
        return 0;
    unsigned int records = count;
    for (unsigned int k = 0; k < records; k++) {
        if (!is_kit(&out[k]))
            continue;
        unsigned int own = (out[k].high >> 16) & 0xFF;
        for (unsigned int i = 0; i < 3; i++) {
            unsigned int dict = field(&out[k], i);
            int skip = dict == 0 || field_slot[i] == own;
            for (unsigned int j = 0; j < count && !skip; j++)
                skip = out[j].dict == dict || (j < records && !is_kit(&out[j]) && out[j].slot == field_slot[i]);
            if (skip || count >= MAX_RECORDS * 2 || engine_item_by_id(dictionary, dict) == 0)
                continue;
            clear(&out[count]);
            out[count].dict = dict;
            out[count].slot = field_slot[i];
            count++;
        }
    }
    list->begin = out;
    list->end = out + count;
    list->capacity = out + count;
    return count;
}

/* `wire` as logical records (a kit's own in its own slot, each dict id it
 * names with item id 0) into `logical`; the number. */
static unsigned int unpack(const struct record *begin, const struct record *end, struct record *logical)
{
    unsigned int count = 0;
    for (const struct record *r = begin; r < end && count < MAX_LOGICAL; r++) {
        logical[count] = *r;
        if (!is_kit(r)) {
            count++;
            continue;
        }
        logical[count].slot = (r->high >> 16) & 0xFF;
        logical[count].low = logical[count].high = 0;
        count++;
        for (unsigned int i = 0; i < 3 && count < MAX_LOGICAL; i++) {
            unsigned int dict = field(r, i);
            if (dict == 0 || field_slot[i] == ((r->high >> 16) & 0xFF))
                continue;
            clear(&logical[count]);
            logical[count].dict = dict;
            logical[count].slot = field_slot[i];
            count++;
        }
    }
    return count;
}

/* `logical` packed as the server packs it (session::v026g::wire,
 * survarium::v026g::weapon_kit::pack): the records of slots 0 and 1 in
 * that order, the kit (its own the first of slots 3, 2, 4, 5 present),
 * then a record of any other slot while there is room. At most `room`. */
static unsigned int pack(const struct record *logical, unsigned int count, struct record *wire, unsigned int room)
{
    static const unsigned char kit_order[4] = {3, 2, 4, 5};
    unsigned int n = 0;
    for (unsigned int slot = 0; slot < 2; slot++)
        for (unsigned int i = 0; i < count; i++)
            if (logical[i].slot == slot && n < room) {
                wire[n++] = logical[i];
                break;
            }
    const struct record *own = 0;
    for (unsigned int o = 0; o < 4 && own == 0; o++)
        for (unsigned int i = 0; i < count; i++)
            if (logical[i].slot == kit_order[o]) {
                own = &logical[i];
                break;
            }
    if (own != 0 && n < room) {
        struct record kit = *own;
        kit.low = 0;
        kit.high = (KIT_MAGIC << 24) | ((unsigned int)own->slot << 16);
        for (unsigned int f = 0; f < 3; f++) {
            if (field_slot[f] == own->slot)
                continue;
            for (unsigned int i = 0; i < count; i++) {
                if (logical[i].slot != field_slot[f])
                    continue;
                unsigned int dict = logical[i].dict & 0xFFFF;
                if (f < 2)
                    kit.low |= dict << field_shift[f];
                else
                    kit.high |= dict;
                break;
            }
        }
        kit.slot = KIT_SLOT;
        for (unsigned int i = 0; i < 7; i++)
            kit.pad[i] = 0;
        wire[n++] = kit;
    }
    for (unsigned int i = 0; i < count && n < room; i++)
        if (logical[i].slot > 5)
            wire[n++] = logical[i];
    return n;
}

/* A logical record is the cosmetic `upgrade` of dict id `dict`: its item id,
 * or a dict id a kit named (no item id). */
static int is_cosmetic(const struct record *r, unsigned int upgrade, unsigned int dict)
{
    return r->id == upgrade || (r->id == 0 && r->dict == dict);
}

static unsigned int remove_if_cosmetic(struct record *logical, unsigned int count, unsigned int upgrade,
                                       unsigned int dict, int slot)
{
    unsigned int n = 0;
    for (unsigned int i = 0; i < count; i++) {
        if (is_cosmetic(&logical[i], upgrade, dict) || (slot >= 0 && logical[i].slot == (unsigned int)slot))
            continue;
        logical[n++] = logical[i];
    }
    return n;
}

/* Patch 48's queue (`changes`, `count` of them) applied to the item record
 * `item`'s records as the server applies them (items_rules::attach_upgrade_in,
 * detach_upgrade): a put (slot 0..5) of a record already there (its slot, its
 * dict id, its item or a kit's name of it) leaves it; else the cosmetic comes
 * off where it was (a decal's place goes with it) and what is in the slot
 * comes off, and it goes in; a take-off takes the cosmetic off. On the logical
 * slots, packed again into the record's own storage. */
void weapon_kit_preview_apply(unsigned char *item, const struct change *changes, unsigned int count)
{
    struct records *records = (struct records *)(item + 0x60);
    struct record logical[MAX_LOGICAL];
    struct record wire[MAX_RECORDS];
    unsigned int n = unpack(records->begin, records->end, logical);
    for (const struct change *c = changes; c < changes + count; c++) {
        if (c->slot == TAKE_OFF) {
            n = remove_if_cosmetic(logical, n, c->upgrade, c->dict, -1);
            continue;
        }
        if (c->slot > 5)
            continue;
        int there = 0;
        unsigned int low = 0, high = 0;
        for (unsigned int i = 0; i < n; i++) {
            if (!is_cosmetic(&logical[i], c->upgrade, c->dict))
                continue;
            if (logical[i].slot == c->slot && logical[i].dict == c->dict)
                there = 1;
            else if (logical[i].dict == c->dict) {
                low = logical[i].low;
                high = logical[i].high;
            }
        }
        if (there)
            continue;
        n = remove_if_cosmetic(logical, n, c->upgrade, c->dict, (int)c->slot);
        if (n >= MAX_LOGICAL)
            continue;
        clear(&logical[n]);
        logical[n].id = c->upgrade;
        logical[n].dict = c->dict;
        logical[n].low = low;
        logical[n].high = high;
        logical[n].slot = (unsigned char)c->slot;
        n++;
    }
    unsigned int room = (unsigned int)(records->capacity - records->begin);
    if (room > MAX_RECORDS)
        room = MAX_RECORDS;
    unsigned int packed = pack(logical, n, wire, room);
    for (unsigned int i = 0; i < packed; i++)
        records->begin[i] = wire[i];
    records->end = records->begin + packed;
}
