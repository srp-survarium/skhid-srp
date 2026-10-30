//! Patch 83, the spike of dropped-weapon pickup (docs/notes/weapon_pickup.md):
//! while a match's kit players load (the match's start, a late joiner's
//! kits), each kit player's cook also cooks a spare inventory of that kit's
//! weapons and their ammunition (`vostok::spare`). The player, and so the
//! game's Ready and the late join's ack, come only once the spare is in. A
//! spare is kept until its kit player is destroyed. Nothing else uses it yet.
//!
//! Every spare is logged to `srp_patches.log`: the place and kit, how long
//! the player's batch took with it, the process's address space before and
//! after, and why one could not be made (it is then skipped: the player is
//! made as before). `SRP_SPARE_WEAPONS=off` makes none and logs the same
//! timings and memory, for a comparison.
#![forbid(unsafe_code)]

use crate::vostok::log::{line, Piece::{Dec, Hex, Text}};
use crate::vostok::spare::{self, Cook, Kit, KitId, Pending, Refusal, Slot, Spare, StageId};
use crate::vostok::{clock, Context, Flow, Lock, NoExit, SpareWeaponsOnQueryExit, Thread};

/// The slots a spare keeps: the two weapons (7, 10) and the eight ammunition
/// slots (8, 9, 11, 12, 19..22); every other slot of its kit is emptied.
const KEEP: [Slot; 10] = {
    let slots = [7, 10, 8, 9, 11, 12, 19, 20, 21, 22];
    let mut out = [match Slot::new(0) { Some(s) => s, None => panic!() }; 10];
    let mut n = 0;
    while n < 10 {
        // A constant: a slot past 22 fails the build, not the game.
        out[n] = match Slot::new(slots[n]) { Some(s) => s, None => panic!() };
        n += 1;
    }
    out
};

/// Kit players cooking or holding a spare at once: 20 places of 3 kits, and
/// room for the kits of players who left while their cook ran.
const ENTRIES: usize = 96;

/// Whether spares are made (read once, from SRP_SPARE_WEAPONS).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Switch {
    Unread,
    On,
    Off,
}

/// Whom a cook or a spare is for.
#[derive(Clone, Copy)]
struct Who {
    place: u8,
    kit: Kit,
}

/// The machine's clock and memory at a moment.
#[derive(Clone, Copy)]
struct Moment {
    ticks: u64,
    memory: Option<(u32, u32)>,
}

impl Moment {
    fn now(thread: &Thread) -> Moment {
        Moment { ticks: clock::counter(thread), memory: spare::memory(thread) }
    }
}

/// An entry of the bank: free, a kit player's cook running (with the spare
/// it asks for, or none: off, or refused), or a kit player's spare held.
enum Entry {
    Free,
    Cooking { stage: StageId, kit_id: KitId, who: Who, since: Moment, spare: Option<Pending> },
    Held { kit_id: KitId, who: Who, spare: Spare },
}

struct Bank {
    switch: Switch,
    entries: [Entry; ENTRIES],
    /// Cooks running, and what they made since the first of a burst.
    cooking: u32,
    made: u32,
    failed: u32,
    first: Option<Moment>,
}

static BANK: Lock<Bank> = Lock::new(Bank {
    switch: Switch::Unread,
    entries: [const { Entry::Free }; ENTRIES],
    cooking: 0,
    made: 0,
    failed: 0,
    first: None,
});

/// The player cook's subresource query: note the cook (and its moment), and
/// when on, ask for the spare in the same batch.
pub fn on_query(ctx: &mut Context<'_>) -> Flow<SpareWeaponsOnQueryExit> {
    let Some(cook) = spare::cook(ctx) else { return Flow::Continue };
    if cook.lobby {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    let since = Moment::now(thread);
    let switch = BANK.with(thread, |bank| {
        if bank.switch == Switch::Unread {
            let off = spare::environment_is(thread, c"SRP_SPARE_WEAPONS", b"off");
            bank.switch = if off { Switch::Off } else { Switch::On };
        }
        bank.switch
    });
    let (spare, submission, refused) = match switch {
        Switch::On => match spare::prepare(ctx, &KEEP) {
            Ok((pending, submission)) => (Some(pending), Some(submission), None),
            Err(refusal) => (None, None, Some(refusal)),
        },
        Switch::Off | Switch::Unread => (None, None, None),
    };
    let who = Who { place: cook.place, kit: cook.kit };
    let noted = BANK.with(ctx.thread(), |bank| note(bank, cook, who, since, spare));
    match noted {
        Noted::Full(spare) => {
            // Dropped outside the lock: the allocations go back.
            drop(spare);
            log_who("spares: no room for", who, &[Text(": the player is made without a spare")]);
            return Flow::Continue;
        }
        Noted::First => log_switch(switch, since),
        Noted::Later => {}
    }
    if let Some(refusal) = refused {
        log_refusal(who, refusal, None);
    }
    match submission {
        Some(submission) => {
            log_who("spares: asked for", who, &memory_pieces(since.memory));
            spare::submit(ctx, submission);
            Flow::Exit(SpareWeaponsOnQueryExit::Queried)
        }
        None => Flow::Continue,
    }
}

enum Noted {
    First,
    Later,
    /// No free entry: the spare (if any) is given back, nothing is asked.
    Full(Option<Pending>),
}

fn note(bank: &mut Bank, cook: Cook, who: Who, since: Moment, spare: Option<Pending>) -> Noted {
    let Some(entry) = bank.entries.iter_mut().find(|e| matches!(e, Entry::Free)) else {
        return Noted::Full(spare);
    };
    *entry = Entry::Cooking { stage: cook.stage, kit_id: cook.kit_id, who, since, spare };
    bank.cooking += 1;
    if bank.first.is_none() {
        bank.first = Some(since);
        bank.made = 0;
        bank.failed = 0;
        Noted::First
    } else {
        Noted::Later
    }
}

/// A player cook's batch is in: keep its spare, log it, and the burst's sum
/// when it was the last running.
pub fn on_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(stage) = spare::ready(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let taken = BANK.with(thread, |bank| {
        let entry = bank.entries.iter_mut().find(|e| matches!(e, Entry::Cooking { stage: s, .. } if *s == stage))?;
        let Entry::Cooking { kit_id, who, since, spare, .. } = core::mem::replace(entry, Entry::Free) else {
            return None;
        };
        bank.cooking = bank.cooking.saturating_sub(1);
        Some((kit_id, who, since, spare))
    });
    let Some((kit_id, who, since, spare)) = taken else { return Flow::Continue };
    let now = Moment::now(thread);
    let ms = elapsed_ms(thread, since, now);
    let made = match spare {
        None => {
            log_who("spares: no spare for", who, &with_cook(ms, since, now));
            None
        }
        Some(pending) => match spare::take(ctx, pending) {
            Ok(spare) => Some(spare),
            Err(refusal) => {
                log_refusal(who, refusal, Some(ms));
                None
            }
        },
    };
    let (replaced, summary) = BANK.with(thread, |bank| {
        let replaced = match made {
            Some(spare) => {
                bank.made += 1;
                let address = spare.address();
                let old = bank.entries.iter_mut()
                    .find(|e| matches!(e, Entry::Held { kit_id: k, .. } if *k == kit_id))
                    .map(|e| core::mem::replace(e, Entry::Free));
                // The entry is handed out of the lock when there is no room
                // (another cook took this one's), and dropped there.
                let leftover = match bank.entries.iter_mut().find(|e| matches!(e, Entry::Free)) {
                    Some(free) => {
                        *free = Entry::Held { kit_id, who, spare };
                        None
                    }
                    None => Some(spare),
                };
                Some((address, old, leftover))
            }
            None => {
                bank.failed += u32::from(bank.switch == Switch::On);
                None
            }
        };
        let summary = (bank.cooking == 0).then(|| (bank.first.take(), bank.made, bank.failed));
        (replaced, summary)
    });
    if let Some((address, old, leftover)) = replaced {
        let mut pieces = [Text(""); 12];
        let n = fill(&mut pieces, &[Text(": made, inventory "), Hex(address)]);
        let n = n + fill(&mut pieces[n..], &with_cook(ms, since, now));
        log_who("spares:", who, &pieces[..n]);
        if let Some(Entry::Held { who: old_who, .. }) = old.as_ref() {
            log_who("spares: replaced the older spare of", *old_who, &[]);
        }
        drop(old);
        if leftover.is_some() {
            log_who("spares: no room to keep the spare of", who, &[Text(": released")]);
        }
        drop(leftover);
    }
    if let Some((Some(first), made, failed)) = summary {
        log_summary(thread, first, now, made, failed);
    }
    Flow::Continue
}

/// A kit player is destroyed: its spare goes with it.
pub fn on_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(kit_id) = spare::destroyed(ctx) else { return Flow::Continue };
    let held = BANK.with(ctx.thread(), |bank| {
        bank.entries.iter_mut()
            .find(|e| matches!(e, Entry::Held { kit_id: k, .. } if *k == kit_id))
            .map(|e| core::mem::replace(e, Entry::Free))
    });
    if let Some(Entry::Held { who, spare, .. }) = held {
        // Released outside the lock: the inventory's own teardown.
        drop(spare);
        log_who("spares: released the spare of", who, &[]);
    }
    Flow::Continue
}

fn elapsed_ms(thread: &Thread, since: Moment, now: Moment) -> u32 {
    let frequency = clock::frequency(thread);
    if frequency == 0 {
        return 0;
    }
    (now.ticks.wrapping_sub(since.ticks) as f32 * 1000.0 / frequency as f32) as u32
}

/// `: cook N ms, address space A -> B MiB, memory load L %`.
fn with_cook(ms: u32, since: Moment, now: Moment) -> [crate::vostok::log::Piece<'static>; 9] {
    let before = since.memory.map_or(0, |(used, _)| used);
    let (after, load) = now.memory.unwrap_or((0, 0));
    [Text(": cook "), Dec(ms), Text(" ms, address space "), Dec(before), Text(" -> "), Dec(after),
     Text(" MiB, memory load "), Dec(load), Text(" %")]
}

fn memory_pieces(memory: Option<(u32, u32)>) -> [crate::vostok::log::Piece<'static>; 5] {
    let (used, load) = memory.unwrap_or((0, 0));
    [Text(": address space "), Dec(used), Text(" MiB, memory load "), Dec(load), Text(" %")]
}

fn fill<'a>(out: &mut [crate::vostok::log::Piece<'a>], pieces: &[crate::vostok::log::Piece<'a>]) -> usize {
    let n = pieces.len().min(out.len());
    out[..n].copy_from_slice(&pieces[..n]);
    n
}

fn log_who(what: &str, who: Who, rest: &[crate::vostok::log::Piece]) {
    let mut pieces = [Text(""); 20];
    let n = fill(&mut pieces, &[Text(what), Text(" place "), Dec(u32::from(who.place)), Text(" kit "),
                                Dec(u32::from(who.kit.number()))]);
    let n = n + fill(&mut pieces[n..], rest);
    line(&pieces[..n]);
}

fn log_refusal(who: Who, refusal: Refusal, ms: Option<u32>) {
    let value = match refusal {
        Refusal::Requests(n) => Some(n),
        Refusal::Count { got, .. } => Some(got),
        _ => None,
    };
    let pieces = [Text(": FAILED, "), Text(refusal.text()), Text(" ("), Dec(value.unwrap_or(0)),
                  Text("); after "), Dec(ms.unwrap_or(0)), Text(" ms; skipped, the player is made without it")];
    log_who("spares:", who, &pieces);
}

fn log_switch(switch: Switch, since: Moment) {
    let state = if switch == Switch::Off { "spares: off (SRP_SPARE_WEAPONS=off): measuring only" } else { "spares: on" };
    let mut pieces = [Text(""); 6];
    let n = fill(&mut pieces, &[Text(state)]);
    let n = n + fill(&mut pieces[n..], &memory_pieces(since.memory));
    line(&pieces[..n]);
}

fn log_summary(thread: &Thread, first: Moment, now: Moment, made: u32, failed: u32) {
    let ms = elapsed_ms(thread, first, now);
    let (before, after) = match (first.memory, now.memory) {
        (Some((b, _)), Some((a, _))) => (b, a),
        _ => (0, 0),
    };
    line(&[Text("spares: all cooks in: "), Dec(made), Text(" spares made, "), Dec(failed),
           Text(" failed, in "), Dec(ms), Text(" ms; address space "), Dec(before), Text(" -> "), Dec(after),
           Text(" MiB (+"), Dec(after.saturating_sub(before)), Text(")")]);
}
