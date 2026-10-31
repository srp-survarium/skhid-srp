//! Patch 83, dropped-weapon pickup (docs/notes/weapon_pickup.md), stage 1:
//! the instance. At a death on every client (the same tick everywhere),
//! when the dead player held one of its two guns, that gun alone is cooked
//! (`vostok::gun_cook`), kept by donor life (its place and spawn time), and
//! given back when the window ends untaken, when the later stages give it
//! back (its holder died or left), or when its donor player is destroyed
//! (it leaves; every player at the match's teardown).
//!
//! The cook takes milliseconds and walking to the body seconds, so a pickup
//! finds it made. When it does not, [`wait`] runs the game's
//! `dispatch_callbacks` until it is (query_resources_and_wait's loop,
//! 0xE5C20): a rare hitch. The one callback that must not run inside a tick
//! is a late joiner's completion (its install changes the roster, the saved
//! states and the animation tables, "between simulation updates"): it can
//! only follow a player cook's completion (`player_cook.ready`), so a wait
//! refuses while a player was made since the last move.
//!
//! A loaded world (a late join, a resync) names its borrowed guns in its
//! records: [`need`] cooks one (or finds it) and waits the same way, at the
//! record's reader, outside any tick.
//!
//! Every cook is logged to `srp_patches.log` (with the process id): asked,
//! made or failed with its death-to-done ms and the address space before and
//! after, any wait and its ms.
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use weapon_pickup::{Kit, Ms, Place, WeaponSlot, WINDOW_MS};

use crate::vostok::gun_cook::{self, CookId, Cooked, Gun, InventoryId, Refusal, TICKETS};
use crate::vostok::log::{line, Piece::{self, Dec, Hex, Text}};
use crate::vostok::world::{GameWorld, Player, PlayerId};
use crate::vostok::{clock, Context, Flow, Lock, NoExit, Thread};

/// After the window, how long a death's gun is still kept: a late input can
/// roll the simulation back into the window (and a pickup in it simulated
/// again) for about this long.
const MARGIN_MS: u32 = 3_000;
/// The longest a wait runs the callbacks before it gives up (the pickup is
/// then refused, logged).
const WAIT_CAP_MS: u32 = 2_000;

/// Where an instance's gun is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A donor's death: its life (spawn time, player+328h) and the death's
    /// simulation time.
    Death { spawned: u32, died: Ms },
    /// Named by a loaded world's record (a late join, a resync).
    Loaded,
}

/// The gun: its donor's place, kit and weapon slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    pub place: Place,
    pub kit: Kit,
    pub slot: WeaponSlot,
}

/// What the log knows of an entry's cook.
#[derive(Clone, Copy)]
enum Report {
    /// Asked; the address space then (MiB).
    Asked { memory: Option<u32> },
    /// Its outcome logged.
    Logged,
}

struct Entry {
    source: Source,
    origin: Origin,
    donor: PlayerId,
    gun: Gun,
    report: Report,
}

static CACHE: Lock<[Option<Entry>; TICKETS]> = Lock::new([const { None }; TICKETS]);

/// A player was made (a player cook's completion) since the last move: a
/// late joiner's install may be among the next callbacks.
static PLAYER_MADE: AtomicBool = AtomicBool::new(false);

/// An instance as the later stages see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instance {
    Cooking,
    Ready(InventoryId),
    Failed,
    /// No cook for it (no such death, or given back).
    Missing,
}

/// Why a wait gave no gun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Missing,
    Failed,
    /// A player was made since the last move: a late join's install may be
    /// due, which must not run here.
    LateJoin,
    /// Not made within [`WAIT_CAP_MS`].
    Timeout,
    /// The cook could not be asked for.
    NotAsked(Refusal),
}

impl Refused {
    fn text(self) -> &'static str {
        match self {
            Refused::Missing => "no cook for it",
            Refused::Failed => "its cook failed",
            Refused::LateJoin => "a player was made since the last move (a late join's install may be due)",
            Refused::Timeout => "not made in time",
            Refused::NotAsked(refusal) => refusal.text(),
        }
    }
}

/// The death of a player (composed from artefact_carry's `carry.client_death`,
/// the player still intact): its gun's cook asked for, unless this life's
/// is (a rollback replaying the death).
pub fn died(player: &Player<'_>, thread: &Thread) {
    let Some(slot) = gun_cook::held_gun(player) else { return };
    let (Some(place), Some(kit)) = (Place::new(player.place()), player.kit()) else { return };
    let spawned = player.spawned();
    let origin = Origin::Death { spawned, died: Ms(player.time()) };
    let known = CACHE.with(thread, |cache| {
        cache.iter().flatten().any(|e| e.source.place == place && matches!(e.origin, Origin::Death { spawned: s, .. } if s == spawned))
    });
    if known {
        return;
    }
    ask(player, thread, Source { place, kit, slot }, origin);
}

/// Asks for `source`'s gun from `player`'s profile and keeps it in the cache.
fn ask(player: &Player<'_>, thread: &Thread, source: Source, origin: Origin) -> Option<CookId> {
    let memory = gun_cook::memory(thread).map(|(used, _)| used);
    let gun = match gun_cook::start(player, source.kit, source.slot) {
        Ok(gun) => gun,
        Err(refusal) => {
            log_source("FAILED to ask for", source, origin, &[Text(": "), Text(refusal.text())]);
            return None;
        }
    };
    let id = gun.id();
    let entry = Entry { source, origin, donor: player.id(), gun, report: Report::Asked { memory } };
    let leftover = CACHE.with(thread, |cache| match cache.iter_mut().find(|e| e.is_none()) {
        Some(free) => {
            *free = Some(entry);
            None
        }
        None => Some(entry),
    });
    let kept = leftover.is_none();
    // No room (a ticket each, so never): given back outside the lock.
    drop(leftover);
    log_source(if kept { "asked for" } else { "no room for" }, source, origin, &memory_pieces(memory));
    kept.then_some(id)
}

/// The instance for the death of `place`'s life `spawned` (ready or not).
pub fn instance(thread: &Thread, place: Place, spawned: u32) -> Instance {
    match death_cook(thread, place, spawned) {
        None => Instance::Missing,
        Some((id, _)) => match gun_cook::cooked(thread, id) {
            Cooked::Cooking { .. } => Instance::Cooking,
            Cooked::Made { inventory, .. } => Instance::Ready(inventory),
            Cooked::Failed { .. } => Instance::Failed,
            Cooked::Gone => Instance::Missing,
        },
    }
}

fn death_cook(thread: &Thread, place: Place, spawned: u32) -> Option<(CookId, Source)> {
    CACHE.with(thread, |cache| {
        cache.iter().flatten()
            .find(|e| e.source.place == place && matches!(e.origin, Origin::Death { spawned: s, .. } if s == spawned))
            .map(|e| (e.gun.id(), e.source))
    })
}

/// The gun of `place`'s death in life `spawned` at a pickup's tick: made,
/// or waited for (the game's callbacks run until it is).
pub fn wait(thread: &Thread, place: Place, spawned: u32) -> Result<InventoryId, Refused> {
    let (id, source) = death_cook(thread, place, spawned).ok_or(Refused::Missing)?;
    wait_for(thread, id, source)
}

/// The gun a loaded world's record names (its donor `donor`, a player of the
/// donor's place, and the kit and slot): one already cooked of that source,
/// or a cook asked for now; waited for. Called at the record's reader,
/// before the world applies, outside any tick.
pub fn need(thread: &Thread, donor: &Player<'_>, kit: Kit, slot: WeaponSlot) -> Result<InventoryId, Refused> {
    let place = Place::new(donor.place()).ok_or(Refused::Missing)?;
    let source = Source { place, kit, slot };
    let known = CACHE.with(thread, |cache| cache.iter().flatten().find(|e| e.source == source).map(|e| e.gun.id()));
    let id = match known {
        Some(id) => id,
        None => ask(donor, thread, source, Origin::Loaded).ok_or(Refused::NotAsked(Refusal::NoTicket))?,
    };
    wait_for(thread, id, source)
}

fn wait_for(thread: &Thread, id: CookId, source: Source) -> Result<InventoryId, Refused> {
    let start = clock::counter(thread);
    let mut waited = false;
    let outcome = loop {
        match gun_cook::cooked(thread, id) {
            Cooked::Made { inventory, .. } => break Ok(inventory),
            Cooked::Failed { .. } => break Err(Refused::Failed),
            Cooked::Gone => break Err(Refused::Missing),
            Cooked::Cooking { .. } => {}
        }
        if PLAYER_MADE.load(Ordering::Acquire) {
            break Err(Refused::LateJoin);
        }
        if elapsed_ms(thread, start, clock::counter(thread)) >= WAIT_CAP_MS {
            break Err(Refused::Timeout);
        }
        waited = true;
        // No lock of ours is held: the callbacks reach our hooks.
        gun_cook::dispatch_callbacks(thread);
    };
    if waited || outcome.is_err() {
        let ms = elapsed_ms(thread, start, clock::counter(thread));
        let what = match outcome {
            Ok(_) => "made",
            Err(refused) => refused.text(),
        };
        line(&[Text("pid "), Dec(gun_cook::process_id()), Text(" pickup: waited "), Dec(ms),
                     Text(" ms for the gun of place "), Dec(u32::from(source.place.get())), Text(" kit "),
                     Dec(u32::from(source.kit.get())), Text(" slot "), Dec(u32::from(source.slot.slot())),
                     Text(": "), Text(what)]);
    }
    outcome
}

/// The later stages give a gun back (its holder died or left): every
/// instance of that source is freed.
pub fn give_back(thread: &Thread, source: Source) {
    release_where(thread, |e| e.source == source);
}

fn release_where(thread: &Thread, mut gone: impl FnMut(&Entry) -> bool) {
    loop {
        // One at a time, each dropped outside the lock (the cooked
        // inventory's own teardown).
        let taken = CACHE.with(thread, |cache| cache.iter_mut().find(|e| e.as_ref().is_some_and(&mut gone))?.take());
        match taken {
            Some(entry) => drop(entry),
            None => break,
        }
    }
}

/// Once a frame after the simulation's move (composed from ragdoll's
/// `game_world.after_move`): a late join's install has had its frame's
/// callbacks; finished cooks are logged; a death's gun past its window and
/// margin is freed.
pub fn after_move(ctx: &Context<'_>) {
    let thread = ctx.thread();
    PLAYER_MADE.store(false, Ordering::Release);
    log_finished(thread);
    let Some(now) = GameWorld::at_after_move(ctx).as_ref().and_then(gun_cook::world_time) else { return };
    release_where(thread, |e| match e.origin {
        Origin::Death { died, .. } => Ms(now).since(died) >= WINDOW_MS + MARGIN_MS && now >= died.0,
        Origin::Loaded => false,
    });
}

fn log_finished(thread: &Thread) {
    loop {
        let found = CACHE.with(thread, |cache| {
            cache.iter_mut().flatten().find_map(|e| {
                let Report::Asked { memory } = e.report else { return None };
                let cooked = gun_cook::cooked(thread, e.gun.id());
                if matches!(cooked, Cooked::Cooking { .. }) {
                    return None;
                }
                e.report = Report::Logged;
                Some((e.source, e.origin, memory, cooked))
            })
        });
        let Some((source, origin, before, cooked)) = found else { break };
        let after = gun_cook::memory(thread).map(|(used, _)| used);
        let (what, inventory, ms) = match cooked {
            Cooked::Made { inventory, since, at } => ("made", inventory.0, elapsed_ms(thread, since, at)),
            Cooked::Failed { since, at } => ("FAILED (empty result)", 0, elapsed_ms(thread, since, at)),
            Cooked::Cooking { .. } | Cooked::Gone => ("given back before it was made", 0, 0),
        };
        log_source("cook of", source, origin, &[
            Text(": "), Text(what), Text(", inventory "), Hex(inventory), Text(", "), Dec(ms),
            Text(" ms after it was asked for, address space "), Dec(before.unwrap_or(0)), Text(" -> "),
            Dec(after.unwrap_or(0)),
            Text(" MiB")]);
    }
}

/// A player is being made (a player cook's batch is in): a late joiner's
/// install may follow in the next callbacks.
pub fn on_player_made(_: &mut Context<'_>) -> Flow<NoExit> {
    PLAYER_MADE.store(true, Ordering::Release);
    Flow::Continue
}

/// A player is destroyed (it leaves; every player at the match's teardown):
/// the guns cooked from it go.
pub fn on_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(donor) = gun_cook::destroyed(ctx) {
        release_where(ctx.thread(), |e| e.donor == donor);
    }
    Flow::Continue
}

fn elapsed_ms(thread: &Thread, since: u64, now: u64) -> u32 {
    let frequency = clock::frequency(thread);
    if frequency == 0 {
        return 0;
    }
    (now.wrapping_sub(since) as f32 * 1000.0 / frequency as f32) as u32
}

fn memory_pieces(memory: Option<u32>) -> [Piece<'static>; 3] {
    [Text(": address space "), Dec(memory.unwrap_or(0)), Text(" MiB")]
}

fn log_source(what: &str, source: Source, origin: Origin, rest: &[Piece<'_>]) {
    let mut pieces = [Text(""); 28];
    let head = [Text("pid "), Dec(gun_cook::process_id()), Text(" pickup: "), Text(what), Text(" the gun of place "),
                Dec(u32::from(source.place.get())), Text(" kit "), Dec(u32::from(source.kit.get())), Text(" slot "),
                Dec(u32::from(source.slot.slot()))];
    let (died, n) = match origin {
        Origin::Death { died, .. } => ([Text(" (death at "), Dec(died.0), Text(" ms)")], 3),
        Origin::Loaded => ([Text(" (a loaded world's)"), Text(""), Text("")], 1),
    };
    let mut at = 0;
    for piece in head.iter().chain(&died[..n]).chain(rest) {
        if let Some(slot) = pieces.get_mut(at) {
            *slot = *piece;
            at += 1;
        }
    }
    line(&pieces[..at]);
}
