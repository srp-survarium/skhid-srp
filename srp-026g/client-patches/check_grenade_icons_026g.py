#!/usr/bin/env python3
"""Patch 194 (grenade_icons): the markers over thrown grenades tell a smoke and a toxic smoke apart
(docs/notes/grenade-icons.md), in Unicorn on the built exe.

    python check_grenade_icons_026g.py ORIGINAL_EXE [--fresh]

Over a constructed followed player, his hud and grenades (made-up memory; the game's own
set_hud_icon_type 0x23BEA0 runs, its Invoke stubbed and recorded):

1. player::st_draw's sensor at `grenade_icons.sensed` (0x2471A4, a teammate's grenade taking
   marker BL) run to the end of the replayed span (0x2471A9): a marker shown a grenade of another
   kind than it was last told gets root.set_player_icon_type(2Fh + BL, type) with the frag 0, a
   smoke (its stack's splash_max_damage 0) 3, a smoke of a thrower with Toxic Smoke 4; the same
   kind again none; a marker a smoke had, taken by a frag next, is told the frag's again; the
   replayed `mov edx,[esi] / mov edx,[edx+1Ch]` and every other register kept; BL is a byte (the
   rest of EBX the host's).
2. initialize_match at `player_icons.enemy_ids` (0x235709, before it makes the markers anew,
   every one a frag's): what the markers were told is forgotten, so a smoke is told again.
3. The original exe at the sensor site: no call.

--fresh links the arena from this tree's sources (patch_026g.py's ARENA, no lock compared) instead
of the lock's bytes: for a tree whose lock link_026g.py --write has not made yet.
"""
import sys

from unicorn.x86_const import UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDX, UC_X86_REG_ESI

import patch_026g as base
import check_hide_ui_026g as hide
import check_technical_smoke_026g as smoke

SENSED, AFTER_SENSED = 0x2471A4, 0x2471A9
ENEMY_IDS, AFTER_ENEMY_IDS = 0x235709, 0x235712
FRAG, SMOKE, TOXIC_SMOKE = 0, 3, 4
FIRST_ICON = 0x2F


class Icons(smoke.Smoke):
    def follower(self):
        """The followed player (his block past +F480h), his hud and its player icons movie."""
        player = self.block(0x10000)
        ui, holder, movies, movie = (self.block(0x400) for _ in range(4))
        self.word(player + 0xF480, ui)
        self.word(ui + 0x10, holder)
        self.word(holder + 0x108, movies)
        self.word(movies + 4, movie)
        self.ui = ui
        return player, movie

    def grenade(self, kind):
        """A teammate's grenade of `kind` and its body (its user the grenade +1Ch, as throw sets it)."""
        thrower = self.player(1, 0, 2 * smoke.TOXIC if kind == TOXIC_SMOKE else 0)
        _, grenade = self.stack_of(thrower, damage=80.0 if kind == FRAG else 0.0)
        body, table = self.block(0x40), self.block(0x40)
        self.word(body, table)
        self.word(table + 0x1C, 0x1C1C1C1C)
        self.word(body + 0xC, grenade + 0x1C)
        return body

    def sense(self, player, body, marker, upper=0xABCD0000):
        regs = {**hide.GAME, UC_X86_REG_ESI: body, UC_X86_REG_EBP: player, UC_X86_REG_EBX: upper | marker}
        after, sp = self.run(SENSED, (AFTER_SENSED,), smoke.STACK - 0x400, regs)
        assert self.stopped == AFTER_SENSED and sp == smoke.STACK - 0x400, (hex(self.stopped or 0), hex(sp))
        assert after == {**regs, UC_X86_REG_EDX: 0x1C1C1C1C}, {hex(k): hex(v) for k, v in after.items()}
        return [(c[1], c[2]) for c in self.calls], self.invoked

    def markers_made(self):
        """initialize_match at the site, the followed player's hud in ESI (its other handlers tell the
        movie of a match of no replay and no Artifact Hunt)."""
        regs = {**hide.GAME, UC_X86_REG_ESI: self.ui}
        self.run(ENEMY_IDS, (AFTER_ENEMY_IDS,), smoke.STACK - 0x400, regs)
        assert self.stopped == AFTER_ENEMY_IDS, hex(self.stopped or 0)


def told(movie, marker, kind):
    return [(movie, b'root.set_player_icon_type')], [(FIRST_ICON + marker, kind)]


def check_patched(data):
    r = Icons(data)
    player, movie = r.follower()
    steps = (
        (0, FRAG, False, 'a frag in a marker initialize_match made (a frag\'s): nothing told'),
        (0, SMOKE, True, 'a smoke takes it: told the smoke (3)'),
        (0, SMOKE, False, 'the same smoke next frame: nothing'),
        (0, FRAG, True, 'a frag takes the marker the smoke had: told the frag again (0)'),
        (5, TOXIC_SMOKE, True, 'a toxic smoke in the last marker (34h): told 4'),
        (5, SMOKE, True, 'a plain smoke takes it: told 3'),
        (1, FRAG, False, 'a frag in a marker never told: nothing'),
    )
    for marker, kind, tells, label in steps:
        calls, invoked = r.sense(player, r.grenade(kind), marker)
        wanted = told(movie, marker, kind) if tells else ([], [])
        assert (calls, invoked) == wanted, (label, calls, invoked)
        print('  ok', label)
    r.markers_made()
    calls, invoked = r.sense(player, r.grenade(SMOKE), 5)
    assert (calls, invoked) == told(movie, 5, SMOKE), (calls, invoked)
    calls, invoked = r.sense(player, r.grenade(FRAG), 0)
    assert (calls, invoked) == ([], []), (calls, invoked)
    print('  ok after initialize_match\'s markers: a smoke is told again, a frag is not')


def check_original(data):
    r = Icons(data)
    player, _ = r.follower()
    for kind in (FRAG, SMOKE, TOXIC_SMOKE):
        assert r.sense(player, r.grenade(kind), 2) == ([], [])
    print('  ok the original: no marker told a kind')


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], 'rb').read()
    fresh = '--fresh' in sys.argv[2:]
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read(), arena_blocks=base.ARENA if fresh else None)
    print('the patched exe')
    check_patched(bytes(patched))
    print('the original exe')
    check_original(original)


if __name__ == '__main__':
    main()
