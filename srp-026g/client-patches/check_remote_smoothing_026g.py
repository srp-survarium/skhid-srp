#!/usr/bin/env python3
"""Patch 104 (remote_smoothing): another player's correction is drawn
smoothly (../docs/notes/remote_player_smoothing.md). Unicorn (2.1.4).

    python check_remote_smoothing_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join and the
arena). The made-up match is check_ragdoll_026g.py's (its Game: the game
world, the live network client with the game's own vtable, the world core
and its places, players with a standing skeleton, mt_draw's bone array);
here the game world also has its match wrapper (+4D4Ch), so the world core
and its time (+18FC0h) are the ones the frame reads.

Each hook runs from its site in the build to the game's next instruction:
tick_events 0x2D230F (a grid point saved: the item's compare), game_world::
tick 0x1F7C3C (the frame), mt_draw 0x247716 (the bones) and render_name
0x1F2B98 (the name tag's eye). Checked:
- the four sites: the original's bytes in the original exe, a jmp to the
  hook's entry in patches.code in the build; the grid save's compare gives
  the original's flags and registers;
- a correction: another player saved at a grid point, then saved there
  again 0.3 m further (a rollback's re-simulation): its bones are drawn
  0.3 m back at the frame of the correction (the body's every bone, so the
  Weapon bone the held item is posed from), its name tag's eye too; 40 ms
  later half of it, 80 ms later none (the game's bones again);
- the local player, the player the camera looks from (its input handler
  set) and a re-save with the same places are drawn as the game has them;
- a correction of 1 m or more is drawn at once (the game's bones);
- a death during a blend ends it: the name tag is placed from the game's
  eye at once; so does a new life;
- nothing of the world is written: the whole made-up memory (the world
  core, the players, their transforms and eyes, the saved item's bytes)
  is bitwise what it was before every hook, but mt_draw's own bone array
  (which only the draw reads) at the bones.
"""
import struct
import sys

from unicorn.x86_const import (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EDX,
                               UC_X86_REG_ESI, UC_X86_REG_EAX, UC_X86_REG_EFLAGS)

import check_ragdoll_026g as rag
import patch_026g as base

SAVED_SITE, SAVED_NEXT = 0x2D230F, 0x2D2316
TAG_SITE, TAG_NEXT = 0x1F2B98, 0x1F2B9E
SITES = {SAVED_SITE, rag.STEP_SITE, rag.APPLY_SITE, TAG_SITE}
ITEM = rag.HEAP + 0x500000                     # a saved history item (its +0C0BACh, the next item, read)
FLAGS = 0x8D5                                  # CF PF AF ZF SF OF


class Match(rag.Game):
    def __init__(self, data):
        super().__init__(data)
        self.u32(rag.WORLD + 0x4D4C, rag.NC_CORE)  # the match wrapper: its +1F8h the world core
        self.u32(ITEM + 0xC0BAC, 0)

    def snapshot(self):
        return bytes(self.emu.mem_read(rag.HEAP, rag.HEAP_SIZE))

    def place(self, player, x, z):
        self.emu.mem_write(player + rag.TRANSFORM, struct.pack("<16f", *rag.matrix((x, 0, z))))
        self.emu.mem_write(player + rag.EYE, struct.pack("<16f", *rag.matrix((x, 1.64, z + 0.08))))

    def saved(self, time):
        """tick_events just after serialize saved the grid point `time`."""
        self.u32(self.core + 0x18FC0, time)
        regs = {UC_X86_REG_EBX: self.core, UC_X86_REG_ESI: ITEM, UC_X86_REG_EDI: 0xD7, UC_X86_REG_EBP: 0xB9,
                UC_X86_REG_EAX: 0xA1, UC_X86_REG_ECX: 0xC1, UC_X86_REG_EDX: 0xD1}
        out = self.run(SAVED_SITE, SAVED_NEXT, regs)
        return {r: v for r, v in out.items() if r != UC_X86_REG_EFLAGS}, out[UC_X86_REG_EFLAGS] & FLAGS

    def frame(self, time):
        self.u32(self.core + 0x18FC0, time)
        self.step()

    def name_tag(self, player):
        """render_name's eye: the transform its tag is placed from."""
        regs = {UC_X86_REG_ESI: player, UC_X86_REG_EAX: 0xA1, UC_X86_REG_EDX: 0xD1, UC_X86_REG_EDI: 0xD7,
                UC_X86_REG_EBP: 0xB9, UC_X86_REG_EBX: 0xB1}
        out = self.run(TAG_SITE, TAG_NEXT, regs)
        assert all(out[r] == v for r, v in regs.items()), "on_name_tag kept the registers"
        return out[UC_X86_REG_ECX], list(self.floats(out[UC_X86_REG_ECX], 16))


def written(before, after):
    """The made-up memory's words that differ, for a failure's message."""
    words = [i for i in range(0, len(before), 4) if before[i:i + 4] != after[i:i + 4]]
    return "".join(f" {rag.HEAP + i:#x}" for i in words[:8])


def world_of(bone, transform):
    """A bone's place in the world: its model-space place x the transform."""
    p = bone[12:15]
    return [sum(p[r] * transform[r * 4 + c] for r in range(3)) + transform[12 + c] for c in range(3)]


def moved(game, player, drawn, by, turn):
    """Every bone drawn `by` (world) from the game's, the rest of it the game's."""
    transform = game.floats(player + rag.TRANSFORM, 16)
    for i, bone in enumerate(drawn):
        stock = rag.matrix(rag.standing_place(i), turn)
        if any(abs(a - b) > 1e-5 for a, b in zip(bone[:12], stock[:12])):
            return False
        want = [a + b for a, b in zip(world_of(stock, transform), by)]
        if any(abs(a - b) > 1e-4 for a, b in zip(world_of(bone, transform), want)):
            return False
    return True


def check(ok, what):
    rag.check(ok, what)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late_join)
    linked = base.arena()
    block = linked.blocks["patches.code"]
    entries = {h["entry"] for h in base.hooks_026g.hooks(exe=base.hooks_026g.original_reader(original))
               if any(m["patch"] == "remote_smoothing" for m in h.get("chain", [h]))}
    hooks = [h for h in base.rust_hooks(original) if h[2] in entries]
    check(len(hooks) == 4 and {h[0] for h in hooks} == SITES,
          "the Rust patch remote_smoothing's four lohooks (remote_smoothing.toml, sites_026g.tsv)")
    for va, old, symbol, opcode in hooks:
        at = base.file_offset(va)
        target = linked.symbols[symbol]
        new = base.hook_bytes(target, va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old and built[at:at + len(new)] == new
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` in the build")

    game, stock = Match(built), Match(original)
    for match in (game, stock):
        local = match.add_player(0, 0.0, 0.0)
        remote = match.add_player(1, 5.0, 0.0)
        watched = match.add_player(2, 10.0, 0.0)
        match.set_local(local)
        for player in (local, remote):
            match.u32(player + rag.INPUT, 0)        # only the watched one holds the input handler
    for flag_item in (0, 0x4000):
        for match in (game, stock):
            match.u32(ITEM + 0xC0BAC, flag_item)
        check(game.saved(500) == stock.saved(500),
              f"the grid save's compare (next item {flag_item:#x}): the original's registers and flags")
    game.u32(ITEM + 0xC0BAC, 0)

    # the frames before: saved at 450 and 500, drawn at 517 as the game has them
    turn = 0.6
    for player, x in ((local, 0.0), (remote, 5.0), (watched, 10.0)):
        game.emu.mem_write(player + rag.TRANSFORM, struct.pack("<16f", *rag.matrix((x, 0, 0), turn)))
    game.saved(450)
    game.saved(500)
    game.frame(517)
    plain = all(not game.draw(p, turn)[1] for p in (local, remote, watched))
    eye = game.floats(remote + rag.EYE, 16)
    check(plain and game.name_tag(remote) == (remote + rag.EYE, list(eye)),
          "no re-save: every player's bones and the name tag's eye are the game's")

    # the late input came: 500 saved again with everyone 0.3 m further on x
    for player, x in ((local, 0.3), (remote, 5.3), (watched, 10.3)):
        game.emu.mem_write(player + rag.TRANSFORM, struct.pack("<16f", *rag.matrix((x, 0, 0), turn)))
        game.emu.mem_write(player + rag.EYE, struct.pack("<16f", *rag.matrix((x, 1.64, 0.08))))
    game.u32(game.core + 0x18FC0, 500)            # the world at the point (saved() sets it so)
    before = game.snapshot()
    game.saved(500)
    after = game.snapshot()
    check(after == before, "the grid save writes nothing: the world core, the players, the saved item"
          + written(before, after))
    game.u32(game.core + 0x18FC0, 533)
    before = game.snapshot()
    game.frame(533)
    after = game.snapshot()
    check(after == before, "the frame writes nothing of the made-up memory" + written(before, after))
    world = game.snapshot()
    drawn, changed = game.draw(remote, turn)
    array = (rag.ARRAY - rag.HEAP, rag.ARRAY - rag.HEAP + 64 * len(rag.COOKED))
    now = game.snapshot()
    check(changed and moved(game, remote, drawn, (-0.3, 0, 0), turn)
          and now[:array[0]] == world[:array[0]] and now[array[1]:] == world[array[1]:],
          "a correction: every bone of the other player drawn 0.3 m back (its model's space, turned), "
          "the rest of the bones the game's; nothing written but mt_draw's own array")
    before = game.snapshot()
    at, tag = game.name_tag(remote)
    after = game.snapshot()
    check(after == before, "the name tag writes nothing of the made-up memory" + written(before, after))
    check(at != remote + rag.EYE and tag[:12] == list(game.floats(remote + rag.EYE, 12))
          and abs(tag[12] - (5.3 - 0.3)) < 1e-5 and tag[13:] == list(game.floats(remote + rag.EYE + 52, 3)),
          "its name tag placed from its eye 0.3 m back (a copy: the player's eye untouched)")
    check(not game.draw(local, turn)[1] and not game.draw(watched, turn)[1],
          "the local player and the one the camera looks from: the game's bones")
    game.frame(573)
    drawn, _ = game.draw(remote, turn)
    check(moved(game, remote, drawn, (-0.15, 0, 0), turn), "40 ms on: half of it")
    game.frame(613)
    check(not game.draw(remote, turn)[1] and game.name_tag(remote)[0] == remote + rag.EYE,
          "80 ms on: the game's bones and eye")

    # the same places saved again: no correction
    game.saved(500)
    game.frame(630)
    check(not game.draw(remote, turn)[1], "a re-save with the same places: drawn as the game has it")

    # a correction of a metre: at once
    game.saved(550)
    game.emu.mem_write(remote + rag.TRANSFORM, struct.pack("<16f", *rag.matrix((6.3, 0, 0), turn)))
    game.saved(550)
    game.frame(650)
    check(not game.draw(remote, turn)[1], "a correction of 1 m: drawn at once")

    # a death during a blend (the same life), then a new life during one
    for change, alive, life, grid in (("death", False, 1000, 600), ("new life", True, 12000, 700)):
        game.emu.mem_write(remote + rag.TRANSFORM, struct.pack("<16f", *rag.matrix((6.0, 0, 0), turn)))
        game.saved(grid)
        game.emu.mem_write(remote + rag.TRANSFORM, struct.pack("<16f", *rag.matrix((6.2, 0, 0), turn)))
        game.saved(grid)
        game.frame(grid + 17)
        check(game.name_tag(remote)[0] != remote + rag.EYE, f"before the {change}: blending")
        game.set_alive(remote, alive, life)
        game.frame(grid + 33)
        check(game.name_tag(remote)[0] == remote + rag.EYE, f"a {change} during the blend: the game's eye at once")
        game.set_alive(remote, True, 9000)
    print("ok: every check")


if __name__ == "__main__":
    main()
