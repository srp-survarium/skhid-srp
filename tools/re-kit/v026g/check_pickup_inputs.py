#!/usr/bin/env python3
"""Exercise native input placement before coupling pickup targets to it.

The actual change_input body, list insertion, input-node pool checkout and
eviction run. Seen-input allocation, history-dirty notifications and logging
are fixture endpoints. This does not simulate a match, install pickup hooks
or prove rollback of transferred weapons.
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STOP, STACK
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESP

CHANGE = 0x1FAB50


class Inputs:
    def __init__(self, data):
        self.g = g = Game(data)
        self.core = core = g.block(0x19000)
        self.nodes = g.block(20 * 0x480)
        self.seen = g.block(20 * 0x190)
        self.out = g.block(4)
        for place in range(20):
            pool = g.block(0x30)
            nodes = g.block(27 * 0x28)
            for n in range(27):
                g.word(nodes + n * 0x28, nodes + (n + 1) * 0x28 if n < 26 else 0)
            g.word(pool + 0x20, nodes)
            g.word(pool + 0x28, 27)
            g.word(self.nodes + place * 0x480 + 0x10, pool)
        # Minimal real history node for move/discard's preceding-state lookup.
        history = 0x23000000
        g.emu.mem_map(history, 0xD0000)
        g.word(history + 0xC0BB0, 500)
        g.word(core + 0x2FC0, history)
        g.word(core + 0x2FCC, self.nodes)
        g.word(core + 0xE3E4, self.seen)
        g.word(core + 0x18FC4, 1000)  # newest simulated time
        g.word(core + 0x18FB4, 500)   # rollback horizon
        g.word(core + 0x18FC8, 0xFFFFFFFF)
        g.word(core + 0x18FCC, 0xFFFFFFFF)
        g.emu.mem_write(core + 0x18FDC, bytes([0, 0, 0]))  # local place, server, reject
        g.word(0x1F1D60C, 0)
        g.word(0x9DAF4C, 0)
        g.word(0x1F1D614, 0)
        self.dirty = []
        self.move_receipts = []
        self.discard_receipts = []
        self.retimes = []
        g.emu.hook_add(UC_HOOK_CODE, self.stub)

    def stub(self, emu, at, size, user):
        g = self.g
        if at == 0x27DCB7:
            frame = emu.reg_read(UC_X86_REG_EBP)
            self.discard_receipts.append((emu.reg_read(UC_X86_REG_EBX),
                                          g.read(frame + 8), g.read(frame + 12)))
            return
        if at == 0x1FB349:
            node, frame = emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EBP)
            self.retimes.append(("merge", emu.reg_read(UC_X86_REG_EBX),
                                 g.read(frame + 0x18) & 255, node, g.read(node + 0x20)))
            return
        if at == 0x2D4F3B:
            core = emu.reg_read(UC_X86_REG_EBX)
            self.retimes.append(("snap", core, bytes(emu.mem_read(core + 0x18FDC, 1))[0],
                                 emu.reg_read(UC_X86_REG_EDI), emu.reg_read(UC_X86_REG_ESI)))
            return
        if at == 0x27D802:
            frame, stack = emu.reg_read(UC_X86_REG_EBP), emu.reg_read(UC_X86_REG_ESP)
            self.move_receipts.append((emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_EBX),
                                       g.read(frame + 8), g.read(frame + 12), g.read(stack + 0x1C)))
            return
        if at not in (0x1FCB00, 0x2D1100, 0x2D1340, 0x111ED0):
            return
        esp = emu.reg_read(UC_X86_REG_ESP)
        cleanup = 0
        if at == 0x1FCB00:
            node = g.block(0x28)
            emu.reg_write(UC_X86_REG_EAX, node)
        elif at == 0x2D1100:
            # Real function invalidates snapshots from the input's tick.
            self.dirty.append(("insert", emu.reg_read(UC_X86_REG_EAX)))
            # No following dirty timeline event in this fixture. Snapping
            # consumes this return after updating the current input's time.
            emu.reg_write(UC_X86_REG_EAX, 0)
            cleanup = 20
        elif at == 0x2D1340:
            self.dirty.append(("remove", emu.reg_read(UC_X86_REG_EAX)))
            cleanup = 12
        # Logging is unrelated, caller cleans its 14 arguments.
        emu.reg_write(UC_X86_REG_EIP, g.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + cleanup)

    def submit(self, time, *, place=1, actions=4, rotation=0, network=True,
               flagged=False, precise=False, corrected=False):
        g = self.g
        g.word(self.out, 0xFFFFFFFF)
        g.call(CHANGE, (self.core, rotation, actions, time,
                       place | (int(flagged) << 8), int(network), int(corrected),
                       int(precise), self.out))
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        result = g.emu.reg_read(UC_X86_REG_EAX)
        placed = g.read(self.out)
        return result, None if placed == 0xFFFFFFFF else placed

    def move(self, before, after, place=0):
        self.g.call(0x27D2D0, (self.core, place, after), eax=before)
        assert self.g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert self.g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        return self.g.emu.reg_read(UC_X86_REG_EAX)

    def discard(self, time, place=0):
        self.g.call(0x27D820, (self.core, place), eax=time)
        assert self.g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert self.g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        return self.g.emu.reg_read(UC_X86_REG_EAX)

    def inputs(self, place):
        g = self.g
        node = g.read(self.nodes + place * 0x480 + 4)
        out = []
        while node:
            assert len(out) < 30
            out.append((g.read(node + 0x20), g.read(node + 8), g.read(node + 12)))
            node = g.read(node)
        return out


def local_pickup_timing(data):
    use = 1 << 21
    # Execute game_world::tick's actual local caller, including its stack
    # setup, wrapper1EEDE0 and native change_input. Stop at the proposed
    # completion boundary, before it reloads the player's input handler.
    i = Inputs(data)
    g = i.g
    world, frame = g.block(0x4D50), g.block(16)
    player, handler = g.block(0xF474), g.block(0x348)
    g.word(player + 0xF470, handler)
    g.word(handler + 0x33C, use | 4)
    g.word(handler + 0x344, 4)  # preceding frame, before Use was pressed
    g.word(world + 0x4D4C, i.core - 0x1F8)
    g.word(frame + 12, 901)
    esp = STACK - 0x100
    for offset, value in [(0x14, 123), (0x18, player), (0x20, 0), (0x24, use | 4), (0x2C, 0)]:
        g.word(esp + offset, value)
    for reg, value in [(UC_X86_REG_ESP, esp), (UC_X86_REG_EBX, world),
                       (UC_X86_REG_EBP, frame), (UC_X86_REG_EDI, esp + 0x20)]:
        g.emu.reg_write(reg, value)
    g.emu.emu_start(0x1F7903, 0x1F7928, count=2_000_000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x1F7928
    assert g.emu.reg_read(UC_X86_REG_EAX) == 0
    assert g.emu.reg_read(UC_X86_REG_ESP) == esp
    assert g.read(esp + 0x14) == g.read(esp + 0x28) == 901
    assert g.read(esp + 0x24) == use | 4
    assert g.read(esp + 0x2C) == 0
    assert i.inputs(0) == [(901, 0, use | 4)]
    assert g.read(handler + 0x344) == 4
    # The completion site sees the old mask. Native code overwrites it only
    # afterward, providing a Use edge without another global key-state cache.
    g.emu.emu_start(0x1F7928, 0x1F794A, count=1000)
    assert g.read(handler + 0x344) == use | 4

    for promote in [False, True]:
        i = Inputs(data)
        g = i.g
        # precise=True has NO effect on the native local path. Only its fire
        # transition decides precision, so a Use input remains coalescible.
        assert i.submit(901, place=0, network=False, actions=use, precise=True) == (0, 901)
        target_node = g.read(i.nodes + 4)
        assert g.emu.mem_read(target_node + 0x24, 1)[0] == 0
        assert g.read(i.core + 0x18FCC) == 901
        if promote:
            # These are the two field writes tested in Rust CompletedLocalInput.
            # This fixture checks their effects on original native consumers.
            g.emu.mem_write(target_node + 0x24, b'\x01')
            g.word(i.core + 0x18FCC, 0xFFFFFFFF)
        _, effective = i.submit(912, place=0, network=False, actions=4)
        assert effective == 912
        if not promote:
            assert i.inputs(0) == [(912, 0, use | 4)]
            continue
        assert i.inputs(0) == [(901, 0, use), (912, 0, 4)]
        # The next ordinary input can still snap normally. It cannot pull
        # the earlier targeted input into its node or move its timestamp.
        event = g.block(0x1C0)
        g.word(event + 0x1BC, 912)
        g.word(i.core + 4, event)
        g.word(i.core + 0x18FBC, 50)
        g.call(0x2D4E40, (i.core, 1000))
        assert i.inputs(0) == [(901, 0, use), (950, 0, 4)]
        assert g.read(target_node + 0x20) == 901
    print('native local pickup timing: caller receipt layout, ignored precise argument, '
          'completed-node promotion and independent subsequent coalescing/tick snap passed')


def run(data):
    local_pickup_timing(data)
    i = Inputs(data)
    assert i.submit(923, precise=True) == (0, 923)
    assert i.inputs(1) == [(923, 0, 4)]
    # Duplicate succeeds without writing an effective time. A wrapper must
    # not create a second target merely because the return code is non-error.
    assert i.submit(923, actions=8, precise=True) == (1, None)
    assert i.inputs(1) == [(923, 0, 4)]
    assert i.dirty == [("insert", 923)]

    old = Inputs(data)
    assert old.submit(423, precise=True) == (2, 1023)
    assert old.inputs(1) == [(1023, 0, 4)]
    assert old.dirty == [("insert", 1023)]

    refused = Inputs(data)
    refused.g.word(refused.core + 0x18FD4, 950)
    assert refused.submit(923) == (6, None)
    assert not refused.inputs(1) and not refused.dirty
    rejected = Inputs(data)
    rejected.g.emu.mem_write(rejected.core + 0x18FDE, b"\x01")
    assert rejected.submit(423, precise=True) == (4, None)
    assert not rejected.inputs(1) and not rejected.dirty

    future = Inputs(data)
    future.g.word(future.core + 0x2FBC, future.g.block(16))  # nonempty history
    assert future.submit(4007, precise=True) == (3, 3457)
    assert future.inputs(1) == [(3457, 0, 4)]

    # Replay calls the local branch (network=false). A following input may
    # pull an off-grid node forward and merge its actions into that node.
    local = Inputs(data)
    assert local.submit(901, place=0, network=False) == (0, 901)
    code, when = local.submit(912, place=0, network=False, actions=8)
    assert when == 912
    assert code == 0  # accepted merge reports the new time as well
    assert local.inputs(0) == [(912, 0, 12)]
    assert local.dirty == [("insert", 901), ("remove", 901), ("insert", 912)]
    node = local.g.read(local.nodes + 4)
    assert local.retimes == [("merge", local.core, 0, node, 912)]

    # Native snap-to-tick operates outside change_input/move_player_input.
    # With a timeline event at the input time, it waits until the destination
    # tick is within the requested limit, then moves that whole node.
    snap = Inputs(data)
    assert snap.submit(912, place=0, network=False) == (0, 912)
    node = snap.g.read(snap.nodes + 4)
    event = snap.g.block(0x1C0)
    snap.g.word(event + 0x1BC, 912)
    snap.g.word(snap.core + 4, event)
    snap.g.word(snap.core + 0x18FBC, 50)
    snap.g.call(0x2D4E40, (snap.core, 925))
    assert snap.g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert snap.g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
    assert snap.inputs(0) == [(912, 0, 4)] and not snap.retimes
    assert snap.g.read(snap.core + 0x18FCC) == 912
    snap.g.call(0x2D4E40, (snap.core, 1000))
    assert snap.g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert snap.g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
    assert snap.inputs(0) == [(950, 0, 4)]
    assert snap.retimes == [("snap", snap.core, 0, node, 950)]
    assert snap.dirty == [("insert", 912), ("remove", 912), ("insert", 950)]
    # Original F0/F2 operations: own and corrected portions coexist. Move
    # carries only the own portion into the corrected portion of its target.
    split = Inputs(data)
    assert split.submit(800, place=0, network=False, actions=4) == (0, 800)
    assert split.submit(800, place=0, corrected=True, actions=2) == (0, 800)
    assert split.submit(850, place=0, network=False, actions=8) == (0, 850)
    assert split.inputs(0) == [(800, 0, 6), (850, 0, 8)]
    assert split.move(800, 850) == 0
    assert split.inputs(0) == [(800, 0, 2), (850, 0, 12)]
    assert split.move(850, 900) == 0
    assert split.inputs(0) == [(800, 0, 2), (850, 0, 4), (900, 0, 8)]
    assert split.move(800, 950) == 1  # corrected-only source has no own part
    assert split.inputs(0) == [(800, 0, 2), (850, 0, 4), (900, 0, 8)]
    assert split.discard(850) == 0
    assert split.inputs(0) == [(800, 0, 2), (900, 0, 8)]
    assert split.discard(875) == 1  # no input at that exact time
    assert split.inputs(0) == [(800, 0, 2), (900, 0, 8)]

    # Move's final status is the destination placement status. A refusal can
    # happen AFTER extraction, unlike a refused standalone change or discard.
    for corrected in (False, True):
        refused_move = Inputs(data)
        assert refused_move.submit(800, place=0, network=False) == (0, 800)
        if corrected:
            assert refused_move.submit(800, place=0, corrected=True, actions=2) == (0, 800)
        refused_move.g.word(refused_move.core + 0x18FD4, 600)
        assert refused_move.move(800, 550) == 6
        assert refused_move.inputs(0) == ([(800, 0, 2)] if corrected else [])

    retimed_move = Inputs(data)
    assert retimed_move.submit(800, place=0, network=False) == (0, 800)
    assert retimed_move.move(800, 423) == 2
    assert retimed_move.inputs(0) == [(1023, 0, 4)]
    assert retimed_move.move_receipts[-1][2:] == (retimed_move.core, 0, 1023)
    assert retimed_move.move_receipts[-1][0] == 2
    for corrected in (False, True):
        # The receipt is reached after late refusal, not after early refusal.
        receipt = Inputs(data)
        assert receipt.submit(800, place=0, network=False) == (0, 800)
        source = receipt.g.read(receipt.nodes + 4)
        if corrected:
            assert receipt.submit(800, place=0, corrected=True, actions=2) == (0, 800)
        receipt.g.word(receipt.core + 0x18FD4, 600)
        assert receipt.move(800, 550) == 6
        assert receipt.move_receipts == [(6, source, receipt.core, 0, 0xFFFFFFFF)]
        assert receipt.move(500, 900) == 6
        assert len(receipt.move_receipts) == 1

    # Real pool checkout, oldest-node eviction, free-list pop and reset all
    # execute. A full pool may evict the corrected source while placing its
    # extracted original, then reuse that exact address for the destination.
    recycled = Inputs(data)
    assert recycled.submit(800, place=0, network=False) == (0, 800)
    assert recycled.submit(800, place=0, corrected=True, actions=2) == (0, 800)
    source = recycled.g.read(recycled.nodes + 4)
    for n in range(26):
        t = 850 + n * 50
        assert recycled.submit(t, place=0) == (0, t)
    assert len(recycled.inputs(0)) == 27
    assert recycled.move(800, 2150) == 0
    assert len(recycled.inputs(0)) == 27
    assert recycled.inputs(0)[0][0] == 850
    assert recycled.inputs(0)[-1] == (2150, 0, 4)
    assert recycled.g.read(recycled.nodes + 8) == source
    assert recycled.move_receipts == [(0, source, recycled.core, 0, 2150)]

    self_move = Inputs(data)
    assert self_move.submit(800, place=0, network=False) == (0, 800)
    assert self_move.submit(800, place=0, corrected=True, actions=2) == (0, 800)
    assert self_move.move(800, 800) == 0
    assert self_move.inputs(0) == [(800, 0, 6)]
    assert self_move.move(800, 850) == 1  # now entirely corrected

    # A local prediction, then a corrected arrival, then an ordinary relay at
    # the predicted time. The original portion is replaced; corrected stays.
    # The effective action mask is cumulative until a move splits it again.
    replaced = Inputs(data)
    assert replaced.submit(1023, place=0, network=False, actions=4) == (0, 1023)
    assert replaced.submit(423, place=0, precise=True, actions=2) == (2, 1023)
    assert replaced.submit(1023, place=0, precise=True, actions=8) == (0, 1023)
    node = replaced.g.read(replaced.nodes + 4)
    assert replaced.g.read(node + 0x14) == 8   # original portion
    assert replaced.g.read(node + 0x1C) == 2   # corrected portion
    assert replaced.inputs(0) == [(1023, 0, 14)]
    assert replaced.move(1023, 1100) == 0
    assert replaced.inputs(0) == [(1023, 0, 2), (1100, 0, 8)]

    # Every nonzero discard status leaves the existing input intact.
    for scenario, expected in [("resync", 6), ("history", 4), ("past_end", 5), ("gap", 1)]:
        refused_discard = Inputs(data)
        assert refused_discard.submit(800, place=0, network=False) == (0, 800)
        time = 800
        if scenario == "resync":
            refused_discard.g.word(refused_discard.core + 0x18FD4, 800)
        elif scenario == "history":
            refused_discard.g.word(refused_discard.core + 0x2FC0, 0)
        elif scenario == "past_end":
            time = 850
        else:
            time = 750
        assert refused_discard.discard(time) == expected
        assert refused_discard.inputs(0) == [(800, 0, 4)]

    print("native pickup input boundary: accepted/duplicate/refused/retimed network "
          "inputs, precise timestamps, local-node merge/tick snapping, split-part moves, refused move "
          "after extraction, full discard/refusal, native pool reuse and post-move receipt passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
