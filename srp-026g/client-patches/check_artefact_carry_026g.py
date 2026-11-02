#!/usr/bin/env python3
"""Execute patch 78's linked Rust and native inventory/world lookup in Unicorn.

Usage: python check_artefact_carry_026g.py <original survarium.exe>
Renderer queues, physics ray results, the byte writer and item event bodies
are fixtures. Carry hooks, state/pose code, snapshot reader, inventory
removal/insertion and world active-kit lookup are the actual executable.
This is an integration check, not an in-game rendering/playtest claim.
"""
from pathlib import Path
import struct
import math
import sys

import patch_026g as base
from check_blackout_026g import Run as InputBuilder
from check_artefacts_026g import STACK, STOP, REGISTER
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tools/re-kit/v026g'))
from check_inventory_transfer import InventoryGame
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EBP

REGISTER = {**REGISTER, 'ebp': UC_X86_REG_EBP}

IDENTITY = (1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.)
# The carry chord's marker (bit 29) with quick slot 0's bit; slot n's is n bits on.
COMMAND, USE = (1 << 29) | (1 << 14), 1 << 21
def slot(n): return (1 << 29) | (1 << (14 + n))


def palm_turned(model, hand, scale):
    """The artefact keeps the palm's normal (row 1) and its scale, and turns at
    most 0.15 rad about that normal as it floats."""
    row = lambda m, r: m[r * 4:r * 4 + 3]
    dot = lambda a, b: sum(x * y for x, y in zip(a, b))
    h0, h1, h2 = (row(hand, r) for r in range(3))
    m0, m1, m2 = (row(model, r) for r in range(3))
    assert all(abs(a - scale * b) < .0001 for a, b in zip(m1, h1)), "artefact lost the palm's normal"
    c, d = dot(m0, h0) / scale, dot(m0, h2) / scale
    assert abs(math.hypot(c, d) - 1) < .001 and abs(math.atan2(d, c)) <= .151, "artefact turned too far"
    for a, b in zip(m0, [scale * (c * x + d * z) for x, z in zip(h0, h2)]): assert abs(a - b) < .0001
    for a, b in zip(m2, [scale * (c * z - d * x) for x, z in zip(h0, h2)]): assert abs(a - b) < .0001


class Run(InventoryGame):
    def __init__(self, data, callback=(0x340, 0x25FB80, 0x25FB86)):
        super().__init__(data)
        self.ends, self.saved, self.rendered = (), bytearray(), []
        self.wall = False
        self.floor = None
        stub = STOP + 0x900
        self.word(0x870118, stub)
        self.emu.mem_write(stub, bytes.fromhex('b801000000 c3'))
        # The log's imports (patch 85 logs a pickup command it refuses: two
        # merged quick bits spell one): GetCurrentProcessId, CreateFileA,
        # WriteFile, CloseHandle.
        for n, (iat, code) in enumerate(((0x8701B8, 'b82a000000 c3'), (0x870048, 'b844000000 c21c00'),
                                         (0x870108, 'b801000000 c21400'), (0x870040, 'b801000000 c20400'))):
            self.word(iat, stub + 0x10 * (n + 1))
            self.emu.mem_write(stub + 0x10 * (n + 1), bytes.fromhex(code))
        self.emu.hook_add(UC_HOOK_CODE, self.stop_at)
        for at in (0x1F3DE0, 0x75E20, 0x21CD0, 0x21F40, 0x22280):
            self.emu.hook_add(UC_HOOK_CODE, self.fixture, begin=at, end=at)
        self.world = self.block(0x19000)
        self.roster = self.block(0x60)
        self.word(self.world + 0x18458, self.roster)
        self.word(self.world + 0x1845C, self.roster + 0x60)
        self.word(self.world + 0x18FD8, 3)
        self.players = [self.player(n) for n in range(2)]
        self.item_ptr = self.item()
        offset, self.draw_entry, self.draw_past = callback
        self.block(offset - 0x340 + 0x100)
        self.callback = self.item_ptr + offset
        self.word(self.item_ptr + 0x280, 3)
        self.emu.mem_write(self.item_ptr + 0x27E, bytes([7]))
        self.word(self.item_ptr + 0x28C, 12345)
        self.inventories = [self.slots([self.item_ptr]), self.slots([0])]
        client_vt = {0x340: 0xA06124, 0x398: 0xA05F44, 0x498: 0xA05FE4, 0xF20: 0xA06084}[offset]
        self.word(self.item_ptr, client_vt)
        for n in (0x28, 0x30, 0x34, 0x58): self.word(client_vt + n, self.read(self.vtable + n))
        self.word(client_vt + 0x30, 0x313470)
        self.word(client_vt + 0x34, 0x313490)
        self.word(client_vt + 0x7C, self.taken)
        self.word(client_vt + 0x84, self.removed)
        self.word(self.item_ptr + 0x248, self.world)
        registry = self.block(8 * 4)
        self.word(registry + 7 * 4, self.item_ptr)
        self.word(self.world + 0x18940, registry)
        self.word(self.world + 0x18944, registry + 8 * 4)
        for player, inventory in zip(self.players, self.inventories):
            self.word(player + 0x10C, inventory)
        self.packet = self.block(8)
        self.reader = self.block(12)
        self.snapshot = self.block(64)

    def player(self, place):
        p, profile, weapon = self.block(0xF500), self.block(0x4600), self.block(0x1820)
        self.word(p + 0xD0, 2)
        self.emu.mem_write(p + 0x114, bytes([place, 0]))
        self.word(p + 0x120, self.world)
        self.word(p + 0x128, weapon)
        self.word(p + 0x6B8, profile)
        self.word(p + 0x350, self.world)
        self.emu.mem_write(p + 0x33C, bytes([1, 0, 0, 1]))
        self.word(weapon, 0xA04174)
        self.word(self.roster + place * 0x30, p)
        transform = list(IDENTITY)
        self.emu.mem_write(p + 0x40C, struct.pack('<16f', *transform))
        transform[13], transform[14] = 1.6, 1.0 if place == 0 else 0.0
        self.emu.mem_write(p + 0x44C, struct.pack('<16f', *transform))
        return p

    def stop_at(self, emu, address, *_):
        if address in self.ends:
            emu.emu_stop()

    def fixture(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        pops = 0
        if address == 0x1F3DE0:
            self.saved.extend(emu.mem_read(emu.reg_read(UC_X86_REG_EAX), 1))
            pops = 20
        elif address == 0x75E20:
            result = self.read(sp + 4)
            direction = struct.unpack('<3f', emu.mem_read(self.read(sp + 12), 12))
            falling = direction == (0., -1., 0.) and self.floor is not None
            self.word(result + 0x1C, 1 if self.wall or falling else 0)
            if falling:
                origin = struct.unpack('<3f', emu.mem_read(self.read(sp + 8), 12))
                emu.mem_write(result, struct.pack('<3f', origin[0], self.floor, origin[2]))
            assert self.read(sp + 20) == 0x24 and self.read(sp + 24) == 2
            pops = 32
        else:
            reference = emu.reg_read(UC_X86_REG_EAX)
            instance = self.read(reference)
            op = {0x21CD0: 'add', 0x21F40: 'remove', 0x22280: 'update'}[address]
            before = emu.mem_read(instance + 0x10D, 1)[0]
            assert (op != 'add' or before == 0) and (op != 'remove' or before == 1)
            emu.mem_write(instance + 0x10D, bytes([0 if op == 'remove' else 1]))
            matrix = None if op == 'remove' else struct.unpack('<16f', emu.mem_read(self.read(sp + 12), 64))
            self.rendered.append((op, instance, matrix))
            pops = 8 if op == 'remove' else 16
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def site(self, start, ends, args=(), **regs):
        sp = STACK - 0x100
        self.word(sp, STOP)
        for n, arg in enumerate(args): self.word(sp + 4 + n * 4, arg)
        for name, value in regs.items(): self.emu.reg_write(REGISTER[name], value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.ends = tuple(ends)
        self.emu.emu_start(start, STOP, count=2_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) in (*ends, STOP)
        self.ends = ()

    def input(self, player, now, actions):
        self.word(self.packet, 0)
        self.word(self.packet + 4, actions)
        self.site(0x2D86E0, [0x2D86E6], (now, self.packet), ecx=player)
        assert self.read(self.packet + 4) == actions, 'shared packet was mutated'
        # Original set_input's copy, then the actual pre-quick-slot hook.
        self.word(player + 0x330, actions)
        self.site(0x2D876F, [0x2D8774], edi=player, eax=player + 0x32C)
        return self.read(player + 0x330)

    def tick(self, player, now):
        self.word(player + 0x324, now)
        self.site(0x2DAAC1, [0x2DAAC7], ebx=player)

    def save(self, player):
        self.saved.clear()
        self.site(0x2DB47E, [0x2DB483], ebx=player, edi=self.snapshot, ebp=STACK)
        assert len(self.saved) == 32
        return bytes(self.saved)

    def load(self, player, snapshot):
        self.emu.mem_write(self.snapshot, snapshot)
        for offset, value in ((0, self.snapshot), (4, self.snapshot), (8, len(snapshot))): self.word(self.reader + offset, value)
        self.site(0x2DBAB5, [0x2DBABC], ebx=player, esi=self.reader)
        assert self.read(self.reader + 4) == self.snapshot + 32

    def static_instance(self):
        instance, resource = self.block(0x300), self.block(0x140)
        self.word(instance + 0x290, resource)
        self.emu.mem_write(resource + 0x108, struct.pack('<6f', -.1, -.1, -.1, .1, .1, .1))
        return instance

    def renderer(self):
        game, scene = self.block(0x5400), self.block(0x100)
        renderer = 0x31000000
        self.emu.mem_map(renderer, 0x201000, UC_PROT_ALL)
        self.word(self.world + 0x9C, game)
        self.word(game + 0x5360, renderer)
        self.word(renderer + 0x200064, scene)
        self.word(self.callback + 0xC, self.world)
        self.word(self.callback + 0x10, 3)
        model, instance = self.block(0x120), self.static_instance()
        self.word(model + 0x108, instance)
        self.word(self.callback + 0x14, model)
        p = self.players[0]
        self.word(p + 0xF474, self.world)
        gun_model, gun_instance = self.block(0x120), self.block(0x120)
        self.word(gun_model + 0x108, gun_instance)
        self.emu.mem_write(gun_instance + 0x10D, b'\1')
        self.word(self.read(p + 0x128) + 0x1810, gun_model)
        self.word(self.read(p + 0x128) + 0x180C, self.world)
        self.emu.mem_write(self.read(p + 0x128) + 0x660, struct.pack('<16f', *IDENTITY))
        self.matrix = self.block(64)
        self.emu.mem_write(self.matrix, struct.pack('<16f', *IDENTITY))
        self.frame, self.bones = self.block(0x200) + 0x100, self.block(8 * 64)
        skeleton, model = self.block(0x300), self.block(0x120)
        self.word(p + 0xF290, model)
        self.word(model + 0x10C, skeleton)
        self.word(p + 0x408, skeleton)
        names = ['object_movement', 'Root', 'LeftArm', 'LeftForeArm', 'LeftHand', 'RightArm', 'RightForeArm', 'RightHand', 'Weapon']
        parents = [0, 0, 1, 2, 3, 1, 5, 6, 1]
        self.word(skeleton + 0x108, len(names))
        for n, (name, parent) in enumerate(zip(names, parents)):
            text = self.block(len(name) + 1)
            self.emu.mem_write(text, name.encode() + b'\0')
            self.emu.mem_write(skeleton + 0x128 + n * 28, bytes([1 if name == "Weapon" else 3]))
            self.word(skeleton + 0x110 + n * 28, text)
            self.word(skeleton + 0x114 + n * 28, skeleton + 0x110 + parent * 28)
        self.word(self.frame - 0x14, self.bones)
        self.word(self.frame - 0x1C, 8)
        for i in range(8):
            m = list(IDENTITY); m[12], m[13] = i * 0.05, 1.3
            self.emu.mem_write(self.bones + i * 64, struct.pack('<16f', *m))
        return instance, gun_instance

    def draw(self):
        p = self.players[0]
        self.site(0x247716, [0x24771C], ebx=p, ebp=self.frame)
        self.site(0x2477C8, [0x2477CE, 0x2477DF], ebx=p, eax=self.matrix)
        self.site(self.draw_entry, [self.draw_past, STOP], ecx=self.callback)


def flower_lifecycle(built):
    g = Run(built, callback=(0x498, 0x25F100, 0x25F106))
    p = g.players[0]
    original, _ = g.renderer()
    held, instance = g.block(0x120), g.static_instance()
    g.word(held + 0x108, instance); g.word(held + 0xD0, 1)
    result = g.block(0xC00); g.word(result + 0x38, 4); g.word(result + 0x9CC, held)
    # Actual query hook keeps the native three requests, callback and parent.
    path = g.block(100)
    g.emu.mem_write(path, b'item/artefacts/artefact_givokost_single\0')
    requests = [(path, 15), (0x1111, 61), (0x2222, 7)]
    sp = STACK - 0x100
    g.emu.mem_write(sp + 0x2C, struct.pack('<6I', *(v for r in requests for v in r)))
    queried = []
    def query(emu, address, *_):
        ptr, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        assert count == 4
        actual = struct.unpack('<8I', emu.mem_read(ptr, 32))
        assert actual[:6] == tuple(v for r in requests for v in r)
        expected = b'item/artefacts/artefact_givokost_single_held\0'
        assert bytes(emu.mem_read(actual[6], len(expected))) == expected
        assert actual[7] == 15
        top = emu.reg_read(UC_X86_REG_ESP)
        assert [g.read(top + 4 + i*4) for i in range(5)] == [STOP, 11, 12, 13, 14]
        queried.append(True)
        emu.reg_write(UC_X86_REG_EIP,g.read(top)); emu.reg_write(UC_X86_REG_ESP,top+4)
    handle = g.emu.hook_add(UC_HOOK_CODE,query,begin=0xE5B60,end=0xE5B60)
    g.site(0x25E91A,[0x25E928],args=[11,12,13,14])
    assert queried
    g.emu.hook_del(handle)
    g.site(0x260278,[0x26027E],edi=g.item_ptr,ebx=result)
    assert g.read(held + 0xD0) == 2
    g.input(p,0,COMMAND);g.tick(p,800);g.draw()
    assert any(model == instance and op == 'add' for op, model, _ in g.rendered) and g.emu.mem_read(original + 0x10D,1) == b'\0'
    # Lowering returns to the untouched original model, without leaking the
    # flower in the scene. Re-offer also reuses this exact resource.
    g.input(p,900,1 << 5);g.tick(p,1800);g.draw()
    assert g.emu.mem_read(instance + 0x10D,1) == b'\0'
    g.input(p,1900,0);g.input(p,2000,COMMAND);g.tick(p,2800);g.draw()
    assert g.emu.mem_read(instance + 0x10D,1) == b'\1'
    # Destruction removes a raised flower, consumes just our ref and allows
    # the native destructor to proceed with its normal model still intact.
    g.word(g.world + 4,1)
    g.site(0x25EFE0,[0x25EFE9],ecx=g.item_ptr)
    assert g.emu.mem_read(instance + 0x10D,1) == b'\0'
    assert g.read(held + 0xD0) == 1
    assert g.read(g.callback + 0x14) != held
    print('ok: held flower query/retention, raise/lower/re-offer and destructor cleanup')


def main():
    original = Path(sys.argv[1]).read_bytes()
    built, _ = base.build(original, 'section', Path(base.default_bin('section')).read_bytes(), None, Path(base.late_join_bin()).read_bytes())
    flower_lifecycle(bytes(built))
    switch_items(bytes(built))
    death_drop(bytes(built), death=0x2D77C0)
    death_drop(bytes(built))
    death_drop(bytes(built), (0x498, 0x25F100, 0x25F106))
    moving_draw(bytes(built))
    death_with_optics_hidden(bytes(built))
    death_rolled_back(bytes(built))
    recipient_hud(bytes(built))
    container_show(bytes(built))
    builder = InputBuilder(bytes(built))
    quick, sprint = 1 << 14, 1 << 9
    assert builder.input(quick) == (quick, True)
    assert builder.input(sprint | quick) == (sprint | quick, True), 'sprint still triggers carry'
    builder.input(0)
    # H's events as the game pushes them (0.10b player_input_handler::action_state_enum;
    # on_keyboard_action is given them as input::enum_keyboard_action 1, 2, 3).
    DOWN, UP, HOLD = 0, 1, 2
    for state in (DOWN, HOLD):
        assert builder.input(quick, held=state) == (COMMAND, True)
        assert builder.input(quick, held=UP) == (COMMAND, True), 'H release activated the held quick key'
        assert builder.input(0) == (0, True)
        assert builder.input(quick) == (quick, True)
        builder.input(0)
    assert builder.input(quick | 1 << 15, held=DOWN) == (0, True), 'two quick keys with H offered or used one'
    builder.input(0)
    assert builder.input(sprint | quick, held=HOLD) == (sprint | COMMAND, True)
    assert builder.input(sprint, held=UP) == (sprint, True)
    print('ok: the artefact action (H) held triggers carry, Shift does not, and its release cannot activate the item')
    # A tick may bring no event of H at all: H down, a tick with none, then
    # 4 is the chord (never 4's quick use), as 4 then H.
    assert builder.input(0, held=DOWN) == (0, True)
    assert builder.input(0) == (0, True)
    assert builder.input(quick) == (COMMAND, True), 'H held, then a quick key, activated the item'
    assert builder.input(quick) == (COMMAND, True)
    assert builder.input(quick, held=UP) == (COMMAND, True)
    assert builder.input(0) == (0, True)
    assert builder.input(quick) == (quick, True), 'H released still took the quick key'
    builder.input(0)
    assert builder.input(quick, held=DOWN) == (COMMAND, True)
    assert builder.input(0, held=UP) == (0, True)
    builder.input(0)
    # Held: the engine sends H's hold events; they keep it down (the owner's
    # rev 38 test: H+4 activated and 4 alone raised when hold read as up).
    assert builder.input(0, held=DOWN) == (0, True)
    assert builder.input(0, held=HOLD) == (0, True)
    assert builder.input(quick, held=HOLD) == (COMMAND, True), 'H held (hold events), then 4, activated the item'
    assert builder.input(0, held=UP) == (0, True)
    assert builder.input(quick) == (quick, True), 'after H up, 4 alone raised the artefact'
    builder.input(0)
    print('ok: H held, then a quick key, offers the artefact (no event of H between), as a quick key, then H')
    # The relays and replays keep 31 bits (0x31847B); a node ORs two inputs
    # together (0x2C5B60): both arrive here as the game reads them.
    g = Run(bytes(built)); p = g.players[0]
    for n in range(6):
        assert slot(n) & 0x7FFFFFFF == slot(n)
    assert g.input(p, 0, slot(0) | slot(3)) == 0, 'a merged chord offered or used an item'
    assert g.save(p)[13] == 0
    assert g.input(p, 10, 0) == 0
    assert g.input(p, 20, slot(0) & 0x7FFFFFFF) == 0 and g.save(p)[13] == 1
    print('ok: chord commands survive the 31-bit wire; merged chords are no command and use nothing')
    g = Run(bytes(built)); donor, recipient = g.players
    assert g.input(donor, 0, COMMAND) == 0
    # Sprint remains suppressed during carry even though it is no longer
    # the chord modifier; a held sprint cannot cancel or activate the item.
    assert g.input(donor, 80, sprint) == 0
    assert g.input(donor, 100, 0) == 0
    raising = g.save(donor); assert raising[13] == 1
    g.tick(donor, 200); assert g.save(donor)[13] == 2
    g.tick(donor, 800); offering = g.save(donor); assert offering[13] == 3
    artefact_model, weapon_model = g.renderer()
    initial_bones = bytes(g.emu.mem_read(g.bones, 8 * 64))
    g.draw()
    rendered_bones = bytes(g.emu.mem_read(g.bones, 8 * 64))
    g.emu.mem_write(g.bones, initial_bones)
    # Native damage mode leaves non-hit bones uncomputed. Poison Weapon;
    # checking every matrix used to reject the entire carry layer here.
    poison = struct.pack('<16f', *([float('nan')] * 16))
    g.emu.mem_write(g.bones + 7 * 64, poison)
    g.site(0x2DA769, [0x2DA76F], edi=donor, esi=g.bones, ebx=8)
    assert bytes(g.emu.mem_read(g.bones, 7 * 64)) == rendered_bones[:7 * 64], "damage and visual arm poses disagree"
    assert bytes(g.emu.mem_read(g.bones + 7 * 64, 64)) == poison
    assert [(op, model) for op, model, _ in g.rendered] == [('add', artefact_model), ('remove', weapon_model)]
    first = next(matrix for op, model, matrix in g.rendered if model == artefact_model and op == 'add')
    hand = struct.unpack('<16f', g.emu.mem_read(g.bones + 6 * 64, 64))
    for n in range(3): assert abs(first[12 + n] - (hand[12 + n] + -hand[n] * .13 + hand[4 + n] * (.12 + .065 + .012 * math.sin(800 * math.tau / 4000)))) < 0.0001
    palm_turned(first, hand, .65)
    g.emu.mem_write(g.bones, initial_bones)  # native render recomputes the damage-only poisoned bone
    g.rendered.clear(); g.draw()
    assert [op for op, _, _ in g.rendered] == ['update']
    print('ok: matching damage/render poses, floating RightHand attachment, weapon removal and idempotent model update')
    # The held hand breathes with the clock; the damage pass moves with it.
    g.emu.mem_write(g.bones, initial_bones)
    g.tick(donor, 850)
    g.draw()
    breathed = bytes(g.emu.mem_read(g.bones, 8 * 64))
    assert breathed[:7 * 64] != rendered_bones[:7 * 64], 'the held hand did not move with the clock'
    wrist = lambda bones: struct.unpack_from('<3f', bones, 6 * 64 + 48)
    moved = math.dist(wrist(breathed), wrist(rendered_bones))
    assert 0 < moved < .02, f'the held hand moved {moved:.4f} m in 50 ms'
    assert g.rendered[-1][2] != first
    g.emu.mem_write(g.bones, initial_bones); g.emu.mem_write(g.bones + 7 * 64, poison)
    g.site(0x2DA769, [0x2DA76F], edi=donor, esi=g.bones, ebx=8)
    assert bytes(g.emu.mem_read(g.bones, 7 * 64)) == breathed[:7 * 64], "damage and visual arm poses disagree later"
    # A deliberately offset, oversized resource exercises the original
    # static get_aabb and prevents a mesh origin/size from moving the hover.
    resource = g.read(artefact_model + 0x290)
    g.emu.mem_write(resource + 0x108, struct.pack('<6f', 1., -2., 3., 1.5, -1.7, 3.4))
    old_eye = bytes(g.emu.mem_read(donor + 0x44C, 64))
    for pitch in (-.7, 0., .7):
        cp, sp = math.cos(pitch), math.sin(pitch)
        basis = ((1., 0., 0.), (0., cp, -sp), (0., sp, cp))
        eye = list(struct.unpack('<16f', old_eye))
        for row in range(4):
            eye[row*4:row*4+3] = [sum(eye[row*4+k] * basis[k][j] for k in range(3)) for j in range(3)]
        g.emu.mem_write(donor + 0x44C, struct.pack('<16f', *eye))
        rotated = bytearray()
        for i in range(8):
            m = list(struct.unpack_from('<16f', initial_bones, i * 64))
            for row in range(4):
                m[row * 4:row * 4 + 3] = [sum(m[row * 4 + k] * basis[k][j] for k in range(3)) for j in range(3)]
            rotated.extend(struct.pack('<16f', *m))
        g.emu.mem_write(g.bones, bytes(rotated)); g.draw()
        hand = struct.unpack('<16f', g.emu.mem_read(g.bones + 6 * 64, 64))
        drawn = bytes(g.emu.mem_read(g.bones, 7 * 64))
        model = g.rendered[-1][2]
        for j in range(3):
            actual = model[12+j] + sum(c * model[r*4+j] for r,c in enumerate((1.25,-2.,3.2)))
            expected = hand[12+j] - .13 * hand[j] + (.12 + .012 * math.sin(850 * math.tau/4000)) * hand[4+j]
            assert abs(actual-expected) < .0001
        palm_turned(model, hand, .28)
        g.emu.mem_write(g.bones, bytes(rotated)); g.emu.mem_write(g.bones + 7*64, poison)
        g.site(0x2DA769, [0x2DA76F], edi=donor, esi=g.bones, ebx=8)
        assert bytes(g.emu.mem_read(g.bones, 7*64)) == drawn
    g.emu.mem_write(donor + 0x44C, old_eye)
    g.emu.mem_write(resource + 0x108, struct.pack('<6f', -.1, -.1, -.1, .1, .1, .1))
    g.emu.mem_write(g.bones, initial_bones)
    print('ok: native model bounds fit and centre oversized meshes; attachment and damage bones follow pitch')
    # LOS and team refusals precede any native mutation.
    g.wall = True; g.input(recipient, 900, USE)
    assert g.contents(g.inventories[0], 1) == [g.item_ptr]
    g.input(recipient, 910, 0); g.wall = False
    g.word(g.read(recipient + 0x6B8) + 0x45EC, 1); g.input(recipient, 920, USE)
    assert g.contents(g.inventories[0], 1) == [g.item_ptr]
    g.word(g.read(recipient + 0x6B8) + 0x45EC, 0); g.input(recipient, 930, 0)
    g.word(recipient + 0x128, 0)  # taking does not require holding a weapon
    occupied = g.item()
    recipient_slot = g.read(g.inventories[1] + 0x168)
    g.word(recipient_slot, occupied)
    before_events = list(g.events)
    assert g.input(recipient, 932, USE) & USE == USE, 'a full container took Use from the game'
    assert g.contents(g.inventories[0], 1) == [g.item_ptr]
    assert g.contents(g.inventories[1], 1) == [occupied] and g.events == before_events
    g.word(recipient_slot, 0); g.input(recipient, 934, 0)
    assert g.input(recipient, 940, USE) & USE == 0
    assert g.contents(g.inventories[0], 1) == [0]
    assert g.contents(g.inventories[1], 1) == [g.item_ptr]
    assert g.read(g.item_ptr + 0x28C) == 12345 and g.read(g.item_ptr + 0xD0) == 3
    assert g.save(donor)[13] == 6
    events = list(g.events)
    assert g.input(recipient, 950, USE) & USE == 0 and g.events == events
    g.rendered.clear(); g.draw()
    assert [(op, model) for op, model, _ in g.rendered] == [('remove', artefact_model)]
    g.tick(donor, 1440); g.draw()
    assert ('add', weapon_model) in [(op, model) for op, model, _ in g.rendered]
    print('ok: team/LOS checks, same-resource transfer, cooldown/references, held-Use latch and donor restoration')
    # Restore prior states exactly, including the generation and input edge.
    for snapshot in (raising, offering, g.save(donor)):
        g.load(donor, snapshot)
        assert g.save(donor) == snapshot
    g.load(donor, offering)
    g.tick(donor, 2000)  # restored offer's inventory no longer contains it
    assert g.save(donor)[13] == 6
    print('ok: native snapshot extension round-trips and stale ownership invalidates the restored offer')
    for name, callback in [('Spring', (0x398, 0x25EBB0, 0x25EBB6)),
                           ('Lifebone', (0x498, 0x25F100, 0x25F106)),
                           ('Onyx', (0xF20, 0x25F640, 0x25F646))]:
        other = Run(bytes(built), callback)
        p = other.players[0]
        other.input(p, 0, COMMAND); other.tick(p, 800)
        artifact, weapon = other.renderer(); other.draw()
        assert [(op, model) for op, model, _ in other.rendered] == [('add', artifact), ('remove', weapon)]
        other.emu.mem_write(p + 0x33C, b'\0')
        other.tick(p, 900); other.rendered.clear()
        other.site(0x2477C8, [0x2477CE, 0x2477DF], ebx=p, eax=other.matrix)
        other.site(other.draw_entry, [other.draw_past, STOP], ecx=other.callback)
        assert other.save(p)[13] == 0
        # The dead body's weapon is back in the scene before its native draw.
        assert [(op, model) for op, model, _ in other.rendered] == [('add', weapon), ('remove', artifact)]
        other.rendered.clear()
        native_weapon = other.read(p + 0x128)
        other.site(0x243FA0, [STOP], (0,), ecx=native_weapon)
        assert other.rendered == [], 'native no-op hide must not restore carry models'
        # Native lifecycle owns its live old weapon even after held changes.
        other.word(p + 0x128, 0)
        other.site(0x243FA0, [STOP], (1,), ecx=native_weapon)
        assert [(op, model) for op, model, _ in other.rendered] == [('remove', weapon)]
        assert other.emu.mem_read(weapon + 0x10D, 1) == b'\0'
        print(f'ok: {name} attachment/death cleanup and native on_hide handoff after held-item change')



def death_with_optics_hidden(built):
    """A carrier killed with the gun and its optics out of the scene: its
    hands held the artefact, so the gun stays out (no gun on the corpse, and
    the ragdoll drops none) and the native draw of the body, which updates
    the optics' models (0x245012, the renderer asserts on one not in the
    scene), is skipped as while alive; native on_hide still gets them back
    before it removes them. A new owner of the place (a respawn, a new kit)
    puts back what the old one took, before the native draw."""
    for case in ('death', 'new owner'):
        g = Run(built); p = g.players[0]
        _, weapon = g.renderer()
        gun = g.read(p + 0x128)
        optics, models = g.block(0x120), []
        for at in (0x108, 0x10C):
            holder, instance = g.block(0x120), g.block(0x120)
            g.word(holder + 0x108, instance); g.emu.mem_write(instance + 0x10D, b'\1')
            g.word(optics + at, holder); models.append(instance)
        g.word(gun + 0x1808, optics)
        g.input(p, 0, COMMAND); g.tick(p, 800); g.draw()
        hidden = [weapon] + models
        assert all(g.emu.mem_read(m + 0x10D, 1) == b'\0' for m in hidden), case
        if case == 'death':
            g.site(0x1F28E0, [0x1F28E5], (0,), ecx=p)  # the native death entry
            g.emu.mem_write(p + 0x33C, b'\0')
        else:
            g.site(0x2D6990, [0x2D6995], (p,))
        g.rendered.clear()
        g.site(0x2477C8, [0x2477CE, 0x2477DF], ebx=p, eax=g.matrix)
        if case == 'death':
            assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2477DF, 'death: the native draw met the removed models'
            assert not any(op == 'add' for op, _, _ in g.rendered), ('death: the gun came back', g.rendered)
            assert all(g.emu.mem_read(m + 0x10D, 1) == b'\0' for m in hidden), case
            g.rendered.clear()
            g.site(0x243FA0, [STOP], (1,), ecx=gun)
            assert sorted((op, m) for op, m, _ in g.rendered if op == 'add') == sorted(('add', m) for m in hidden), \
                ('death: native on_hide removed models not in the scene', g.rendered)
            continue
        assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2477CE, f'{case}: the native weapon draw was skipped'
        assert sorted((op, m) for op, m, _ in g.rendered) == sorted(('add', m) for m in hidden), (case, g.rendered)
        assert all(g.emu.mem_read(m + 0x10D, 1) == b'\1' for m in hidden), case
        g.rendered.clear()
        g.site(0x243FA0, [STOP], (1,), ecx=gun)
        assert ('add', weapon) not in [(op, m) for op, m, _ in g.rendered], f'{case}: restored twice'
    print('ok: a carrier\'s death keeps its gun and optics out with the native draw skipped (on_hide still gets them '
          'back); a new owner puts all three back before the native draw')


def death_rolled_back(built):
    """A carrier's death that a rollback takes back (alive again, the world of
    before the carry loaded): the gun shows as the live carry says, and a
    later death in the same life records its own hands (here the gun)."""
    g = Run(built); p = g.players[0]
    _, weapon = g.renderer()
    before = g.save(p)
    g.input(p, 0, COMMAND); g.tick(p, 800); g.draw()
    assert g.emu.mem_read(weapon + 0x10D, 1) == b'\0'
    g.site(0x1F28E0, [0x1F28E5], (0,), ecx=p)  # the native death entry
    g.emu.mem_write(p + 0x33C, b'\0')
    g.emu.mem_write(p + 0x33C, b'\1'); g.load(p, before)  # the rollback: +33Ch is loaded before the hook
    for case in ('rolled back', 'a later death'):
        if case == 'a later death':
            g.site(0x1F28E0, [0x1F28E5], (0,), ecx=p)
            g.emu.mem_write(p + 0x33C, b'\0')
        g.rendered.clear()
        g.site(0x2477C8, [0x2477CE, 0x2477DF], ebx=p, eax=g.matrix)
        assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2477CE, f'{case}: the gun stayed out as at the taken-back death'
        assert g.emu.mem_read(weapon + 0x10D, 1) == b'\1', (case, g.rendered)
    print('ok: a death a rollback takes back leaves no record: the gun shows, and a later death records afresh')


def moving_draw(built):
    g = Run(built); p = g.players[0]
    g.input(p, 0, COMMAND); g.tick(p, 800)
    artifact, _ = g.renderer()
    initial = bytes(g.emu.mem_read(g.bones, 8 * 64))
    g.draw(); first = next(m for op, model, m in g.rendered if model == artifact)
    saved = g.save(p)
    for n in range(1, 4):
        g.load(p, saved)
        g.rendered.clear()
        # Deliberately run the callback BEFORE the bones, as another task can.
        g.site(g.draw_entry, [g.draw_past, STOP], ecx=g.callback)
        assert not g.rendered, 'callback replayed an older transform or flickered after restore'
        eye = list(struct.unpack('<16f', g.emu.mem_read(p + 0x44C, 64))); eye[12] = n
        root = list(IDENTITY); root[12] = n
        g.emu.mem_write(p + 0x40C, struct.pack('<16f', *root))
        g.emu.mem_write(p + 0x44C, struct.pack('<16f', *eye))
        g.emu.mem_write(g.bones, initial); g.draw()
        models = [m for op, model, m in g.rendered if model == artifact]
        assert len(models) == 1 and abs(models[0][12] - first[12] - n) < .0001
    print('ok: same-frame moving hand attachment, callback-before-bones and snapshot restore without flicker')


def switch_items(built):
    g = Run(built); donor, recipient = g.players
    old_model, weapon = g.renderer()
    other = g.item(); g.block(0x100)
    g.word(other, g.read(g.item_ptr)); g.word(other + 0x248, g.world)
    g.word(other + 0x280, 3); g.emu.mem_write(other + 0x27E, b'\x08')
    g.word(other + 0x28C, 23456)
    inventory = g.slots([g.item_ptr, other]); g.word(donor + 0x10C, inventory)
    callback = other + 0x340
    g.word(callback + 0xC, g.world); g.word(callback + 0x10, 3)
    holder, new_model = g.block(0x120), g.static_instance()
    g.word(holder + 0x108, new_model); g.word(callback + 0x14, holder)
    registry = g.block(9 * 4)
    g.word(registry + 7 * 4, g.item_ptr); g.word(registry + 8 * 4, other)
    g.word(g.world + 0x18940, registry); g.word(g.world + 0x18944, registry + 9 * 4)
    raw_bones = bytes(g.emu.mem_read(g.bones, 8 * 64))
    def draw():
        g.emu.mem_write(g.bones, raw_bones)
        g.draw()
        g.site(g.draw_entry, [g.draw_past, STOP], ecx=callback)
    g.input(donor, 0, COMMAND); g.tick(donor, 800); draw()
    g.input(donor, 810, 0); g.input(donor, 820, slot(1))
    g.tick(donor, 1000); saved = g.save(donor)
    assert saved[13] == 7 and saved[14] == 7 and saved[26] == 8
    g.load(donor, saved); assert g.save(donor) == saved
    g.rendered.clear(); draw()
    assert not any(op == 'add' and model == new_model for op, model, _ in g.rendered)
    assert g.emu.mem_read(old_model + 0x10D, 1) == b'\1'
    assert g.input(recipient, 1000, USE) == USE, 'switching item was offered early'
    assert g.contents(inventory, 2) == [g.item_ptr, other]
    g.tick(donor, 1320); saved = g.save(donor)
    assert saved[13] == 2 and saved[14] == 8
    g.load(donor, saved); assert g.save(donor) == saved
    g.rendered.clear(); draw()
    assert ('remove', old_model) in [(op, model) for op, model, _ in g.rendered]
    assert ('add', new_model) in [(op, model) for op, model, _ in g.rendered]
    assert g.emu.mem_read(weapon + 0x10D, 1) == b'\0'
    g.tick(donor, 1920); assert g.save(donor)[13] == 3
    g.input(recipient, 1930, 0); assert g.input(recipient, 1940, USE) == 0
    assert g.contents(inventory, 2) == [g.item_ptr, 0]
    assert g.contents(g.inventories[1], 1) == [other]
    assert g.read(other + 0x28C) == 23456
    print('ok: switching lowers old model, raises new model, restores both snapshots, and offers only the new item after raising')


def death_drop(built, callback=(0x340, 0x25FB80, 0x25FB86), death=0x1F28E0):
    g = Run(built, callback); donor, recipient = g.players
    g.input(donor, 0, COMMAND); g.tick(donor, 800)
    artifact, _ = g.renderer()
    flower = None
    if callback[0] == 0x498:
        held, flower = g.block(0x120), g.static_instance()
        g.word(held+0x108,flower); g.word(held+0xD0,1)
        result=g.block(0xC00); g.word(result+0x38,4); g.word(result+0x9CC,held)
        g.site(0x260278,[0x26027E],edi=g.item_ptr,ebx=result)
    g.draw()
    g.floor = 0.
    # Native death entry, before its inventory/effect teardown.
    g.site(death, [death + 5], (0,), ecx=donor)
    assert g.contents(g.inventories[0], 1) == [0]
    assert g.read(g.item_ptr + 0x250) == 0 and g.read(g.item_ptr + 0x280) == 6
    assert g.events[-1] == ('removed', g.item_ptr)
    assert g.save(donor)[13] == 0
    refs = g.read(g.item_ptr + 0xD0)
    # State 6 extends exactly the native item's stream; preserve it on replay.
    g.saved.clear()
    g.site(0x313735, [0x31373B], esi=g.item_ptr, edi=g.snapshot, ebx=0)
    release = bytes(g.saved); assert len(release) == 20
    g.emu.mem_write(g.snapshot, release)
    for off,val in ((0,g.snapshot),(4,g.snapshot),(8,20)): g.word(g.reader+off,val)
    g.site(0x3137EC, [0x3137F1], ebx=g.item_ptr, edi=g.reader, esi=0)
    assert g.read(g.reader+4) == g.snapshot+20
    # Execute native artefact_base serialization/deserialization around the
    # extension, including cooldown and the following active-start timestamp.
    # Inventory-item base serialization is a separately tested boundary.
    g.emu.mem_write(0x2FFDF0,bytes.fromhex('c20800'))
    g.emu.mem_write(0x2FFF10,bytes.fromhex('c20c00'))
    def write32(emu, address, *_):
        sp=emu.reg_read(UC_X86_REG_ESP)
        g.saved.extend(emu.mem_read(emu.reg_read(UC_X86_REG_EAX),4))
        emu.reg_write(UC_X86_REG_EIP,g.read(sp)); emu.reg_write(UC_X86_REG_ESP,sp+24)
    g.emu.hook_add(UC_HOOK_CODE,write32,begin=0x1F48A0,end=0x1F48A0)
    g.word(g.item_ptr+0x2A0,0x12345678); g.saved.clear()
    g.site(0x313670,[STOP],(g.snapshot,0),ecx=g.item_ptr)
    stream=bytes(g.saved)
    assert stream == b'\6'+struct.pack('<I',12345)+release+struct.pack('<I',0x12345678)
    g.emu.mem_write(g.snapshot,stream)
    for off,val in ((0,g.snapshot),(4,g.snapshot),(8,len(stream))): g.word(g.reader+off,val)
    g.word(g.item_ptr+0x2A0,0)
    g.site(0x313760,[0x31383F],(g.reader,0,0),ecx=g.item_ptr)
    assert g.read(g.reader+4) == g.snapshot+len(stream) and g.read(g.item_ptr+0x2A0) == 0x12345678
    g.rendered.clear(); g.word(g.world+0x18FC0, 1800)
    g.site(g.draw_entry, [g.draw_past, STOP], ecx=g.callback)
    model = g.rendered[-1][2]
    if flower:
        assert g.emu.mem_read(flower+0x10D,1) == b'\0'
        assert g.emu.mem_read(artifact+0x10D,1) == b'\1', 'held flower replaced the full ground model'
    assert model and abs(model[13] - .1) < .0001, 'drop did not settle at static collision surface'
    # Aim at the fallen item; enemies can collect it too.
    eye = list(IDENTITY); eye[12:15] = [model[12], .1, model[14] - 1.]
    g.emu.mem_write(recipient+0x44C, struct.pack('<16f', *eye))
    g.word(recipient+0x324, 1800)
    g.word(g.read(recipient+0x6B8)+0x45EC, 1)
    for count in (0, 1):
        g.emu.mem_write(g.inventories[1]+0x16C, bytes([count]))
        slot = g.read(g.inventories[1]+0x168)
        occupied = g.item() if count else 0
        g.word(slot, occupied)
        before = list(g.events)
        # Nothing to take into: Use is the game's (a battery, a trap, a pickup).
        assert g.input(recipient, 1810, USE) & USE == USE
        assert g.events == before and g.read(g.item_ptr+0x280) == 6
        g.input(recipient, 1820, 0)
    g.word(slot, 0)
    assert g.input(recipient, 1830, USE) & USE == 0
    assert g.contents(g.inventories[1], 1) == [g.item_ptr]
    assert g.read(g.item_ptr+0xD0) == refs+1
    assert g.read(g.item_ptr+0x280) == 3, 'native on_taken did not restore passive state'
    before = list(g.events); g.input(recipient, 1840, USE)
    assert g.events == before
    g.rendered.clear(); g.site(g.draw_entry, [g.draw_past, STOP], ecx=g.callback)
    assert [(op,model) for op,model,_ in g.rendered] == [('remove', artifact)]
    print('ok: held death drops same resource; synchronized fall, ground pickup, enemy recipient and container refusals')


def recipient_hud(built):
    g = Run(built); donor, recipient = g.players
    g.input(donor, 0, COMMAND); g.tick(donor, 800)
    g.renderer()
    ui, holder, view, movie, client = [g.block(n) for n in (0x20,0x110,8,4,8)]
    g.word(ui+0x18, g.world); g.word(ui+8, holder)
    g.word(holder+0x108,view); g.word(view+4,movie)
    game = g.read(g.world+0x9C)
    g.word(game+0x53AC,client); g.word(client+4,recipient)
    seen = []
    def string(ptr):
        result = bytearray()
        while True:
            c = bytes(g.emu.mem_read(ptr,1)); ptr += 1
            if c == b'\0': return result.decode()
            result.extend(c)
    def invoke(emu, address, *_):
        sp=emu.reg_read(UC_X86_REG_ESP)
        method=string(g.read(sp+4)); arg=g.read(sp+12)
        if method == 'root.show_artefact_offer':
            assert g.read(arg+4) == 6 and g.read(sp+16) == 1
            seen.append(string(g.read(arg+8)))
        emu.reg_write(UC_X86_REG_EAX,1)
        emu.reg_write(UC_X86_REG_EIP,g.read(sp)); emu.reg_write(UC_X86_REG_ESP,sp+20)
    g.emu.hook_add(UC_HOOK_CODE,invoke,begin=0x511600,end=0x511600)
    def hint(expected):
        g.site(0x23AA10,[0x23AA16],(ui,))
        assert seen[-1] == expected, (seen,expected)
    hint('st_artefact_take_teammate')
    # Without a free container the native hint stays: nothing would be taken.
    g.emu.mem_write(g.inventories[1]+0x16C,b'\0'); hint('')
    g.emu.mem_write(g.inventories[1]+0x16C,b'\1')
    hint('st_artefact_take_teammate')
    container=g.read(g.inventories[1]+0x168); g.word(container,g.item()); hint('')
    g.word(container,0); hint('st_artefact_take_teammate')
    g.wall=True; hint('')
    print('ok: real HUD hook shows the take hint only when Use would take, and hides a blocked target')

def container_show(built):
    """A container's artefact, given by the original transfer_artefact (0x31C8A0)
    and take_artefact, is shown in the taker's hand for 2 s (patch 78's tag 9):
    fire filtered meanwhile, nothing pressed ends it, the snapshot holds it, then
    fire passes again. Into a free slot and in place of the first of full slots;
    no show without a gun in hand; a death during it drops nothing."""
    FIRE, START, SHOW = 0x20, 1000, 2000
    def give(g, taker):
        """use_execute's call: the container's own artefact (state 2, id 5)."""
        item = g.item()
        g.word(item, g.read(g.item_ptr))  # the client vtable: the real on_taken
        g.word(item + 0x280, 2)
        g.emu.mem_write(item + 0x27E, bytes([5]))
        g.word(item + 0x248, g.world)
        g.word(g.read(g.world + 0x18940) + 5 * 4, item)
        container = g.block(0x80)
        g.word(container + 0x4C, item)
        g.tick(taker, START)
        g.site(0x31C8A0, [STOP], (taker + 0x108,), edi=container, ebx=taker)
        assert g.read(container + 0x4C) == 0 and g.read(item + 0x280) == 3
        return item
    for taker_place, slot in ((1, 0), (0, 0)):
        g = Run(built); taker = g.players[taker_place]
        item = give(g, taker)
        assert g.contents(g.inventories[taker_place], 1) == [item]
        shown = g.save(taker)
        assert shown[13] == 9 and shown[14] == 5 and shown[15] == slot, shown.hex()
        for now, press in ((START + 100, FIRE), (START + 600, (1 << 29) | (1 << 14)), (START + 900, 1 << 12), (START + 1200, 0)):
            assert g.input(taker, now, press) & (FIRE | (1 << 12) | (63 << 14)) == 0, hex(press)
            g.tick(taker, now)
            assert g.save(taker)[13] == 9, 'a press ended the show'
        g.tick(taker, START + SHOW - 1); assert g.save(taker)[13] == 9
        g.load(taker, shown); assert g.save(taker) == shown
        g.tick(taker, START + SHOW); assert g.save(taker)[13] == 0
        assert g.input(taker, START + SHOW, FIRE) == FIRE
        assert g.contents(g.inventories[taker_place], 1) == [item]
    # Taken with no gun in hand (a medkit, a grenade): no show, the artefact
    # in its slot as before.
    g = Run(built); taker = g.players[1]
    g.word(g.read(taker + 0x128), 0xA00000)
    item = give(g, taker)
    assert g.save(taker)[13] == 0 and g.contents(g.inventories[1], 1) == [item]
    # Killed during it: the artefact stays in its slot (no drop, state 3).
    g = Run(built); taker = g.players[1]
    item = give(g, taker)
    g.tick(taker, START + 700)
    g.site(0x1F28E0, [0x1F28E5], (0,), ecx=taker)
    assert g.contents(g.inventories[1], 1) == [item] and g.read(item + 0x280) == 3
    assert g.save(taker)[13] == 0
    print('ok: a container\'s artefact (free and full slots) is shown 2 s, fire held, '
          'snapshot kept; none without a gun; a death mid-show drops nothing')


if __name__ == '__main__': main()
