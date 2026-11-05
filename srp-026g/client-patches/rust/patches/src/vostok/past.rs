//! What patch 105 (`past_poses`, docs/notes/past_poses.md) reads of the
//! world and keeps in it: a step's end (`core.ticked`), each place's kit
//! choices (the profile inputs its declared age rides in), and the place's
//! record at the end of its player's state (`carry.save` / `carry.load`,
//! after patch 78's carry). Every field is fields_026g.tsv's, every
//! function engine_026g.tsv's.

use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::Thread;
use super::types::{GameTime, Place};
use super::world::{Player, WorldKey, read};
use crate::raw;

/// Nodes of a place's kit choices walked at most: more than its pool holds.
const KIT_CHOICES_WALKED: usize = 64;

/// The world core at `core.ticked`: atomic_tick has run a step's seven
/// delegate lists to [`CoreTick::time`] (the players moved, the bullets
/// ticked), and writes the time next; on the grid, tick_events saves that
/// point after it.
pub struct CoreTick<'t> {
    core: raw::CoreWorld,
    time: GameTime,
    thread: &'t Thread,
}

impl<'t> CoreTick<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<CoreTick<'t>> {
        use raw::site::core_ticked as site;
        // SAFETY: the context is this site's: the core atomic_tick steps,
        // alive while it does, on its thread; EDI the step's time.
        let (core, time) = unsafe { (site::core(ctx)?, site::time(ctx)) };
        Some(CoreTick { core, time: GameTime(time), thread: ctx.thread() })
    }

    /// The time the step ends at.
    pub fn time(&self) -> GameTime {
        self.time
    }

    /// Which match's world it is.
    pub fn world(&self) -> WorldKey {
        WorldKey::of(self.core)
    }

    /// Each place in the world (its bit of the active mask, the world's own
    /// state) and its current player, if it has one, to `f`.
    pub fn each_place(&self, mut f: impl FnMut(Place, Option<Player<'_>>)) {
        // SAFETY: the core is alive for the hook.
        let active = unsafe { read(self.core.active_places()) };
        for place in Place::all() {
            if active >> place.get() & 1 == 0 {
                continue;
            }
            f(place, active_player(self.core, place, self.thread));
        }
    }

    /// The place's newest kit choice at or before `at` (what its tick
    /// 0x321790 takes: the first from the newest end whose time is not
    /// after it, compared unsigned as there), its time and its byte.
    pub fn newest_kit_choice(&self, place: Place, at: GameTime) -> Option<(GameTime, u8)> {
        // SAFETY: the core's lists, a place's 480h bytes of them, its
        // nodes linked from the newest: alive for the hook (the network
        // thread inserts only between moves, on this thread).
        unsafe {
            let mut node = self.core.kit_choices()?.at(place.index()).newest();
            for _ in 0..KIT_CHOICES_WALKED {
                let this = node?;
                let time = read(this.time());
                if time <= at.0 {
                    return Some((GameTime(time), read(this.value())));
                }
                node = this.older();
            }
            None
        }
    }
}

/// The place's current player in `core` (core_active_player: a place in the
/// world, its current kit's player). The lookup adds a reference; it is
/// given back at once (the place holds its own), as NetworkClient::player
/// does.
fn active_player<'t>(core: raw::CoreWorld, place: Place, thread: &'t Thread) -> Option<Player<'t>> {
    let mut out = None;
    // SAFETY: the live core; the native lookup bounds the place by the
    // places' vector and the world's mask, and writes `out`.
    unsafe { raw::engine_core_active_player(Some(core), u32::from(place.get()), &mut out) };
    let this = out?;
    // SAFETY: a player, its reference count a u32 the game changes with
    // locked instructions.
    unsafe {
        let count = this.references();
        if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
            raw::engine_release_object(Some(this), count);
            return None;
        }
    }
    Some(Player { this, thread })
}

/// The player base_player::serialize writes at `carry.save` (after its last
/// field and patch 78's carry): its place's record goes after them.
pub fn saved_player<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    super::carry_runtime::player(ctx, super::carry_runtime::PlayerSite::Save)
}

/// `bytes` after the serialized state, with the game's byte writer (it grows
/// the state as the carry's do).
pub fn write_record(ctx: &Context<'_>, bytes: &[u8]) {
    // SAFETY: base_player::serialize's epilogue, its live writer (EDI).
    let Some(writer) = (unsafe { raw::site::carry_save::writer(ctx) }) else { return };
    for &byte in bytes {
        // SAFETY: the live writer; write_u8 copies the byte before it
        // returns; the labels are static and terminated.
        unsafe {
            raw::engine_write_u8((&raw const byte).cast(), Some(writer), c"past_poses".as_ptr().cast(), 1,
                                 c"serialize".as_ptr().cast(), c"record".as_ptr().cast())
        }
    }
}

/// The player base_player::deserialize read at `carry.load` (patch 78's
/// carry read before), and the `N` bytes after it, if the state has them
/// (the reader is moved past them only then).
pub fn loaded_record<'t, const N: usize>(ctx: &'t Context<'_>) -> Option<(Player<'t>, Option<[u8; N]>)> {
    let player = super::carry_runtime::player(ctx, super::carry_runtime::PlayerSite::Load)?;
    // SAFETY: base_player::deserialize's live reader (ESI).
    let reader = unsafe { raw::site::carry_load::reader(ctx) };
    Some((player, reader.and_then(super::carry_runtime::read_bytes::<N>)))
}

impl Player<'_> {
    /// The simulated transform's place (+43Ch): his feet, game frame; what
    /// assign_physics_transform moves his hit model to.
    pub fn feet(&self) -> [f32; 3] {
        let m = self.transform();
        [m[12], m[13], m[14]]
    }
}

impl super::world::GameWorld<'_> {
    /// Queues `value` as the local player's kit choice at the game's time,
    /// as the hud's kit tabs do (select_profile 0x1F8070: change_input<
    /// profile_input>, sent as 0xDE, relayed to every game as 0xF3 and played
    /// at its tick there). False, and nothing queued, without a local player
    /// (select_profile reads his place unchecked).
    /// The time of the local place's newest kit choice (its own, a pick
    /// on the death screen or before the start, or a declaration), if any.
    pub fn newest_own_kit_choice(&self) -> Option<GameTime> {
        // SAFETY: the game world's match and core, its lists, alive at the
        // after_move on the game's thread.
        unsafe {
            let core = self.this.pvp_match()?.core();
            let owner = Place::new(u32::from(read(core.owner())))?;
            let newest = core.kit_choices()?.at(owner.index()).newest()?;
            Some(GameTime(read(newest.time())))
        }
    }

    pub fn choose_kit_byte(&self, value: u8) -> bool {
        if self.network_client().and_then(|client| client.local_player()).is_none() {
            return false;
        }
        // SAFETY: the game world at its after_move, on the game's thread, with
        // a local player (checked above, the same field select_profile reads).
        unsafe { raw::engine_select_profile(Some(self.this), u32::from(value)) };
        true
    }
}
