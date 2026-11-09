#!/usr/bin/env python3
"""Native receipt lifecycle: rejection, successful load, floods, numbered handshake refusals,
a full pool (the receipt waits for a frame with room) and teardown. Patch 126's memory
report at the first world and at the close: its carriers (the census, the client, the
largest allocations outside the arenas, named) and its log lines."""
import argparse
import struct
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
from check_message_check_026g import MessageRun, END
from check_world_check_026g import CLIENT, HOLDER, POOL, PACKET, FRAME, STUBS, state, body, player, sections, EPILOGUE, LOADED, \
    MAPPED, REPORT_MAGIC

D3D11_SLOT, D3D11 = 0xAF30D0, 0x6A000000     # the delayed import's module slot; a module there (DXVK's: no layered exports)
D3D11_PATH = b"C:\\Game\\game\\binaries\\x86\\dxvk\\d3d11.dll"


def report_entries(carriers, match_id, moment):
    """A report's entries from its carriers (srp_validate::report and
    ::memory): (kind, body) each, the parts in order and all there."""
    entries, parts = [], None
    for part, carrier in enumerate(carriers):
        assert carrier[:8] == REPORT_MAGIC and int.from_bytes(carrier[8:12], "little") == match_id, carrier[:16].hex()
        kind, at, count = carrier[12:15]
        assert kind == 1 and at == part and (parts is None or parts == count), (kind, at, count)
        parts = count
        body = carrier[15:]
        assert body[0] == moment and len(body) == 2 + 46 * body[1], (body[:2], len(body))
        entries += [(body[2 + 46 * i], body[3 + 46 * i:2 + 46 * (i + 1)]) for i in range(body[1])]
    assert parts == len(carriers), (parts, len(carriers))
    return entries


def check_report(run, moment):
    """The memory report the world's load (moment 0) or the close (1) sent and logged."""
    entries = report_entries(run.reports, 0x81234567, moment)
    kinds = [kind for kind, _ in entries]
    # the process; no arena (the harness registered none); the census; the client; the allocations
    assert kinds[:3] == [0, 2, 4] and set(kinds[3:]) == {3} and len(kinds) <= 3 + 20, kinds
    census = struct.unpack_from("<9I", entries[1][1])
    image, mapped, private = census[0:3], census[3:6], census[6:9]
    assert image[2] == 1 and mapped == (0x400000, 0x400000, 1) and private[2] == 9, census
    assert entries[2][1][:2] == bytes([0, 4]), entries[2][1][:2].hex()  # Windows; DXVK from a dxvk folder
    allocations = [(struct.unpack_from("<IBII", body), body[13:45].rstrip(b"\0")) for _, body in entries[3:]]
    sizes = [a[0][2] for a in allocations]
    assert sizes == sorted(sizes, reverse=True), sizes
    named = {a[0][0]: (a[0][1], a[1]) for a in allocations}
    assert named[0x10000] == (0, b"survarium_skhid.exe") and named[MAPPED] == (1, b"resources.db"), named
    assert all(name == b"" for (base, kind, _, _), name in allocations if kind == 2), allocations
    text = b"".join(run.lines)
    for line in (b"memory census: image ", b"mapped 4194304/4194304 in 1", b"memory client: windows, d3d11 dxvk (dxvk folder), baked kept 0 of 0, listed 0 of 0",
                 b'memory allocation 0x00010000 image "survarium_skhid.exe" size ', b'image "survarium_skhid.exe"',
                 b'memory allocation 0x46000000 mapped "resources.db" size 4194304, committed 4194304'):
        assert line in text, (line, text)
    return len(run.reports)

GAME_TICK, AFTER_TICK = 0x1EC8EC, 0x1EC8F2   # the game.tick site, the instruction after its six bytes
TICKING = 0x45000000                         # a game whose network client (+53ACh) is CLIENT

def frame(run):
    """One frame: game.tick on a game of CLIENT. The receipts it sent, as
    their (generation, outcome) bytes."""
    if not getattr(run, "ticking", False):
        run.emu.mem_map(TICKING, 0x10000, UC_PROT_ALL); run.word(TICKING+0x53AC, CLIENT)
        run.emu.hook_add(UC_HOOK_CODE, run.stop, begin=AFTER_TICK, end=AFTER_TICK); run.ticking = True
    at,_,_ = run.start(GAME_TICK, {UC_X86_REG_EBX: TICKING}, FRAME-0x100)
    assert at==AFTER_TICK and not run.calls, (hex(at or 0), run.calls)
    return [receipt[12:14] for receipt in run.receipts]

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('exe');args=parser.parse_args()
    original=open(args.exe,'rb').read()
    built,_=base.build(original,'section',open(base.default_bin('section'),'rb').read(),None,open(base.late_join_bin(),'rb').read())
    run=MessageRun(bytes(built));run.advance=0;run.tick_step=0;run.ticks=100
    bad=(0xeb,b'\0'*6+b'\0\0\0\x40')
    logs=0
    for _ in range(1000):
        at,_,_=run.message(*bad)
        assert at==END and not run.calls and not run.receipts
        logs+=len(run.lines)
    assert logs==1,logs
    # A valid input after the flood still reaches native dispatch.
    at,_,_=run.message(0xeb,bytes(10));assert at!=END and not run.calls
    # A flood of one corrupt world allocates one packet: its refusal is told once.
    sent=0
    for _ in range(1000):
        at,_,_=run.message(0xe8,b'\x04\x02'+bytes(8))
        assert at==END and not run.calls
        sent+=len(run.receipts)
    assert sent==1,sent
    # The next generation's refusal is told at once, however soon: the server
    # waits for it (no time may drop it), and only once.
    run.message(0xe8,b'\x05\x02'+bytes(8))
    assert len(run.receipts)==1 and run.receipts[0][12:14]==b'\x05\x01'
    run.ticks+=1001
    run.message(0xe8,b'\x05\x02'+bytes(8))
    assert not run.receipts
    # The floods' refusals that got no line are counted in the next one.
    assert len(run.lines)==1 and b'; 2000 more refused since the last line' in run.lines[0],run.lines
    # Each refused handshake message is told, the second at once too, with
    # its number on the connection: the server tells a new refusal from a
    # copy of one (0xE0 is plain and resent until acked) by it.
    # The numbers go on across the seed's change mid-connection (the native
    # E3 reader writes it, 0x1F0E7A, once one is taken): a count started
    # again would send 1 twice, and the server take the second for a copy.
    run.word(CLIENT+0x1F1C64,0)
    for number,seed in ((1,0x81234567),(2,0x1234ABCD)):
        run.word(HOLDER+0x577BC,seed)
        run.message(0xe4,b"\0\x01")
        assert len(run.receipts)==1 and run.receipts[0][12:14]==bytes([number,2]) and not run.calls,run.receipts
        assert run.receipts[0][8:12]==seed.to_bytes(4,"little")
    run.word(HOLDER+0x577BC,0x81234567)
    run.word(CLIENT+0x1F1C64,2)
    # A world whose generation cannot be read (its reader past its buffer, or
    # no byte) is refused by number, counted with the handshake's: the
    # generation stored is an older world's, which the server would take as
    # stale. Copies of it are told apart by the number; each goes.
    for number,payload,invalid in ((3,b"\x05\x02"+bytes(8),True),(4,b"",False),(5,b"",False)):
        at,_,_=run.message(0xe8,payload,invalid_cursor=invalid)
        assert at==END and not run.calls and len(run.receipts)==1 and run.receipts[0][12:14]==bytes([number,3]),(number,run.receipts)
    # A refused late-join proposal (0xE4 to a game in play) is no handshake.
    at,_,_=run.message(0xe4,b'\xffLJ\x01'+bytes(4))
    assert at==END and not run.receipts and not run.calls
    # No allocator call when the pool lacks the same reserve as send_prepared.
    run.ticks+=1001;run.word(POOL+0x30,99)
    before=run.packet_allocations
    run.message(0xe8,b"\x06\x02"+bytes(8))
    assert not run.receipts and run.packet_allocations==before and not run.calls
    # It waits, and goes on the first frame with room, once. A numbered
    # refusal meanwhile (here a handshake's, number 6) waits beside it, and
    # never in its place: the world's receipt is still due.
    assert not frame(run) and run.packet_allocations==before
    run.word(CLIENT+0x1F1C64,0)
    run.message(0xe4,b"\0\x01")
    assert not run.receipts and run.packet_allocations==before
    run.word(CLIENT+0x1F1C64,2)
    run.word(POOL+0x30,0)
    sent=frame(run); assert sent==[b"\x06\x01",bytes([6,2])] and run.packet_allocations==before+2,(sent,run.packet_allocations,before,run.lines)
    assert not frame(run)
    # Complete native deserialize is required for the Loaded receipt. The
    # hook runs after the load; rejected bytes never invoke native deserialize.
    run.ticks+=1001;run.advance=2000
    valid=state(body([],sections()))
    run.word(D3D11_SLOT,D3D11);run.modules[D3D11]=D3D11_PATH
    result=run.raw(valid)
    assert result[0]==LOADED and [call[0] for call in run.calls]==['load']
    assert len(run.receipts)==1 and run.receipts[0][12:14]==b'\x05\x00'
    # The connection's first world: patch 126's report, sent and logged.
    loaded_parts=check_report(run,0)
    assert (D3D11,b"D3D11CoreRegisterLayers") in run.asked,run.asked
    result=run.raw(valid[:-1])
    assert result[0]==EPILOGUE and not run.calls and run.receipts[0][13]==1
    # A later world of the same connection reports nothing.
    assert not run.reports
    # Match close clears throttling even if the next match reuses addresses
    # and the clock has not advanced. We stop before the native teardown.
    run.advance=0
    run.emu.hook_del(run.close_stub)
    handle=run.emu.hook_add(UC_HOOK_CODE,run.stop,begin=0x27A796,end=0x27A796)
    run.start(0x27A790,{UC_X86_REG_ECX:CLIENT},FRAME-0x100)
    run.emu.hook_del(handle)
    # The close: the ended report, the same entries.
    assert check_report(run,1)==loaded_parts
    run.message(0xe8,b'\x05\x02'+bytes(8))
    # Its line waits for the log's second (the log is the game's, not the
    # match's) and is counted in the next.
    assert len(run.receipts)==1 and not run.lines and not run.calls,(run.receipts,run.lines,run.calls)
    # A new match seed also changes ownership if a connection is replaced.
    run.word(HOLDER+0x577BC,0x81234568);run.ticks+=1001
    run.message(0xe8,b'\x05\x02'+bytes(8))
    assert len(run.lines)==1 and b'; 1 more refused since the last line' in run.lines[0],run.lines
    assert run.receipts[0][8:12]==bytes.fromhex('68452381')
    print('Native recovery lifecycle and 2,000-message flood passed; no victim close, bounded logs and allocations; '
          f'the memory report sent at the first world and the close ({loaded_parts} parts each).')
if __name__=='__main__':main()
