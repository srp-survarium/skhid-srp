#!/usr/bin/env python3
"""Native receipt lifecycle: rejection, successful load, floods, full pool and teardown."""
import argparse
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as base
from check_message_check_026g import MessageRun, END
from check_world_check_026g import CLIENT, HOLDER, POOL, PACKET, FRAME, STUBS, state, body, player, sections, EPILOGUE, LOADED

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
    # Each refused handshake message is told, the second at once too.
    run.word(CLIENT+0x1F1C64,0)
    for _ in range(2):
        run.message(0xe4,b'\0\x01')
        assert len(run.receipts)==1 and run.receipts[0][13]==2 and not run.calls
    run.word(CLIENT+0x1F1C64,2)
    # A refused late-join proposal (0xE4 to a game in play) is no handshake.
    at,_,_=run.message(0xe4,b'\xffLJ\x01'+bytes(4))
    assert at==END and not run.receipts and not run.calls
    # No allocator call when the pool lacks the same reserve as send_prepared.
    run.ticks+=1001;run.word(POOL+0x30,99)
    before=run.packet_allocations
    run.message(0xe8,b'\x06\x02'+bytes(8))
    assert not run.receipts and run.packet_allocations==before and not run.calls
    run.word(POOL+0x30,0)
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
