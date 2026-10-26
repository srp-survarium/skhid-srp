//! Patch 30: one upgrade on several items; taking it off names the item it
//! comes off (client_patch.md section 3.28).
//!
//! The lobby's `46 02 <kit> <upgrade>` named no item, and taking an upgrade
//! off took it off the first item holding it. Here the upgrade window's
//! drop sends `46 02 <kit> <upgrade> <item>`, the server's answer `83 46 02
//! <upgrade> <item>` takes it off that item, and `83 46 01`'s detach_first
//! takes it off the item the answer puts it on; process_detach_upgrade
//! looks for the item the caller named (an id of 0: the original's
//! search).

#![forbid(unsafe_code)]

use crate::vostok::upgrades::{DetachAnswer, DetachFirst, DropSent, Holders};
use crate::vostok::{Context, Flow, GameCell, NoExit, Reg, Thread};

/// The item process_detach_upgrade takes the upgrade off: the one its
/// caller names while the call runs (0: none, the original's search).
static HOST: GameCell<u32> = GameCell::new(0);

/// `46 02`: the drop's sub-message.
const DROP: u8 = 2;

/// `lobby_upgrade.drop_sent`: `46 02 <kit> <upgrade> <item>` instead of the
/// same without the item.
pub fn on_drop_sent(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(drop) = DropSent::at(ctx) {
        let mut bytes = [0u8; 13];
        bytes[0] = DROP;
        bytes[1..5].copy_from_slice(&drop.kit.to_le_bytes());
        bytes[5..9].copy_from_slice(&drop.upgrade.to_le_bytes());
        bytes[9..13].copy_from_slice(&drop.item.to_le_bytes());
        drop.client.send(&bytes);
    }
    Flow::Skip
}

/// `inventory_action.detach_answer`: `83 46 02 <upgrade> <item>`.
pub fn on_detach_answer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(answer) = DetachAnswer::at(ctx) else { return Flow::Continue };
    let [upgrade, item] = answer.read_dwords::<2>();
    let client = answer.client;
    let result = with_host(ctx.thread(), item, || client.process_detach_upgrade(upgrade));
    ctx.set_reg(Reg::Eax, result);
    Flow::Skip
}

/// `inventory_action.detach_first`: the upgrade off the item the answer
/// puts it on.
pub fn on_detach_first(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(first) = DetachFirst::at(ctx) else { return Flow::Continue };
    let result = with_host(ctx.thread(), first.item, || first.client.process_detach_upgrade(first.upgrade));
    ctx.set_reg(Reg::Eax, result);
    Flow::Skip
}

/// `call` with HOST the named item for its time.
fn with_host(thread: &Thread, host: u32, call: impl FnOnce() -> u32) -> u32 {
    HOST.set(thread, host);
    let result = call();
    HOST.set(thread, 0);
    result
}

/// `detach_upgrade.find_holder`: the named item (or the end), or with none
/// named the original's search.
pub fn on_find_holder(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let host = HOST.get(ctx.thread());
    if host == 0 {
        return Flow::Continue;
    }
    let holders = Holders::at(ctx);
    let found = holders.find(host);
    ctx.set_reg(Reg::Eax, found);
    ctx.set_reg(Reg::Ecx, holders.end());
    ctx.set_reg(Reg::Edx, host);
    Flow::Skip
}
