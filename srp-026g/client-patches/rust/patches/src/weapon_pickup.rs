//! Patch 83, dropped-weapon pickup (docs/notes/weapon_pickup.md).
//!
//! Stage 1, the instance: at a death on every client (the same tick
//! everywhere), when the dead player held one of its two own guns, that gun
//! alone is cooked (`vostok::gun_cook`), kept by donor life, and given back
//! when the window ends untaken, when its holder gives it back (it died or
//! left), or when its donor player is destroyed and nobody holds it.
//!
//! Stage 2a, what a match computes (the same on every client, in replays,
//! rollbacks and late joins):
//! - the record: each player's `weapon_pickup::Record` (its drop, the gun it
//!   borrows) at its state's offset 2 (`pickup.save`, `pickup.load`); kept
//!   by place of the match ([`RECORDS`]); a record that does not decode is
//!   logged and read as empty;
//! - the pickup, at the input's apply point (artefact_carry's `carry.filter`
//!   calls [`on_filter`] with the raw actions first): the command, the rule
//!   (`weapon_pickup::decide`), the gun (made, or waited for), the install
//!   (`vostok::pickup`); the picker's record borrows, the donor's drop is
//!   taken;
//! - the restore: at `base_player::remove(1)` (death, respawn), at the
//!   player's destruction, and when a loaded record no longer names the
//!   installed borrow (the reconcile at the record's read, before the
//!   native readers); a loaded record naming a borrow that is not installed
//!   installs it there (the gun found or cooked, waited for);
//! - the crate: the native resupply and its checks see the own gun in place
//!   of the borrowed one (a view); the borrowed reserve is refilled to one
//!   magazine for free and a check says "available" while it is below.
//!
//! Every refusal is logged to `srp_patches.log` (with the process id), never
//! a crash.
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use weapon_pickup::{Borrow, Drop as DropState, Kit, Ms, Picker, Place, Record, WeaponSlot, PLACES, WINDOW_MS};

use crate::vostok::gun_cook::{self, CookId, Cooked, Gun, InventoryId, Refusal, TICKETS};
use crate::vostok::log::{line, Piece::{self, Dec, Hex, Text}};
use crate::vostok::pickup::{self as native, Installed, Mode, Site};
use crate::vostok::world::{GameWorld, Player, PlayerId};
use crate::vostok::{clock, Context, Flow, Lock, NoExit, Thread};
use crate::vostok::{WeaponPickupOnCanUseExit, WeaponPickupOnUseHintExit};

/// After the window, how long a death's gun is still kept: a late input can
/// roll the simulation back into the window (and a pickup in it simulated
/// again) for about this long.
const MARGIN_MS: u32 = 3_000;
/// The longest a wait runs the callbacks before it gives up (the pickup is
/// then refused, logged).
const WAIT_CAP_MS: u32 = 2_000;
/// Borrowers at once: a player object each (20 places, three kits).
const HOLDERS_MAX: usize = 60;

/// Where an instance's gun is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A donor's death: its life (spawn time, player+328h) and the death's
    /// simulation time.
    Death { spawned: u32, died: Ms },
    /// Named by a loaded world's record, or a pickup of a death this client
    /// did not see (a late join).
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

/// Whether a cooked gun is installed in a player's hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    Kept,
    /// Installed for this player: freed only once it is given back.
    Lent(PlayerId),
}

struct Entry {
    source: Source,
    origin: Origin,
    donor: PlayerId,
    gun: Gun,
    report: Report,
    using: Use,
}

static CACHE: Lock<[Option<Entry>; TICKETS]> = Lock::new([const { None }; TICKETS]);

/// A player was made (a player cook's completion) since the last move: a
/// late joiner's install may be among the next callbacks.
static PLAYER_MADE: AtomicBool = AtomicBool::new(false);

/// A place's synced record, of the match (its world core).
#[derive(Clone, Copy)]
struct Placed {
    world: u32,
    record: Record,
}

/// The records by place: what each player state carries (its drop stays
/// with the place when it respawns with another kit).
static RECORDS: Lock<[Option<Placed>; PLACES as usize]> = Lock::new([None; PLACES as usize]);

/// An installed borrow (runtime, not synced): what restores it.
#[derive(Clone, Copy)]
struct Holder {
    player: PlayerId,
    borrow: Borrow,
    installed: Installed,
}

static HOLDERS: Lock<[Option<Holder>; HOLDERS_MAX]> = Lock::new([None; HOLDERS_MAX]);

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

/// Why a pickup command did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    /// The player has no place or world.
    NoPlace,
    /// The owner's rule (`weapon_pickup::decide`).
    Rule(weapon_pickup::Refusal),
    /// No gun for it on this client.
    Gun(Refused),
    /// The donor's place has no player to cook from.
    NoDonor,
    Install(native::Refusal),
}

impl Why {
    fn text(self) -> &'static str {
        use weapon_pickup::Refusal as R;
        match self {
            Why::NoPlace => "the player has no place or world",
            Why::Rule(R::Dead) => "the picker is dead",
            Why::Rule(R::Borrowing) => "the picker already holds a borrowed gun",
            Why::Rule(R::NoGunInHands) => "no gun of its own in its hands",
            Why::Rule(R::NotAvailable) => "no drop to take (none, taken, past 5 s, or its own)",
            Why::Rule(R::OtherKind) => "the gun is not of the held gun's kind",
            Why::Gun(refused) => refused.text(),
            Why::NoDonor => "the donor's place has no player",
            Why::Install(refusal) => refusal.text(),
        }
    }
}

// ---- the records ------------------------------------------------------------------------------

fn record_at(thread: &Thread, world: u32, place: Place) -> Record {
    RECORDS.with(thread, |records| match records[usize::from(place.get())] {
        Some(p) if p.world == world => p.record,
        _ => Record::EMPTY,
    })
}

fn update_record(thread: &Thread, world: u32, place: Place, f: impl FnOnce(&mut Record)) {
    RECORDS.with(thread, |records| {
        let slot = &mut records[usize::from(place.get())];
        let mut record = match *slot {
            Some(p) if p.world == world => p.record,
            _ => Record::EMPTY,
        };
        f(&mut record);
        *slot = Some(Placed { world, record });
    });
}

/// The player's place and match.
fn located(player: &Player<'_>) -> Option<(u32, Place)> {
    Some((player.world_key()?, Place::new(player.place())?))
}

fn holder_of(thread: &Thread, player: PlayerId) -> Option<Holder> {
    HOLDERS.with(thread, |holders| holders.iter().flatten().find(|h| h.player == player).copied())
}

fn set_holder(thread: &Thread, player: PlayerId, holder: Option<Holder>) -> bool {
    HOLDERS.with(thread, |holders| {
        let at = holders.iter().position(|h| h.is_some_and(|h| h.player == player))
            .or_else(|| holders.iter().position(|h| h.is_none()));
        match at {
            Some(at) => {
                holders[at] = holder;
                true
            }
            None => holder.is_none(),
        }
    })
}

fn set_use(thread: &Thread, cook: CookId, using: Use) {
    CACHE.with(thread, |cache| {
        if let Some(entry) = cache.iter_mut().flatten().find(|e| e.gun.id() == cook) {
            entry.using = using;
        }
    });
}

/// `pickup.save` (base_player::serialize's start): the player's record.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, Site::Save) {
        let record = located(&player).map_or(Record::EMPTY, |(world, place)| record_at(ctx.thread(), world, place));
        native::write_record(ctx, &record.encode());
    }
    Flow::Continue
}

/// `pickup.load` (base_player::deserialize's start): the player's record,
/// then its borrow made what the record says before the native readers run.
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let bytes = native::read_record(ctx);
    let Some(player) = native::player(ctx, Site::Load) else { return Flow::Continue };
    let record = match bytes.and_then(Record::decode) {
        Some(record) => record,
        None => {
            log(&[Text("a player state's record of place "), Dec(u32::from(player.place())),
                  Text(" does not decode (read as empty): "),
                  Text(if bytes.is_some() { "bad bytes" } else { "the state is too short" })]);
            Record::EMPTY
        }
    };
    let thread = ctx.thread();
    if let Some((world, place)) = located(&player) {
        update_record(thread, world, place, |r| *r = record);
    }
    reconcile(&player, thread, record.borrow);
    Flow::Continue
}

/// The installed borrow made `want`: restored, installed (without the
/// switch: the native reader switches the held object), or left.
fn reconcile(player: &Player<'_>, thread: &Thread, want: Option<Borrow>) {
    let have = holder_of(thread, player.id());
    if have.map(|h| h.borrow) == want {
        return;
    }
    if let Some(holder) = have {
        give_back_from(player, thread, holder, "a loaded record without it");
    }
    let Some(borrow) = want else { return };
    let outcome = (|| {
        let donor = peer(player, borrow.donor).ok_or(Why::NoDonor)?;
        let cook = need_cook(thread, &donor, borrow.kit, borrow.slot).map_err(Why::Gun)?;
        let installed = native::install(player, borrow.own, cook, borrow.slot, Mode::Load).map_err(Why::Install)?;
        Ok::<_, Why>((cook, installed))
    })();
    match outcome {
        Ok((cook, installed)) => {
            let kept = set_holder(thread, player.id(), Some(Holder { player: player.id(), borrow, installed }));
            set_use(thread, cook, Use::Lent(player.id()));
            log_borrow("a loaded record: installed", player, borrow, if kept { "" } else { " (NO ROOM to remember it)" });
        }
        Err(why) => log_borrow("a loaded record's borrow NOT installed (the state will not match)", player, borrow,
                               why.text()),
    }
}

/// Restores `holder`'s install and gives its gun back to the cache (kept
/// for a rollback's pickup again until the window ends).
fn give_back_from(player: &Player<'_>, thread: &Thread, holder: Holder, why: &str) {
    let outcome = native::restore(player, &holder.installed);
    set_holder(thread, player.id(), None);
    set_use(thread, holder.installed.cook(), Use::Kept);
    match outcome {
        Ok(()) => log_borrow("restored", player, holder.borrow, why),
        Err(refusal) => log_borrow("FAILED to restore", player, holder.borrow, refusal.text()),
    }
}

/// `pickup.remove` (base_player::remove's entry): the death/respawn
/// removal puts the own gun back first, so its unload charges the own
/// profile; the borrowed gun stays held, and remove deactivates it.
pub fn on_remove(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !native::removing(ctx) {
        return Flow::Continue;
    }
    let Some(player) = native::player(ctx, Site::Remove) else { return Flow::Continue };
    let thread = ctx.thread();
    if let Some(holder) = holder_of(thread, player.id()) {
        give_back_from(&player, thread, holder, "its removal (death or respawn)");
        if let Some((world, place)) = located(&player) {
            update_record(thread, world, place, |r| r.borrow = None);
        }
    }
    Flow::Continue
}

// ---- the death and the pickup -----------------------------------------------------------------

/// The death of a player (composed from artefact_carry's `carry.client_death`,
/// the player still intact): its drop is the gun it held if one of its own
/// (its cook asked for, unless this life's is: a rollback replaying the
/// death); none when it held a borrowed gun (it vanishes) or anything else.
pub fn died(player: &Player<'_>, thread: &Thread) {
    let held = if holder_of(thread, player.id()).is_some() { None } else { gun_cook::held_gun(player) };
    let died = Ms(player.time());
    let drop = match (held, player.kit()) {
        (Some(slot), Some(kit)) => DropState::Open { kit, slot, died },
        _ => DropState::None,
    };
    if let Some((world, place)) = located(player) {
        update_record(thread, world, place, |r| r.drop = drop);
    }
    let (DropState::Open { kit, slot, .. }, Some(place)) = (drop, Place::new(player.place())) else { return };
    let spawned = player.spawned();
    let known = CACHE.with(thread, |cache| {
        cache.iter().flatten().any(|e| e.source.place == place && matches!(e.origin, Origin::Death { spawned: s, .. } if s == spawned))
    });
    if !known {
        ask(player, thread, Source { place, kit, slot }, Origin::Death { spawned, died });
    }
}

/// The input's apply point (composed from artefact_carry's `carry.filter`,
/// before patch 78 filters the actions): a pickup command is played.
pub fn on_filter(player: &Player<'_>, thread: &Thread) {
    let Some(target) = weapon_pickup::command(player.input_actions()) else { return };
    match take(player, thread, target) {
        Ok(borrow) => log_borrow("took", player, borrow, ""),
        Err(why) => log(&[Text("place "), Dec(u32::from(player.place())), Text(" asked for the gun of place "),
                          Dec(u32::from(target.get())), Text(" at "), Dec(player.time()), Text(" ms: refused, "),
                          Text(why.text())]),
    }
}

fn take(player: &Player<'_>, thread: &Thread, target: Place) -> Result<Borrow, Why> {
    let (world, place) = located(player).ok_or(Why::NoPlace)?;
    let now = Ms(player.time());
    let mine = record_at(thread, world, place);
    let theirs = record_at(thread, world, target);
    let picker = Picker { place, alive: player.alive(), held: player.gun_in_hands(), borrow: mine.borrow };
    // The rule without the gun first: no cook is waited for a refusal.
    weapon_pickup::decide(picker, target, theirs.drop, now, |_, _| true).map_err(Why::Rule)?;
    let DropState::Open { kit, slot, died } = theirs.drop else { return Err(Why::Rule(weapon_pickup::Refusal::NotAvailable)) };
    let own = picker.held.ok_or(Why::Rule(weapon_pickup::Refusal::NoGunInHands))?;
    let source = Source { place: target, kit, slot };
    let donor = peer(player, target);
    let cook = match death_cook(thread, source, died) {
        Some(id) => wait_for(thread, id, source).map_err(Why::Gun)?,
        // A death this client did not see (it joined after): cooked now.
        None => need_cook(thread, donor.as_ref().ok_or(Why::NoDonor)?, kit, slot).map_err(Why::Gun)?,
    };
    let take = weapon_pickup::decide(picker, target, theirs.drop, now,
                                     |_, slot| native::same_kind(player, own, cook, slot) == Some(true))
        .map_err(Why::Rule)?;
    let magazine = donor.filter(|d| d.kit() == Some(kit)).and_then(|d| d.magazine(slot));
    let installed = native::install(player, own, cook, slot, Mode::Switch(magazine)).map_err(Why::Install)?;
    let borrow = take.borrow;
    if !set_holder(thread, player.id(), Some(Holder { player: player.id(), borrow, installed })) {
        log(&[Text("NO ROOM to remember a borrow (it will not be restored)")]);
    }
    set_use(thread, cook, Use::Lent(player.id()));
    update_record(thread, world, place, |r| r.borrow = Some(borrow));
    update_record(thread, world, target, |r| r.drop = DropState::Taken);
    Ok(borrow)
}

/// The place's current player, seen from `player`'s world.
fn peer<'a>(player: &'a Player<'_>, place: Place) -> Option<Player<'a>> {
    player.carry_peer(artefact_carry::Place::parse(place.get())?)
}

// ---- the instance -----------------------------------------------------------------------------

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
    let entry = Entry { source, origin, donor: player.id(), gun, report: Report::Asked { memory }, using: Use::Kept };
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

/// The cook of `source`'s death at `died`, if this client saw it and nobody holds it.
fn death_cook(thread: &Thread, source: Source, died: Ms) -> Option<CookId> {
    CACHE.with(thread, |cache| {
        cache.iter().flatten()
            .find(|e| e.source == source && e.using == Use::Kept
                  && matches!(e.origin, Origin::Death { died: d, .. } if d == died))
            .map(|e| e.gun.id())
    })
}

/// The instance for the drop of `place`'s death at `died` (ready or not):
/// what the local prompt (stage 2b) asks.
pub fn instance(thread: &Thread, place: Place, died: Ms) -> Instance {
    let found = CACHE.with(thread, |cache| {
        cache.iter().flatten()
            .find(|e| e.source.place == place && matches!(e.origin, Origin::Death { died: d, .. } if d == died))
            .map(|e| e.gun.id())
    });
    match found.map(|id| gun_cook::cooked(thread, id)) {
        None | Some(Cooked::Gone) => Instance::Missing,
        Some(Cooked::Cooking { .. }) => Instance::Cooking,
        Some(Cooked::Made { inventory, .. }) => Instance::Ready(inventory),
        Some(Cooked::Failed { .. }) => Instance::Failed,
    }
}

/// The gun a record names (its donor `donor`, a player of the donor's
/// place, and the kit and slot), not held by anyone: one already cooked of
/// that source, or a cook asked for now; waited for.
fn need_cook(thread: &Thread, donor: &Player<'_>, kit: Kit, slot: WeaponSlot) -> Result<CookId, Refused> {
    let place = Place::new(donor.place()).ok_or(Refused::Missing)?;
    let source = Source { place, kit, slot };
    let known = CACHE.with(thread, |cache| {
        cache.iter().flatten().find(|e| e.source == source && e.using == Use::Kept).map(|e| e.gun.id())
    });
    let id = match known {
        Some(id) => id,
        None => ask(donor, thread, source, Origin::Loaded).ok_or(Refused::NotAsked(Refusal::NoTicket))?,
    };
    wait_for(thread, id, source)
}

/// The gun a loaded world's record names, made or waited for (stage 1's
/// interface).
pub fn need(thread: &Thread, donor: &Player<'_>, kit: Kit, slot: WeaponSlot) -> Result<InventoryId, Refused> {
    let id = need_cook(thread, donor, kit, slot)?;
    match gun_cook::cooked(thread, id) {
        Cooked::Made { inventory, .. } => Ok(inventory),
        _ => Err(Refused::Missing),
    }
}

fn wait_for(thread: &Thread, id: CookId, source: Source) -> Result<CookId, Refused> {
    let start = clock::counter(thread);
    let mut waited = false;
    let outcome = loop {
        match gun_cook::cooked(thread, id) {
            Cooked::Made { .. } => break Ok(id),
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
        log(&[Text("waited "), Dec(ms), Text(" ms for the gun of place "), Dec(u32::from(source.place.get())),
              Text(" kit "), Dec(u32::from(source.kit.get())), Text(" slot "), Dec(u32::from(source.slot.slot())),
              Text(": "), Text(what)]);
    }
    outcome
}

/// Every instance of `source` nobody holds is freed.
pub fn give_back(thread: &Thread, source: Source) {
    release_where(thread, |e| e.source == source && e.using == Use::Kept);
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
/// callbacks; finished cooks are logged; a gun nobody holds is freed past
/// its death's window and margin (a loaded record's at once).
pub fn after_move(ctx: &Context<'_>) {
    let thread = ctx.thread();
    PLAYER_MADE.store(false, Ordering::Release);
    log_finished(thread);
    let Some(now) = GameWorld::at_after_move(ctx).as_ref().and_then(gun_cook::world_time) else { return };
    release_where(thread, |e| e.using == Use::Kept && match e.origin {
        Origin::Death { died, .. } => Ms(now).since(died) >= WINDOW_MS + MARGIN_MS && now >= died.0,
        Origin::Loaded => true,
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
/// a borrow it holds is restored first; the guns cooked from it that nobody
/// holds go.
pub fn on_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(player) = native::player(ctx, Site::Destroy) {
        if let Some(holder) = holder_of(thread, player.id()) {
            give_back_from(&player, thread, holder, "its destruction");
            if let Some((world, place)) = located(&player) {
                update_record(thread, world, place, |r| r.borrow = None);
            }
        }
        let donor = player.id();
        release_where(thread, |e| e.donor == donor && e.using == Use::Kept);
    }
    Flow::Continue
}

// ---- the crate --------------------------------------------------------------------------------

fn holder_at<'t>(ctx: &'t Context<'_>, site: Site) -> Option<(Player<'t>, Holder)> {
    let player = native::player(ctx, site)?;
    let holder = holder_of(ctx.thread(), player.id())?;
    Some((player, holder))
}

/// `pickup.can_use`: a borrower's crate is available while the borrowed
/// reserve is below a magazine; else the native check sees the own loadout.
pub fn on_can_use(ctx: &mut Context<'_>) -> Flow<WeaponPickupOnCanUseExit> {
    match check(ctx, Site::CanUse) {
        Check::Available => Flow::Exit(WeaponPickupOnCanUseExit::Available),
        Check::View => Flow::Skip,
        Check::Native => Flow::Continue,
    }
}

/// `pickup.use_hint`: as can_use, for the crate's hint.
pub fn on_use_hint(ctx: &mut Context<'_>) -> Flow<WeaponPickupOnUseHintExit> {
    match check(ctx, Site::UseHint) {
        Check::Available => Flow::Exit(WeaponPickupOnUseHintExit::Available),
        Check::View => Flow::Skip,
        Check::Native => Flow::Continue,
    }
}

enum Check {
    Available,
    /// The view is handed to check_resupply in place of the inventory.
    View,
    Native,
}

fn check(ctx: &mut Context<'_>, site: Site) -> Check {
    let Some((player, holder)) = holder_at(ctx, site) else { return Check::Native };
    if native::reserve_low(&player, &holder.installed) {
        return Check::Available;
    }
    let Some(view) = native::crate_view(&player, &holder.installed, ctx.thread()) else { return Check::Native };
    native::hand_view(ctx, site, view);
    Check::View
}

/// `pickup.resupply`: the crate's use refills the borrowed reserve to one
/// magazine for free; the native resupply sees the own loadout.
pub fn on_resupply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    resupply(ctx, Site::Resupply)
}

/// `pickup.client_resupply`: the client's own finish_use, the same.
pub fn on_client_resupply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    resupply(ctx, Site::ClientResupply)
}

fn resupply(ctx: &mut Context<'_>, site: Site) -> Flow<NoExit> {
    let Some((player, holder)) = holder_at(ctx, site) else { return Flow::Continue };
    native::refill(&player, &holder.installed);
    let Some(view) = native::crate_view(&player, &holder.installed, ctx.thread()) else { return Flow::Continue };
    native::hand_view(ctx, site, view);
    Flow::Skip
}

// ---- for stage 2b (the prompt, the ragdoll's gun) ----------------------------------------------

/// The drop of `place` the local prompt may offer to `viewer` now (its
/// time): open, within the window, its gun made on this client.
pub fn available_drop(thread: &Thread, viewer: &Player<'_>, place: Place) -> Option<(Kit, WeaponSlot)> {
    let world = viewer.world_key()?;
    let drop = record_at(thread, world, place).drop;
    let (kit, slot) = drop.available(Ms(viewer.time()))?;
    let DropState::Open { died, .. } = drop else { return None };
    matches!(instance(thread, place, died), Instance::Ready(_)).then_some((kit, slot))
}

/// Whether `place`'s last drop was taken (its gun is hidden on its ragdoll).
pub fn drop_taken(thread: &Thread, viewer: &Player<'_>, place: Place) -> bool {
    viewer.world_key().is_some_and(|world| record_at(thread, world, place).drop == DropState::Taken)
}

/// Whether `player` holds a borrowed gun (its ragdoll drops none).
pub fn borrowing(thread: &Thread, player: &Player<'_>) -> bool {
    holder_of(thread, player.id()).is_some()
}

// ---- the log ----------------------------------------------------------------------------------

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

/// A line `pid P pickup: ...`.
fn log(rest: &[Piece<'_>]) {
    let mut pieces = [Text(""); 28];
    let head = [Text("pid "), Dec(gun_cook::process_id()), Text(" pickup: ")];
    let mut at = 0;
    for piece in head.iter().chain(rest) {
        if let Some(slot) = pieces.get_mut(at) {
            *slot = *piece;
            at += 1;
        }
    }
    line(&pieces[..at]);
}

fn log_borrow(what: &str, player: &Player<'_>, borrow: Borrow, why: &str) {
    log(&[Text("place "), Dec(u32::from(player.place())), Text(" at "), Dec(player.time()), Text(" ms "), Text(what),
          Text(": the gun of place "), Dec(u32::from(borrow.donor.get())), Text(" kit "), Dec(u32::from(borrow.kit.get())),
          Text(" slot "), Dec(u32::from(borrow.slot.slot())), Text(" in place of its slot "),
          Dec(u32::from(borrow.own.slot())), Text(if why.is_empty() { "" } else { " (" }), Text(why),
          Text(if why.is_empty() { "" } else { ")" })]);
}

fn log_source(what: &str, source: Source, origin: Origin, rest: &[Piece<'_>]) {
    let mut pieces = [Text(""); 25];
    let head = [Text(what), Text(" the gun of place "), Dec(u32::from(source.place.get())), Text(" kit "),
                Dec(u32::from(source.kit.get())), Text(" slot "), Dec(u32::from(source.slot.slot()))];
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
    log(&pieces[..at]);
}
