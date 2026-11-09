#!/usr/bin/env python3
"""Native receipt lifecycle: rejection, successful load, floods, numbered handshake refusals,
a full pool (the receipt waits for a frame with room) and teardown."""
import argparse
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
from check_message_check_026g import MessageRun, END
from check_world_check_026g import CLIENT, HOLDER, POOL, PACKET, FRAME, STUBS, state, body, player, sections, EPILOGUE, LOADED

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
    built,_=base.build(original,'section',open(base.default_bin('section'),'rb').read(),None,True)
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
    result=run.raw(valid)
    assert result[0]==LOADED and [call[0] for call in run.calls]==['load']
    assert len(run.receipts)==1 and run.receipts[0][12:14]==b'\x05\x00'
    result=run.raw(valid[:-1])
    assert result[0]==EPILOGUE and not run.calls and run.receipts[0][13]==1
    # Match close clears throttling even if the next match reuses addresses
    # and the clock has not advanced. We stop before the native teardown.
    run.advance=0
    run.emu.hook_del(run.close_stub)
    handle=run.emu.hook_add(UC_HOOK_CODE,run.stop,begin=0x27A796,end=0x27A796)
    run.start(0x27A790,{UC_X86_REG_ECX:CLIENT},FRAME-0x100)
    run.emu.hook_del(handle)
    run.message(0xe8,b'\x05\x02'+bytes(8))
    # Its line waits for the log's second (the log is the game's, not the
    # match's) and is counted in the next.
    assert len(run.receipts)==1 and not run.lines and not run.calls,(run.receipts,run.lines,run.calls)
    # A new match seed also changes ownership if a connection is replaced.
    run.word(HOLDER+0x577BC,0x81234568);run.ticks+=1001
    run.message(0xe8,b'\x05\x02'+bytes(8))
    assert len(run.lines)==1 and b'; 1 more refused since the last line' in run.lines[0],run.lines
    assert run.receipts[0][8:12]==bytes.fromhex('68452381')
    print('Native recovery lifecycle and 2,000-message flood passed; no victim close, bounded logs and allocations.')
if __name__=='__main__':main()
