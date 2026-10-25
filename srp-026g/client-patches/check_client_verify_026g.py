#!/usr/bin/env python3
"""The client check's Unicorn check (patches 75, 77, 78;
../docs/notes/client_world_check.md sections 8, 10, 12 and 14), with
Unicorn (2.1.4). It runs the arena as patch_026g.py links it (ARENA): the
Rust hooks of rust/client_verify (srp-validate's checks)
and the NASM entries of client_verify_026g.asm, on made-up game objects.

    python check_client_verify_026g.py [--drift DRIFT.tsv] [--counts OUT.tsv]
                                       [--packets DIR] [--quiet]

- The entries: cv_world_load with a world the check reads goes on to
  game_world_core::deserialize (0x2D0B70) with its four arguments, the
  stack and the kept registers as the call site left them; with one it
  refuses, the reader is consumed, close_current_match (0x27A790) is called
  with ECX the network client (EDI) and 2, and the handler's epilogue
  (0x27C1E0) is reached with EBP the handler's frame. cv_message alike: the
  dispatch's table entry of the id, or the match left and the dispatch's
  epilogue (0x27A72C). cv_anomaly and cv_artefact: an index in its table
  read as the game reads it, one past it on the game's own none path.
  Each at the stack alignments 0, 4, 8 and 12 (rustc's i686 code assumes
  16 at a call; the entries align).
- The drift test (--drift, the output of srp-validate's drift_dump example
  on the kept worlds): every world and its mutated copies, the i686
  payload's verdict against the host build's, with a world core whose rule
  list says the match drift_dump chose (the mode's rule by its vtable with
  its batteries, the respawn rule with its points). Any difference fails.
- The cost (--counts): the instructions the arena runs for each kept
  world from cv_world_load (and from cv_message for every message of the packet logs with
  --packets), written as TSV and summed up (min, median, p99, max).
"""
import argparse
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

WORLD_LOAD, WORLD_EPILOGUE, CLOSE_MATCH = 0x2D0B70, 0x27C1E0, 0x27A790
DISPATCH_TABLE, DISPATCH_EPILOGUE = 0x27A738, 0x27A72C
ANOMALY_BACK, ANOMALY_NO_STATE = 0x31B108, 0x31B0FB
ARTEFACT_BACK, ARTEFACT_NONE = 0x2FE663, 0x2FE67A
# The rule vtables (client_world_check.md section 7) and the core's fields.
GATHER, VI_SPAWN, TDM, RESPAWN, TIMELIMIT, KD = 0xA06AB4, 0xA06C04, 0xA06CBC, 0xA06D1C, 0xA06E2C, 0xA177D4
RULES_BEGIN = 0x18824
MODE_VTABLES = {2: GATHER, 3: VI_SPAWN, 4: TDM}

GAME = 0x10000                    # the game's code pages: stops only
STATE, CORE, RULES, OBJECTS = 0x51000000, 0x52000000, 0x53000000, 0x54000000
STACK, SENTINEL = 0x60000000, 0x6F000000


class Payload:
    def __init__(self):
        self.linked = base.arena()
        self.symbols = self.linked.symbols
        emu = Uc(UC_ARCH_X86, UC_MODE_32)
        size = (self.linked.size + 0xFFF) & ~0xFFF
        emu.mem_map(self.linked.va, size, UC_PROT_ALL)
        emu.mem_write(self.linked.va, bytes(self.linked.image))
        emu.mem_map(GAME, 0x1000000 - GAME, UC_PROT_ALL)   # 0xCC: any reach stops
        emu.mem_write(GAME, b"\xCC" * (0x1000000 - GAME))
        for base_va, size in ((STATE, 0x20000), (CORE, 0x20000), (RULES, 0x10000), (OBJECTS, 0x40000),
                              (STACK - 0x100000, 0x100000), (SENTINEL, 0x1000)):
            emu.mem_map(base_va, size, UC_PROT_ALL)
        emu.mem_write(SENTINEL, b"\xF4" * 0x1000)
        self.emu = emu
        self.research = self.symbols["rn_rule_vtable"] + 4
        # A run stops where the code reaches the game (its pages hold int3):
        # the address is the run's end.
        self.reached = []

        def stop(uc, address, size, data):
            self.reached.append(address)
            uc.emu_stop()
        emu.hook_add(UC_HOOK_CODE, stop, begin=GAME, end=0x1000000 - 1)

    def core(self, game_mode, batteries, respawn):
        """A world core whose rule list is the match's: timelimit, respawn,
        the mode's rule, kd stats (the order of the mode's config)."""
        emu = self.emu
        emu.mem_write(RULES, b"\0" * 0x10000)
        mode_vtable = MODE_VTABLES.get(game_mode, self.research)
        objects = [(TIMELIMIT, {}), (RESPAWN, {0x194: respawn, 0x198: 0, 0x19C: 0}),
                   (mode_vtable, {0x110: OBJECTS, 0x114: OBJECTS + 4 * batteries}), (KD, {})]
        pointers = []
        for n, (vtable, fields) in enumerate(objects):
            at = RULES + 0x1000 * (n + 1)
            emu.mem_write(at, struct.pack("<I", vtable))
            for offset, value in fields.items():
                emu.mem_write(at + offset, struct.pack("<I", value))
            pointers.append(at)
        emu.mem_write(RULES, struct.pack(f"<{len(pointers)}I", *pointers))
        emu.mem_write(CORE + RULES_BEGIN, struct.pack("<II", RULES, RULES + 4 * len(pointers)))
        return CORE

    def run(self, entry, esp, regs, count=False):
        """Runs the entry from `esp` with `regs` until the code reaches the
        game (an address of its pages): that address (None: it never did),
        and the instructions run in the arena when `count`."""
        emu = self.emu
        self.reached.clear()
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, esp)
        ran = [0]
        hook = None
        if count:
            def tick(uc, address, size, data):
                ran[0] += 1
            # Code translated before the hook runs without it: drop it.
            emu.ctl_remove_cache(self.linked.va, self.linked.va + self.linked.size)
            hook = emu.hook_add(UC_HOOK_CODE, tick, begin=self.linked.va, end=self.linked.va + self.linked.size)
        try:
            emu.emu_start(entry, 0xFFFFFFFF, count=50_000_000)
        finally:
            if hook is not None:
                emu.hook_del(hook)
        return (self.reached[0] if self.reached else None), ran[0]

    def world(self, state, game_mode, batteries, respawn, count=False):
        """cv_world_load on `state` (the time, then the body) as the 0xE8
        handler's call enters it: (1 refused / 0 read, instructions)."""
        self.emu.mem_write(STATE, state)
        core = self.core(game_mode, batteries, respawn)
        reader = OBJECTS + 0x30000
        self.emu.mem_write(reader, struct.pack("<III", STATE - 2, STATE, len(state) + 2))
        esp = STACK - 0x2000
        self.emu.mem_write(esp, struct.pack("<IIIII", 0x27C0DE, core, reader, 1, 8))
        at, ran = self.run(self.symbols["cv_world_load"], esp, {UC_X86_REG_EDI: 0x0C11E000}, count)
        if at not in (WORLD_LOAD, CLOSE_MATCH):
            raise SystemExit(f"cv_world_load reached {at!r}")
        return int(at == CLOSE_MATCH), ran

    def message(self, message_id, payload, count=False):
        """cv_message on a message as the dispatch enters it: (1 refused /
        0 read, instructions)."""
        self.emu.mem_write(STATE, payload)
        reader = OBJECTS + 0x30000
        self.emu.mem_write(reader, struct.pack("<III", STATE - 1, STATE, len(payload) + 1))
        ebp = STACK - 0x3000
        self.emu.mem_write(ebp + 0xC, struct.pack("<I", message_id))
        handler = 0x00E6E6E6
        self.emu.mem_write(DISPATCH_TABLE + 4 * (message_id - 0xE3), struct.pack("<I", handler))
        at, ran = self.run(self.symbols["cv_message"], STACK - 0x3100,
                           {UC_X86_REG_EBX: reader, UC_X86_REG_ESI: 0x0C11E000, UC_X86_REG_EBP: ebp,
                            UC_X86_REG_EAX: message_id - 0xE3}, count)
        if at not in (handler, CLOSE_MATCH):
            raise SystemExit(f"cv_message reached {at!r}")
        return int(at == CLOSE_MATCH), ran


def check_entries(payload, world_state, world_match, quiet):
    """The NASM entries' paths, at four stack alignments."""
    emu, sym = payload.emu, payload.symbols
    failures = []

    def go(entry, esp, regs):
        return payload.run(entry, esp, regs)[0]

    bad_state = bytearray(world_state)
    bad_state[4:8] = b"\xff\xff\xff\xff"      # a mask naming 32 places
    reader = OBJECTS + 0x30000
    for align in (0, 4, 8, 12):
        for state, wanted in ((world_state, WORLD_LOAD), (bytes(bad_state), CLOSE_MATCH)):
            emu.mem_write(STATE, state)
            core = payload.core(*world_match)
            emu.mem_write(reader, struct.pack("<III", STATE - 2, STATE, len(state) + 2))
            esp = STACK - 0x2000 - align
            args = struct.pack("<IIIII", 0x27C0DE, core, reader, 1, 8)
            emu.mem_write(esp, args)
            regs = {UC_X86_REG_EDI: 0x0C11E000, UC_X86_REG_EBP: 0x77770000, UC_X86_REG_EBX: 0x1111,
                    UC_X86_REG_ESI: 0x2222}
            at = go(sym["cv_world_load"], esp, regs)
            if at != wanted:
                failures.append(f"cv_world_load (align {align}): reached {at!r}, wanted {wanted:#x}")
                continue
            esp_now = emu.reg_read(UC_X86_REG_ESP)
            if wanted == WORLD_LOAD:
                if esp_now != esp or emu.mem_read(esp, 20) != args or emu.reg_read(UC_X86_REG_EBP) != 0x77770000 \
                        or emu.reg_read(UC_X86_REG_EDI) != 0x0C11E000 or emu.reg_read(UC_X86_REG_EBX) != 0x1111 \
                        or emu.reg_read(UC_X86_REG_ESI) != 0x2222:
                    failures.append(f"cv_world_load (align {align}): the world load is not called as the site did")
            else:
                # close_current_match: thiscall, ECX the client, [esp+4] 2.
                reason = struct.unpack("<I", emu.mem_read(esp_now + 4, 4))[0]
                pos = struct.unpack("<I", emu.mem_read(reader + 4, 4))[0]
                if emu.reg_read(UC_X86_REG_ECX) != 0x0C11E000 or reason != 2 or pos != STATE + len(state):
                    failures.append(f"cv_world_load (align {align}): the refusal's close_current_match or "
                                    "the reader is wrong")
                # Its `ret 4`, then the epilogue.
                ret = struct.unpack("<I", emu.mem_read(esp_now, 4))[0]
                emu.reg_write(UC_X86_REG_ESP, esp_now + 8)
                at = go(ret, esp_now + 8, {})
                if at != WORLD_EPILOGUE or emu.reg_read(UC_X86_REG_EBP) != 0x77770000:
                    failures.append(f"cv_world_load (align {align}): after the leave, {at!r}, wanted the epilogue")
        # cv_message: a ping (0xE6, 4 bytes) read, then one of 3 refused.
        for payload_bytes, wanted in ((b"\x01\x02\x03\x04", 0x00E6E6E6), (b"\x01\x02\x03", CLOSE_MATCH)):
            emu.mem_write(STATE, payload_bytes)
            emu.mem_write(reader, struct.pack("<III", STATE - 1, STATE, len(payload_bytes) + 1))
            ebp = STACK - 0x3000
            emu.mem_write(ebp + 0xC, struct.pack("<I", 0xE6))
            emu.mem_write(DISPATCH_TABLE + 4 * (0xE6 - 0xE3), struct.pack("<I", 0x00E6E6E6))
            esp = STACK - 0x3100 - align
            at = go(sym["cv_message"], esp, {UC_X86_REG_EBX: reader, UC_X86_REG_ESI: 0x0C11E000,
                                             UC_X86_REG_EBP: ebp, UC_X86_REG_EAX: 0xE6 - 0xE3})
            if at != wanted:
                failures.append(f"cv_message (align {align}): reached {at!r}, wanted {wanted:#x}")
                continue
            if wanted != CLOSE_MATCH:
                if emu.reg_read(UC_X86_REG_ESP) != esp or emu.reg_read(UC_X86_REG_EBX) != reader:
                    failures.append(f"cv_message (align {align}): the handler is not reached as the jump did")
            else:
                esp_now = emu.reg_read(UC_X86_REG_ESP)
                ret = struct.unpack("<I", emu.mem_read(esp_now, 4))[0]
                at = go(ret, esp_now + 8, {})
                pos = struct.unpack("<I", emu.mem_read(reader + 4, 4))[0]
                if at != DISPATCH_EPILOGUE or emu.reg_read(UC_X86_REG_EBP) != ebp or pos != STATE + 3:
                    failures.append(f"cv_message (align {align}): the refusal does not leave by the epilogue")
    emu.mem_write(DISPATCH_TABLE, b"\xCC" * 0x50)
    # cv_anomaly: states at OBJECTS, three of them.
    anomaly = OBJECTS + 0x100
    emu.mem_write(OBJECTS, struct.pack("<III", 0xA1, 0xA2, 0xA3))
    emu.mem_write(anomaly + 0x68, struct.pack("<II", OBJECTS, OBJECTS + 12))
    for state_id, (wanted, ebp) in ((2, (ANOMALY_BACK, 0xA3)), (3, (ANOMALY_NO_STATE, None)),
                                    (0xFE, (ANOMALY_NO_STATE, None))):
        at = go(sym["cv_anomaly"], STACK - 0x4000, {UC_X86_REG_EDI: anomaly, UC_X86_REG_ECX: state_id})
        if at != wanted or (ebp is not None and emu.reg_read(UC_X86_REG_EBP) != ebp):
            failures.append(f"cv_anomaly: id {state_id} reached {at!r}")
    # cv_artefact: a table of two at OBJECTS.
    holder = OBJECTS + 0x1000
    emu.mem_write(holder + 0x18944, struct.pack("<I", OBJECTS + 8))
    for artefact, wanted, eax in ((1, ARTEFACT_BACK, 0xA2), (2, ARTEFACT_NONE, None), (254, ARTEFACT_NONE, None)):
        at = go(sym["cv_artefact"], STACK - 0x4000, {UC_X86_REG_EAX: artefact, UC_X86_REG_EDX: OBJECTS,
                                                      UC_X86_REG_ECX: holder, UC_X86_REG_EBP: 0x5555})
        if at != wanted or (eax is not None and emu.reg_read(UC_X86_REG_EAX) != eax) \
                or (eax is None and emu.reg_read(UC_X86_REG_EBP) != 0):
            failures.append(f"cv_artefact: id {artefact} reached {at!r}")
    emu.mem_write(GAME, b"\xCC" * (0x1000000 - GAME))
    if not quiet:
        print("entries: " + ("ok" if not failures else f"{len(failures)} failures"))
    return failures


def drift_rows(path):
    for line in open(path):
        file, time, game_mode, batteries, respawn, verdicts = line.rstrip("\n").split("\t")
        yield file, int(time), int(game_mode), int(batteries), int(respawn), [int(v) for v in verdicts]


def state_of(file, time):
    return struct.pack("<I", time) + open(file, "rb").read()


def pct(values, p):
    values = sorted(values)
    return values[min(len(values) - 1, round((len(values) - 1) * p / 100))]


def packet_messages(folder):
    """(log, id, payload) of every message the server sent in the packet logs
    under `folder` (the `OUT` datagrams; parts joined), as the bench reads them."""
    for root, _, files in os.walk(folder):
        for name in sorted(files):
            if not (name.startswith("match-") and name.endswith(".log")):
                continue
            datagram, parts = None, {}
            for line in open(os.path.join(root, name), errors="replace"):
                if line.startswith("20"):
                    datagram = None
                    if " OUT " in line and " len=" in line:
                        words = line.split(" len=", 1)[1].split()
                        datagram = bytes(int(w, 16) for w in words[1:])
                    continue
                if not line.startswith("    msg id=") or datagram is None:
                    continue
                fields = dict(w.split("=", 1) for w in line.split()[1:] if "=" in w)
                message_id = int(fields["id"], 16)
                if not 0xE2 <= message_id <= 0xF6 or "len" not in fields:
                    continue
                at, size = int(fields["at"]), int(fields["len"])
                piece = datagram[at:at + size]
                if "part" not in fields:
                    yield name, message_id, piece
                    continue
                x, part = fields.get("x", "0"), fields["part"]
                if "/" in part:
                    first, total = part.split("/")
                    if first == "0":
                        parts[x] = [int(total), bytearray(piece), 1]
                elif x in parts:
                    parts[x][1] += piece
                    parts[x][2] += 1
                if x in parts and parts[x][2] == parts[x][0]:
                    yield name, message_id, bytes(parts.pop(x)[1])


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--drift")
    parser.add_argument("--counts")
    parser.add_argument("--packets")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()
    payload = Payload()
    failures = []

    rows = list(drift_rows(args.drift)) if args.drift else []
    passing = next((r for r in rows if r[5][0] == 0), None)
    if passing is None:
        # A world of no player: the mask 0, six empty sections but the
        # statistics and rules a check refuses; enough for the entries' paths
        # only when no drift rows are given.
        raise SystemExit("the entries' check needs --drift with a world the check reads")
    failures += check_entries(payload, state_of(passing[0], passing[1]), passing[2:5], args.quiet)

    if rows:
        differ = 0
        worlds = 0
        for file, time, game_mode, batteries, respawn, verdicts in rows:
            body = open(file, "rb").read()
            for k, wanted in enumerate(verdicts):
                mutated = bytearray(body)
                if k > 0 and len(body):
                    mutated[len(body) * (k - 1) // (len(verdicts) - 1)] ^= 0xFF
                verdict, _ = payload.world(struct.pack("<I", time) + bytes(mutated), game_mode, batteries, respawn)
                worlds += 1
                if verdict != wanted:
                    differ += 1
                    if differ <= 10:
                        failures.append(f"drift: {file} copy {k}: the payload says {verdict}, the host {wanted}")
        if not args.quiet:
            print(f"drift: {len(rows)} worlds, {worlds} checks with the mutated copies, {differ} differ")

    if args.counts:
        out = open(args.counts, "w")
        out.write("kind\tsource\tbytes\tverdict\tinstructions\n")
        counts = []
        for file, time, game_mode, batteries, respawn, verdicts in rows:
            state = state_of(file, time)
            verdict, ran = payload.world(state, game_mode, batteries, respawn, count=True)
            counts.append((verdict, ran))
            out.write(f"world\t{file}\t{len(state)}\t{verdict}\t{ran}\n")
        for verdict_wanted, label in ((0, "read"), (1, "refused")):
            ran = [r for v, r in counts if v == verdict_wanted]
            if ran and not args.quiet:
                print(f"cv_world_load, worlds {label}: n={len(ran)} instructions min {min(ran)} "
                      f"median {pct(ran, 50)} p99 {pct(ran, 99)} max {max(ran)}")
        if args.packets:
            by_id = {}
            for log, message_id, data in packet_messages(args.packets):
                verdict, ran = payload.message(message_id, data, count=True)
                by_id.setdefault(message_id, []).append(ran)
                out.write(f"message {message_id:02X}\t{log}\t{len(data)}\t{verdict}\t{ran}\n")
            if not args.quiet:
                for message_id, ran in sorted(by_id.items()):
                    print(f"cv_message {message_id:02X}: n={len(ran)} instructions min {min(ran)} "
                          f"median {pct(ran, 50)} max {max(ran)}")
        out.close()

    for failure in failures:
        print("FAIL", failure)
    raise SystemExit(1 if failures else 0)


if __name__ == "__main__":
    main()
