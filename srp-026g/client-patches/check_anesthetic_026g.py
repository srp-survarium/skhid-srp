#!/usr/bin/env python3
"""Run the original's drug (medkit) code on made-up Anesthetics (painkiller.options),
for the anesthetic buff: hit_coeff 0.9 -> 0.85, activity_time_sec 10 -> 15.

    python check_anesthetic_026g.py <original survarium.exe>

Requires Python `unicorn`, `patch_026g.py` and `check_booster_modifiers_026g.py`
(its Game: the kit lists, the getter, the allocator stubs) next to this file. The
exe's sections are loaded as they are, and the game's own code runs:

- the medkit constructor 0x33CAC0 after its item base constructor (0x33CADC up to
  0x33D299, the config lookups 0xF6B10) on a binary config written here like the
  painkiller's: data/activity_time_sec, data/activation_delay_sec,
  parameters/activity_time_skill_influence, data/damage_protection (a list of
  {body_part, hit_type, hit_coeff, threshold}; each entry 90h bytes at +294h);
- the use: action 0x33D700 -> set_active(1) 0x33D390, which links each protection
  entry into its pool's reducer list (find_body_part 0x2E58B0, the pool +0BCh);
- body_part::hit's walk of that list (0x32A020 up to 0x32A058), through the
  delegate 0x3FE2C0 / 0x2E5A90 into reduce_damage 0x33DB30 (its entry lookup
  0x33DAC0, the getter 0x2D5AE0 for index 58 drug_hit_coeff);
- tick 0x33D790 every 50 ms: the delay, then the activity, then set_active(0),
  which unlinks the entry again.

Says nothing about the pain pool's own effects (thresholds, death at 0), the HUD or
a real match.
"""
import hashlib
import math
import struct
import sys
import zlib

import check_booster_modifiers_026g as boosters
import patch_026g as base
from check_booster_modifiers_026g import f32, HOLDER_VT, PLAYER, STUB

CTOR_READ, CTOR_READ_END = 0x33CADC, 0x33D299  # esi = the config, ebp = the medkit
ACTION, TICK = 0x33D700, 0x33D790        # thiscall (pressed, time) `ret 8`; (elapsed, now, -) `ret 0Ch`
REDUCE_DAMAGE = 0x33DB30
POOL_WALK, POOL_WALK_END = 0x32A020, 0x32A058  # ebp = pool, edi = hit_info, ebx = &D, [esp+144h] = &X
HIT_TYPES = 0x9257E0                     # hit type names, by enum
DRUG_USING_TIME, DRUG_ACTIVITY_TIME, DRUG_HIT_COEFF = 33, 34, 58
NAME_TABLE = boosters.NAME_TABLE
GET_DAMAGE_MODEL = STUB + 0x10           # the holder's slot 8: a pointer to the damage model's pointer
TICK_MS = 50


def r32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def config_bytes(tree, at):
    """check_booster_modifiers_026g's binary config, with lists (type 4, children in
    order) and strings (type 5, a pointer to the text) as the painkiller has them."""
    values, names = [], bytearray()

    def name(text):
        offset = len(names)
        names.extend(text.encode() + b"\0")
        return offset

    def put(key, value, slot):
        crc = zlib.crc32(key.encode()) if key else 0
        if isinstance(value, (dict, list)):
            children = (sorted(value.items(), key=lambda kv: zlib.crc32(kv[0].encode()))
                        if isinstance(value, dict) else [("", v) for v in value])
            first = len(values)
            values.extend([None] * len(children))
            for i, (k, v) in enumerate(children):
                put(k, v, first + i)
            values[slot] = ("table", first, key, crc, 3 if isinstance(value, dict) else 4, len(children))
        elif isinstance(value, str):
            values[slot] = ("text", value, key, crc, 5, 0)
        elif isinstance(value, float):
            values[slot] = ("raw", f32(value), key, crc, 2, 0)
        else:
            values[slot] = ("raw", value & 0xFFFFFFFF, key, crc, 1, 0)

    values.append(None)
    put("", tree, 0)
    names_at = at + 24 * len(values)
    out = bytearray()
    for kind, data, key, crc, type_, count in values:
        if kind == "table":
            data = at + 24 * data
        elif kind == "text":
            data = names_at + name(data)
        out += struct.pack("<QQIHH", data, names_at + name(key), crc, type_, count)
    return bytes(out + names)


def painkiller(hit_coeff, activity, threshold=0, delay=1, k=1):
    """The painkiller.options values the medkit reads (the rest of the file is the
    item base's)."""
    return {"parameters": {"activity_time_skill_influence": k},
            "data": {"activity_time_sec": activity, "activation_delay_sec": delay, "type": 0,
                     "influences": {}, "remove_affects": {},
                     "damage_protection": [{"body_part": "pain", "hit_type": "ranged",
                                            "hit_coeff": hit_coeff, "threshold": threshold}]}}


class Game(boosters.Game):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")          # alive
        self.damage_model = self.block(0x600)
        holder = self.block(4)
        self.word(holder, self.damage_model)
        self.emu.mem_write(GET_DAMAGE_MODEL, b"\xB8" + struct.pack("<I", holder) + b"\xC3")
        self.word(HOLDER_VT + 8, GET_DAMAGE_MODEL)
        self.pools = {}
        for pool in ("body", "pain"):                         # the container's list (+108h, head +8)
            at = self.block(0x200)
            self.word(at + 0x80, self.text(pool))
            self.word(at, self.read(self.damage_model + 0x108 + 8))
            self.word(self.damage_model + 0x108 + 8, at)
            self.pools[pool] = at
        self.world = self.block(0x18FB0)                     # +188A0h the tick list, +18FA8h no listener

    def text(self, value):
        at = self.block(len(value) + 1)
        self.emu.mem_write(at, value.encode() + b"\0")
        return at

    def string(self, at, size=64):
        return bytes(self.emu.mem_read(at, size)).split(b"\0")[0].decode()

    def medkit(self, tree):
        obj, inventory, desc, config = self.block(0x300), self.block(0x200), self.block(0x240), self.block(0x1000)
        self.word(inventory + 0x174, boosters.HOLDER)
        self.word(obj + 0x250, inventory)
        self.word(obj + 0x248, self.world)
        self.word(obj + 0x25C, desc)                          # +22Ah 0: not a counted item
        self.emu.mem_write(config, config_bytes(tree, config))
        self.run(CTOR_READ, CTOR_READ_END, esi=config, ebp=obj)
        return obj

    def entries(self, obj):
        first = self.read(obj + 0x294)
        return [first + 0x90 * i for i in range(self.emu.mem_read(obj + 0x298, 1)[0])]

    def linked(self, pool):
        """The entries in a pool's reducer list (+0BCh: count, +8 head, next +68h)."""
        lst, out = self.pools[pool] + 0xBC, []
        at = self.read(lst + 8)
        while at:
            out.append(at)
            at = self.read(at + 0x68)
        return self.read(lst), out

    def use(self, obj, now):
        self.run(ACTION, args=(1, now), ecx=obj)

    def tick(self, obj, elapsed, now):
        self.run(TICK, args=(elapsed, now, 0), ecx=obj)

    def hit(self, pool, name, hit_type, damage, pierce=0.0):
        """body_part::hit's walk of `pool`'s reducers on a hit {name, type}: D after."""
        info, d, x = self.block(0x40), self.block(4), self.block(4)
        self.word(info, self.text(name))
        self.word(info + 0x1C, hit_type)
        self.put_float(d, damage)
        self.put_float(x, pierce)
        self.run(POOL_WALK, POOL_WALK_END, args=[0] * 80 + [x], ebp=self.pools[pool], edi=info, ebx=d)
        return self.float(d)


def run(data):
    checks = 0

    def expect(ok, what):
        nonlocal checks
        checks += 1
        if not ok:
            sys.exit(f"FAIL: {what}")

    def close(a, b, eps=1e-6):
        return abs(a - b) <= eps

    g = Game(data)
    types = [g.string(g.read(HIT_TYPES + 4 * i)) for i in range(8)]
    expect(types == ["ranged", "blunt", "slash", "explosive", "toxic", "radiation", "fire", "shock"],
           "the hit types 0x9257E0")
    names = [g.string(g.read(NAME_TABLE + 4 * i)) for i in (DRUG_USING_TIME, DRUG_ACTIVITY_TIME, DRUG_HIT_COEFF)]
    expect(names == ["drug_using_time_modifier", "drug_activity_time_modifier", "drug_hit_coeff_modifier"],
           "indexes 33, 34, 58")
    RANGED = types.index("ranged")

    # The painkiller's own parameters: none of them is one of the 68 modifiers (its
    # "stamina_regenation_speed_modifier" is misspelt), so its index-3 node (+2B4h) is 0.
    params = {"money_bonus": 0, "aiming_speed_modifier": 0, "stamina_spending_speed_modifier": 0,
              "degrade_per_death": 0, "weight": 0.1, "dispersion_modifier": 0, "movement_speed_modifier": 0,
              "aim_time_modifier": 0, "recoil_modifier": 0, "damage_protection_pain": 0.9,
              "activity_time_sec": 10, "breath_vibration_modifier": 0, "drop_weight": 80,
              "pocket_slot_size_modifier": 0, "artefact_search_time_modifier": 0, "reputation_bonus": 0,
              "stamina_regenation_speed_modifier": 0, "activity_time_skill_influence": 1,
              "artefact_slots_modifier": 0, "ammo_bags_modifier": 0, "pain_health_modifier": 0,
              "max_carried_weight_modifier": 0, "heal_amount": 0, "respawn_time_modifier": 0,
              "match_making_level": 3, "degrade_per_match": 0, "faction_affinity": 0, "item_level": 3,
              "degrade_per_use": 0, "health_regeneration_speed_modifier": 0, "melee_damage_modifier": 0,
              "reload_speed_modifier": 0, "pocket_slots_modifier": 0, "heal_type": 0, "activation_time": 1,
              "experience_bonus": 0}
    expect(Game(data).read_parameters(params) == [0.0] * 68, "the painkiller's parameters: 68 zeros")

    # 1. The config reader.
    print("config reader (0x33CAC0): activity ms +29Ch, k +2A0h, delay ms +2A4h; "
          "damage_protection 90h each at +294h (count +298h)")
    for coeff, activity in ((0.9, 10), (0.85, 15), (0.15, 15)):
        m = Game(data)
        obj = m.medkit(painkiller(coeff, activity))
        expect(m.read(obj + 0x29C) == activity * 1000, "activity ms")
        expect(m.float(obj + 0x2A0) == 1.0, "k = activity_time_skill_influence")
        expect(m.read(obj + 0x2A4) == 1000, "delay ms")
        expect(m.emu.mem_read(obj + 0x288, 1)[0] == 0 and m.emu.mem_read(obj + 0x290, 1)[0] == 0,
               "no influences, no remove_affects")
        (entry,) = m.entries(obj)
        expect(m.string(entry + 0x70, 16) == "pain", "entry +70h: the pool's name")
        expect(m.read(entry + 0x80) == RANGED, "entry +80h: the hit type")
        expect(m.float(entry + 0x84) == r32(coeff), "entry +84h: hit_coeff")
        expect(m.float(entry + 0x88) == 0.0, "entry +88h: threshold")
        expect([m.read(entry + o) for o in (8, 0x10, 0x18)] == [0xA1C78D, REDUCE_DAMAGE, obj],
               "entry +8: a delegate to reduce_damage on this medkit")
        print(f"  hit_coeff {coeff}, {activity} s: +29Ch {m.read(obj + 0x29C)}, +2A4h {m.read(obj + 0x2A4)}, "
              f"entry {{pain, ranged ({RANGED}), {m.float(entry + 0x84):.6g}, {m.float(entry + 0x88):g}}}")
    m = Game(data)
    obj = m.medkit(painkiller(0.9, 0.0004))
    expect(m.read(obj + 0x29C) == 1, "activity at least 0.001 s")

    # 2. reduce_damage, through the pain pool's walk, while active.
    print("reduce_damage (0x33DB30), the pain pool's reducers: D < threshold -> 0, "
          "else (D - threshold) x hit_coeff x max(0, 1 + S58)")
    for coeff in (0.9, 0.85, 0.15):
        m = Game(data)
        obj = m.medkit(painkiller(coeff, 15))
        expect(m.hit("pain", "pain", RANGED, 0.5) == r32(0.5), "before the use: no reducer")
        m.use(obj, 0)
        expect(m.linked("pain") == (1, m.entries(obj)) and m.linked("body") == (0, []),
               "the use links the entry into the pain pool only")
        got = {d: m.hit("pain", "pain", RANGED, d) for d in (0.2, 0.5, 1.0)}
        for d, v in got.items():
            expect(close(v, r32(d) * r32(coeff)), f"ranged pain {d} x {coeff}")
        for t in ("blunt", "slash", "explosive", "fire", "shock"):
            expect(m.hit("pain", "pain", types.index(t), 0.5) == r32(0.5), f"{t}: unchanged")
        expect(m.hit("pain", "body", RANGED, 0.5) == r32(0.5), "another pool's hit: unchanged")
        expect(m.hit("body", "body", RANGED, 0.5) == r32(0.5), "the body pool: no reducer")
        expect(m.hit("pain", "pain", RANGED, 0.5, pierce=0.7) == r32(0.5) * r32(coeff), "pierce plays no part")
        print(f"  hit_coeff {coeff}: ranged pain " + ", ".join(f"{d} -> {v:.6g}" for d, v in got.items())
              + "; blunt/slash/explosive/fire/shock, the body pool: unchanged")
    m = Game(data)
    obj = m.medkit(painkiller(0.9, 10, threshold=0.1))
    m.use(obj, 0)
    got = [m.hit("pain", "pain", RANGED, d) for d in (0.05, 0.1, 0.5)]
    expect(got[:2] == [0.0, 0.0] and close(got[2], (r32(0.5) - r32(0.1)) * r32(0.9)), "threshold 0.1")
    print(f"  threshold 0.1 (none in 0.26g's anesthetic), hit_coeff 0.9: 0.05 -> {got[0]:g}, 0.1 -> {got[1]:g}, "
          f"0.5 -> {got[2]:.6g}")
    for s58 in (-0.5, 0.5, -1.5):
        m = Game(data)
        obj = m.medkit(painkiller(0.85, 15))
        m.append_node(DRUG_HIT_COEFF, s58)
        m.use(obj, 0)
        v = m.hit("pain", "pain", RANGED, 0.5)
        expect(close(v, r32(0.5) * r32(0.85) * max(0.0, 1 + s58)), f"S58 {s58}")
        print(f"  S58 {s58:+}: 0.5 -> {v:.6g} (0.85 x {max(0.0, 1 + s58):g})")

    # 3. The window: from the press to set_active(0).
    print(f"window (ticks every {TICK_MS} ms): linked at the press, delay = floor(delay ms x max(0, 1 + S33)), "
          "then activity = floor(activity ms x max(0, 1 + k x S34)), then unlinked")
    for coeff, activity in ((0.9, 10), (0.85, 15)):
        for s33, s34, k in ((0.0, 0.0, 1), (0.0, 0.5, 1), (0.0, -0.2, 1), (-0.5, 0.0, 1), (0.0, 0.5, 0.5)):
            m = Game(data)
            obj = m.medkit(painkiller(coeff, activity, k=k))
            if s33:
                m.append_node(DRUG_USING_TIME, s33)
            if s34:
                m.append_node(DRUG_ACTIVITY_TIME, s34)
            m.use(obj, 0)
            delay_ms, activity_ms = m.read(obj + 0x2AC), m.read(obj + 0x2A8)
            expect(delay_ms == math.floor(r32(1000 * r32(max(0.0, r32(1 + r32(s33)))))), "delay ms")
            expect(activity_ms == math.floor(r32(activity * 1000 * r32(max(0.0, r32(1 + r32(r32(s34) * k)))))),
                   "activity ms")
            now, last, first_active, reduced = 0, None, None, True
            while m.linked("pain")[0] and now < 100_000:
                reduced &= close(m.hit("pain", "pain", RANGED, 0.5), r32(0.5) * r32(coeff))
                last = now
                now += TICK_MS
                m.tick(obj, TICK_MS, now)
                if first_active is None and m.read(obj + 0x2AC) == 0:
                    first_active = now
            expect(reduced, "reduced at every tick while linked")
            expect(m.hit("pain", "pain", RANGED, 0.5) == r32(0.5), "after the end: unchanged")
            expect(m.emu.mem_read(obj + 0x2B0, 1)[0] == 0, "inactive after the end")
            expect(first_active == delay_ms and now == delay_ms + activity_ms, "unlinked at delay + activity")
            print(f"  hit_coeff {coeff}, {activity} s, S33 {s33:+} S34 {s34:+} k {k}: delay {delay_ms} ms, "
                  f"activity {activity_ms} ms; reduced from the press to {last} ms, unchanged at {now} ms")
    m = Game(data)
    obj = m.medkit(painkiller(0.85, 15))
    m.use(obj, 0)
    m.tick(obj, 5000, 5000)
    m.emu.mem_write(PLAYER + 0x33C, b"\x00")
    m.tick(obj, TICK_MS, 5000 + TICK_MS)
    expect(m.linked("pain")[0] == 0, "death ends it")
    print("  the owner dead (+33Ch 0) at a tick: unlinked")
    return checks


def main():
    data = open(sys.argv[1], "rb").read()
    if hashlib.sha256(data).hexdigest() != base.EXE_SHA256:
        sys.exit("not the original 0.26g survarium.exe")
    print(f"anesthetic, the original: {run(data)} checks as expected")


if __name__ == "__main__":
    main()
