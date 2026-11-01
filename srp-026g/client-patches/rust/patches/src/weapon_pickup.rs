//! Patch 85, dropped-weapon pickup (docs/notes/weapon_pickup.md).
//!
//! Stage 1, the instance: at a death on every client (the same tick
//! everywhere), when the dead player held one of its two own guns, that gun
//! alone is cooked (`vostok::gun_cook`), kept by donor life, and freed when
//! nobody holds it 3 s after its body left the world (or after its give-back
//! when later), or when its donor player is destroyed.
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
//! - the restore: at `base_player::remove(1)` (respawn, departure), at the
//!   player's destruction, and when a loaded record no longer names the
//!   installed borrow (the reconcile at the record's read, before the
//!   native readers); a loaded record naming a borrow that is not installed
//!   installs it there (the gun found or cooked, waited for);
//! - the crate: the native resupply and its checks see the own gun in place
//!   of the borrowed one (a view); each borrowed stack is refilled to the
//!   picker's allowance for its slot for free, and a check says "available" while one is below;
//! - the drop's end: `base_player::remove(1)` of the place's player (its body
//!   and gun leave the world: the respawn, the departure) ends it.
//!
//! Stage 2b, the local client (what this client draws and sends):
//! - the ragdoll's gun, at artefact_carry's `carry.weapon` ([`ragdoll_gun`]):
//!   an open drop's gun kept by place (its transform, its model's box), a
//!   taken drop's gun and a borrowed gun hidden as patch 78 hides a
//!   carrier's;
//! - the prompt, at artefact_carry's on_hud after patch 78's own targets
//!   ([`aim`]): the gun the local player looks at (2.2 m, its box padded by
//!   5 cm, in sight) that a pickup would take now, "Pick up <gun> (E)";
//! - the command, at the input builder's end after patch 78's chord step
//!   ([`on_input_end`]): Use's press on that gun sent as its pickup, if the
//!   server's check passes it.
//!
//! Every refusal is logged to `srp_patches.log` (with the process id), never
//! a crash.
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use weapon_pickup::{Borrow, Drop as DropState, Handedness, Kit, Ms, Picker, Place, Record, WeaponSlot, PLACES};

use crate::vostok::gun_cook::{self, CookId, Cooked, Gun, Refusal, TICKETS};
use crate::vostok::log::Piece::{self, Dec, Hex, Text};
use crate::vostok::pickup::{self as native, GunBox, GunPose, Installed, Mode, NameKey, Site};
use crate::vostok::world::{GameWorld, Player, PlayerId};
use crate::vostok::{clock, ActionMask, Context, Flow, GameCell, GameWorldUi, InputHandler, Lock, NoExit, Thread};
use crate::vostok::{WeaponPickupOnCanUseExit, WeaponPickupOnUseHintExit};

/// After its body left the world, how long a death's gun is still kept: a
/// late input can roll the simulation back to before that (and a pickup
/// then simulated again) for about this long.
const MARGIN_MS: u32 = 3_000;
/// The longest a wait runs the callbacks before it gives up (the pickup is
/// then refused, logged).
const WAIT_CAP_MS: u32 = 2_000;
/// Borrowers at once: a player object each (20 places, three kits).
const HOLDERS_MAX: usize = 60;

/// Where an instance's gun is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A donor's death: its life (spawn time, player+328h), the death's
    /// simulation time and, once its body left the world (its respawn, its
    /// departure: base_player::remove(1)), the time its margin counts from:
    /// that, or a later give-back of the gun.
    Death { spawned: u32, died: Ms, gone: Option<Ms> },
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
    /// Its outcome logged; the made gun's model box (read once here, on
    /// the logic thread: what the ragdoll's gun is matched against).
    Logged { model: Option<GunBox> },
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

/// The player object and life whose death this client saw last at a place:
/// its ragdoll holds that death's drop (runtime, not synced; a loaded
/// record leaves it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Body {
    player: PlayerId,
    spawned: u32,
}

/// A place's synced record, of the match (its world core), and its last
/// death's body.
#[derive(Clone, Copy)]
struct Placed {
    world: u32,
    record: Record,
    body: Option<Body>,
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
            Why::NoPlace => "the player has no place, world or profile",
            Why::Rule(R::Dead) => "the picker is dead",
            Why::Rule(R::Borrowing) => "the picker borrows a gun and holds its other one",
            Why::Rule(R::NoGunInHands) => "no gun of its own in its hands",
            Why::Rule(R::NotAvailable) => "no drop to take (none, taken, its body gone, or its own)",
            Why::Rule(R::SecondWeaponDisabled) => "its kit carries one gun only, in slot 7 (modifier 64)",
            Why::Rule(R::TwoDoubleHanded) => "a second double-handed gun without modifier 25",
            Why::Gun(refused) => refused.text(),
            Why::NoDonor => "the donor's place has no player",
            Why::Install(refusal) => refusal.text(),
        }
    }
}

// ---- the records ------------------------------------------------------------------------------

fn record_at(thread: &Thread, world: u32, place: Place) -> Record {
    placed_at(thread, world, place).0
}

/// The place's record and its last death's body.
fn placed_at(thread: &Thread, world: u32, place: Place) -> (Record, Option<Body>) {
    RECORDS.with(thread, |records| match records[usize::from(place.get())] {
        Some(p) if p.world == world => (p.record, p.body),
        _ => (Record::EMPTY, None),
    })
}

fn update_placed(thread: &Thread, world: u32, place: Place, f: impl FnOnce(&mut Record, &mut Option<Body>)) {
    RECORDS.with(thread, |records| {
        let slot = &mut records[usize::from(place.get())];
        let (mut record, mut body) = match *slot {
            Some(p) if p.world == world => (p.record, p.body),
            _ => (Record::EMPTY, None),
        };
        f(&mut record, &mut body);
        *slot = Some(Placed { world, record, body });
    });
}

fn update_record(thread: &Thread, world: u32, place: Place, f: impl FnOnce(&mut Record)) {
    update_placed(thread, world, place, |record, _| f(record));
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

fn lend(thread: &Thread, cook: CookId, player: PlayerId) {
    CACHE.with(thread, |cache| {
        if let Some(entry) = cache.iter_mut().flatten().find(|e| e.gun.id() == cook) {
            entry.using = Use::Lent(player);
        }
    });
}

/// The cook given back at `at`: kept; when its body has left the world
/// before `at`, for the margin from `at` (a rollback may take it again).
fn keep(thread: &Thread, cook: CookId, at: Ms) {
    CACHE.with(thread, |cache| {
        if let Some(entry) = cache.iter_mut().flatten().find(|e| e.gun.id() == cook) {
            entry.using = Use::Kept;
            if let Origin::Death { gone: Some(gone), .. } = &mut entry.origin
                && at.since(*gone) < u32::MAX / 2 {
                *gone = at;
            }
        }
    });
}

/// `pickup.save` (base_player::serialize's start): the player's record.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, Site::Save) {
        let record = located(&player)
            .map_or(Record::EMPTY, |(world, place)| placed_at(ctx.thread(), world, place).0);
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
            lend(thread, cook, player.id());
            log_borrow("a loaded record: installed", player, borrow, if kept { "" } else { " (NO ROOM to remember it)" });
        }
        Err(why) => log_borrow("a loaded record's borrow NOT installed (the state will not match)", player, borrow,
                               why.text()),
    }
}

/// Restores `holder`'s install and gives its gun back to the cache (kept
/// for a rollback's pickup again until its body has left the world and the
/// margin passed since the later of that and now).
fn give_back_from(player: &Player<'_>, thread: &Thread, holder: Holder, why: &str) {
    let outcome = native::restore(player, &holder.installed);
    set_holder(thread, player.id(), None);
    keep(thread, holder.installed.cook(), Ms(player.time()));
    match outcome {
        Ok(()) => log_borrow("restored", player, holder.borrow, why),
        Err(refusal) => log_borrow("FAILED to restore", player, holder.borrow, refusal.text()),
    }
}

/// `pickup.remove` (base_player::remove's entry): the removal of the
/// place's player from the world (flag 1: the respawn tick's 0x3169A0, a
/// departure's 0x316BE0, a snapshot's kit change 0x2D2A68 before the new
/// kit's state is read) ends its life's pickup state. A borrower's own gun
/// is put back first, so its unload charges the own profile (the borrowed
/// gun stays held, and remove deactivates it). A dead player's body, and its
/// gun, leave the world here: its drop ends (the prompt and the gun on the
/// body agree on every client), and that death's cooked gun is kept for
/// [`MARGIN_MS`] more.
pub fn on_remove(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !native::removing(ctx) {
        return Flow::Continue;
    }
    let Some(player) = native::player(ctx, Site::Remove) else { return Flow::Continue };
    let thread = ctx.thread();
    if let Some(holder) = holder_of(thread, player.id()) {
        give_back_from(&player, thread, holder, "its removal (a respawn or a departure)");
    }
    if let Some((world, place)) = located(&player) {
        let (record, _) = placed_at(thread, world, place);
        update_record(thread, world, place, |r| *r = Record::EMPTY);
        // Its gun's last pose goes with the body (the prompt reads none).
        SEEN.with(thread, |seen| seen[usize::from(place.get())] = None);
        if let DropState::Open { kit, slot, .. } = record.drop {
            log(&[Text("place "), Dec(u32::from(place.get())), Text(" at "), Dec(player.time()),
                  Text(" ms: its body left the world, the drop of its kit "), Dec(u32::from(kit.get())),
                  Text(" slot "), Dec(u32::from(slot.slot())), Text(" ends")]);
        }
    }
    let (donor, spawned, at) = (player.id(), player.spawned(), Ms(player.time()));
    CACHE.with(thread, |cache| {
        for entry in cache.iter_mut().flatten().filter(|e| e.donor == donor) {
            if let Origin::Death { spawned: s, died, gone: None } = entry.origin
                && s == spawned {
                    entry.origin = Origin::Death { spawned, died, gone: Some(at) };
                }
        }
    });
    Flow::Continue
}

// ---- the death and the pickup -----------------------------------------------------------------

/// The death of a player (composed from artefact_carry's `carry.client_death`,
/// the player still intact): its drop is the gun it held if one of its own
/// (its cook asked for, unless this life's is: a rollback replaying the
/// death); none when it held a borrowed gun (it vanishes) or anything else.
pub fn died(player: &Player<'_>, thread: &Thread) {
    log(&[Text("death hook: place "), Dec(u32::from(player.place())), Text(" life "), Dec(player.spawned()),
          Text(" at "), Dec(player.time()), Text(" ms")]);
    let held = if holder_of(thread, player.id()).is_some() { None } else { gun_cook::held_gun(player) };
    let drop = match (held, player.kit()) {
        (Some(slot), Some(kit)) => DropState::Open { kit, slot },
        _ => DropState::None,
    };
    if let Some((world, place)) = located(player) {
        let body = Body { player: player.id(), spawned: player.spawned() };
        update_placed(thread, world, place, |r, b| {
            r.drop = drop;
            *b = Some(body);
        });
    }
    let (DropState::Open { kit, slot, .. }, Some(place)) = (drop, Place::new(player.place())) else { return };
    let spawned = player.spawned();
    let known = CACHE.with(thread, |cache| {
        cache.iter().flatten().any(|e| e.source.place == place && matches!(e.origin, Origin::Death { spawned: s, .. } if s == spawned))
    });
    if !known {
        ask(player, thread, Source { place, kit, slot }, Origin::Death { spawned, died: Ms(player.time()), gone: None });
    }
}

/// The input's apply point (composed from artefact_carry's `carry.filter`,
/// before patch 78 filters the actions): a pickup command is played.
pub fn on_filter(player: &Player<'_>, thread: &Thread) {
    let Some(target) = weapon_pickup::command(player.input_actions()) else { return };
    match take(player, thread, target) {
        Ok(taken) => {
            log_borrow("took", player, taken.borrow, "");
            log_installed(player, taken.magazine, &taken.installed);
        }
        Err(why) => log(&[Text("place "), Dec(u32::from(player.place())), Text(" asked for the gun of place "),
                          Dec(u32::from(target.get())), Text(" at "), Dec(player.time()), Text(" ms: refused, "),
                          Text(why.text())]),
    }
}

/// A pickup played: the borrow, the donor's magazine and chamber it was
/// given (None: its gun was not there, a full magazine) and what was
/// installed.
struct Taken {
    borrow: Borrow,
    magazine: Option<(u8, bool)>,
    installed: Installed,
}

fn take(player: &Player<'_>, thread: &Thread, target: Place) -> Result<Taken, Why> {
    let (world, place) = located(player).ok_or(Why::NoPlace)?;
    let mine = record_at(thread, world, place);
    let theirs = record_at(thread, world, target);
    let picker = picker_of(player, place, mine.borrow).ok_or(Why::NoPlace)?;
    // The rule without the gun first (a pistol refuses least): no cook is
    // waited for a refusal.
    weapon_pickup::decide(picker, target, theirs.drop, |_, _| Handedness::One).map_err(Why::Rule)?;
    let DropState::Open { kit, slot } = theirs.drop else { return Err(Why::Rule(weapon_pickup::Refusal::NotAvailable)) };
    let own = picker.held.ok_or(Why::Rule(weapon_pickup::Refusal::NoGunInHands))?;
    let source = Source { place: target, kit, slot };
    let donor = peer(player, target);
    let cook = match kept_cook(thread, source) {
        Some(id) => wait_for(thread, id, source).map_err(Why::Gun)?,
        // A death this client did not see (it joined after): cooked now.
        None => need_cook(thread, donor.as_ref().ok_or(Why::NoDonor)?, kit, slot).map_err(Why::Gun)?,
    };
    let incoming = native::handedness(thread, cook, slot).ok_or(Why::Install(native::Refusal::NoCookedGun))?;
    let take = weapon_pickup::decide(picker, target, theirs.drop, |_, _| incoming).map_err(Why::Rule)?;
    let magazine = donor.filter(|d| d.kit() == Some(kit)).and_then(|d| d.magazine(slot));
    // Over a borrowed gun in hands: that one goes (restored to its cooked
    // inventory), the own gun and ammunition stay parked under the new one.
    let replaced = holder_of(thread, player.id());
    let mode = replaced.as_ref().map_or(Mode::Switch(magazine), |h| Mode::Replace(magazine, &h.installed));
    let installed = native::install(player, own, cook, slot, mode).map_err(Why::Install)?;
    if let Some(old) = replaced {
        keep(thread, old.installed.cook(), Ms(player.time()));
        log_borrow("gave back, replaced", player, old.borrow, "");
    }
    let borrow = take.borrow;
    if !set_holder(thread, player.id(), Some(Holder { player: player.id(), borrow, installed })) {
        log(&[Text("NO ROOM to remember a borrow (it will not be restored)")]);
    }
    lend(thread, cook, player.id());
    update_record(thread, world, place, |r| r.borrow = Some(borrow));
    update_record(thread, world, target, |r| r.drop = DropState::Taken);
    Ok(Taken { borrow, magazine, installed })
}

/// The place's current player, seen from `player`'s world.
fn peer<'a>(player: &'a Player<'_>, place: Place) -> Option<Player<'a>> {
    player.carry_peer(artefact_carry::Place::parse(place.get())?)
}

/// What `player` is for the rule: alive, the slot of its own gun in hands,
/// the other slot's gun, its kit's carry rule (None: no profile).
fn picker_of(player: &Player<'_>, place: Place, borrow: Option<Borrow>) -> Option<Picker> {
    let held = player.gun_in_hands();
    Some(Picker { place, alive: player.alive(), held,
                  other: held.and_then(|slot| player.weapon_handedness(slot.other())),
                  rule: player.carry_rule()?, borrow })
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

/// A cook of `source`'s gun nobody holds, if this client has one: the same
/// gun whichever of its deaths it was cooked at (a rollback replays a death
/// at another time).
fn kept_cook(thread: &Thread, source: Source) -> Option<CookId> {
    CACHE.with(thread, |cache| {
        cache.iter().flatten().find(|e| e.source == source && e.using == Use::Kept).map(|e| e.gun.id())
    })
}

/// The model box of a made cook of `source`'s gun nobody holds.
fn kept_box(thread: &Thread, source: Source) -> Option<GunBox> {
    CACHE.with(thread, |cache| {
        cache.iter().flatten().find_map(|e| match e.report {
            Report::Logged { model } if e.source == source && e.using == Use::Kept => model,
            _ => None,
        })
    })
}

/// The gun a record names (its donor `donor`, a player of the donor's
/// place, and the kit and slot), not held by anyone: one already cooked of
/// that source, or a cook asked for now; waited for.
fn need_cook(thread: &Thread, donor: &Player<'_>, kit: Kit, slot: WeaponSlot) -> Result<CookId, Refused> {
    let place = Place::new(donor.place()).ok_or(Refused::Missing)?;
    let source = Source { place, kit, slot };
    let id = match kept_cook(thread, source) {
        Some(id) => id,
        None => ask(donor, thread, source, Origin::Loaded).ok_or(Refused::NotAsked(Refusal::NoTicket))?,
    };
    wait_for(thread, id, source)
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

fn release_where(thread: &Thread, mut gone: impl FnMut(&Entry) -> bool) {
    loop {
        // One at a time, each dropped outside the lock (the cooked
        // inventory's own teardown).
        let taken = CACHE.with(thread, |cache| cache.iter_mut().find(|e| e.as_ref().is_some_and(&mut gone))?.take());
        match taken {
            Some(entry) => {
                log_source("freeing the cook of", entry.source, entry.origin, &[]);
                drop(entry);
            }
            None => break,
        }
    }
}

/// Once a frame after the simulation's move (composed from ragdoll's
/// `game_world.after_move`): a late join's install has had its frame's
/// callbacks; finished cooks are logged; a gun nobody holds is freed
/// [`MARGIN_MS`] after its body left the world (a loaded record's at once).
pub fn after_move(ctx: &Context<'_>) {
    let thread = ctx.thread();
    PLAYER_MADE.store(false, Ordering::Release);
    log_finished(thread);
    let Some(now) = GameWorld::at_after_move(ctx).as_ref().and_then(gun_cook::world_time) else { return };
    release_where(thread, |e| e.using == Use::Kept && match e.origin {
        Origin::Death { gone: Some(at), .. } => Ms(now).since(at) >= MARGIN_MS && now >= at.0,
        Origin::Death { gone: None, .. } => false,
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
                let model = matches!(cooked, Cooked::Made { .. })
                    .then(|| native::gun_box(thread, e.gun.id(), e.source.slot)).flatten();
                e.report = Report::Logged { model };
                Some((e.source, e.origin, memory, cooked, model.is_some()))
            })
        });
        let Some((source, origin, before, cooked, boxed)) = found else { break };
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
            Text(if boxed || !matches!(cooked, Cooked::Made { .. }) { " MiB" } else { " MiB, its model box UNREAD" })]);
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
/// stacks are below their allowance; else the native check sees the own loadout.
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

/// `pickup.resupply`: the crate's use refills the borrowed stacks to the
/// picker's allowance for free; the native resupply sees the own loadout.
pub fn on_resupply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    resupply(ctx, Site::Resupply)
}

/// `pickup.client_resupply`: the client's own finish_use, the same.
pub fn on_client_resupply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    resupply(ctx, Site::ClientResupply)
}

fn resupply(ctx: &mut Context<'_>, site: Site) -> Flow<NoExit> {
    let Some((player, holder)) = holder_at(ctx, site) else { return Flow::Continue };
    log_refill(&player, &native::refill(&player, &holder.installed));
    let Some(view) = native::crate_view(&player, &holder.installed, ctx.thread()) else { return Flow::Continue };
    native::hand_view(ctx, site, view);
    Flow::Skip
}

// ---- stage 2b: the ragdoll's gun, the prompt, the command --------------------------------------

/// The prompt's reach (patch 78's) and how much larger than the gun's model
/// box its target is (metres).
const REACH: f32 = 2.2;
const PAD: f32 = 0.05;

/// A drop's gun as its donor's ragdoll last drew it (by place).
#[derive(Clone, Copy)]
struct Seen {
    body: Body,
    pose: GunPose,
}

/// Written at `carry.weapon` (mt_draw, which may run in parallel tasks: a
/// Lock), read by the prompt.
static SEEN: Lock<[Option<Seen>; PLACES as usize]> = Lock::new([None; PLACES as usize]);

/// What a dead player's ragdoll draws of its gun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RagdollGun {
    Shown,
    /// Out of the scene, as patch 78 hides a carrier's gun.
    Hidden,
}

/// `carry.weapon` for a dead player (composed from artefact_carry's
/// on_weapon; locks only, no GameCell: mt_draw may run in a task): a
/// borrowed gun vanishes with its holder; the gun of a drop taken is hidden
/// on its donor's ragdoll; the gun of an open drop is kept for the prompt
/// (the site's transform and its cooked gun's box), the body being that
/// drop's death's. Nothing of the body's own objects is read.
pub fn ragdoll_gun(ctx: &Context<'_>, player: &Player<'_>) -> RagdollGun {
    let thread = ctx.thread();
    if borrowing(thread, player) {
        return RagdollGun::Hidden;
    }
    let Some((world, place)) = located(player) else { return RagdollGun::Shown };
    let body = Body { player: player.id(), spawned: player.spawned() };
    let (record, last) = placed_at(thread, world, place);
    if last != Some(body) {
        return RagdollGun::Shown;
    }
    match record.drop {
        DropState::Taken => RagdollGun::Hidden,
        DropState::Open { kit, slot } => {
            let pose = kept_box(thread, Source { place, kit, slot }).and_then(|model| native::drawn_gun(ctx, model));
            SEEN.with(thread, |seen| seen[usize::from(place.get())] = pose.map(|pose| Seen { body, pose }));
            RagdollGun::Shown
        }
        DropState::None => RagdollGun::Shown,
    }
}

/// Whether `player` holds a borrowed gun (its ragdoll drops none).
pub fn borrowing(thread: &Thread, player: &Player<'_>) -> bool {
    holder_of(thread, player.id()).is_some()
}

/// The pickup Use would send now: the place whose gun the local player
/// looks at, its own place and the match's places (the server's check).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Aimed {
    target: Place,
    sender: Place,
    places: u8,
}

/// Set by the prompt every frame; read at the input builder's end.
static AIMED: GameCell<Option<Aimed>> = GameCell::new(None);

/// The prompt (composed from artefact_carry's on_hud, after patch 78's own
/// targets: `viewer` None when one of those is shown or there is no local
/// player): the gun `viewer` looks at, its name key, when a pickup would
/// take it now. The gun is a drop's on its donor's ragdoll as this client
/// drew it: the eye's 2.2 m segment enters its model box (padded by 5 cm),
/// nothing of the level in between (patch 78's ray), the drop open (its body
/// still in the world), its gun made on this client and the rule's
/// (`weapon_pickup::decide`, the carry rules too) yes. Remembered for Use
/// ([`on_input_end`]).
pub fn aim(ctx: &Context<'_>, viewer: Option<&Player<'_>>) -> Option<NameKey> {
    let found = viewer.and_then(|viewer| aimed_at(ctx, viewer));
    AIMED.set(ctx.thread(), found.map(|(aimed, _)| aimed));
    found.map(|(_, name)| name)
}

fn aimed_at(ctx: &Context<'_>, viewer: &Player<'_>) -> Option<(Aimed, NameKey)> {
    let thread = ctx.thread();
    if !viewer.alive() || !viewer.inserted() {
        return None;
    }
    // Most frames see no drop: nothing else is read then.
    let seen = SEEN.with(thread, |seen| *seen);
    if seen.iter().all(Option::is_none) {
        return None;
    }
    let (world, sender) = located(viewer)?;
    let physics = viewer.physics_world()?;
    let (eye, forward) = viewer.carry_eye();
    let forward = unit(forward)?;
    let mut best: Option<(f32, Place, CookId, WeaponSlot)> = None;
    let mut sights = [None; PLACES as usize];
    for (n, seen) in seen.iter().enumerate() {
        let (Some(seen), Some(target)) = (seen, Place::new(n as u8)) else { continue };
        let sight = match enters(&seen.pose, eye, forward, REACH, PAD) {
            None if within(&seen.pose, eye, REACH, PAD) => Err(Sight::NotAimedAt),
            None => Err(Sight::OutOfReach),
            Some(distance) => {
                offer(thread, viewer, world, sender, target, seen.body).and_then(|(cook, slot)| {
                    let blocked = distance > 0.01 && physics.ray_test(eye, forward, distance, 0x24, 2).is_some();
                    if blocked { Err(Sight::Blocked) } else { Ok((distance, cook, slot)) }
                })
            }
        };
        if let Ok((distance, cook, slot)) = sight
            && !best.is_some_and(|(nearest, ..)| nearest <= distance) {
            best = Some((distance, target, cook, slot));
        }
        sights[n] = Some((seen.body, sight.map(|_| ()).err().unwrap_or(Sight::Offered)));
    }
    log_sights(thread, viewer, sights);
    let (_, target, cook, slot) = best?;
    let places = GameWorldUi::at_update_icons(ctx)?.places()?;
    weapon_pickup::allowed(sender, target, places).then_some(())?;
    let name = native::gun_name(thread, cook, slot).unwrap_or(NameKey::EMPTY);
    Some((Aimed { target, sender, places }, name))
}

/// What the prompt makes of a drop's gun it sees on its ragdoll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sight {
    /// Farther than the reach from the eye.
    OutOfReach,
    /// Within reach, the eye's segment misses the gun's box.
    NotAimedAt,
    /// The body drawn is not the drop's death's.
    NotItsBody,
    Taken,
    /// The place has no drop (its record's).
    NoDrop,
    /// The gun of that death not made on this client (yet).
    CookNotReady,
    /// The viewer has no place, profile or kit.
    NoPicker,
    /// The cooked gun's handedness could not be read.
    NoHandedness,
    /// The rule says no: the picker's held slot and its gun's handedness,
    /// the other slot's gun's, its kit's carry rule and the gun's handedness.
    Refused { why: weapon_pickup::Refusal, held: Option<(WeaponSlot, Option<Handedness>)>,
              other: Option<Handedness>, rule: weapon_pickup::CarryRule, incoming: Handedness },
    /// The level's ray is not clear up to the gun.
    Blocked,
    /// It would be taken (the nearest such one is the prompt's).
    Offered,
}

/// Each place's drop body and what was last logged of it.
static SIGHTS: GameCell<[Option<(Body, Sight)>; PLACES as usize]> = GameCell::new([None; PLACES as usize]);

/// One line for each drop whose sight changed (a new body, or a new reason).
fn log_sights(thread: &Thread, viewer: &Player<'_>, now: [Option<(Body, Sight)>; PLACES as usize]) {
    let before = SIGHTS.get(thread);
    for (n, (was, is)) in before.iter().zip(now.iter()).enumerate() {
        let Some((_, sight)) = is.filter(|is| *was != Some(*is)) else { continue };
        log_sight(viewer, n as u8, sight);
    }
    // A drop not seen this frame keeps what was logged of it.
    let mut kept = before;
    for (k, is) in kept.iter_mut().zip(now) {
        if is.is_some() {
            *k = is;
        }
    }
    SIGHTS.set(thread, kept);
}

fn slot_text(slot: Option<(WeaponSlot, Option<Handedness>)>) -> &'static str {
    match slot {
        Some((WeaponSlot::First, _)) => "7 ",
        Some((WeaponSlot::Second, _)) => "10 ",
        None => "none",
    }
}

fn hand_text(hand: Option<Handedness>) -> &'static str {
    match hand {
        Some(Handedness::One) => "one-handed",
        Some(Handedness::Two) => "two-handed",
        None => "none",
    }
}

fn log_sight(viewer: &Player<'_>, target: u8, sight: Sight) {
    let head = [Text("place "), Dec(u32::from(viewer.place())), Text(" sees the gun of place "), Dec(u32::from(target)),
                Text(": ")];
    let text = match sight {
        Sight::OutOfReach => "out of reach (2.2 m)",
        Sight::NotAimedAt => "in reach, not aimed at its box",
        Sight::NotItsBody => "not offered, the body drawn is not its drop's",
        Sight::Taken => "not offered, already taken",
        Sight::NoDrop => "not offered, no drop in its record",
        Sight::CookNotReady => "not offered, its gun not made on this client",
        Sight::NoPicker => "not offered, the viewer has no place, profile or kit",
        Sight::NoHandedness => "not offered, the gun's handedness unread",
        Sight::Blocked => "not offered, no line of sight (the level's ray)",
        Sight::Offered => "offered",
        Sight::Refused { why, held, other, rule, incoming } => {
            let rule = match rule {
                weapon_pickup::CarryRule::OneWeapon => "1 gun (mod 64)",
                weapon_pickup::CarryRule::OneDoubleHanded => "1 two-handed",
                weapon_pickup::CarryRule::TwoDoubleHanded => "2 two-handed (mod 25)",
            };
            return log_joined(&[&head, &[Text("not offered, "), Text(Why::Rule(why).text()), Text("; held "),
                                         Text(slot_text(held)), Text(held.map_or("", |(_, own)| hand_text(own))),
                                         Text(", other "), Text(hand_text(other)),
                                         Text(", gun "), Text(hand_text(Some(incoming))), Text(", rule "), Text(rule)]]);
        }
    };
    log_joined(&[&head, &[Text(text)]]);
}

/// What a pickup of `target`'s drop by `viewer` would take now: its cook
/// (made on this client) and the drop's slot; else why not (`body` not that
/// drop's dead body, the drop not open, its gun not made, the rule).
fn offer(thread: &Thread, viewer: &Player<'_>, world: u32, place: Place, target: Place, body: Body)
         -> Result<(CookId, WeaponSlot), Sight> {
    let (theirs, last) = placed_at(thread, world, target);
    if last != Some(body) {
        return Err(Sight::NotItsBody);
    }
    // Open until the body leaves the world (the record ends it there).
    let (kit, slot) = match theirs.drop {
        DropState::Open { kit, slot } => (kit, slot),
        DropState::Taken => return Err(Sight::Taken),
        DropState::None => return Err(Sight::NoDrop),
    };
    let cook = kept_cook(thread, Source { place: target, kit, slot }).ok_or(Sight::CookNotReady)?;
    if !matches!(gun_cook::cooked(thread, cook), Cooked::Made { .. }) {
        return Err(Sight::CookNotReady);
    }
    let picker = picker_of(viewer, place, record_at(thread, world, place).borrow).ok_or(Sight::NoPicker)?;
    let incoming = native::handedness(thread, cook, slot);
    let incoming = incoming.ok_or(Sight::NoHandedness)?;
    let take = weapon_pickup::decide(picker, target, theirs.drop, |_, _| incoming)
        .map_err(|why| Sight::Refused { why, held: picker.held.map(|slot| (slot, viewer.weapon_handedness(slot))),
                                        other: picker.other, rule: picker.rule, incoming })?;
    Ok((cook, take.borrow.slot))
}

/// Whether the gun's box (padded by `pad`) is within `reach` of `eye`: its
/// centre nearer than the reach and the box's half diagonal.
fn within(pose: &GunPose, eye: [f32; 3], reach: f32, pad: f32) -> bool {
    let m = &pose.transform;
    let mut centre = [m[12], m[13], m[14]];
    let mut radius = 0.0f32;
    for axis in 0..3 {
        let row = [m[axis * 4], m[axis * 4 + 1], m[axis * 4 + 2]];
        let middle = (pose.model.lo[axis] + pose.model.hi[axis]) * 0.5;
        for (c, r) in centre.iter_mut().zip(row) {
            *c += middle * r;
        }
        let half = (pose.model.hi[axis] - pose.model.lo[axis]) * 0.5 * ragdoll::math::sqrt(dot(row, row)) + pad;
        radius += half * half;
    }
    let d = [centre[0] - eye[0], centre[1] - eye[1], centre[2] - eye[2]];
    let limit = reach + ragdoll::math::sqrt(radius);
    dot(d, d) <= limit * limit
}

fn unit([x, y, z]: [f32; 3]) -> Option<[f32; 3]> {
    let length = ragdoll::math::sqrt(x * x + y * y + z * z);
    (length > 1e-6).then(|| [x / length, y / length, z / length])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How far along the segment from `eye` along the unit `forward`, `reach`
/// long, it enters the gun's box padded by `pad` (world metres); None when
/// it misses. The transform's rows are taken as orthogonal (a rotation and
/// a scale per axis): a world point w is at ((w - t) . r_i) / |r_i|^2.
fn enters(pose: &GunPose, eye: [f32; 3], forward: [f32; 3], reach: f32, pad: f32) -> Option<f32> {
    let m = &pose.transform;
    let offset = [eye[0] - m[12], eye[1] - m[13], eye[2] - m[14]];
    let (mut near, mut far) = (0.0f32, reach);
    for axis in 0..3 {
        let row = [m[axis * 4], m[axis * 4 + 1], m[axis * 4 + 2]];
        let squared = dot(row, row);
        if squared < 1e-8 {
            return None;
        }
        let origin = dot(offset, row) / squared;
        let step = dot(forward, row) / squared;
        let grow = pad / ragdoll::math::sqrt(squared);
        let (lo, hi) = (pose.model.lo[axis] - grow, pose.model.hi[axis] + grow);
        if step.abs() < 1e-9 {
            if origin < lo || origin > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - origin) / step, (hi - origin) / step);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// Use at the input builder: up, held (the game's), or a press sent as a
/// pickup, held still (the game sees no Use until it is let go, as patch 78
/// keeps a consumed Use).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UseKey {
    Up,
    Held,
    Picked,
}

static USE_KEY: GameCell<UseKey> = GameCell::new(UseKey::Up);

/// One input's Use: on its down edge while a gun is the prompt's (and the
/// input carries no carry command of patch 78), the input's actions become
/// the pickup command (`weapon_pickup::encode`, Use and the quick bits out)
/// if the server's check (`weapon_pickup::sane`) passes it; then the press
/// stays the pickup's until Use is let go.
fn press(key: UseKey, aimed: Option<Aimed>, actions: u32) -> (UseKey, u32) {
    if actions & weapon_pickup::USE == 0 {
        return (UseKey::Up, actions);
    }
    match key {
        UseKey::Picked => (UseKey::Picked, actions & !weapon_pickup::USE),
        UseKey::Held => (UseKey::Held, actions),
        UseKey::Up => {
            let command = aimed.filter(|_| actions & weapon_pickup::CHORD == 0)
                .map(|a| (a, weapon_pickup::encode(actions, a.target)))
                .filter(|&(a, sent)| weapon_pickup::sane(sent, a.sender, a.places));
            match command {
                Some((_, sent)) => (UseKey::Picked, sent),
                None => (UseKey::Held, actions),
            }
        }
    }
}

/// The input builder's end (composed from artefact_carry's on_input_end,
/// after patch 78's chord step): Use's press on a gun is sent as its pickup.
pub fn on_input_end(handler: &mut InputHandler<'_>, thread: &Thread) {
    let actions = handler.actions().bits();
    let before = USE_KEY.get(thread);
    let aimed = AIMED.get(thread);
    let (key, sent) = press(before, aimed, actions);
    USE_KEY.set(thread, key);
    if sent != actions {
        handler.set_actions(ActionMask::from_bits(sent));
    }
    if let (UseKey::Up, UseKey::Picked, Some(a)) = (before, key, aimed) {
        log(&[Text("place "), Dec(u32::from(a.sender.get())), Text(" sends a pickup of the gun of place "),
              Dec(u32::from(a.target.get()))]);
    }
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

fn log(rest: &[Piece<'_>]) {
    log_joined(&[rest]);
}

fn log_borrow(what: &str, player: &Player<'_>, borrow: Borrow, why: &str) {
    log(&[Text("place "), Dec(u32::from(player.place())), Text(" at "), Dec(player.time()), Text(" ms "), Text(what),
          Text(": the gun of place "), Dec(u32::from(borrow.donor.get())), Text(" kit "), Dec(u32::from(borrow.kit.get())),
          Text(" slot "), Dec(u32::from(borrow.slot.slot())), Text(" in place of its slot "),
          Dec(u32::from(borrow.own.slot())), Text(if why.is_empty() { "" } else { " (" }), Text(why),
          Text(if why.is_empty() { "" } else { ")" })]);
}

/// `parts` as one line `pid P pickup: ...` of `srp_patches.log` (the first
/// 34 pieces: the line's room; written and the file closed at once).
fn log_joined(parts: &[&[Piece<'_>]]) {
    let mut pieces = [Text(""); 34];
    let head = [Text("pid "), Dec(gun_cook::process_id()), Text(" pickup: ")];
    let mut at = 0;
    for piece in head.iter().chain(parts.iter().flat_map(|part| part.iter())) {
        if let Some(slot) = pieces.get_mut(at) {
            *slot = *piece;
            at += 1;
        }
    }
    crate::vostok::log::line(&pieces[..at]);
}

/// A pickup's ammunition: the donor's magazine and chamber, then a line for
/// each borrowed stack placed (its slot, ammunition, count, the cooked slot
/// it came from) and the own stack parked from that slot.
fn log_installed(player: &Player<'_>, magazine: Option<(u8, bool)>, installed: &Installed) {
    let place = [Text("place "), Dec(u32::from(player.place())), Text(" pickup: ")];
    match magazine {
        Some((loaded, chamber)) => log_joined(&[&place, &[Text("the donor's magazine "), Dec(u32::from(loaded)),
                                                          Text(", chamber "), Dec(u32::from(chamber))]]),
        None => log_joined(&[&place, &[Text("the donor's gun not there: a full magazine")]]),
    }
    let dict = |s: native::StackSeen| s.dict_id.map_or(0xFFFF, u32::from);
    for p in installed.placed(player).iter().flatten() {
        let stack = [Text("slot "), Dec(u32::from(p.slot)), Text(" ammo "), Dec(dict(p.stack)), Text(" x "),
                     Dec(u32::from(p.stack.count)), Text(" (cooked slot "), Dec(u32::from(p.from)), Text(")")];
        match p.parked {
            Some(own) => log_joined(&[&place, &stack, &[Text("; own ammo "), Dec(dict(own)), Text(" x "),
                                                        Dec(u32::from(own.count)), Text(" parked")]]),
            None => log_joined(&[&place, &stack, &[Text("; no own stack there")]]),
        }
    }
}

/// A crate's refill of the borrowed gun: a line for each own ammunition of
/// its weapon slot in the kit (its bags), then one for each borrowed stack
/// (its count before and after; the loaded one with the target's inputs:
/// the bags, the rounds of its ammunition they carry, the magazine and
/// chamber inside).
fn log_refill(player: &Player<'_>, c: &native::Crate) {
    let place = [Text("place "), Dec(u32::from(player.place())), Text(" crate: ")];
    for o in c.own.iter().flatten() {
        let (clip, required, bags) = o.ammo.bags.map_or((0, 0, 0), |b| (b.clip_size(), b.required_bags(), b.bags(o.rounds)));
        log_joined(&[&place, &[Text("the weapon slot's own ammo: slot "), Dec(u32::from(o.slot)), Text(", the kit's "),
                               Dec(u32::from(o.rounds)), Text(" of ammo "), Dec(u32::from(o.ammo.dict_id)),
                               Text(" (clip "), Dec(clip), Text("/"), Dec(required), Text(") = "), Dec(bags),
                               Text(" bags")]]);
    }
    for s in c.stacks.iter().flatten() {
        let head = [Text("slot "), Dec(u32::from(s.slot)), Text(" ammo "), Dec(u32::from(s.ammo.dict_id)), Text(" "),
                    Dec(u32::from(s.before)), Text(" -> "), Dec(u32::from(s.after()))];
        match (s.refill, s.ammo.bags, s.held) {
            (Some(refill), Some(bags), Some((loaded, chamber))) => log_joined(&[&place, &head, &[
                Text("; the weapon slot's "), Dec(refill.bags), Text(" bags = "), Dec(u32::from(refill.total)),
                Text(" (clip "), Dec(bags.clip_size()), Text("/"), Dec(bags.required_bags()), Text(") less "),
                Dec(u32::from(loaded) + u32::from(chamber))]]),
            (_, None, Some(_)) => log_joined(&[&place, &head, &[Text(" (its config has no clip_size and required_bags: left)")]]),
            _ => log_joined(&[&place, &head, &[Text(" (not the gun's loaded stack: left)")]]),
        }
    }
}

fn log_source(what: &str, source: Source, origin: Origin, rest: &[Piece<'_>]) {
    let head = [Text(what), Text(" the gun of place "), Dec(u32::from(source.place.get())), Text(" kit "),
                Dec(u32::from(source.kit.get())), Text(" slot "), Dec(u32::from(source.slot.slot()))];
    let (died, n) = match origin {
        Origin::Death { died, .. } => ([Text(" (death at "), Dec(died.0), Text(" ms)")], 3),
        Origin::Loaded => ([Text(" (a loaded world's)"), Text(""), Text("")], 1),
    };
    log_joined(&[&head, &died[..n], rest]);
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];

    fn pose(transform: [f32; 16]) -> GunPose {
        // A rifle 1 m long along its model's z, 10 cm thick.
        GunPose::at(transform, GunBox::parse([-0.05, -0.05, -0.5, 0.05, 0.05, 0.5]).unwrap()).unwrap()
    }

    #[test]
    fn the_segment_enters_the_padded_box() {
        let mut at = IDENTITY;
        (at[12], at[13], at[14]) = (0.0, 0.2, 2.0);
        let gun = pose(at);
        // Looking along z from the origin: the box's near face is 1.5 m away.
        let hit = enters(&gun, [0., 0.2, 0.], [0., 0., 1.], REACH, PAD).unwrap();
        assert!((hit - (1.5 - PAD)).abs() < 1e-4, "{hit}");
        assert_eq!(enters(&gun, [0., 0.2, 0.], [0., 0., -1.], REACH, PAD), None, "behind");
        assert_eq!(enters(&gun, [0., 0.2, -1.], [0., 0., 1.], REACH, PAD), None, "beyond the reach");
        // Just off its side: 5 cm of padding catches 9 cm off the centre.
        assert!(enters(&gun, [0.09, 0.2, 0.], [0., 0., 1.], REACH, PAD).is_some());
        assert_eq!(enters(&gun, [0.11, 0.2, 0.], [0., 0., 1.], REACH, PAD), None);
        // Inside it: at once.
        assert_eq!(enters(&gun, [0., 0.2, 2.], [1., 0., 0.], REACH, PAD), Some(0.0));
    }

    #[test]
    fn the_segment_follows_the_guns_turn() {
        // The gun lying across x (turned a quarter about y: its z along x).
        let at = [0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., 1.0, 0.0, 0.0, 1.];
        let gun = pose(at);
        // Looking down at x = 1.4 (0.4 m along the gun from its centre).
        assert!(enters(&gun, [1.4, 1.0, 0.], [0., -1., 0.], REACH, PAD).is_some());
        assert_eq!(enters(&gun, [1.0, 1.0, 0.4], [0., -1., 0.], REACH, PAD), None, "its thickness is across z");
    }

    #[test]
    fn a_missed_gun_is_out_of_reach_or_not_aimed_at() {
        let mut at = IDENTITY;
        (at[12], at[13], at[14]) = (0.0, 0.2, 2.0);
        let gun = pose(at);
        // Its centre 2.01 m away, its box's half diagonal (padded) 0.6 m.
        assert!(within(&gun, [0., 0., 0.], REACH, PAD), "looking away from it in reach");
        assert!(within(&gun, [0., 0.2, -0.75], REACH, PAD), "its centre 2.75 m away");
        assert!(!within(&gun, [0., 0.2, -1.0], REACH, PAD));
        // Turned across x, at (1, 0, 0).
        let turned = pose([0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., 1.0, 0.0, 0.0, 1.]);
        assert!(within(&turned, [-1.5, 0., 0.], REACH, PAD) && !within(&turned, [-2.0, 0., 0.], REACH, PAD));
    }

    fn aimed(target: u8) -> Option<Aimed> {
        Some(Aimed { target: Place::new(target).unwrap(), sender: Place::new(3).unwrap(), places: 8 })
    }

    #[test]
    fn a_press_on_a_gun_is_sent_once_as_its_pickup() {
        let (fire, use_) = (0x20, weapon_pickup::USE);
        let (key, sent) = press(UseKey::Up, aimed(5), use_ | fire | 0x4000);
        assert_eq!(key, UseKey::Picked);
        assert_eq!(weapon_pickup::command(sent), Place::new(5));
        assert_eq!(sent & (use_ | fire), fire);
        // Held: the game sees no Use until it is let go.
        assert_eq!(press(key, aimed(5), use_ | fire), (UseKey::Picked, fire));
        assert_eq!(press(key, None, fire), (UseKey::Up, fire));
        // Held from before the gun was aimed at: the game's.
        assert_eq!(press(UseKey::Held, aimed(5), use_), (UseKey::Held, use_));
        assert_eq!(press(UseKey::Up, None, use_), (UseKey::Held, use_));
    }

    #[test]
    fn the_server_check_and_a_carry_command_keep_use_the_games() {
        let use_ = weapon_pickup::USE;
        assert_eq!(press(UseKey::Up, aimed(3), use_), (UseKey::Held, use_), "its own place");
        assert_eq!(press(UseKey::Up, aimed(9), use_), (UseKey::Held, use_), "no such place in the match");
        let chord = weapon_pickup::CHORD | 0x4000 | use_;
        assert_eq!(press(UseKey::Up, aimed(5), chord), (UseKey::Held, chord), "patch 78's carry command");
    }
}
