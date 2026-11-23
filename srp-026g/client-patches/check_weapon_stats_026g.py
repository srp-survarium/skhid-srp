#!/usr/bin/env python3
"""Patch 165 (weapon_stats): a match's kills (headshots, through cover, by a
ricochet), deaths, shots, hits, damage, kill distances and time held per
place and weapon, kept beside the statistics and saved after their tail
(docs/notes/kill_weapon.md).

Runs the game's own code on made-up objects, in the original exe and in the
build, the same calls in both:
- base_player::on_fire (0x2D7800), whole: the handler's
  on_player_weapon_firing counts event 22; the build also counts the bullets
  for the weapon in the shooter's hands (its inventory's selected slot's
  item when that is the item held), none known with a battery in the hands;
- base_player::generate_hit_event (0x2D9F40), whole, the event queue
  stubbed: the damage record and event 23 as the original's; the build also
  counts the hit and the pain for the bullet's weapon, under the
  statistics' own tests (an enemy place, the flag, no stock hit);
- generate_killed_event from the handler's load to its call's return
  (0x2DA3E5..0x2DA46E), the frame as the function's own code leaves it:
  events 2, 0, 15 and the rest as the original's; the build also counts the
  death for the weapon in the victim's hands and the kill, the headshot, the
  distance and what the bullet keeps of a pierce and of a bounce for the
  damage source's dict id;
- base_player::tick's site (0x2DAAD3, to the push after the replayed
  instructions; the site's other patches run too): the build counts a tick
  of 50 ms for what a live player has in its hands;
- the statistics' constructor (0x2F4B40), their reset (0x2F4C20) and their
  serialize (0x2F5AC0) and deserialize (0x2F5BE0), whole.

Checked: every byte of the game's objects, every register and the stack are
the original's after each call; the build's world is the original's with the
table after the tail; each place's weapons add up to its record's events
(kills, headshots and deaths as the record's bytes, shots and hits as its
u16); a load gives the table back (a rollback, a second game), a world
without it leaves the game writing none until its statistics are reset.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

ON_FIRE = 0x2D7800                       # thiscall (ecx the player + 110h; the count); ret 4
HIT_EVENT = 0x2D9F40                     # ecx the victim; 7 arguments; ret 1Ch
KILL_FROM, KILL_TO = 0x2DA3E5, 0x2DA46E  # generate_killed_event: mov ecx,[eax+18FA8h] .. after the handler's call
TICK, TICK_AFTER = 0x2DAAD3, 0x2DAAD8    # base_player::tick: mov ecx,[esp+10h] (4) / push edi (1), then push ecx
CONSTRUCT, RESET = 0x2F4B40, 0x2F4C20    # edi the shared statistics; ret
SERIALIZE, DESERIALIZE = 0x2F5AC0, 0x2F5BE0   # esi the statistics, eax the writer / the reader; ret
QUEUE_NODE, QUEUE_PUSH = 0x2D43B0, 0x2D0A60   # the world's event queue: a node in eax; ret
WRITERS = {0x1F3DE0: 1, 0x80FA0: 1, 0x2F5DA0: 2, 0x1F48A0: 4, 0x804E0: 4}   # eax the value; writer + 4 names; ret 14h
APPEND = 0x7AC10                         # buffer_writer::w: the writer, the bytes; edi the count; ret 8
HANDLER_VTABLE = 0xA0755C
GRENADE_CATEGORY = 0x1F1DA1C             # a pointer to the category byte (0x2F5141)

DATA, SIZE = 0x30000000, 0x200000
CORE = DATA + 0x10000                    # the handler at +18FA8h, the places' vector at +18458h
HANDLER = DATA + 0x40000
SHARED = HANDLER + 8
OPTIONS = DATA + 0x50000                 # a place's team at +45ECh + place x 45FCh
PLACES = DATA + 0xB0000                  # 30h a place: three kit players, +0Ch the kit in use
PLAYERS = DATA + 0xC0000                 # 2000h a player
PROFILE = DATA + 0xF0000
INVENTORIES = DATA + 0x100000            # 200h each: +10Ch its 23 slots, +170h the selected one
ITEMS = DATA + 0x110000                  # 800h an object; its inventory item at +10h
ENTRIES = DATA + 0x120000                # 400h a dictionary entry: +0 the dict id, +228h the category
CODE = DATA + 0x130000
BULLETS = DATA + 0x140000                # 100h each
NODE = DATA + 0x150000
RETURN, WRITER, READER, STREAM = DATA + 0x160000, DATA + 0x160100, DATA + 0x160200, DATA + 0x170000
RECORD, PLACE_SIZE, KITS, COUNT = 0x1F8, 0x45FC, 3, 20
STANCE = CODE                            # mov eax,3 / ret: a held item's vtable +7Ch
TRANSFORM = CODE + 0x10                  # lea eax,[ecx+2F4h] / ret: the player's +118h interface's +28h
ITEM_VTABLE, PLAYER_VTABLE = CODE + 0x100, CODE + 0x200
BATTERY = 31                             # the item index the made-up battery is: no inventory slot's
GRENADE = 6                              # the item whose category is the grenades'


def event(place, number, kit):
    """A count's address: the place's record, then (3 x event + kit) u16."""
    return SHARED + RECORD * place + 2 * (KITS * number + kit)


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(DATA, SIZE, UC_PROT_ALL)
        self.written = bytearray()
        for at in WRITERS:
            emu.hook_add(UC_HOOK_CODE, self.write, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, self.append, begin=APPEND, end=APPEND)
        emu.hook_add(UC_HOOK_CODE, self.node, begin=QUEUE_NODE, end=QUEUE_NODE)
        emu.hook_add(UC_HOOK_CODE, self.ret, begin=QUEUE_PUSH, end=QUEUE_PUSH)
        # query_resources (cdecl): returns, nothing loads (Forest Protection's glow, patch 111,
        # asks its effect at a player's first tick)
        emu.hook_add(UC_HOOK_CODE, self.ret, begin=0xE5B60, end=0xE5B60)
        emu.mem_write(STANCE, bytes.fromhex("b803000000c3"))
        emu.mem_write(TRANSFORM, bytes.fromhex("8d81f4020000c3"))
        for slot in range(0, 0x100, 4):
            self.word(ITEM_VTABLE + slot, STANCE)
            self.word(PLAYER_VTABLE + slot, TRANSFORM)
        # the grenades' item category, which on_player_killed compares the source's with (event 25)
        self.word(GRENADE_CATEGORY, CODE + 0x20)
        emu.mem_write(CODE + 0x20, bytes([9]))
        self.word(HANDLER, HANDLER_VTABLE)
        self.word(HANDLER + 0x3238, CORE)
        self.start()
        self.word(CORE + 0x18FA8, HANDLER)
        self.word(CORE + 0x18458, PLACES)
        self.word(CORE + 0x1845C, PLACES + 0x30 * COUNT)
        for place in range(COUNT):
            self.word(OPTIONS + PLACE_SIZE * place + 0x45EC, place % 2)
            for kit in range(KITS):
                self.word(PLACES + 0x30 * place + 4 * kit, self.player(place, kit))
            self.kit(place, 0)
        for index in range(32):
            item = ITEMS + 0x800 * index
            self.word(item, ITEM_VTABLE)
            self.word(item + 0x10 + 0x25C, ENTRIES + 0x400 * index)
            self.word(item + 0x10 + 0x254, 7)
            emu.mem_write(ENTRIES + 0x400 * index, struct.pack("<H", 0 if index == BATTERY else 100 + index))
            emu.mem_write(ENTRIES + 0x400 * index + 0x228, bytes([9 if index == GRENADE else 1]))

    def start(self):
        """What the handler's start_match gives the statistics: the options, the places' count."""
        self.word(SHARED + 0x31C8, OPTIONS)
        self.emu.mem_write(SHARED + 0x31E0, bytes([COUNT]))

    def player(self, place, kit):
        """The place's kit's player, made (its interfaces, world, profile and inventory)."""
        player = PLAYERS + 0x2000 * (KITS * place + kit)
        self.emu.mem_write(player + 0x114, bytes([place, kit]))
        self.emu.mem_write(player + 0x33C, b"\x01")
        self.word(player + 0x118, PLAYER_VTABLE)
        self.word(player + 0x120, CORE)
        self.word(player + 0x10C, INVENTORIES + 0x200 * (KITS * place + kit))
        self.word(player + 0x6B8, PROFILE)
        return player

    def kit(self, place, kit):
        self.emu.mem_write(PLACES + 0x30 * place + 0xC, bytes([kit]))

    def playing(self, place):
        """The player the place plays: its kit in use's."""
        return self.read(PLACES + 0x30 * place + 4 * self.read(PLACES + 0x30 * place + 0xC, 1)[0])

    def hold(self, place, index, slot=7, selected=None):
        """The place's player holds item `index` (None: nothing), which its
        inventory has in `slot` (None: in none); `selected` the slot it last
        selected (the item's own by default)."""
        player = self.playing(place)
        inventory = self.read(player + 0x10C)
        self.emu.mem_write(inventory + 0x10C, bytes(4 * 23))
        item = 0 if index is None else ITEMS + 0x800 * index
        self.word(player + 0x128, item)
        if item and slot is not None:
            self.word(inventory + 0x10C + 4 * slot, item + 0x10)
        self.word(inventory + 0x170, 0x17 if selected is None and slot is None else slot if selected is None else selected)

    def carry(self, place, index):
        """The place's player takes item `index` into its hands, its inventory as it was (a battery)."""
        self.word(self.playing(place) + 0x128, ITEMS + 0x800 * index)

    def stand(self, place, x, y=0.0, z=0.0):
        self.emu.mem_write(self.playing(place) + 0x40C, struct.pack("<16f", 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1))

    def bullet(self, index, weapon, stock=False, pierces=0, bounces=0):
        """A bullet of item `weapon` that pierced and bounced so often; `stock`: its data are the
        gun's own melee's."""
        bullet = BULLETS + 0x100 * index
        item = ITEMS + 0x800 * weapon
        self.emu.mem_write(bullet, bytes(0x100))
        self.word(bullet + 4, 0x700 + index)
        self.word(bullet + 0xC, item + 0x10)
        self.word(bullet + 0x40, item + 0x53C if stock else ENTRIES)
        self.word(bullet + 0x84, pierces + bounces)       # m_change_trajectory_count
        self.emu.mem_write(bullet + 0x88, bytes([bounces]))   # m_ricochet_count
        return bullet

    def node(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, NODE)
        self.ret(emu)

    def ret(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4)

    def write(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 4) == WRITER, hex(self.read(sp + 4))
        self.written.extend(emu.mem_read(emu.reg_read(UC_X86_REG_EAX), WRITERS[address]))
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 24)

    def append(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 4) == WRITER, hex(self.read(sp + 4))
        self.written.extend(emu.mem_read(self.read(sp + 8), emu.reg_read(UC_X86_REG_EDI)))
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 12)

    def call(self, start, regs, arguments=(), popped=None, stop=RETURN, frame=None, pushed=0):
        """The game's function at `start` with `arguments` on the stack (or
        `frame` at ESP and no return address, to `stop`): the registers and
        ESP after, with every byte it left of the game's objects."""
        sp = hide.STACK + 0x9000
        self.emu.mem_write(sp - 0x400, bytes(0x800))
        if frame is None:
            self.emu.mem_write(sp, struct.pack("<%dI" % (1 + len(arguments)), RETURN, *arguments))
            after = sp + 4 + (4 * len(arguments) if popped is None else popped)
        else:
            for offset, value in frame.items():
                self.emu.mem_write(sp + offset, value)
            after = sp - pushed
        registers, sp_after = self.run(start, (stop,), sp, {**hide.GAME, **regs})
        assert self.stopped == stop, hex(self.stopped or 0)
        assert sp_after == after, (hex(sp_after), hex(after))
        return registers, self.read(DATA, SIZE)

    def fire(self, place, count):
        player = self.playing(place)
        self.word(player + 0x110 + 0x3B8, 0)
        self.word(player + 0x110 + 0x3BC, 0)
        return self.call(ON_FIRE, {UC_X86_REG_ECX: player + 0x110}, (count,))

    def hit(self, victim, attacker, bullet=0, flag=1, pain=0.25):
        f32 = lambda value: struct.unpack("<I", struct.pack("<f", value))[0]
        return self.call(HIT_EVENT, {UC_X86_REG_ECX: self.playing(victim)},
                         (5000, attacker, victim, f32(0.5), f32(pain), bullet, flag))

    def kill(self, victim, killer, source=None, headshot=False, bullet=0, position=True, stock=False):
        """The handler told of `victim`'s death by `killer` (0xFF: nobody)
        with item `source` (None: no source), as generate_killed_event's
        code tells it from its own frame."""
        player = self.playing(killer) if killer < COUNT else 0
        at = self.read(player + 0x40C + 0x30, 12) if player and position else bytes(12)
        frame = {
            0x10: bytes([0]), 0x14: bytes([stock]), 0x18: struct.pack("<I", 0),
            0x1C: bytes([self.read(player + 0x115, 1)[0] if player else 0]), 0x20: bytes([0]),
            0x24: struct.pack("<I", 0xFFFFFFFF), 0x34: at,
            0x44: struct.pack("<II", 7000, killer),
            0x50: struct.pack("<I", ITEMS + 0x800 * source + 0x10 if source is not None else 0),
            0x54: bytes([headshot]),
        }
        return self.call(KILL_FROM, {UC_X86_REG_ESI: self.playing(victim), UC_X86_REG_EBP: bullet,
                                     UC_X86_REG_EDI: 0, UC_X86_REG_EAX: CORE}, stop=KILL_TO, frame=frame)

    def tick(self, place, alive=True):
        """base_player::tick's site for the place's player, to the push after it."""
        player = self.playing(place)
        self.emu.mem_write(player + 0x33C, bytes([alive]))
        registers, data = self.call(TICK, {UC_X86_REG_EBX: player, UC_X86_REG_EDI: 123450}, stop=TICK_AFTER,
                                    frame={0x10: struct.pack("<I", 0x5EC0DE)}, pushed=4)
        assert self.read(hide.STACK + 0x9000 - 4) == 123450, 'push edi replayed'
        self.emu.mem_write(player + 0x33C, b"\x01")
        return registers, data

    def construct(self):
        """The statistics' constructor, then a match's start."""
        result = self.call(CONSTRUCT, {UC_X86_REG_EDI: SHARED})
        self.start()
        return result

    def reset(self):
        """The reset between matches (it forgets the options and the count), then the next match's start."""
        result = self.call(RESET, {UC_X86_REG_EDI: SHARED})
        self.start()
        return result

    def serialize(self):
        self.written = bytearray()
        self.call(SERIALIZE, {UC_X86_REG_ESI: SHARED, UC_X86_REG_EAX: WRITER})
        return bytes(self.written)

    def deserialize(self, stream):
        self.emu.mem_write(STREAM, stream + bytes(16))
        for offset, value in ((0, STREAM), (4, STREAM), (8, len(stream))):
            self.word(READER + offset, value)
        self.call(DESERIALIZE, {UC_X86_REG_ESI: SHARED, UC_X86_REG_EAX: READER})
        return self.read(READER + 4) - STREAM

    def total(self, place, number):
        """The record's count of an event over the three kits."""
        return sum(struct.unpack("<H", self.read(event(place, number, kit), 2))[0] for kit in range(KITS))


ENTRY = struct.Struct("<8HIIHI")         # the dict id, then COUNTS
COUNTS = ("kills", "headshots", "wall", "ricochet", "deaths", "shots", "hits", "damage", "distance", "longest", "held")
EVENTS = {"kills": 0, "headshots": 15, "deaths": 2, "shots": 22, "hits": 23}   # the record's event each adds up to


def table(block):
    """A block's places: {dict id: its COUNTS} each."""
    version, length = struct.unpack_from("<BH", block)
    assert version == 1 and length == len(block) - 3, (version, length, len(block))
    at, places = 3, []
    for _ in range(COUNT):
        count = block[at]
        at += 1
        entries = [ENTRY.unpack_from(block, at + ENTRY.size * i) for i in range(count)]
        at += ENTRY.size * count
        assert count <= 16 and [e[0] for e in entries] == sorted({e[0] for e in entries}), entries
        places.append({e[0]: e[1:] for e in entries})
    assert at == len(block), (at, len(block))
    return places


def play(run):
    """The same match in either exe: the calls, and after each the registers
    and the game's bytes; and what the build's table must hold after them."""
    steps, expected = [], [dict() for _ in range(COUNT)]

    def count(place, dict_id, **added):
        entry = expected[place].setdefault(dict_id, dict.fromkeys(COUNTS, 0))
        for name, amount in added.items():
            entry[name] = max(entry[name], amount) if name == "longest" else entry[name] + amount

    steps.append(run.reset())
    # Places 0, 2, 4 .. are of team 0, the odd ones of team 1. Item i's dict id is 100 + i.
    run.hold(0, 1), run.hold(1, 2), run.hold(2, 3), run.hold(3, None, slot=None)
    run.stand(0, 10.0), run.stand(1, 40.0, 0.0, 40.0), run.stand(2, 40.0, 3.04, 40.0), run.stand(3, 7000.0)

    # Shots: a rifle's, a shotgun's pellets, another kit's weapon, a battery in the hands, nothing.
    for place, bullets, weapon in ((0, 1, 101), (0, 1, 101), (1, 8, 102), (2, 3, 103)):
        steps.append(run.fire(place, bullets))
        count(place, weapon, shots=bullets)
    run.kit(0, 2), run.hold(0, 4, slot=10)
    steps.append(run.fire(0, 5))
    count(0, 104, shots=5)
    run.carry(0, BATTERY)
    steps.append(run.fire(0, 2))
    count(0, 0, shots=2)
    run.kit(0, 0), run.hold(0, 1)
    steps.append(run.fire(0, 0x12345))
    count(0, 101, shots=0x2345)

    # Hits. An enemy's bullet counts; a teammate's, nobody's, one without the flag and the gun's
    # own stock do not (the stock's and the flagless one's pain still is the weapon's).
    rifle, shotgun, other = run.bullet(0, 1), run.bullet(1, 2), run.bullet(2, 9)
    steps.append(run.hit(1, 0, rifle, pain=0.25))
    count(0, 101, hits=1, damage=2500)
    steps.append(run.hit(1, 0, other, pain=0.1))
    count(0, 109, hits=1, damage=1000)
    steps.append(run.hit(0, 1, shotgun, pain=0.3333))
    count(1, 102, hits=1, damage=3333)
    steps.append(run.hit(2, 0, rifle))
    steps.append(run.hit(1, 0xFF))                # nobody's: no bullet names nobody
    steps.append(run.hit(1, 0, rifle, flag=0, pain=0.5))
    count(0, 101, damage=5000)
    steps.append(run.hit(1, 0, run.bullet(3, 1, stock=True), pain=0.2))
    count(0, 101, damage=2000)
    steps.append(run.hit(1, 0, 0, pain=1.0))          # a grenade: no bullet
    count(0, 0, hits=1, damage=10000)
    steps.append(run.hit(3, 2, run.bullet(5, 3), pain=float("nan")))
    count(2, 103, hits=1)

    # Kills: place 0 kills place 1 with its rifle by a headshot from 50 m; with a grenade (item 6)
    # with another weapon in its hands; place 1 kills place 2 from 3.04 m; a team kill, a suicide,
    # a death by nobody: a death each, no kill. Place 3 (nothing in the hands) from 7 km: the
    # longest a u16 holds; a killer with no position: no distance.
    steps.append(run.kill(1, 0, source=1, headshot=True, bullet=rifle))
    count(0, 101, kills=1, headshots=1, distance=500, longest=500), count(1, 102, deaths=1)
    steps.append(run.kill(1, 0, source=GRENADE))
    count(0, 106, kills=1, distance=500, longest=500), count(1, 102, deaths=1)
    steps.append(run.kill(2, 1, source=2, bullet=shotgun))
    count(1, 102, kills=1, distance=30, longest=30), count(2, 103, deaths=1)
    steps.append(run.kill(2, 0, source=1, headshot=True))
    count(2, 103, deaths=1)
    steps.append(run.kill(0, 0, source=1))
    count(0, 101, deaths=1)
    steps.append(run.kill(3, 0xFF))
    count(3, 0, deaths=1)
    steps.append(run.kill(0, 3, source=None))
    count(3, 0, kills=1, distance=0xFFFF, longest=0xFFFF), count(0, 101, deaths=1)
    steps.append(run.kill(0, 1, source=2, position=False))
    count(1, 102, kills=1), count(0, 101, deaths=1)
    # What the killing bullet keeps: a pierce, a pierce and a bounce, a bounce alone; the stock's
    # counts are not looked at.
    for index, (pierces, bounces, stock) in enumerate(((1, 0, False), (2, 1, False), (0, 2, False), (1, 1, True))):
        through = run.bullet(8 + index, 1, stock=stock, pierces=pierces, bounces=bounces)
        steps.append(run.kill(1, 0, source=1, bullet=through, stock=stock))
        count(0, 101, kills=1, distance=500, longest=500, wall=int(pierces > 0 and not stock),
              ricochet=int(bounces > 0 and not stock))
        count(1, 102, deaths=1)

    # Ticks: 50 ms each for what a live player holds; none known with a battery or nothing in the
    # hands; a dead player's are not counted.
    for place, weapon in ((0, 101), (0, 101), (1, 102), (3, 0)):
        steps.append(run.tick(place))
        count(place, weapon, held=50)
    steps.append(run.tick(1, alive=False))
    run.carry(2, BATTERY)
    steps.append(run.tick(2))
    count(2, 0, held=50)

    # A seventeenth weapon of a place is counted with no weapon known.
    for index in range(10, 28):
        run.hold(5, index)
        steps.append(run.fire(5, 1))
        count(5, 100 + index if index < 25 else 0, shots=1)
    return steps, [{dict_id: tuple(entry.values()) for dict_id, entry in place.items()} for place in expected]


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    # The original's serialize walks the options' places; the build's the twenty (STATISTICS_COUNT).
    stock, r = Run(original), Run(patched)
    (before, _), (after, expected) = play(stock), play(r)
    assert len(before) == len(after)
    for index, ((stock_regs, stock_data), (regs, data)) in enumerate(zip(before, after)):
        assert regs == stock_regs, (index, regs, stock_regs)
        assert data == stock_data, (index, [hex(DATA + i) for i in range(SIZE) if data[i] != stock_data[i]][:8])
    print('ok on_fire, generate_hit_event, the handler\'s kill call, base_player::tick\'s site and the reset: '
          '%d calls, every register and every byte of the game\'s objects the original\'s' % len(after))

    native, world = stock.serialize(), r.serialize()
    assert world[:len(native)] == native and len(native) > 20 * 30
    counted = table(world[len(native):])
    assert counted == expected, (counted, expected)
    print('ok serialize: the original\'s bytes, then the table (%d bytes): each event under its place and weapon'
          % (len(world) - len(native)))
    for place, weapons in enumerate(counted):
        for name, number in EVENTS.items():
            total = sum(entry[COUNTS.index(name)] for entry in weapons.values())
            mask = 0xFFFF if number in (22, 23) else 0xFF       # the record carries the others as a byte
            assert total & mask == r.total(place, number) & mask, (place, name, total, r.total(place, number))
    assert r.total(0, 0) == 6 and r.total(0, 2) == 3 and r.total(0, 22) == 2 + 5 + 2 + 0x2345 and r.total(0, 23) == 3
    print('ok each place\'s weapons add up to its record: kills (event 0), headshots (15), deaths (2), '
          'shots (22), hits (23)')

    # A rollback: more is counted, the world is loaded, the next world is the one loaded.
    r.fire(0, 7), r.kill(1, 0, source=1), r.hit(1, 0, r.bullet(0, 1))
    assert r.serialize() != world
    assert r.deserialize(world) == len(world) and r.serialize() == world
    print('ok a load gives the statistics and the table back: the next world is the one loaded')
    # A second game, which counted nothing, loads it and counts on as the first does.
    joiner = Run(patched)
    assert joiner.serialize() == Run(original).serialize()
    assert joiner.deserialize(world) == len(world) and joiner.serialize() == world
    for game in (r, joiner):
        game.hold(0, 1)
        game.fire(0, 3)
    assert joiner.serialize() == r.serialize() != world
    print('ok a game that counted nothing writes no table; from its load of the world it writes the same worlds')

    # A game's statistics as its network client makes them: counted from their constructor on.
    worlds = []
    for data in (original, patched):
        game = Run(data)
        made = game.construct()
        game.hold(0, 1)
        game.fire(0, 3)
        worlds.append((made, game.serialize()))
    (stock_made, native), (made, world) = worlds
    assert made == stock_made and world[:len(native)] == native
    assert table(world[len(native):]) == [{101: (0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0)}] + [{}] * 19
    print('ok the constructor: as the original\'s; the statistics are counted from it on')

    # A world without the table (an older game's): its own fields load, and no table is written
    # until the statistics are reset.
    native = stock.serialize()
    for game in (r, stock):
        assert game.deserialize(native) == len(native)
        game.hold(0, 1)
        game.fire(0, 3), game.kill(1, 0, source=1)
    assert r.serialize() == stock.serialize() != native
    for game in (r, stock):
        game.reset()
        game.fire(0, 3)
    world, native = r.serialize(), stock.serialize()
    assert world[:len(native)] == native
    assert table(world[len(native):]) == [{101: (0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0)}] + [{}] * 19
    print('ok a world without the table: the statistics load, the game writes none until their reset')
    # A table that does not read (a version this game does not know) is none.
    assert r.deserialize(native + bytes([2, 1, 0, 0])) == len(native) + 4
    assert r.serialize() == native
    print('ok a table of another version: read past, none kept')

if __name__ == '__main__':
    main()
