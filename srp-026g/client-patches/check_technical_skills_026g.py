#!/usr/bin/env python3
"""Patch 180 (the technical tree, docs/notes/technical-skills.md) in Unicorn on the built exe.

Each skill's hook runs at its site of the patched exe, on a constructed player (the medical
fixture's: its profile, kits and damage model) whose current kit carries the packed ranks in
modifier 47, with and without the skill, the oxygen tank on, off or absent, on two kits:

- Deep Breaths: player_stamina::sprint's cost after Numb Legs (0x329AAD): x0.8/0.7/0.6 with the
  tank on, else as it was.
- Steady Breath: breath_vibration_calculator::tick's stores (0x31D95B): both sway offsets
  x0.9/0.8 with the tank on; a shot's dispersion (0x2E1DBE, XMM1) the same.
- Last Gasp: the tank's tick after the air is spent (0x33E661): an empty tank fills its holder's
  stamina and saves the boost's end (+294h, the stamina clock + 20 s); the movement speed's end
  (0x2DBC91) x1.1 while it lasts (its end is the crate's Gasp test); a tank with air
  left, or no skill: nothing.
- Dexterous Hands: the take expression's stored scale (0x338B61) x1.1/1.2/1.3 for the taker; the ms
  passed at an ammo crate (update_use, 0x2F8147) x (10 + rank) / 10 for its user.
- Sapper: use_execute's defuse ms (0x304F92) x 20 / (20 + 3 x rank).
- Rearm: a bullet's hit on a plain trap of a skilled owner (0x305552 to its ret 0x305571): disarmed
  by the game's switch_to_state, no state timer, the action time = the bullet's ms + 30 s (rank 2: 20 s); an
  explosive trap or an owner without the skill: the game's path. The tick at the action time
  (0x304DCF): armed again (switch_to_state(1), insert_collision called) and never the native
  delayed action for a plain trap.
- Blast Drill: damage_model's hit at 0x2E50C7: a grenade's blast (a grenade set's vtable, kind 3)
  from the victim itself x0.75/0.5; a trap's set, a bullet, another kind: as it was.
- Bear Trap: hit_by_type's store of the new health (0x32A28B, the medical tree's handler before
  it): a leg's, from a hit whose item is a trap set of a skilled owner, is 0; another pool, another
  item or an owner without the skill: as it was.

A constructed player, not a played match. Tank Pocket/Depth (the lobby's apply_kit_parameter, its
item configs), Scavenger, Trophy Trap and the tank's save/load are checked by the crates' tests and
in a game, not here.
Usage: python check_technical_skills_026g.py ORIGINAL_EXE
"""
import struct
import sys
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_XMM1
import patch_026g as base
from check_artefacts_026g import PLAYER, STACK, KITS
from check_medical_skills_026g import MedicalGame, bits, close

RANKS = 47                    # spot_enemy_traps: the packed technical ranks
TANK_VTABLE = 0xA1C874        # oxygen_tank's
GRENADE_SET_VTABLE = 0xA0B56C
TRAP_SET_VTABLE = 0xA044D4


# Modifier 47 is a mixed-radix number, a digit per skill (technical_skills::FIELDS: id, ranks).
FIELDS = [(401, 3), (402, 3), (403, 2), (404, 2), (411, 3), (412, 1), (413, 2), (414, 2), (422, 1), (423, 1),
          (431, 3), (432, 1), (433, 2), (441, 3), (442, 1), (443, 1), (444, 1)]
WEIGHTS = {}
_weight = 1
for _skill, _ranks in FIELDS[:-1]:
    WEIGHTS[_skill] = _weight
    _weight *= _ranks + 1
WEIGHTS[444] = -_weight  # the last digit counts down: values within -LOW..LOW (technical_skills::LOW)


def rank(skill, value):
    return value * WEIGHTS[skill]


def fixture(data, kit, ranks, tank=None):
    """The medical fixture's player on `kit` with `ranks` in modifier 47 and, if given, an oxygen tank
    (on or off) in its inventory's slot 3 (the back)."""
    g = MedicalGame(data)
    g.emu.mem_write(PLAYER + 0x115, bytes([kit]))
    g.word(KITS + 0xF4 + kit * 0x704 + RANKS * 4, bits(ranks))
    inventory = g.read(PLAYER + 0x10C)
    if not inventory:
        inventory = g.block(0x200)
        g.word(PLAYER + 0x10C, inventory)
    g.word(inventory + 0x174, PLAYER + 0x108)
    item = None
    if tank is not None:
        item = g.block(0x300)
        g.word(item, TANK_VTABLE)
        g.word(item + 0x250, inventory)
        g.emu.mem_write(item + 0x268, bytes([int(tank)]))
        g.word(inventory + 0x10C + 3 * 4, item)
    return g, item


def check_deep_breaths(data, kit):
    for ranks, tank, want in [(0, True, 1.0), (rank(431, 1), True, 0.8), (rank(431, 2), True, 0.7), (rank(431, 3), True, 0.6),
                              (rank(431, 3), False, 1.0), (rank(431, 3), None, 1.0), (0.5, True, 1.0)]:
        g, _ = fixture(data, kit, ranks, tank)
        have = g.step(0x329AAD, 6, xmm=2.0, edi=PLAYER + 0x6C0)
        close(have, 2.0 * want)
    print(f'Deep Breaths kit {kit}: the sprint\'s stamina cost x0.8/0.7/0.6 with the oxygen tank on (after Numb Legs); off, absent, no skill or a refused payload: as it was: PASS')


def check_steady_breath(data, kit):
    for ranks, tank, want in [(0, True, 1.0), (rank(403, 1), True, 0.9), (rank(403, 2), True, 0.8),
                              (rank(403, 2), False, 1.0), (rank(403, 2), None, 1.0)]:
        g, _ = fixture(data, kit, ranks, tank)
        calc = g.block(0x100)
        g.word(calc + 0x3C, PLAYER)
        g.step(0x31D95B, 6, eax=bits(0.02), ecx=bits(-0.01), edi=calc)
        close(g.float(calc + 0x44), 0.02 * want)
        close(g.float(calc + 0x48), -0.01 * want)
        # The shot's dispersion: XMM1, stored by the site's own movss [esp+7Ch],xmm1.
        g, _ = fixture(data, kit, ranks, tank)
        weapon = g.block(0x40)
        g.word(weapon + 8, PLAYER)
        g.emu.reg_write(UC_X86_REG_XMM1, bits(0.5))
        g.step(0x2E1DBE, 6, ebx=weapon)
        close(g.float(STACK - 0x100 + 0x7C), 0.5 * want)
    print(f'Steady Breath kit {kit}: both sway offsets and a shot\'s dispersion x0.9/0.8 with the oxygen tank on, else as they were: PASS')


def check_last_gasp(data, kit):
    for ranks, left, starts in [(rank(442, 1), 0, True), (rank(442, 1), 500, False), (0, 0, False)]:
        g, tank = fixture(data, kit, ranks, False)
        stamina = PLAYER + 0x6C0
        g.word(stamina, bits(100.0))
        g.word(stamina + 0x6C, bits(12.0))
        g.word(stamina + 0x70, 50_000)
        g.emu.mem_write(PLAYER + 0x33E, b'\0')
        g.step(0x33E661, 8, xmm=0.5, ecx=left, edi=tank)
        if starts:
            close(g.float(stamina + 0x6C), 100.0)
            assert g.read(tank + 0x294) == 70_000 and g.emu.mem_read(PLAYER + 0x33E, 1)[0] == 1
        else:
            close(g.float(stamina + 0x6C), 12.0)
            assert g.read(tank + 0x294) == 0
        # The speed while it lasts. (Its end at the tick, `battery.carrier_tick` after four other
        # patches' handlers, is technical_skills::Gasp's test.)
        speed = g.step(0x2DBC91, 5, xmm=4.0, ebx=PLAYER, ebp=STACK + 0x40)
        close(speed, 4.4 if starts else 4.0)
    print(f'Last Gasp kit {kit}: a tank that runs out fills its holder\'s stamina and saves the boost until the stamina clock + 20 s '
          '(x1.1 movement); air left or no skill: nothing: PASS')


def check_dexterous_hands(data, kit):
    for ranks, want in [(0, 1.0), (rank(401, 1), 1.1), (rank(401, 2), 1.2), (rank(401, 3), 1.3)]:
        g, _ = fixture(data, kit, ranks)
        battery, state = g.block(0x200), g.block(0x200)
        g.word(battery + 8, PLAYER)
        g.word(state + 0x1C, battery)
        stack = [bits(2.0)] + [0] * 8
        g.step(0x338B61, 6, stack=stack, esi=state, eax=0)
        # The replay pushed four dwords: the scale is now at ESP+16.
        close(g.float(g.emu.reg_read(UC_X86_REG_ESP) + 16), 2.0 * want)
    for ranks, want in [(0, 2500), (rank(401, 1), 2750), (rank(401, 2), 3000), (rank(401, 3), 3250)]:
        g, _ = fixture(data, kit, ranks)
        zone = g.block(0x80)
        g.word(zone + 0x50, 3000)
        g.step(0x2F8147, 5, eax=2500, ecx=zone, edi=PLAYER)
        assert g.emu.reg_read(UC_X86_REG_EAX) == want, (ranks, g.emu.reg_read(UC_X86_REG_EAX))
    print(f'Dexterous Hands kit {kit}: the take animation\'s scale x1.1/1.2/1.3 for a skilled taker; 2500 ms at an ammo crate '
          'count as 2750/3000/3250: PASS')


def check_sapper(data, kit):
    for ranks, want in [(0, 5000), (rank(411, 1), 4347), (rank(411, 2), 3846), (rank(411, 3), 3448)]:
        g, _ = fixture(data, kit, ranks)
        user = g.block(0x40)
        g.word(user, PLAYER)
        g.step(0x304F92, 5, stack=[0] * 4, eax=5000, ebx=user)
        assert g.emu.reg_read(UC_X86_REG_EAX) == want, (ranks, g.emu.reg_read(UC_X86_REG_EAX))
    print(f'Sapper kit {kit}: the 5 s disarm 4347/3846/3448 ms for a skilled disarmer: PASS')


def trap(g, explosive, state):
    """A trap of the fixture player's set: its set's inventory is the player's, the set's
    disarmed life 8 s and defuse_by_hit `ranged` (0)."""
    inventory = g.read(PLAYER + 0x10C)
    trap_set = g.block(0x300)
    g.word(trap_set, TRAP_SET_VTABLE)
    g.word(trap_set + 0x250, inventory)
    g.word(trap_set + 0x288, 0)
    g.word(trap_set + 0x290, 8000)
    g.word(trap_set + 0x298, 0)
    core = g.block(0x300)
    vtable, explode = g.block(0x80), g.block(8)
    g.emu.mem_write(explode, b'\xC2\x04\x00')   # explode (vtable +40h): ret 4
    g.word(vtable + 0x40, explode)
    g.word(core, vtable)
    g.word(core + 0x194, trap_set)
    g.emu.mem_write(core + 0x198, bytes([state]))
    g.emu.mem_write(core + 0x220, bytes([int(explosive)]))
    g.word(core + 0x260, 0xFFFFFFFF)
    return core


def run_until(g, start, stop, stack=(), **regs):
    from check_medical_skills_026g import REGISTER
    g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
    for i, v in enumerate(stack):
        g.word(STACK - 0x100 + 4 * i, v)
    for name, value in regs.items():
        g.emu.reg_write(REGISTER[name], value)
    g.emu.emu_start(start, stop, count=2_000_000)
    return g.emu.reg_read(UC_X86_REG_EIP)


def check_rearm(data, kit):
    # The shot: a plain trap of a skilled owner, not yet placed (state 0: no armed leave-taking).
    for ranks, explosive, kept in [(rank(433, 1), False, 150_000), (rank(433, 2), False, 140_000), (0, False, None),
                                   (rank(433, 2), True, None)]:
        g, _ = fixture(data, kit, ranks)
        core = trap(g, explosive, 0)
        eip = run_until(g, 0x305552, 0x305567 if explosive else 0x305571, stack=[0, 120_000], ecx=core)
        if kept:
            assert eip == 0x305571
            assert g.emu.mem_read(core + 0x198, 1)[0] == 3
            assert (g.read(core + 0x268), g.read(core + 0x260)) == (0, kept)
        else:
            assert g.read(core + 0x260) == 0xFFFFFFFF
    # The reopening: a disarmed plain trap at its action time is armed again, collisions inserted.
    inserted = []
    g, _ = fixture(data, kit, rank(433, 2))
    g.emu.hook_add(UC_HOOK_CODE, lambda emu, a, s, _: inserted.append(emu.reg_read(UC_X86_REG_ESI)), begin=0x304E20, end=0x304E20)
    g.emu.mem_write(0x304E20, b'\xC3')  # insert_collision: a ret (the physics world is not built)
    core = trap(g, False, 3)
    g.word(core + 0x260, 140_000)
    eip = run_until(g, 0x304DCF, 0x304DDC, stack=[0] * 8, esi=core, ebx=140_000, eax=core + 0x240)
    assert eip == 0x304DDC and g.emu.mem_read(core + 0x198, 1)[0] == 1 and g.read(core + 0x260) == 0xFFFFFFFF and inserted == [core]
    print(f'Rearm kit {kit}: a shot plain trap of a skilled owner stays disarmed (no timer) until the bullet\'s ms + 30 s (rank 1) or 20 s (rank 2), '
          'then is armed again with its collisions; explosive or no skill: the game\'s path: PASS')


def check_blast_drill(data, kit):
    for ranks, item_vtable, kind, want in [(rank(404, 1), GRENADE_SET_VTABLE, 3, 0.75), (rank(404, 2), GRENADE_SET_VTABLE, 3, 0.5),
                                           (0, GRENADE_SET_VTABLE, 3, 1.0),
                                           (rank(404, 2), TRAP_SET_VTABLE, 3, 1.0), (rank(404, 2), GRENADE_SET_VTABLE, 0, 1.0)]:
        g, _ = fixture(data, kit, ranks)
        item = g.block(0x300)
        g.word(item, item_vtable)
        hit = g.block(0x80)
        g.word(hit + 0x1C, kind)
        g.word(hit + 0x40, item)
        g.emu.mem_write(hit + 0x44, bytes([0]))   # the victim's own place: its own grenade
        model = g.read(PLAYER + 0x2E0)
        stack = [0, 0, bits(0.8)] + [0] * 8
        g.step(0x2E50C7, 5, stack=stack, edi=model, esi=hit)
        close(g.float(STACK - 0x100 + 8), 0.8 * want)
    print(f'Blast Drill kit {kit}: its own grenade\'s blast x0.75/0.5 for a skilled victim; a trap, a bullet or no skill: as it was: PASS')


def check_bear_trap(data, kit):
    for ranks, name, item_vtable, want in [(rank(444, 1), 'left_leg', TRAP_SET_VTABLE, 0.0), (rank(444, 1), 'right_leg', TRAP_SET_VTABLE, 0.0),
                                           (0, 'left_leg', TRAP_SET_VTABLE, 0.2), (rank(444, 1), 'body', TRAP_SET_VTABLE, 0.2),
                                           (rank(444, 1), 'left_leg', GRENADE_SET_VTABLE, 0.2), (rank(444, 1), 'left_leg', None, 0.2)]:
        g, _ = fixture(data, kit, ranks)
        pool, text = g.block(0x400), g.block(16)
        g.emu.mem_write(text, name.encode() + b'\0')
        g.word(pool + 0x80, text)
        g.word(pool + 0x7C, g.read(PLAYER + 0x2E0))
        for off in [0x9C, 0xA0, 0xB4]:
            g.word(pool + off, bits(0.5))
        hit = g.block(0x80)
        if item_vtable is not None:
            # The set's inventory is the fixture player's: the trap's owner.
            item = g.block(0x300)
            g.word(item, item_vtable)
            g.word(item + 0x250, g.read(PLAYER + 0x10C))
            g.word(hit + 0x40, item)
        g.step(0x32A28B, 8, xmm=0.2, ebp=pool, esi=hit)
        close(g.float(pool + 0xA0), want)
    print(f'Bear Trap kit {kit}: a leg caught by a skilled owner\'s trap is left with no health; another pool, a grenade, '
          'no item or no skill: as it was: PASS')


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    for kit in (0, 2):
        check_deep_breaths(patched, kit)
        check_steady_breath(patched, kit)
        check_last_gasp(patched, kit)
        check_dexterous_hands(patched, kit)
        check_sapper(patched, kit)
        check_rearm(patched, kit)
        check_blast_drill(patched, kit)
        check_bear_trap(patched, kit)


if __name__ == '__main__':
    main()
