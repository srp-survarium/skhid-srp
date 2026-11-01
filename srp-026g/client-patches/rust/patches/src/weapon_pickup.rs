//! Patch 85, dropped-weapon pickup (docs/notes/weapon_pickup.md).
//!
//! The drop: at a death on every client (the same tick everywhere), when
//! the dead player held one of its two own guns, that gun alone is cooked
//! (`vostok::gun_cook`), the drop of that body (by place: a place has one
//! body). Untaken (never registered nor in hands), it is freed once its body
//! left the world; a copy ever registered or in hands is kept until its last
//! holder is destroyed, and re-used for its donor's next drop.
//!
//! What a match computes (the same on every client, in replays, rollbacks
//! and late joins):
//! - the record: each player's `weapon_pickup::Record` (its drop; for each
//!   of its weapon slots 7 and 10, the gun picked into it: its tick and its
//!   recipe) at its state's offset 2 (`pickup.save`, `pickup.load`); kept by
//!   place of the match ([`RECORDS`]); a record that does not decode is
//!   logged and read as empty;
//! - the pickup, at the input's apply point (artefact_carry's `carry.filter`
//!   calls [`on_filter`] with the raw actions first): the command, the rule
//!   (`weapon_pickup::decide`: the weapon slot in hands), the drop's gun
//!   (made, or waited for), registered under the picker's own pending id
//!   (`vostok::pickup::RegistryId::reserved`) as its pending object: the
//!   game's own switch holsters the gun in hands ([`on_tick_active`]: the
//!   pending object changed, a death or a removal cancel it, the gun back
//!   in its drop); at the switch ([`on_switch`]) the install
//!   (`vostok::pickup`): the slot's own gun and ammunition hidden (parked in
//!   the cooked inventory) and the gun in its place under the slot's
//!   reserved id, or a gun picked into the slot before let go for it; the
//!   gun is the picker's from then on;
//! - the restore: at `base_player::remove(1)` (death's respawn, departure)
//!   and the player's destruction each slot gets its own gun back and its
//!   picked gun is let go; a loaded record (a rollback, a late join) makes
//!   each slot what it names, before the native readers: an undone pickup's
//!   gun goes back to its body's drop, a named one is taken back from what
//!   was let go or cooked anew from its recipe;
//! - the crate: the native resupply and its checks see the own guns (a
//!   view); a picked gun's loaded stack is refilled by the game's rule;
//! - the drop's end: `base_player::remove(1)` of the place's player (its body
//!   and gun leave the world: the respawn, the departure) ends it.
//!
//! The local client (what this client draws and sends):
//! - the ragdoll's gun, at artefact_carry's `carry.weapon` ([`ragdoll_gun`]):
//!   an open drop's gun kept by place (its transform, its model's box), a
//!   taken drop's gun and a picked gun hidden as patch 78 hides a carrier's;
//! - the prompt, at artefact_carry's on_hud after patch 78's own targets
//!   ([`aim`]): the gun the local player looks at (2.2 m, its box padded by
//!   15 cm, in sight) that a pickup would take now, "Pick up <gun> (E)";
//! - the command, at the input builder's end after patch 78's chord step
//!   ([`on_input_end`]): Use's press on that gun sent as its pickup, if the
//!   server's check passes it.
//!
//! Every refusal is logged to `srp_patches.log` (with the process id), never
//! a crash.
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use core::ffi::CStr;

use world_frame::Frame;

use weapon_pickup::{Drop as DropState, Handedness, Hands, Ms, Parked, Pending, Pick, Picker, Place, Recipe, Record, WeaponSlot,
                    PLACES};

use crate::vostok::gun_cook::{self, CookId, Cooked, Gun, Refusal, TICKETS};
use crate::vostok::log::Piece::{self, Dec, Hex, Text};
use crate::vostok::pickup::{self as native, Failure, Fresh, GunBox, GunPose, Installed, Mode, NameKey, Registered,
                            RegistryId, Role, Select, Site};
use crate::vostok::world::{GameWorld, ItemId, Player, PlayerId, WorldKey};
use crate::vostok::{clock, ActionMask, Context, Flow, GameCell, GameWorldUi, InputHandler, Lock, NoExit, Thread, WorldLoad};
use crate::vostok::{WeaponPickupOnCanUseExit, WeaponPickupOnLoadQuietExit, WeaponPickupOnSaveQuietExit,
                    WeaponPickupOnSwitchExit, WeaponPickupOnUseHintExit};

/// The longest a wait runs the callbacks before it gives up (the pickup is
/// then refused, logged).
const WAIT_CAP_MS: u32 = 2_000;
const SLOTS: [WeaponSlot; 2] = [WeaponSlot::First, WeaponSlot::Second];

/// What the log knows of an entry's cook.
#[derive(Clone, Copy)]
enum Report {
    /// Asked; the address space then (MiB).
    Asked { memory: Option<u32> },
    /// Its outcome logged; the made gun's model box (read once here, on
    /// the logic thread: what the ragdoll's gun is matched against).
    Logged { model: Option<GunBox> },
}

/// A body's drop: its place, its player object (whose removal ends it) and
/// whether its body is still in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DropOf {
    place: Place,
    body: PlayerId,
    lies: Lies,
}

/// Where a drop's body is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lies {
    InWorld,
    /// Its player was taken out of the world (base_player::remove(1)) at
    /// that simulation time: a rollback can still bring it back (a loaded
    /// record with its drop open), so an untaken copy is kept
    /// [`KEEP_GONE_MS`] past it.
    Gone { at: Ms },
}

/// How long an untaken drop's copy outlives its body's removal: past any
/// rollback (a client's history is 550 ms; 128 items with
/// -debug_hash_mismatch, 6.4 s), so that a pickup replayed by a rollback
/// across the respawn finds it on every client and never cooks or waits in
/// a tick.
const KEEP_GONE_MS: u32 = 10_000;

/// Where a cooked gun is.
#[derive(Clone, Copy)]
enum At {
    /// A body's drop, not taken.
    Drop(DropOf),
    /// A pickup waiting for the weapon switch, within its deadline.
    Pending(Waiting),
    /// A pickup waiting for the weapon switch past [`SWITCH_DEADLINE_MS`]:
    /// the game is asked to look at the switch each tick (logged once, at
    /// the move here; runtime only).
    Overdue(Waiting),
    /// Picked into `picker`'s weapon slot `slot` (its own gun hidden in the
    /// cooked inventory); `from` the drop it was taken from, where a
    /// rollback undoing the pickup puts it back.
    Held { picker: PlayerId, slot: WeaponSlot, pick: Pick, installed: Installed, from: DropOf },
    /// Let go (its picker died or took another): idle, kept for a rollback
    /// that names it again or the donor's next drop (never freed mid-match).
    LetGo { picker: PlayerId, slot: WeaponSlot, pick: Pick, from: DropOf },
}

/// A gun registered as `picker`'s pending object, waiting for the weapon
/// switch into `pending.slot`; `from` the drop it was taken from, where a
/// cancel or a rollback undoing the pickup puts it back.
#[derive(Clone, Copy)]
struct Waiting {
    picker: PlayerId,
    pending: Pending,
    registered: Registered,
    from: DropOf,
}

/// A gun of ours: its cook (the kit and weapon slot of its recipe, the gun's
/// slot in the cooked inventory), where it is and what the log knows.
struct Entry {
    gun: Gun,
    recipe: Recipe,
    /// The player object it was cooked from (its next drops re-use it).
    donor: PlayerId,
    at: At,
    report: Report,
    history: History,
}

/// What a copy has been on this client: its lifetime follows (the owner's
/// keep-and-reuse model, 2026-11-01).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum History {
    /// Never registered nor in hands: an untaken drop's, freed once its
    /// body leaves the world (nothing of the game ever saw it).
    Fresh,
    /// Was registered as `holder`'s pending object.
    Registered { holder: PlayerId },
    /// Was in `holder`'s hands (installed): a loaded record re-installing it
    /// makes no weapon call of ours. Kept until its holder is destroyed.
    Used { holder: PlayerId },
}

impl History {
    /// The player whose destruction frees it (None: freed with its body's drop).
    fn holder(self) -> Option<PlayerId> {
        match self {
            History::Fresh => None,
            History::Registered { holder } | History::Used { holder } => Some(holder),
        }
    }
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
    world: WorldKey,
    record: Record,
    body: Option<Body>,
}

/// The records by place: what each player state carries (its drop stays
/// with the place when it respawns with another kit).
static RECORDS: Lock<[Option<Placed>; PLACES as usize]> = Lock::new([None; PLACES as usize]);

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
    /// The recipe's place has no player to cook from.
    NoDonor,
    Install(native::Refusal),
}

/// Why a pickup waiting for its switch was cancelled (its gun back in its drop).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cancel {
    Death,
    /// base_player::remove(1) (a respawn, a departure).
    Removal,
    Destruction,
    /// The install at the switch was refused.
    Refused(native::Refusal),
    /// A loaded record without it (a rollback undoing the pickup).
    Loaded,
}

impl Cancel {
    fn text(self) -> &'static str {
        match self {
            Cancel::Death => "its death",
            Cancel::Removal => "its removal (a respawn or a departure)",
            Cancel::Destruction => "its destruction",
            Cancel::Refused(refusal) => refusal.text(),
            Cancel::Loaded => "a loaded record without it",
        }
    }
}

impl Why {
    fn text(self) -> &'static str {
        use weapon_pickup::Refusal as R;
        match self {
            Why::NoPlace => "the player has no place, world or profile",
            Why::Rule(R::Dead) => "the picker is dead",
            Why::Rule(R::NoGunInHands) => "no gun of its two weapon slots in its hands",
            Why::Rule(R::Switching) => "a weapon switch under way",
            Why::Rule(R::NotAvailable) => "no drop to take (none, taken, its body gone, or its own)",
            Why::Rule(R::SecondWeaponDisabled) => "its kit carries one gun only, in slot 7 (modifier 64)",
            Why::Rule(R::TwoDoubleHanded) => "a second double-handed gun without modifier 25",
            Why::Gun(refused) => refused.text(),
            Why::NoDonor => "the place to cook it from has no player",
            Why::Install(refusal) => refusal.text(),
        }
    }
}

// ---- the records ------------------------------------------------------------------------------

fn record_at(thread: &Thread, world: WorldKey, place: Place) -> Record {
    placed_at(thread, world, place).0
}

/// The place's record and its last death's body.
fn placed_at(thread: &Thread, world: WorldKey, place: Place) -> (Record, Option<Body>) {
    RECORDS.with(thread, |records| match records[usize::from(place.get())] {
        Some(p) if p.world == world => (p.record, p.body),
        _ => (Record::EMPTY, None),
    })
}

fn update_placed(thread: &Thread, world: WorldKey, place: Place, f: impl FnOnce(&mut Record, &mut Option<Body>)) {
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

fn update_record(thread: &Thread, world: WorldKey, place: Place, f: impl FnOnce(&mut Record)) {
    update_placed(thread, world, place, |record, _| f(record));
}

/// The player's place and match.
fn located(player: &Player<'_>) -> Option<(WorldKey, Place)> {
    Some((player.world_key()?, Place::new(player.place())?))
}

/// What `player`'s weapon slot `slot` holds picked: its pick and install.
fn held(thread: &Thread, player: PlayerId, slot: WeaponSlot) -> Option<(CookId, Pick, Installed)> {
    CACHE.with(thread, |cache| cache.iter().flatten().find_map(|e| match e.at {
        At::Held { picker, slot: s, pick, installed, .. } if picker == player && s == slot => Some((e.gun.id(), pick, installed)),
        _ => None,
    }))
}

/// `player`'s pickup waiting for its switch: its cook, the pending pickup,
/// its registration and the drop it came from.
fn pending_of(thread: &Thread, player: PlayerId) -> Option<(CookId, Pending, Registered, DropOf)> {
    CACHE.with(thread, |cache| cache.iter().flatten().find_map(|e| match e.at {
        At::Pending(w) | At::Overdue(w) if w.picker == player => Some((e.gun.id(), w.pending, w.registered, w.from)),
        _ => None,
    }))
}

/// Cancels `player`'s pickup waiting for its switch: the gun unregistered
/// (the pending object, if still the gun, the held one again) and back in
/// its drop. Played in a tick (`reopen`), the drop's record is open again
/// while its body is in the world; a loaded record says what it is itself.
fn cancel(player: &Player<'_>, thread: &Thread, why: Cancel) {
    let Some((id, pending, registered, from)) = pending_of(thread, player.id()) else { return };
    native::unregister_pending(player, &registered);
    move_to(thread, id, |_| At::Drop(from));
    if let Some((world, place)) = located(player) {
        update_record(thread, world, place, |r| r.pending = None);
        let r = pending.pick.recipe;
        if why != Cancel::Loaded && from.lies == Lies::InWorld {
            update_record(thread, world, r.place, |d| {
                if d.drop == DropState::Taken {
                    d.drop = DropState::Open { kit: r.kit, slot: r.slot };
                }
            });
        }
    }
    log_pick("CANCELLED the switch to", player, pending.slot, pending.pick, why.text());
}

/// Whether the cook `id`'s gun was ever installed on this client.
fn used(thread: &Thread, id: CookId) -> bool {
    CACHE.with(thread, |cache| cache.iter().flatten().any(|e| e.gun.id() == id && matches!(e.history, History::Used { .. })))
}

/// The cook `id`'s gun is `holder`'s now: registered, or (`installed`) in
/// its hands. Kept until `holder` is destroyed.
fn mark_held(thread: &Thread, id: CookId, holder: PlayerId, installed: bool) {
    CACHE.with(thread, |cache| {
        if let Some(e) = cache.iter_mut().flatten().find(|e| e.gun.id() == id) {
            e.history = match (installed, e.history) {
                (true, _) | (false, History::Used { .. }) => History::Used { holder },
                (false, _) => History::Registered { holder },
            };
        }
    });
}

/// Moves the cook `id` to `at` (`f` of where it is).
fn move_to(thread: &Thread, id: CookId, f: impl FnOnce(At) -> At) {
    CACHE.with(thread, |cache| {
        if let Some(e) = cache.iter_mut().flatten().find(|e| e.gun.id() == id) {
            e.at = f(e.at);
        }
    });
}

/// Where a slot's picked gun goes when the slot gets its own gun back.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Unpick {
    /// Let go (its picker's destruction): kept until its holder is destroyed.
    LetGo,
    /// Let go at base_player::remove(1) (a respawn, a departure): put away
    /// first (remove would deactivate it after its restore); kept idle, for
    /// a rollback naming it or its donor's next drop.
    Removed,
    /// Back in the drop it came from (a rollback undoing its pickup).
    Undo,
}

/// `player`'s slot `slot` gets its own gun back; its picked gun goes as `how` says.
fn unpick(player: &Player<'_>, thread: &Thread, slot: WeaponSlot, how: Unpick, why: &str) {
    let Some((id, pick, installed)) = held(thread, player.id(), slot) else { return };
    if how != Unpick::LetGo {
        // Undo and Removed: out of the hands and the animation player before
        // it is moved (the native reader and remove then skip it).
        let done = native::put_away(player, &installed);
        log_gun(player, "put away", installed.gun(), Some(installed.id()), slot, done.text());
    }
    log_gun(player, if how == Unpick::Undo { "back to its drop" } else { "let go" }, installed.gun(),
            Some(installed.id()), slot, why);
    let outcome = native::restore(player, &installed);
    move_to(thread, id, |at| match at {
        At::Held { from, .. } if how == Unpick::Undo => At::Drop(from),
        At::Held { picker, slot, pick, from, .. } => At::LetGo { picker, slot, pick, from },
        other => other,
    });
    match outcome {
        Ok(()) => log_pick(if how == Unpick::Undo { "gave back to its drop" } else { "let go" }, player, slot, pick, why),
        Err(refusal) => log_pick("FAILED to restore", player, slot, pick, refusal.text()),
    }
}

/// `pickup.save` (base_player::serialize's start): the player's record.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, Site::Save) {
        let thread = ctx.thread();
        let located = located(&player);
        let mut record = located.map_or(Record::EMPTY, |(world, place)| placed_at(thread, world, place).0);
        record.parked = SLOTS.map(|slot| parked_now(&player, thread, &record, slot));
        // Kept as written (hashed): what the pick's end refunds (`on_remove`).
        if let Some((world, place)) = located {
            update_record(thread, world, place, |r| r.parked = record.parked);
        }
        native::write_record(ctx, &record.encode());
    }
    Flow::Continue
}

/// What `record`'s pick in `slot` keeps hidden of the own kit, as the save
/// writes it: as installed here (a loaded record's made so, `reconcile`); a
/// pick not installed here keeps the record's (the state then differs
/// anyway, logged).
fn parked_now(player: &Player<'_>, thread: &Thread, record: &Record, slot: WeaponSlot) -> Option<Parked> {
    let loaded = record.parked[usize::from(slot == WeaponSlot::Second)];
    record.pick(slot).and_then(|_| match held(thread, player.id(), slot) {
        Some((_, _, installed)) => native::parked(player, &installed),
        None => loaded,
    })
}

/// `pickup.load` (base_player::deserialize's start): the player's record,
/// then its weapon slots made what the record says before the native readers run.
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
    // A rollback to before its body's removal: its drop's copy in the world again.
    if matches!(record.drop, DropState::Open { .. }) {
        let body = player.id();
        CACHE.with(thread, |cache| {
            for e in cache.iter_mut().flatten() {
                if let At::Drop(d) = &mut e.at && d.body == body {
                    d.lies = Lies::InWorld;
                }
            }
        });
    }
    reconcile(&player, thread, record);
    Flow::Continue
}

/// Each weapon slot, and the pickup waiting for its switch, made what
/// `record` says: a picked or pending gun the record does not name goes back
/// to its drop (a rollback undoing its pickup), one it names is installed
/// (without the switch: the native reader switches the held object) or
/// registered (the reader resolves the pending id): taken back from what was
/// let go, or cooked anew from its recipe.
fn reconcile(player: &Player<'_>, thread: &Thread, record: Record) {
    let want = record.picks;
    let pending_differs = pending_of(thread, player.id()).map(|p| p.1) != record.pending;
    if pending_differs {
        cancel(player, thread, Cancel::Loaded);
    }
    let differs: [bool; 2] = SLOTS.map(|s| held(thread, player.id(), s).map(|h| h.1) != want[usize::from(s == WeaponSlot::Second)]);
    for (slot, _) in SLOTS.into_iter().zip(differs).filter(|&(_, d)| d) {
        let back_to_pending = held(thread, player.id(), slot)
            .is_some_and(|h| record.pending.is_some_and(|p| p.slot == slot && p.pick == h.1));
        unpick(player, thread, slot, Unpick::Undo, if back_to_pending {
            "a loaded record from its holster: pending again below"
        } else {
            "a loaded record without it"
        });
    }
    for (slot, _) in SLOTS.into_iter().zip(differs).filter(|&(_, d)| d) {
        let Some(pick) = want[usize::from(slot == WeaponSlot::Second)] else { continue };
        let outcome = (|| {
            let (id, from) = gun_for(player, thread, slot, pick)?;
            let at = RegistryId::reserved(Place::new(player.place()).ok_or(Why::NoPlace)?, Role::Picked(slot));
            let fresh = if used(thread, id) { Fresh::No } else { Fresh::Yes };
            let installed = native::install(player, slot, id, pick.recipe.slot, at, Mode::Load(fresh))
                .map_err(|f| Why::Install(f.refusal()))?;
            move_to(thread, id, |_| At::Held { picker: player.id(), slot, pick, installed, from });
            mark_held(thread, id, player.id(), true);
            log_gun(player, "a loaded record re-installed", installed.gun(), Some(installed.id()), slot,
                    if fresh == Fresh::No { "no weapon call (the reader set_users and activates it)" }
                    else { "fresh: set_user + initialize" });
            Ok::<_, Why>(())
        })();
        match outcome {
            Ok(()) => log_pick("a loaded record: installed", player, slot, pick, ""),
            Err(why) => log_pick("a loaded record's pick NOT installed (the state will not match)", player, slot, pick, why.text()),
        }
    }
    // The hidden own kit as the record has it (the picker's real one; a
    // client that loaded the record installed it over its own objects):
    // what each save writes back, and what the restore gives back.
    for slot in SLOTS {
        let (Some(parked), Some((_, pick, installed))) = (record.parked[usize::from(slot == WeaponSlot::Second)],
                                                          held(thread, player.id(), slot)) else { continue };
        if !native::apply_parked(player, &installed, parked) {
            log_pick("a loaded record's hidden own kit NOT as its record (the state will not match)", player, slot,
                     pick, "");
        }
    }
    let Some(pending) = record.pending.filter(|_| pending_differs) else { return };
    let outcome = (|| {
        let (id, from) = gun_for(player, thread, pending.slot, pending.pick)?;
        let at = RegistryId::reserved(Place::new(player.place()).ok_or(Why::NoPlace)?, Role::Pending);
        let registered = native::register_pending(player, id, pending.pick.recipe.slot, at, Select::ByReader)
            .map_err(Why::Install)?;
        move_to(thread, id, |_| At::Pending(Waiting { picker: player.id(), pending, registered, from }));
        mark_held(thread, id, player.id(), false);
        Ok::<_, Why>(registered)
    })();
    match outcome {
        Ok(registered) => log_registered("a loaded record: registered", player, pending, registered),
        Err(why) => log_pick("a loaded record's pending pickup NOT registered (the state will not match)", player,
                             pending.slot, pending.pick, why.text()),
    }
}
/// The gun a loaded record names for `player`'s `slot`: one this client has
/// ([`let_go`]). A world sent (0xE8: a late join, a resync) had the guns it
/// names that this client lacked cooked before its load
/// ([`before_world_load`]); a rollback's are this client's own. Else (a
/// replay's key frame, a cook that failed there) cooked anew from its recipe
/// and waited for, refused while a player was made since the last move
/// ([`Dispatch::Guarded`]: a late join's install must not run in a load);
/// so is one found still cooking (not made by [`before_world_load`]'s
/// deadline).
fn gun_for(player: &Player<'_>, thread: &Thread, slot: WeaponSlot, pick: Pick) -> Result<(CookId, DropOf), Why> {
    let (id, from) = match let_go(thread, player.id(), slot, pick) {
        Some(found) => found,
        None => {
            let cook_from = peer(player, pick.recipe.place).ok_or(Why::NoDonor)?;
            let from = DropOf { place: pick.recipe.place, body: cook_from.id(), lies: Lies::Gone { at: Ms(0) } };
            let id = ask(&cook_from, thread, pick.recipe, At::Drop(from))
                .ok_or(Why::Gun(Refused::NotAsked(Refusal::NoTicket)))?;
            (id, from)
        }
    };
    Ok((wait_for(thread, id, pick.recipe, Dispatch::Guarded, Deadline::from_now(thread)).map_err(Why::Gun)?, from))
}

/// A gun this client has for `player`'s `slot` that `pick` names: the one it
/// let go, else an untaken drop's of the same recipe (the same gun: a
/// rollback gave it back to its drop; or one cooked for the world being
/// loaded), else another idle copy of the recipe (let go by another picker).
fn let_go(thread: &Thread, player: PlayerId, slot: WeaponSlot, pick: Pick) -> Option<(CookId, DropOf)> {
    CACHE.with(thread, |cache| {
        let flat = || cache.iter().flatten();
        flat().find_map(|e| match e.at {
            At::LetGo { picker, slot: s, pick: p, from, .. } if picker == player && s == slot && p == pick => Some((e.gun.id(), from)),
            _ => None,
        }).or_else(|| flat().find_map(|e| match e.at {
            At::Drop(d) if e.recipe == pick.recipe => Some((e.gun.id(), d)),
            _ => None,
        })).or_else(|| flat().find_map(|e| match e.at {
            At::LetGo { from, .. } if e.recipe == pick.recipe => Some((e.gun.id(), from)),
            _ => None,
        }))
    })
}

/// The most picks a world can name: each place's two weapon slots and its
/// pickup waiting for its switch.
const NAMED: usize = 3 * PLACES as usize;

/// A world about to be loaded (0xE8: world_check's `world_load.unpacked` and
/// `world_load.raw`, once it parses; the handler process_dynamic_match_info,
/// outside any tick, before game_world_core::deserialize 0x2D0B70). Every
/// gun its players' records name that this client has no copy for (a late
/// joiner's first world: none) is cooked now from its recipe and waited for,
/// so that the records' readers ([`reconcile`] at `pickup.load`, inside the
/// load) find each one made ([`let_go`]) and never wait. A wait here runs the
/// game's callbacks as its frame's dispatch would ([`Dispatch::BeforeLoad`]):
/// a late join's install among them binds its slot before the world is
/// installed, between simulation updates, as at any frame's dispatch.
/// A named pick is one the reader keeps (the place's player holds it, or has
/// it pending, as the record says) or takes from the idle copies of its recipe
/// (in a drop or let go); the rest of each recipe is cooked.
pub fn before_world_load(load: &WorldLoad<'_>, frame: &Frame<'_>) {
    let thread = load.thread();
    let client = load.network_client();
    let mut named = [None; NAMED];
    let mut count = 0;
    let mut drops = [None; PLACES as usize];
    let mut dropped = 0;
    for (slot, state) in frame.states() {
        let Some(place) = Place::new(slot.index()) else { continue };
        let Some(record) = state.get(2..2 + Record::BYTES).and_then(|b| b.try_into().ok()).and_then(Record::decode)
        else { continue };
        // An open drop this client has no copy of (a death it did not see):
        // its gun, so that its pickup never cooks or waits in a tick.
        if let (DropState::Open { kit, slot }, None) = (record.drop, body_drop(thread, place))
            && let Some(body) = client.player(u32::from(place.get())) {
            let recipe = Recipe { place, kit, slot };
            let from = DropOf { place, body: body.id(), lies: Lies::InWorld };
            log_recipe("a world to load has the open drop", recipe, &[Text(", which this client has no copy of: \
                                                                            cooked before the load")]);
            if let (Some(id), Some(room)) = (ask(&body, thread, recipe, At::Drop(from)), drops.get_mut(dropped)) {
                *room = Some((id, recipe));
                dropped += 1;
            }
        }
        if record.picks.iter().all(Option::is_none) && record.pending.is_none() {
            continue;
        }
        let holder = client.player(u32::from(place.get())).map(|p| p.id());
        let keeps = |slot: WeaponSlot, pick: Pick| holder.is_some_and(|h| held(thread, h, slot).is_some_and(|(_, p, _)| p == pick));
        let mut needs = SLOTS.map(|slot| record.pick(slot).filter(|&pick| !keeps(slot, pick)).map(|pick| pick.recipe));
        let pending = record.pending
            .filter(|&pending| !holder.is_some_and(|h| pending_of(thread, h).is_some_and(|p| p.1 == pending)))
            .map(|pending| pending.pick.recipe);
        for recipe in needs.iter_mut().map(Option::take).chain([pending]).flatten() {
            if let Some(room) = named.get_mut(count) {
                *room = Some(recipe);
                count += 1;
            }
        }
    }
    let mut asked = [None; NAMED];
    let mut n = 0;
    for (k, recipe) in named.iter().enumerate() {
        let Some(recipe) = *recipe else { continue };
        // Each recipe once, at its first: how many it is named, how many idle copies there are.
        if named[..k].contains(&Some(recipe)) {
            continue;
        }
        let wanted = named.iter().filter(|&&r| r == Some(recipe)).count();
        let idle = CACHE.with(thread, |cache| cache.iter().flatten()
            .filter(|e| e.recipe == recipe && matches!(e.at, At::Drop(_) | At::LetGo { .. })).count());
        if wanted <= idle {
            continue;
        }
        let Some(donor) = client.player(u32::from(recipe.place.get())) else {
            log_recipe("a world to load names", recipe, &[Text(": its place has no player to cook it from (the reader \
                                                                 will not install it)")]);
            continue;
        };
        for _ in idle..wanted {
            let from = DropOf { place: recipe.place, body: donor.id(), lies: Lies::Gone { at: Ms(0) } };
            log_recipe("a world to load names", recipe, &[Text(", which this client has no idle copy of: cooked before \
                                                                 the load")]);
            if let (Some(id), Some(room)) = (ask(&donor, thread, recipe, At::Drop(from)), asked.get_mut(n)) {
                *room = Some((id, recipe));
                n += 1;
            }
        }
    }
    if n == 0 && dropped == 0 {
        return;
    }
    // One deadline for them all: the 0xE8 handler is held at most
    // WAIT_CAP_MS however many guns the world names (the server drops a
    // silent peer after 10 s).
    let deadline = Deadline::from_now(thread);
    for (id, recipe) in asked.iter().flatten().chain(drops.iter().flatten()) {
        if let Err(refused) = wait_for(thread, *id, *recipe, Dispatch::BeforeLoad, deadline) {
            log_recipe("NOT made before the world's load:", *recipe, &[
                Text(" ("), Text(refused.text()), Text("; the record's reader waits for it, refused while a player \
                                                           was made since the last move)")]);
        }
    }
}

/// `pickup.remove` (base_player::remove's entry): the removal of the
/// place's player from the world (flag 1: the respawn tick's 0x3169A0, a
/// departure's 0x316BE0, a snapshot's kit change 0x2D2A68 before the new
/// kit's state is read) ends its life's pickup state. Each weapon slot gets
/// its own gun back first (so its unload charges the own profile; a picked
/// gun held stays held, and remove deactivates it), its picked gun let go.
/// A dead player's body, and its gun, leave the world here: its drop ends
/// (the prompt and the gun on the body agree on every client); that death's
/// gun is freed if nothing ever registered it, else kept for re-use.
pub fn on_remove(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !native::removing(ctx) {
        return Flow::Continue;
    }
    let Some(player) = native::player(ctx, Site::Remove) else { return Flow::Continue };
    let thread = ctx.thread();
    cancel(&player, thread, Cancel::Removal);
    // What the unload then refunds of each pick's own kit: the parked block
    // the saves wrote (hashed in every game), not whatever objects are where.
    let parked = located(&player).map(|(world, place)| {
        let record = record_at(thread, world, place);
        SLOTS.map(|slot| record.pick(slot).and_then(|_| record.parked[usize::from(slot == WeaponSlot::Second)])
            .or_else(|| parked_now(&player, thread, &record, slot)))
    });
    for slot in SLOTS {
        unpick(&player, thread, slot, Unpick::Removed, "its removal (a respawn or a departure)");
    }
    for (slot, parked) in SLOTS.into_iter().zip(parked.unwrap_or([None; 2])) {
        if let Some(parked) = parked {
            native::unload_as_parked(&player, slot, parked);
        }
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
    // Its drop's gun, and one taken from it (a rollback can give it back).
    let body = player.id();
    CACHE.with(thread, |cache| {
        for e in cache.iter_mut().flatten() {
            let (At::Drop(d) | At::Pending(Waiting { from: d, .. }) | At::Overdue(Waiting { from: d, .. })
                 | At::Held { from: d, .. } | At::LetGo { from: d, .. }) = &mut e.at;
            if d.body == body {
                d.lies = Lies::Gone { at: Ms(player.time()) };
            }
        }
    });
    Flow::Continue
}

// ---- the death and the pickup -----------------------------------------------------------------

/// The death of a player (composed from artefact_carry's `carry.client_death`,
/// the player still intact): its drop is the gun it held if one of its own
/// (its cook asked for, unless its body's is there: a rollback replaying the
/// death); none when it held a picked gun (it vanishes) or anything else.
pub fn died(player: &Player<'_>, thread: &Thread) {
    log(&[Text("death hook: place "), Dec(u32::from(player.place())), Text(" life "), Dec(player.spawned()),
          Text(" at "), Dec(player.time()), Text(" ms")]);
    // The only cancel of a pickup waiting for its switch (the gun in hands drawn again).
    cancel(player, thread, Cancel::Death);
    let gun = gun_cook::held_gun(player).filter(|&slot| held(thread, player.id(), slot).is_none());
    let drop = match (gun, player.kit()) {
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
    let (DropState::Open { kit, slot }, Some(place)) = (drop, Place::new(player.place())) else { return };
    if body_drop(thread, place).is_none() {
        let from = DropOf { place, body: player.id(), lies: Lies::InWorld };
        let recipe = Recipe { place, kit, slot };
        match reuse(thread, player.id(), recipe, from) {
            Some(gun) => log_recipe("re-using a kept copy of", recipe, &[Text(": gun "), Hex(gun.address())]),
            None => {
                ask(player, thread, recipe, At::Drop(from));
            }
        }
    }
}

/// An idle kept copy of `recipe` cooked from `donor` (let go, or back in a
/// drop whose body left the world: in no one's hands, pending or live drop)
/// becomes the drop `from`'s gun; its address. Its magazine and ammunition
/// are set again by the pickup's switch (Ammo::grant), as for a new cook's.
fn reuse(thread: &Thread, donor: PlayerId, recipe: Recipe, from: DropOf) -> Option<ItemId> {
    let id = CACHE.with(thread, |cache| {
        let e = cache.iter_mut().flatten().find(|e| e.donor == donor && e.recipe == recipe && e.history != History::Fresh
            && matches!(e.at, At::LetGo { .. } | At::Drop(DropOf { lies: Lies::Gone { .. }, .. })))?;
        e.at = At::Drop(from);
        Some(e.gun.id())
    })?;
    native::gun_id(thread, id, recipe.slot)
}

/// The cook of the drop of `place`'s body in the world, if this client has it.
fn body_drop(thread: &Thread, place: Place) -> Option<(CookId, Recipe, DropOf)> {
    CACHE.with(thread, |cache| cache.iter().flatten().find_map(|e| match e.at {
        At::Drop(d) if d.place == place && d.lies == Lies::InWorld => Some((e.gun.id(), e.recipe, d)),
        _ => None,
    }))
}

/// The input's apply point (composed from artefact_carry's `carry.filter`,
/// before patch 78 filters the actions): a pickup command is played.
pub fn on_filter(player: &Player<'_>, thread: &Thread) {
    nudge(player, thread);
    if let Some(target) = weapon_pickup::command(player.input_actions()) {
        play(player, thread, target);
    }
}

/// The input's fire (0x20) and reload (0x40) bits.
const FIRE_AND_RELOAD: u32 = 0x20 | 0x40;

/// The owner (2026-11-01): E interrupts the firing. From the pickup's
/// command to its swap the player's fire and reload are cleared from the
/// actions as they are applied (keyed on the synced record's pending
/// pickup: every client, every replay alike). The held gun then ends its
/// current shot (the game's fire -> hide waits for the shot's animation,
/// 0.10b weapon_core.cpp 193/210), starts no other and no reload (reload ->
/// hide does not wait, line 185), is ready to be put away, and the game's
/// switch holsters it and draws the picked one. After the swap the actions
/// are the player's again. Composed from artefact_carry's `carry.filter`
/// after patch 78's filter, which writes the whole mask.
pub fn hold_fire(player: &Player<'_>, thread: &Thread) {
    let Some((world, place)) = located(player) else { return };
    if record_at(thread, world, place).pending.is_some() {
        player.suppress_actions(FIRE_AND_RELOAD);
    }
}

/// A pickup command played (on the input's apply point).
fn play(player: &Player<'_>, thread: &Thread, target: Place) {
    match take(player, thread, target) {
        Ok((pending, registered)) => {
            log_registered("took (the gun in hands holstering)", player, pending, registered);
            log_gun(player, "registered pending", registered.gun(), Some(registered.id()), pending.slot, "");
        }
        Err(why) => log(&[Text("place "), Dec(u32::from(player.place())), Text(" asked for the gun of place "),
                          Dec(u32::from(target.get())), Text(" at "), Dec(player.time()), Text(" ms: refused, "),
                          Text(why.text())]),
    }
}

/// A pickup played (the owner: committed at the command, the holster only
/// its animation): every check of its install made now, the drop taken,
/// the gun the picker's pending object under its own registry id; the
/// game's switch holsters the gun in hands and [`on_switch`] installs it.
fn take(player: &Player<'_>, thread: &Thread, target: Place) -> Result<(Pending, Registered), Why> {
    let (world, place) = located(player).ok_or(Why::NoPlace)?;
    let theirs = record_at(thread, world, target);
    let picker = picker_of(player, place).ok_or(Why::NoPlace)?;
    // The rule without the gun first (a pistol refuses least): no cook is
    // waited for a refusal.
    weapon_pickup::decide(picker, target, theirs.drop, |_, _| Handedness::One).map_err(Why::Rule)?;
    let DropState::Open { kit, slot } = theirs.drop else { return Err(Why::Rule(weapon_pickup::Refusal::NotAvailable)) };
    let body = peer(player, target).ok_or(Why::NoDonor)?;
    let recipe = Recipe { place: target, kit, slot };
    let (cook, from) = match body_drop(thread, target) {
        Some((id, _, from)) => (wait_for(thread, id, recipe, Dispatch::Guarded, Deadline::from_now(thread)).map_err(Why::Gun)?, from),
        // A death this client did not see (it joined after): cooked now.
        None => {
            let from = DropOf { place: target, body: body.id(), lies: Lies::InWorld };
            let id = ask(&body, thread, recipe, At::Drop(from)).ok_or(Why::Gun(Refused::NotAsked(Refusal::NoTicket)))?;
            (wait_for(thread, id, recipe, Dispatch::Guarded, Deadline::from_now(thread)).map_err(Why::Gun)?, from)
        }
    };
    let incoming = native::handedness(thread, cook, slot).ok_or(Why::Install(native::Refusal::NoCookedGun))?;
    let into = weapon_pickup::decide(picker, target, theirs.drop, |_, _| incoming).map_err(Why::Rule)?;
    let magazine = (body.kit() == Some(kit)).then(|| body.magazine(slot)).flatten();
    // Everything the switch's install needs, checked now: the switch cannot be refused.
    let replaced = held(thread, player.id(), into).map(|h| h.2);
    let ids = (RegistryId::reserved(place, Role::Picked(into)), RegistryId::reserved(place, Role::Pending));
    native::can_install(player, into, cook, slot, ids, replaced.as_ref()).map_err(Why::Install)?;
    let registered = native::register_pending(player, cook, slot, ids.1, Select::Now).map_err(Why::Install)?;
    // As a weapon key: the switch is looked at this tick (the hide starts now).
    player.select_animations_now();
    let pending = Pending { slot: into, pick: Pick { taken: Ms(player.time()), recipe }, magazine };
    move_to(thread, cook, |_| At::Pending(Waiting { picker: player.id(), pending, registered, from }));
    mark_held(thread, cook, player.id(), false);
    update_record(thread, world, place, |r| r.pending = Some(pending));
    update_record(thread, world, target, |r| r.drop = DropState::Taken);
    Ok((pending, registered))
}

/// How long a pickup waits for the game's switch before it asks the game,
/// every tick, to look at it again (m_need_to_select_animations): the
/// fallback for a switch the game was never asked to look at. The swap is
/// still the game's own, when the gun in hands is ready to be put away
/// (is_ready_to_be_deactivated): never forced. A gun firing is not ready
/// (the game's fire -> hide transition waits for the trigger and its
/// animation, as for a 1/2 key), and a gun put away before it is ready
/// keeps animation intervals forget_active_object does not drop: their
/// events reached it with no user (the 83Lb41 crashes, both after a forced
/// swap at this deadline).
const SWITCH_DEADLINE_MS: u32 = 3_000;

/// `pickup.tick_active` (tick_active_object's entry, each player each
/// tick): a pickup waiting for its switch keeps the picked gun the pending
/// object (a weapon key, or anything else that set another, is ignored until
/// the swap: the pending object and the inventory's selected slot +170h are
/// put back).
pub fn on_tick_active(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(player) = native::player(ctx, Site::TickActive) else { return Flow::Continue };
    let Some((_, pending, registered, _)) = pending_of(thread, player.id()) else { return Flow::Continue };
    if !player.pending_is(&registered) {
        native::select_pending(&player, &registered, pending.slot);
        log_pick("ignored another weapon switch while holstering for", &player, pending.slot, pending.pick, "");
    }
    Flow::Continue
}

/// At the input's apply point, each tick: a pickup waiting for its switch
/// past [`SWITCH_DEADLINE_MS`] asks the game to look at the switch this tick
/// (m_need_to_select_animations, as a changed input does); logged once.
fn nudge(player: &Player<'_>, thread: &Thread) {
    let Some((cook, pending, _, _)) = pending_of(thread, player.id()) else { return };
    if Ms(player.time()).since(pending.pick.taken) < SWITCH_DEADLINE_MS {
        return;
    }
    player.select_animations_now();
    let mut first = false;
    move_to(thread, cook, |at| match at {
        At::Pending(waiting) => {
            first = true;
            At::Overdue(waiting)
        }
        other => other,
    });
    if first {
        log_pick("still waiting for the gun in hands to be put away (no switch within the deadline; the game is \
                  asked to look at it each tick, never forced) for", player, pending.slot, pending.pick, "");
    }
}

/// `pickup.switch` (tick_active_object's switch: the gun in hands is
/// holstered): the picked gun pending takes its slot, the switch's calls
/// made here; on a defensive refusal the pickup is cancelled and the native
/// switch draws the gun in hands again.
pub fn on_switch(ctx: &mut Context<'_>) -> Flow<WeaponPickupOnSwitchExit> {
    let thread = ctx.thread();
    let Some(player) = native::player(ctx, Site::Switch) else { return Flow::Continue };
    match switch(&player, thread) {
        Some(true) => Flow::Exit(WeaponPickupOnSwitchExit::Switched),
        Some(false) | None => Flow::Continue,
    }
}

/// `player`'s pickup waiting for its switch installed now: Some(true)
/// switched (or, refused after the gun in hands was put away, that gun drawn
/// again: done either way), Some(false) refused before anything changed
/// (cancelled: the held object stays, the pending one is it again; the
/// native switch draws it), None no pickup waits.
fn switch(player: &Player<'_>, thread: &Thread) -> Option<bool> {
    let (cook, pending, registered, from) = pending_of(thread, player.id())?;
    let (world, place) = located(player)?;
    let replaced = held(thread, player.id(), pending.slot);
    let mode = match &replaced {
        Some((_, _, installed)) => Mode::Replace(pending.magazine, installed, &registered),
        None => Mode::Switch(pending.magazine, &registered),
    };
    let id = RegistryId::reserved(place, Role::Picked(pending.slot));
    let installed = match native::install(player, pending.slot, cook, pending.pick.recipe.slot, id, mode) {
        Ok(installed) => installed,
        Err(Failure::Refused(refusal)) => {
            // Nothing changed: the native switch draws the gun in hands again.
            cancel(player, thread, Cancel::Refused(refusal));
            return Some(false);
        }
        Err(Failure::Redrawn(refusal)) => {
            // Drawn again here: the native switch must not run.
            cancel(player, thread, Cancel::Refused(refusal));
            return Some(true);
        }
    };
    log_gun(player, "installed at the switch (put away first: deactivate(1) + forget_active_object)", installed.gun(),
            Some(installed.id()), pending.slot, "");
    log_gun(player, "out of hands at the switch", replaced.as_ref().map_or(installed.own_gun(), |r| r.2.gun()),
            replaced.as_ref().map(|r| r.2.id()), pending.slot, if replaced.is_some() { "the replaced pick, let go" }
            else { "the own gun, hidden" });
    mark_held(thread, cook, player.id(), true);
    if let Some((old, pick, _)) = replaced {
        move_to(thread, old, |at| match at {
            At::Held { picker, slot, pick, from, .. } => At::LetGo { picker, slot, pick, from },
            other => other,
        });
        log_pick("let go, replaced", player, pending.slot, pick, "");
    }
    move_to(thread, cook, |_| At::Held { picker: player.id(), slot: pending.slot, pick: pending.pick, installed, from });
    update_record(thread, world, place, |r| {
        r.set_pick(pending.slot, Some(pending.pick));
        r.pending = None;
    });
    log_pick("switched (the gun in hands holstered)", player, pending.slot, pending.pick, "");
    log_installed(player, pending.magazine, &installed);
    Some(true)
}

/// The place's current player, seen from `player`'s world.
fn peer<'a>(player: &'a Player<'_>, place: Place) -> Option<Player<'a>> {
    player.carry_peer(artefact_carry::Place::parse(place.get())?)
}

/// What `player` is for the rule: alive, the slot of the gun in its hands,
/// the other slot's gun, its kit's carry rule (None: no profile).
fn picker_of(player: &Player<'_>, place: Place) -> Option<Picker> {
    let hands = player.hands();
    let other = match hands {
        Hands::Gun(slot) => player.weapon_handedness(slot.other()),
        Hands::Switching | Hands::NoGun => None,
    };
    Some(Picker { place, alive: player.alive(), hands, other, rule: player.carry_rule()? })
}

// ---- the cooked guns --------------------------------------------------------------------------

/// Asks for `recipe`'s gun from `player`'s profile and keeps it at `at`.
fn ask(player: &Player<'_>, thread: &Thread, recipe: Recipe, at: At) -> Option<CookId> {
    let memory = gun_cook::memory(thread).map(|(used, _)| used);
    let gun = match gun_cook::start(player, recipe.kit, recipe.slot) {
        Ok(gun) => gun,
        Err(refusal) => {
            log_recipe("FAILED to ask for", recipe, &[Text(": "), Text(refusal.text())]);
            return None;
        }
    };
    let id = gun.id();
    let entry = Entry { gun, recipe, donor: player.id(), at, report: Report::Asked { memory }, history: History::Fresh };
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
    log_recipe(if kept { "asked for" } else { "no room for" }, recipe, &memory_pieces(memory));
    kept.then_some(id)
}

/// The model box of the made cook of the drop of `place`'s body.
fn drop_box(thread: &Thread, place: Place) -> Option<GunBox> {
    CACHE.with(thread, |cache| cache.iter().flatten().find_map(|e| match (e.at, e.report) {
        (At::Drop(d), Report::Logged { model }) if d.place == place && d.lies == Lies::InWorld => model,
        _ => None,
    }))
}

/// Where a wait for a cook runs the game's callbacks (`dispatch_callbacks`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dispatch {
    /// In a tick (the pickup's command) or inside a world's load (a loaded
    /// record): refused while a player was made since the last move, a late
    /// join's install (which must run between simulation updates, outside a
    /// load) maybe among the callbacks.
    Guarded,
    /// In the 0xE8 handler before its load ([`before_world_load`]): outside
    /// any tick and any load, where the frame's own dispatch could have run
    /// them all.
    BeforeLoad,
}

/// When waits give up: [`WAIT_CAP_MS`] after their start (one wait's, or
/// shared by several).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Deadline {
    since: u64,
}

impl Deadline {
    fn from_now(thread: &Thread) -> Deadline {
        Deadline { since: clock::counter(thread) }
    }

    fn passed(self, thread: &Thread) -> bool {
        elapsed_ms(thread, self.since, clock::counter(thread)) >= WAIT_CAP_MS
    }
}

fn wait_for(thread: &Thread, id: CookId, recipe: Recipe, how: Dispatch, deadline: Deadline) -> Result<CookId, Refused> {
    let start = clock::counter(thread);
    let mut waited = false;
    let outcome = loop {
        match gun_cook::cooked(thread, id) {
            Cooked::Made { .. } => break Ok(id),
            Cooked::Failed { .. } => break Err(Refused::Failed),
            Cooked::Gone => break Err(Refused::Missing),
            Cooked::Cooking { .. } => {}
        }
        if how == Dispatch::Guarded && PLAYER_MADE.load(Ordering::Acquire) {
            break Err(Refused::LateJoin);
        }
        if deadline.passed(thread) {
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
        log_recipe("waited for", recipe, &[Text(": "), Dec(ms), Text(" ms, "), Text(what)]);
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
                let address = native::gun_id(thread, entry.gun.id(), entry.recipe.slot).map_or(0, ItemId::address);
                log_recipe("freeing the cook of", entry.recipe, &[Text(": gun "), Hex(address),
                    Text(match entry.history {
                        History::Used { .. } => " (was in hands)",
                        History::Registered { .. } => " (was registered)",
                        History::Fresh => " (never in hands)",
                    })]);
                drop(entry);
            }
            None => break,
        }
    }
}

/// Once a frame after the simulation's move (composed from ragdoll's
/// `game_world.after_move`): a late join's install has had its frame's
/// callbacks; finished cooks are logged; a drop's gun that was never
/// registered nor in hands is freed once its body left the world (nothing
/// of the game ever saw it). No other copy is freed mid-match.
pub fn after_move(ctx: &Context<'_>) {
    let thread = ctx.thread();
    PLAYER_MADE.store(false, Ordering::Release);
    log_finished(thread);
    let world = GameWorld::at_after_move(ctx);
    if let Some(key) = world.as_ref().and_then(gun_cook::world_key) {
        diagnose_world(thread, key);
    }
    let Some(now) = world.as_ref().and_then(gun_cook::world_time).map(Ms) else { return };
    release_where(thread, |e| e.history == History::Fresh && matches!(e.at,
        At::Drop(DropOf { lies: Lies::Gone { at }, .. }) if now.since(at) >= KEEP_GONE_MS));
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
                    .then(|| native::gun_box(thread, e.gun.id(), e.recipe.slot)).flatten();
                e.report = Report::Logged { model };
                Some((e.gun.id(), e.recipe, memory, cooked, model.is_some()))
            })
        });
        let Some((id, recipe, before, cooked, boxed)) = found else { break };
        let after = gun_cook::memory(thread).map(|(used, _)| used);
        let (what, inventory, ms) = match cooked {
            Cooked::Made { inventory, since, at } => ("made", inventory.address(), elapsed_ms(thread, since, at)),
            Cooked::Failed { since, at } => ("FAILED (empty result)", 0, elapsed_ms(thread, since, at)),
            Cooked::Cooking { .. } | Cooked::Gone => ("given back before it was made", 0, 0),
        };
        let address = native::gun_id(thread, id, recipe.slot).map_or(0, ItemId::address);
        log_recipe("cook of", recipe, &[
            Text(": "), Text(what), Text(", inventory "), Hex(inventory), Text(", gun "), Hex(address), Text(", "), Dec(ms),
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
/// its slots get their own guns back first; its body's untaken drop goes.
pub fn on_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(player) = native::player(ctx, Site::Destroy) {
        cancel(&player, thread, Cancel::Destruction);
        for slot in SLOTS {
            unpick(&player, thread, slot, Unpick::LetGo, "its destruction");
        }
        if let Some((world, place)) = located(&player) {
            update_record(thread, world, place, |r| r.picks = [None; 2]);
        }
        // Its untaken drops' guns, and every copy it was the last holder of
        // (let go, or back in a drop): the game destroys its own objects
        // with it; nothing of it refers to them after.
        let gone = player.id();
        release_where(thread, |e| match (e.history.holder(), e.at) {
            (None, At::Drop(d)) => d.body == gone,
            (Some(holder), At::LetGo { .. } | At::Drop(_)) => holder == gone,
            _ => false,
        });
        if CACHE.with(thread, |cache| cache.iter().all(Option::is_none)) {
            diagnose("the last copy freed (the match's end)", thread, true);
        }
    }
    Flow::Continue
}

/// The local diagnostics (a diagnostics build only): the survarium and ai
/// arenas' allocated bytes and the copies kept, beside the owner's bound
/// (TICKETS: 180 copies, about 9.5 MiB of the survarium arena).
fn diagnose(what: &str, thread: &Thread, statistics: bool) {
    if !crate::vostok::trace::enabled() {
        return;
    }
    let (survarium, ai) = gun_cook::arena_usage(thread);
    let copies = CACHE.with(thread, |cache| cache.iter().flatten().count() as u32);
    log(&[Text("memory ("), Text(what), Text("): survarium arena "), Dec(survarium >> 10), Text(" KiB, ai arena "),
          Dec(ai >> 10), Text(" KiB, copies kept "), Dec(copies), Text(" of "), Dec(TICKETS as u32)]);
    if statistics {
        gun_cook::dump_statistics(thread);
    }
}

/// The world of the last move, for the diagnostics' log at a match's load.
static WORLD_SEEN: GameCell<Option<WorldKey>> = GameCell::new(None);

fn diagnose_world(thread: &Thread, world: WorldKey) {
    if WORLD_SEEN.get(thread) != Some(world) {
        WORLD_SEEN.set(thread, Some(world));
        diagnose("a match loaded", thread, false);
    }
}

// ---- the crate --------------------------------------------------------------------------------

/// The player at `site` and its slots' installs, if a gun is picked into one.
fn picked_at<'t>(ctx: &'t Context<'_>, site: Site) -> Option<(Player<'t>, [Option<Installed>; 2])> {
    let player = native::player(ctx, site)?;
    let installs = SLOTS.map(|s| held(ctx.thread(), player.id(), s).map(|h| h.2));
    installs.iter().any(Option::is_some).then_some((player, installs))
}

/// `pickup.can_use`: a picker's crate is available while a picked gun's
/// stack is below its refill; else the native check sees the own loadout.
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
    let Some((player, installs)) = picked_at(ctx, site) else { return Check::Native };
    if installs.iter().flatten().any(|i| native::reserve_low(&player, i)) {
        return Check::Available;
    }
    let Some(view) = native::crate_view(&player, &installs, ctx.thread()) else { return Check::Native };
    native::hand_view(ctx, site, view);
    Check::View
}

/// `pickup.resupply`: the crate's use refills the picked guns by the game's
/// rule for free; the native resupply sees the own loadout.
pub fn on_resupply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    resupply(ctx, Site::Resupply)
}

/// `pickup.client_resupply`: the client's own finish_use, the same.
pub fn on_client_resupply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    resupply(ctx, Site::ClientResupply)
}

fn resupply(ctx: &mut Context<'_>, site: Site) -> Flow<NoExit> {
    let Some((player, installs)) = picked_at(ctx, site) else { return Flow::Continue };
    for installed in installs.iter().flatten() {
        log_refill(&player, &native::refill(&player, installed));
    }
    let Some(view) = native::crate_view(&player, &installs, ctx.thread()) else { return Flow::Continue };
    native::hand_view(ctx, site, view);
    Flow::Skip
}

// ---- stage 2b: the ragdoll's gun, the prompt, the command --------------------------------------

/// The prompt's reach (patch 78's) and how much larger than the gun's model
/// box its target is (metres).
const REACH: f32 = 2.2;
const PAD: f32 = 0.15;

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
/// picked gun in hands vanishes with its holder; the gun of a drop taken is hidden
/// on its donor's ragdoll; the gun of an open drop is kept for the prompt
/// (the site's transform and its cooked gun's box), the body being that
/// drop's death's. Nothing of the body's own objects is read.
pub fn ragdoll_gun(ctx: &Context<'_>, player: &Player<'_>) -> RagdollGun {
    let thread = ctx.thread();
    if picked_in_hands(thread, player) {
        return RagdollGun::Hidden;
    }
    let Some((world, place)) = located(player) else { return RagdollGun::Shown };
    let body = Body { player: player.id(), spawned: player.spawned() };
    let (record, last) = placed_at(thread, world, place);
    if last != Some(body) {
        return RagdollGun::Shown;
    }
    match record.drop {
        // Taken: hidden, and nothing left to aim at there.
        DropState::Taken => {
            SEEN.with(thread, |seen| seen[usize::from(place.get())] = None);
            RagdollGun::Hidden
        }
        DropState::Open { .. } => {
            let pose = drop_box(thread, place).and_then(|model| native::drawn_gun(ctx, model));
            SEEN.with(thread, |seen| seen[usize::from(place.get())] = pose.map(|pose| Seen { body, pose }));
            RagdollGun::Shown
        }
        DropState::None => RagdollGun::Shown,
    }
}

/// Whether `player` holds a picked gun in its hands (its ragdoll drops none).
pub fn picked_in_hands(thread: &Thread, player: &Player<'_>) -> bool {
    matches!(player.hands(), Hands::Gun(slot) if held(thread, player.id(), slot).is_some())
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
/// drew it: the eye's 2.2 m segment enters its model box (padded by 15 cm),
/// nothing of the level in between (patch 78's ray), the drop open (its body
/// still in the world), its gun made on this client and the rule's
/// (`weapon_pickup::decide`, the carry rules too) yes. Remembered for Use
/// ([`on_input_end`]).
pub fn aim(ctx: &Context<'_>, viewer: Option<&Player<'_>>) -> Option<Prompt> {
    let found = viewer.and_then(|viewer| aimed_at(ctx, viewer));
    AIMED.set(ctx.thread(), found.and_then(|(aimed, _)| aimed));
    found.map(|(_, prompt)| prompt)
}

/// What the prompt shows for the gun aimed at (in reach and in sight).
#[derive(Clone, Copy)]
pub enum Prompt {
    /// "Pick up <gun> (E)": Use takes it (its name once its cook is made
    /// here; until then the pickup waits for it).
    Take(NameKey),
    /// Why a pickup cannot happen: a dictionary key (Use does nothing).
    Cannot(&'static CStr),
}

/// The line for each refusal of the rule: none is silent.
fn refusal_key(why: weapon_pickup::Refusal) -> &'static CStr {
    use weapon_pickup::Refusal as R;
    match why {
        R::TwoDoubleHanded => c"st_weapon_cannot_carry",
        R::SecondWeaponDisabled => c"st_weapon_one_gun_only",
        R::NoGunInHands => c"st_weapon_no_gun_in_hands",
        R::Switching => c"st_weapon_switching",
        // A dead viewer has no prompt; the own place is never aimed at.
        R::Dead | R::NotAvailable => c"st_weapon_cannot_pick_up",
    }
}

/// The line for a gun aimed at that cannot be taken.
fn cannot_key(why: Sight) -> &'static CStr {
    match why {
        Sight::Refused { why, .. } => refusal_key(why),
        Sight::Taken => c"st_weapon_already_taken",
        // Not a drop the prompt can name (and the untargeted ones, never here).
        Sight::NotItsBody | Sight::NoDrop | Sight::NoPicker | Sight::OutOfReach | Sight::NotAimedAt
        | Sight::Blocked | Sight::Offered => c"st_weapon_cannot_pick_up",
    }
}

fn aimed_at(ctx: &Context<'_>, viewer: &Player<'_>) -> Option<(Option<Aimed>, Prompt)> {
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
    // The nearest gun aimed at (in reach, in sight): taken, or why not.
    let mut best: Option<(f32, Place, Offered)> = None;
    let mut sights = [None; PLACES as usize];
    for (n, seen) in seen.iter().enumerate() {
        let (Some(seen), Some(target)) = (seen, Place::new(n as u8)) else { continue };
        let sight = match enters(&seen.pose, eye, forward, REACH, PAD) {
            None if within(&seen.pose, eye, REACH, PAD) => Err(Sight::NotAimedAt),
            None => Err(Sight::OutOfReach),
            Some(distance) if distance > 0.01 && physics.ray_test(eye, forward, distance, 0x24, 2).is_some() => {
                Err(Sight::Blocked)
            }
            Some(distance) => Ok((distance, offer(thread, viewer, world, sender, target, seen.body))),
        };
        let logged = match sight {
            Ok((distance, offered)) => {
                if !best.is_some_and(|(nearest, ..)| nearest <= distance) {
                    best = Some((distance, target, offered));
                }
                offered.err().unwrap_or(Sight::Offered)
            }
            Err(why) => why,
        };
        sights[n] = Some((seen.body, logged, seen.pose));
    }
    log_sights(thread, viewer, sights);
    let (_, target, offered) = best?;
    let gun = match offered {
        Ok(gun) => gun,
        Err(why) => return Some((None, Prompt::Cannot(cannot_key(why)))),
    };
    let places = GameWorldUi::at_update_icons(ctx)?.places()?;
    weapon_pickup::allowed(sender, target, places).then_some(())?;
    let name = gun.and_then(|(cook, slot)| native::gun_name(thread, cook, slot)).unwrap_or(NameKey::EMPTY);
    Some((Some(Aimed { target, sender, places }), Prompt::Take(name)))
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
    /// The viewer has no place, profile or kit.
    NoPicker,
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
/// A new drop's gun box is logged once (its world centre and half sizes).
fn log_sights(thread: &Thread, viewer: &Player<'_>, now: [Option<(Body, Sight, GunPose)>; PLACES as usize]) {
    let before = SIGHTS.get(thread);
    for (n, (was, is)) in before.iter().zip(now.iter()).enumerate() {
        let Some((body, sight, pose)) = *is else { continue };
        if was.map(|(b, _)| b) != Some(body) {
            log_box(viewer, n as u8, &pose);
        }
        if *was != Some((body, sight)) {
            log_sight(viewer, n as u8, sight);
        }
    }
    // A drop not seen this frame keeps what was logged of it.
    let mut kept = before;
    for (k, is) in kept.iter_mut().zip(now) {
        if let Some((body, sight, _)) = is {
            *k = Some((body, sight));
        }
    }
    SIGHTS.set(thread, kept);
}

/// `place P: the gun box of place T at (x, y, z), half sizes (a, b, c) mm` (world).
fn log_box(viewer: &Player<'_>, target: u8, pose: &GunPose) {
    let (centre, half) = world_box(pose);
    let mm = |v: f32| [Text(if v < 0.0 { "-" } else { "" }), Dec((v.abs() * 1000.0) as u32)];
    let [c0, c1, c2] = centre.map(mm);
    let [h0, h1, h2] = half.map(mm);
    log_joined(&[&[Text("place "), Dec(u32::from(viewer.place())), Text(": the gun box of place "), Dec(u32::from(target)),
                   Text(" at (")], &c0, &[Text(", ")], &c1, &[Text(", ")], &c2, &[Text("), half sizes (")],
                 &h0, &[Text(", ")], &h1, &[Text(", ")], &h2, &[Text(") mm")]]);
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
        Sight::NoPicker => "not offered, the viewer has no place, profile or kit",
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
/// and the drop's slot once made here (None until then: offered all the
/// same, the pickup waits for it); else why not (`body` not that drop's dead
/// body, the drop not open, the rule).
type Offered = Result<Option<(CookId, WeaponSlot)>, Sight>;

fn offer(thread: &Thread, viewer: &Player<'_>, world: WorldKey, place: Place, target: Place, body: Body) -> Offered {
    let (theirs, last) = placed_at(thread, world, target);
    if last != Some(body) {
        return Err(Sight::NotItsBody);
    }
    // Open until the body leaves the world (the record ends it there).
    let slot = match theirs.drop {
        DropState::Open { slot, .. } => slot,
        DropState::Taken => return Err(Sight::Taken),
        DropState::None => return Err(Sight::NoDrop),
    };
    let gun = body_drop(thread, target).map(|(cook, ..)| (cook, slot))
        .filter(|&(cook, _)| matches!(gun_cook::cooked(thread, cook), Cooked::Made { .. }));
    let picker = picker_of(viewer, place).ok_or(Sight::NoPicker)?;
    // Not made yet: the rule as for a pistol (the pickup asks it again).
    let incoming = gun.and_then(|(cook, slot)| native::handedness(thread, cook, slot)).unwrap_or(Handedness::One);
    weapon_pickup::decide(picker, target, theirs.drop, |_, _| incoming)
        .map_err(|why| Sight::Refused { why, held: match picker.hands {
                                            Hands::Gun(slot) => Some((slot, viewer.weapon_handedness(slot))),
                                            Hands::Switching | Hands::NoGun => None,
                                        },
                                        other: picker.other, rule: picker.rule, incoming })?;
    Ok(gun)
}

/// Whether the gun's box (padded by `pad`) is within `reach` of `eye`: its
/// centre nearer than the reach and the box's half diagonal.
fn within(pose: &GunPose, eye: [f32; 3], reach: f32, pad: f32) -> bool {
    let (centre, half) = world_box(pose);
    let radius: f32 = half.iter().map(|h| (h + pad) * (h + pad)).sum();
    let d = [centre[0] - eye[0], centre[1] - eye[1], centre[2] - eye[2]];
    let limit = reach + ragdoll::math::sqrt(radius);
    dot(d, d) <= limit * limit
}

/// The gun's box in the world: its centre, and its half sizes along its
/// own axes (the model box's, times each axis's scale), metres.
fn world_box(pose: &GunPose) -> ([f32; 3], [f32; 3]) {
    let m = &pose.transform;
    let mut centre = [m[12], m[13], m[14]];
    let mut half = [0.0f32; 3];
    for axis in 0..3 {
        let row = [m[axis * 4], m[axis * 4 + 1], m[axis * 4 + 2]];
        let middle = (pose.model.lo[axis] + pose.model.hi[axis]) * 0.5;
        for (c, r) in centre.iter_mut().zip(row) {
            *c += middle * r;
        }
        half[axis] = (pose.model.hi[axis] - pose.model.lo[axis]) * 0.5 * ragdoll::math::sqrt(dot(row, row));
    }
    (centre, half)
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

/// `place P at T ms <what>: slot S's picked gun (taken at T', cooked from place P' kit K slot S'')`.
fn log_pick(what: &str, player: &Player<'_>, slot: WeaponSlot, pick: Pick, why: &str) {
    let r = pick.recipe;
    log(&[Text("place "), Dec(u32::from(player.place())), Text(" at "), Dec(player.time()), Text(" ms "), Text(what),
          Text(": slot "), Dec(u32::from(slot.slot())), Text("'s picked gun (taken at "), Dec(pick.taken.0),
          Text(" ms, the gun of place "), Dec(u32::from(r.place.get())), Text(" kit "), Dec(u32::from(r.kit.get())),
          Text(" slot "), Dec(u32::from(r.slot.slot())), Text(")"), Text(if why.is_empty() { "" } else { " (" }),
          Text(why), Text(if why.is_empty() { "" } else { ")" })]);
}

/// D (the owner, 2026-11-01): every picked gun's object address at each
/// step, to match a crash's registers: `place P at T ms gun 0x...: <what>,
/// registry id N, slot S (why)`.
fn log_gun(player: &Player<'_>, what: &str, gun: ItemId, id: Option<RegistryId>, slot: WeaponSlot, why: &str) {
    let id = id.map_or(0xFF, |id| u32::from(id.get()));
    log(&[Text("place "), Dec(u32::from(player.place())), Text(" at "), Dec(player.time()), Text(" ms gun "), Hex(gun.address()),
          Text(": "), Text(what), Text(", registry id "), Dec(id), Text(", slot "), Dec(u32::from(slot.slot())),
          Text(if why.is_empty() { "" } else { " (" }), Text(why), Text(if why.is_empty() { "" } else { ")" })]);
}

/// `place P at T ms <what>: slot S's pending gun (...), registry id N`.
fn log_registered(what: &str, player: &Player<'_>, pending: Pending, registered: Registered) {
    let r = pending.pick.recipe;
    log(&[Text("place "), Dec(u32::from(player.place())), Text(" at "), Dec(player.time()), Text(" ms "), Text(what),
          Text(": slot "), Dec(u32::from(pending.slot.slot())), Text("'s pending gun (taken at "), Dec(pending.pick.taken.0),
          Text(" ms, the gun of place "), Dec(u32::from(r.place.get())), Text(" kit "), Dec(u32::from(r.kit.get())),
          Text(" slot "), Dec(u32::from(r.slot.slot())), Text("), registry id "), Dec(u32::from(registered.id().get()))]);
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

fn log_recipe(what: &str, recipe: Recipe, rest: &[Piece<'_>]) {
    log_joined(&[&[Text(what), Text(" the gun of place "), Dec(u32::from(recipe.place.get())), Text(" kit "),
                   Dec(u32::from(recipe.kit.get())), Text(" slot "), Dec(u32::from(recipe.slot.slot()))], rest]);
}

// ---- a bullet's record (vostok::pickup's "a bullet's record") --------------------------------

/// `bullet.save_quiet`: the quiet byte always written, whatever the ammo.
pub fn on_save_quiet(_: &mut Context<'_>) -> Flow<WeaponPickupOnSaveQuietExit> {
    Flow::Exit(WeaponPickupOnSaveQuietExit::Write)
}

/// `bullet.load_quiet`: the quiet byte always read, whatever the ammo
/// the record's slot resolves to now.
pub fn on_load_quiet(ctx: &mut Context<'_>) -> Flow<WeaponPickupOnLoadQuietExit> {
    native::load_quiet(ctx);
    Flow::Exit(WeaponPickupOnLoadQuietExit::Read)
}

/// `bullet.load_ordinal`: an ordinal past the resolved gun's ammo is none.
pub fn on_load_ordinal(ctx: &mut Context<'_>) -> Flow<NoExit> {
    native::clamp_bullet_ordinal(ctx);
    Flow::Continue
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
        // Just off its side: 15 cm of padding catches 19 cm off the centre.
        assert!(enters(&gun, [0.19, 0.2, 0.], [0., 0., 1.], REACH, PAD).is_some());
        assert_eq!(enters(&gun, [0.21, 0.2, 0.], [0., 0., 1.], REACH, PAD), None);
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
    fn no_refusal_is_silent() {
        use weapon_pickup::Refusal as R;
        for why in [R::Dead, R::NoGunInHands, R::Switching, R::NotAvailable, R::SecondWeaponDisabled, R::TwoDoubleHanded] {
            assert!(refusal_key(why).to_bytes().starts_with(b"st_weapon_"), "{why:?}");
        }
        assert_eq!(cannot_key(Sight::Taken), c"st_weapon_already_taken");
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
