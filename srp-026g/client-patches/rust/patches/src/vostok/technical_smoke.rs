//! Dense Smoke (402) and Toxic Smoke (441), patch 180 (technical-skills.md, "Dense Smoke and Toxic
//! Smoke"): the smoke grenade's cloud as the game has it, read once into types. The cloud is only
//! the particle system the blast plays (`fx/explosions/smoke_grenade`, one instance a grenade
//! object); its timeline and shape are `technical_skills::smoke`'s.
//!
//! - The grenade object (survarium::grenade, the only class the game makes) gets a tail of ours:
//!   the cook allocates 270h instead of 258h (`smoke.grenade_size`), the ctor clears it. It holds
//!   the toxic cloud (simulation state, saved with the grenade) and the grenade's green instance
//!   (presentation, owned by the grenade until its destructor).
//! - The simulation: a smoke's blast (`smoke.explode`, every game, a re-simulation too) keeps a
//!   toxic cloud on the grenade when its thrower has Toxic Smoke, and clears it otherwise; the cloud
//!   stays until that grenade object goes off again (as the stock particle, which a new blast of
//!   the same object replays). Saved after the thrown fields (`smoke.save_*`, `smoke.load_cloud`):
//!   bit 3 of current_state, then [`Cloud::BYTES`]. Each player's tick (`battery.carrier_tick`)
//!   hits a living player whose eyes are in an enemy's toxic cloud, as a gas zone hits: the face,
//!   hit type 4, the dose of the tick's milliseconds in the cloud's time.
//! - The presentation: the cook asks a fourth resource of a smoke grenade, the green copy of its
//!   particle (`smoke.cook_query`, `smoke.cook_ready`); the blast plays it for a thrower with Toxic
//!   Smoke (`smoke.play`); Dense Smoke's pause holds the playing instance still from
//!   [`smoke::HOLD_FROM_MS`] for its extra time (`smoke.particle_tick`, the particle world's
//!   thread, through [`HOLDS`]).
use core::ffi::{CStr, c_void};
use core::ptr::null;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use technical_skills::smoke::{self, Cloud};
use technical_skills::{Clock, Skills};

use super::carry_runtime::{read_bytes, write_bytes};
use super::context::{Context, Reg};
use super::hook::Thread;
use super::sync::Lock;
use super::world::{floats, read, write_floats};
use crate::raw;

/// The client grenade's bytes with our tail (natively 258h, its last field +250h).
const GRENADE_SIZE: u32 = 0x270;
/// The particle system of the stock smoke grenade (rdg-m's `particle_explosion`) and its green copy
/// (the `toxic-smoke` content pack).
const SMOKE_PARTICLE: &CStr = c"fx/explosions/smoke_grenade";
const TOXIC_PARTICLE: &CStr = c"fx/explosions/smoke_grenade_toxic";
/// A particle system instance's resource class (the grenade cook's second request).
const PARTICLE_CLASS: u32 = 0x3D;
/// The cook's requests: its own three, and ours with the green copy.
const NATIVE_REQUESTS: usize = 3;
const EXTENDED_REQUESTS: u32 = 4;
/// current_state's bit for our cloud (the native bits are 0..2; deserialize 0x303F80 tests them
/// one by one and ignores this one).
const CLOUD_BIT: u8 = 8;
/// The tail's cloud word.
const NO_CLOUD: u32 = 0;
const TOXIC_CLOUD: u32 = 1;
/// A hit's kind: toxic (survarium::hit_type, the gas zones' damage_type).
const TOXIC: u32 = 4;
/// The part the gas zones hit (their hit_parts_filter), a native terminated name.
const FACE: &CStr = c"face";
/// The places' bit set is a u32.
const PLACE_BITS: usize = 32;

/// A grenade object of the game's own class (none for another: its fields past grenade_core's
/// are not there).
///
/// # Safety
/// `grenade` is a live grenade.
unsafe fn ours(grenade: raw::GrenadeCore) -> Option<raw::GrenadeCore> {
    // SAFETY: the object's own vtable word.
    (unsafe { read(grenade.vtable()) } == raw::image::GRENADE_VTABLE).then_some(grenade)
}

/// The grenade's toxic cloud, read once from our tail; none for another word (never written) or a
/// centre that is not finite.
///
/// # Safety
/// `grenade` is a live grenade of the game's class ([`ours`]).
unsafe fn cloud(grenade: raw::GrenadeCore) -> Option<Cloud> {
    // SAFETY: our tail's words (the allocation is GRENADE_SIZE).
    unsafe {
        if read(grenade.cloud()) != TOXIC_CLOUD {
            return None;
        }
        let blast = Clock(read(grenade.cloud_blast()));
        let centre: [f32; 3] = floats(grenade.cloud_centre());
        centre.iter().all(|v| v.is_finite()).then_some(Cloud { blast, centre })
    }
}

/// Sets the grenade's cloud (none clears it).
///
/// # Safety
/// As [`cloud`].
unsafe fn set_cloud(grenade: raw::GrenadeCore, value: Option<Cloud>) {
    // SAFETY: our tail's words.
    unsafe {
        match value {
            Some(cloud) => {
                *grenade.cloud_blast() = cloud.blast.0;
                write_floats(grenade.cloud_centre(), cloud.centre);
                *grenade.cloud() = TOXIC_CLOUD;
            }
            None => {
                *grenade.cloud() = NO_CLOUD;
                *grenade.cloud_blast() = 0;
                write_floats(grenade.cloud_centre(), [0.0; 3]);
            }
        }
    }
}

/// The player whose stack `grenade` is of (its kit's player: set -> inventory -> holder).
///
/// # Safety
/// `grenade` is a live grenade of a live stack.
pub(super) unsafe fn thrower(grenade: raw::GrenadeCore) -> Option<raw::Player> {
    // SAFETY: the stack's item and its owning inventory and holder, alive with it.
    unsafe { Some(grenade.stack()?.item().inventory()?.holder()?.player()) }
}

/// Whether `grenade` is a smoke grenade: its stack's blast damages nobody (splash_max_damage 0,
/// rdg-m alone of the game's grenades).
///
/// # Safety
/// As [`thrower`].
pub(super) unsafe fn smoke_grenade(grenade: raw::GrenadeCore) -> bool {
    // SAFETY: the stack's configured field.
    unsafe { grenade.stack().is_some_and(|set| read(set.splash_max_damage()) == 0.0) }
}

/// `item` as a grenade stack, none for another item.
///
/// # Safety
/// `item` is a live item.
unsafe fn grenade_set(item: raw::Item) -> Option<raw::GrenadeSet> {
    // SAFETY: the item's own vtable word; the cast only after it is a grenade set's.
    unsafe {
        let table = read(item.vtable());
        (table == raw::image::GRENADE_SET_VTABLE || table == raw::image::GRENADE_SET_CORE_VTABLE)
            .then(|| item.grenade_set())
    }
}

// ---- the grenade's tail -------------------------------------------------------------------------

/// The cook's size (`smoke.grenade_size`): EAX the grenade's bytes with our tail; the site's
/// `mov eax,258h` is skipped.
pub fn grenade_size(ctx: &mut Context<'_>) {
    ctx.set_reg(Reg::Eax, GRENADE_SIZE);
}

/// The ctor's last store (`smoke.grenade_construct`): no cloud, no green instance (the allocator
/// leaves the tail as it was).
pub fn grenade_construct(ctx: &Context<'_>) {
    // SAFETY: the site's EDI is the grenade being made, GRENADE_SIZE bytes (smoke.grenade_size).
    unsafe {
        let Some(grenade) = raw::site::smoke_grenade_construct::grenade(ctx) else { return };
        set_cloud(grenade, None);
        grenade.set_toxic_particle(None);
    }
}

/// The destructor (`smoke.grenade_destroy`), before the grenade releases its own resources: its
/// instances leave Dense Smoke's holds (resumed: a held instance the particle world still plays
/// finishes), and its green instance's reference is released.
pub fn grenade_destroy(ctx: &Context<'_>) {
    // SAFETY: the dtor's live grenade, its instances alive with it (it holds a reference to each
    // until this body releases them, after the site).
    unsafe {
        let Some(grenade) = raw::site::smoke_grenade_destroy::grenade(ctx).and_then(|g| ours(g)) else { return };
        let green = grenade.toxic_particle();
        for instance in [grenade.particle(), green].into_iter().flatten() {
            release_hold(ctx.thread(), instance);
        }
        if let Some(instance) = green {
            grenade.set_toxic_particle(None);
            let count = instance.references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_particle(Some(instance), count);
            }
        }
    }
}

/// The grenade cook's query (`smoke.cook_query`): a smoke grenade's object asks, after the cook's
/// own three, the green copy of its particle. True when the hook made the query (the site's `done`
/// exit, past the native call).
pub fn cook_query(ctx: &Context<'_>) -> bool {
    use raw::site::smoke_cook_query as site;
    // SAFETY: 0x25E154, the cook's three requests by ESP and the five pushed arguments; query
    // copies the requests before it returns (as it copies the cook's stack array), the callback,
    // its user data and the parent query stay the cook's.
    unsafe {
        let Some(frame) = site::frame(ctx) else { return false };
        let first = frame.requests();
        let particle = first.at(1);
        if read(particle.class()) != PARTICLE_CLASS || particle.path().is_null() {
            return false;
        }
        // the path and its NUL against ours, byte by byte (the arena has no strlen)
        let path = particle.path();
        let same = SMOKE_PARTICLE.to_bytes_with_nul().iter().enumerate().all(|(i, byte)| path.add(i).read_volatile() == *byte);
        if !same {
            return false;
        }
        let request = |n: usize| {
            let r = first.at(n);
            [r.path() as usize as u32, read(r.class())]
        };
        let mut extended = [[0u32; 2]; EXTENDED_REQUESTS as usize];
        for (n, slot) in extended.iter_mut().enumerate().take(NATIVE_REQUESTS) {
            *slot = request(n);
        }
        extended[NATIVE_REQUESTS] = [TOXIC_PARTICLE.as_ptr() as usize as u32, PARTICLE_CLASS];
        raw::engine_query_resources(extended.as_ptr().cast(), EXTENDED_REQUESTS, frame.callback().cast(),
            frame.allocator().cast(), frame.user_data().cast(), frame.parent().cast(), read(frame.flag()));
    }
    true
}

/// The cook's callback after the particle's store (`smoke.cook_ready`): a smoke grenade's fourth
/// result, its green instance, kept with a reference of the grenade's own (a missing file leaves
/// none: the blast plays the stock grey).
pub fn cook_ready(ctx: &Context<'_>) {
    use raw::site::smoke_cook_ready as site;
    // SAFETY: the callback's live grenade and results; the fourth only when the query had four
    // (ours), the count atomic as the game's own takers do it.
    unsafe {
        let (Some(grenade), Some(result)) = (site::grenade(ctx).and_then(|g| ours(g)), site::result(ctx)) else { return };
        if read(result.count()) != EXTENDED_REQUESTS || grenade.toxic_particle().is_some() {
            return;
        }
        let Some(instance) = result.toxic_particle() else { return };
        AtomicU32::from_ptr(instance.references()).fetch_add(1, Ordering::AcqRel);
        grenade.set_toxic_particle(Some(instance));
    }
}

// ---- the simulation -----------------------------------------------------------------------------

/// The blast (`smoke.explode`): a smoke grenade of a thrower with Toxic Smoke keeps its cloud (the
/// explode time, the tick's, and its centre); any other blast of the object clears it. Reads only
/// saved and simulated state: the grenade's explode time and centre, the thrower's kit (static).
pub fn explode(ctx: &Context<'_>) {
    // SAFETY: explode's live grenade of a live stack (the tick calls it for a grenade in its world).
    unsafe {
        let Some(grenade) = raw::site::smoke_explode::grenade(ctx).and_then(|g| ours(g)) else { return };
        let toxic = smoke_grenade(grenade)
            && thrower(grenade).is_some_and(|player| super::technical::skills(player).toxic_rate() > 0.0);
        let value = toxic.then(|| Cloud { blast: Clock(read(grenade.explode_time())), centre: floats(grenade.centre()) });
        set_cloud(grenade, value);
    }
}

/// Toxic Smoke's hits at a player's tick (`battery.carrier_tick`, EDI the tick's time, EBP the ms
/// since its last): a living player whose eyes are in a toxic cloud, anyone's (the thrower's and its
/// team's too: the owner, 2026-11-22), takes that cloud's dose of the tick as a gas zone's hit: the
/// face, toxic, no piercing, the thrower's place the attacker (the kill the thrower's), no initiator
/// and no source item, as a zone's. An item without an initiator ends the game: hit_impl reads the
/// initiator's vtable once a hit with an item was applied (0x2D9CD5). An active oxygen tank protects
/// by the stock rule. The places, their kits'
/// players, the slots and the stacks' objects are taken in order, so every game hits alike.
pub fn toxic_tick(ctx: &Context<'_>) {
    use raw::site::battery_carrier_tick as site;
    let now = Clock(ctx.reg(Reg::Edi));
    let since = Clock(now.0.wrapping_sub(ctx.reg(Reg::Ebp)));
    // SAFETY: base_player::tick's live player and its world core; the core's places own their kit
    // players for the match (kept when a place's player leaves), each with its inventory, whose
    // slots' stacks own their grenade objects (made at the cook). The hit is the game's own call,
    // as base_player's own quick-slot suicide makes it inside the tick.
    unsafe {
        let Some(victim) = site::player(ctx) else { return };
        if read(victim.alive()) == 0 {
            return;
        }
        let Some(core) = victim.world_core() else { return };
        let (Some(places), Some(end)) = (core.places(), core.places_end()) else { return };
        let count = end.index_from(places).unwrap_or(0).min(PLACE_BITS);
        let active = read(core.active_places());
        // A round about to start (Freeze Tag, Artifact Hunt) ends every toxic cloud, so none hits
        // into the next round (the owner, 2026-11-22: "smokes should be reset in those").
        if super::round_countdown::Play::of(core) == super::round_countdown::Play::RoundCountdown {
            for place in (0..count).filter(|place| active >> place & 1 != 0) {
                let entry = places.at(place);
                for thrower in [entry.kit0(), entry.kit1(), entry.kit2()].into_iter().flatten() {
                    // Each object's cloud is read as it is reached: clearing one leaves the rest.
                    for (grenade, _) in clouds(thrower) {
                        set_cloud(grenade, None);
                    }
                }
            }
            return;
        }
        let feet: [f32; 3] = floats(victim.feet());
        let mut part = raw::HitPartStorage::zeroed();
        part.handle().set_name(FACE.as_ptr().cast_mut().cast());
        for place in 0..count {
            if active >> place & 1 == 0 {
                continue;
            }
            let entry = places.at(place);
            for thrower in [entry.kit0(), entry.kit1(), entry.kit2()].into_iter().flatten() {
                let skills = super::technical::skills(thrower);
                if skills.toxic_rate() == 0.0 {
                    continue;
                }
                for (_, cloud) in clouds(thrower) {
                    if read(victim.alive()) == 0 {
                        return;
                    }
                    if !cloud.inside(feet) {
                        continue;
                    }
                    let dose = cloud.toxic_dose(skills, since, now);
                    if dose > 0.0 {
                        raw::engine_player_hit(Some(victim.hit_receiver()), now.0, Some(thrower.owner()),
                            Some(part.handle()), TOXIC, dose, 0.0, null::<c_void>(), None);
                    }
                }
            }
        }
    }
}

/// Whether `player` has its eyes in a toxic cloud at `now`, anyone's: Forest Protection takes the
/// cloud for an anomaly (`smoke.anomaly_zones`, patch 111's glow). The places and their kits'
/// players as [`toxic_tick`] takes them.
///
/// # Safety
/// `player` is a live player of a world whose places own their kit players.
pub(super) unsafe fn in_toxic_cloud(player: raw::Player, now: Clock) -> bool {
    // SAFETY: as toxic_tick: the core's places, their kit players, stacks and grenade objects.
    unsafe {
        let Some(core) = player.world_core() else { return false };
        let (Some(places), Some(end)) = (core.places(), core.places_end()) else { return false };
        let count = end.index_from(places).unwrap_or(0).min(PLACE_BITS);
        let active = read(core.active_places());
        let feet: [f32; 3] = floats(player.feet());
        (0..count).filter(|place| active >> place & 1 != 0).any(|place| {
            let entry = places.at(place);
            [entry.kit0(), entry.kit1(), entry.kit2()].into_iter().flatten().any(|thrower| {
                let skills = super::technical::skills(thrower);
                skills.toxic_rate() > 0.0
                    && clouds(thrower).any(|(_, cloud)| cloud.inside(feet) && cloud.toxic_at(skills, now))
            })
        })
    }
}

/// Forest Protection in a toxic cloud (`smoke.anomaly_zones`, where a hit with an owner reads the
/// victim's damage zones before modifier 41): true when the victim is in no zone but in a toxic
/// cloud at the hit's time, so the native factor applies as in a zone.
pub fn anomaly_zones(ctx: &Context<'_>) -> bool {
    use raw::site::smoke_anomaly_zones as site;
    // SAFETY: hit_body_part's live victim and hit.
    unsafe {
        let (Some(player), Some(hit)) = (site::player(ctx), site::hit(ctx)) else { return false };
        read(player.damage_zones()) == 0 && in_toxic_cloud(player, Clock(read(hit.time())))
    }
}

/// The toxic clouds of `player`'s stacks, in slot and object order, each with its stack.
///
/// # Safety
/// `player` is a live kit player; its inventory, stacks and their objects outlive the iterator.
unsafe fn clouds(player: raw::Player) -> impl Iterator<Item = (raw::GrenadeCore, Cloud)> {
    // SAFETY: the caller's live inventory, its 23 slots' items and each stack's object vector.
    unsafe {
        let slots = player.inventory().map(|inventory| (inventory.slots(), inventory.slots_end()));
        let count = slots.map_or(0, |(first, end)| end.index_from(first).unwrap_or(0));
        (0..count).flat_map(move |slot| {
            let set = slots.and_then(|(first, _)| first.at(slot).item()).and_then(|item| grenade_set(item));
            let objects = set.and_then(|set| Some((set.grenades()?, set.grenades_end()?)));
            let n = objects.map_or(0, |(first, end)| end.index_from(first).unwrap_or(0));
            (0..n).filter_map(move |at| {
                let (first, _) = objects?;
                let grenade = ours(first.at(at).grenade()?)?;
                Some((grenade, cloud(grenade)?))
            })
        })
    }
}

// ---- the saved state ----------------------------------------------------------------------------

/// The grenade behind a serializable subobject, of the game's class.
///
/// # Safety
/// `core` is a live grenade_core's serializable subobject.
unsafe fn grenade_of(core: Option<raw::GrenadeSerializable>) -> Option<raw::GrenadeCore> {
    // SAFETY: the subobject is the grenade's +10h.
    unsafe { ours(core?.grenade()) }
}

/// serialize's state byte (`smoke.save_state`): bit 3 when the grenade keeps a toxic cloud.
pub fn save_state(ctx: &Context<'_>) {
    use raw::site::smoke_save_state as site;
    // SAFETY: serialize's live grenade and its frame's state byte, written next by w<u8>.
    unsafe {
        let (Some(grenade), Some(frame)) = (grenade_of(site::core(ctx)), site::frame(ctx)) else { return };
        if cloud(grenade).is_some() {
            *frame.state() |= CLOUD_BIT;
        }
    }
}

/// serialize after the thrown fields (`smoke.save_cloud`): the cloud's bytes, when bit 3 said so.
pub fn save_cloud(ctx: &Context<'_>) {
    use raw::site::smoke_save_cloud as site;
    // SAFETY: serialize's live grenade and writer.
    unsafe {
        let (Some(grenade), Some(writer)) = (grenade_of(site::core(ctx)), site::writer(ctx)) else { return };
        if let Some(cloud) = cloud(grenade) {
            write_bytes(writer, &cloud.to_bytes());
        }
    }
}

/// deserialize after the thrown fields (`smoke.load_cloud`): the cloud the state byte's bit 3 says
/// follows, read checked against the stream's end; none without it (a load sets the grenade's
/// cloud whatever it held: a rollback's and a late joiner's are the world's).
pub fn load_cloud(ctx: &Context<'_>) {
    use raw::site::smoke_load_cloud as site;
    // SAFETY: deserialize's live grenade, its reader and the frame's state byte (read at 0x303FB8).
    unsafe {
        let (Some(grenade), Some(reader), Some(frame)) = (grenade_of(site::core(ctx)), site::reader(ctx), site::frame(ctx))
        else {
            return;
        };
        let value = match read(frame.state()) & CLOUD_BIT {
            0 => None,
            _ => read_bytes::<{ Cloud::BYTES }>(reader).and_then(Cloud::parse),
        };
        set_cloud(grenade, value);
    }
}

// ---- the presentation ---------------------------------------------------------------------------

/// The blast's particle (`smoke.play`, once a throw): for a smoke grenade, the green instance when
/// its thrower has Toxic Smoke (and the pack gave one), and Dense Smoke's hold of the instance it
/// plays. True when the hook chose the green one (EAX; the site's load is skipped).
pub fn play(ctx: &mut Context<'_>) -> bool {
    // SAFETY: explode's live grenade of a live stack; its instances alive with it.
    let chosen = unsafe {
        let Some(grenade) = raw::site::smoke_play::grenade(ctx).and_then(|g| ours(g)) else { return false };
        if !smoke_grenade(grenade) {
            return false;
        }
        let skills = thrower(grenade).map(super::technical::skills).unwrap_or_default();
        let green = grenade.toxic_particle().filter(|_| skills.toxic_rate() > 0.0);
        let instance = green.or(grenade.particle());
        if let Some(instance) = instance {
            hold(ctx.thread(), instance, skills);
        }
        green
    };
    match chosen {
        Some(instance) => {
            ctx.set_reg(Reg::Eax, instance.address());
            true
        }
        None => false,
    }
}

/// Dense Smoke's hold of one playing instance, in the particle world's seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// From the play until the cloud is full and still; then paused for `extra`.
    Waiting { left: f32, extra: f32 },
    /// Paused, `left` more.
    Holding { left: f32 },
}

impl Phase {
    /// A hold started at a play: [`smoke::HOLD_FROM_MS`], then the thrower's extra time; none
    /// without Dense Smoke.
    fn start(skills: Skills) -> Option<Phase> {
        let extra = skills.smoke_extra_ms();
        (extra > 0).then(|| Phase::Waiting { left: smoke::HOLD_FROM_MS as f32 / 1000.0, extra: extra as f32 / 1000.0 })
    }

    /// One tick of `dt` seconds: the phase after it (none: the hold is over) and whether the
    /// instance is paused for it.
    fn step(self, dt: f32) -> (Option<Phase>, bool) {
        match self {
            Phase::Waiting { left, extra } if left > dt => (Some(Phase::Waiting { left: left - dt, extra }), false),
            Phase::Waiting { extra, .. } => (Some(Phase::Holding { left: extra }), true),
            Phase::Holding { left } if left > dt => (Some(Phase::Holding { left: left - dt }), true),
            Phase::Holding { .. } => (None, false),
        }
    }
}

#[derive(Clone, Copy)]
struct Hold {
    instance: u32,
    phase: Phase,
}

const HOLD_ROOM: usize = 32;

/// The holds, by instance: the game's thread starts one at the play and ends one at its grenade's
/// destructor, the particle world's thread steps it. An entry names an instance its grenade keeps
/// alive (the destructor ends it first), so the pointer it holds is never stale.
static HOLDS: Lock<[Option<Hold>; HOLD_ROOM]> = Lock::new([None; HOLD_ROOM]);
/// How many holds there are: the particle tick's test before the lock.
static HELD: AtomicU32 = AtomicU32::new(0);

fn held(holds: &[Option<Hold>; HOLD_ROOM]) -> u32 {
    holds.iter().flatten().count() as u32
}

/// Starts `instance`'s hold for a thrower with `skills`. A replay of an instance restarts its
/// hold; a replay without Dense Smoke resumes it at its next tick. With every entry in use the
/// cloud is not held (32 held clouds at once).
fn hold(thread: &Thread, instance: raw::ParticleSystemInstance, skills: Skills) {
    let address = instance.address();
    HOLDS.with(thread, |holds| {
        let slot = holds.iter().position(|h| h.is_some_and(|h| h.instance == address));
        let phase = Phase::start(skills);
        match (slot, phase) {
            (Some(at), Some(phase)) => holds[at] = Some(Hold { instance: address, phase }),
            (Some(at), None) => holds[at] = Some(Hold { instance: address, phase: Phase::Holding { left: 0.0 } }),
            (None, Some(phase)) => {
                if let Some(at) = holds.iter().position(Option::is_none) {
                    holds[at] = Some(Hold { instance: address, phase });
                }
            }
            (None, None) => {}
        }
        HELD.store(held(holds), Ordering::Release);
    });
}

/// Ends `instance`'s hold and resumes it (its grenade's destructor: the instance is alive).
///
/// # Safety
/// `instance` is alive.
unsafe fn release_hold(thread: &Thread, instance: raw::ParticleSystemInstance) {
    let address = instance.address();
    let found = HOLDS.with(thread, |holds| {
        let found = holds.iter().position(|h| h.is_some_and(|h| h.instance == address));
        if let Some(at) = found {
            holds[at] = None;
        }
        HELD.store(held(holds), Ordering::Release);
        found.is_some()
    });
    if found {
        // SAFETY: the caller's live instance; the byte is read by the particle world's thread.
        unsafe { AtomicU8::from_ptr(instance.paused()).store(0, Ordering::Release) };
    }
}

/// The particle world's tick of one instance at its pause test (`smoke.particle_tick`, XMM0 the
/// tick's seconds): a held instance is paused from [`smoke::HOLD_FROM_MS`] after its play for its
/// extra time, then resumed and let go. The pause is the instance's own (+302h), which the game
/// never sets: the tick returns at once, and the still puffs stay as they are.
pub fn particle_tick(ctx: &Context<'_>) {
    if HELD.load(Ordering::Acquire) == 0 {
        return;
    }
    let dt = ctx.xmm_f32(0);
    // SAFETY: the tick's live instance (EDI), on the particle world's thread.
    let Some(instance) = (unsafe { raw::site::smoke_particle_tick::instance(ctx) }) else { return };
    let address = instance.address();
    let paused = HOLDS.with(ctx.thread(), |holds| {
        let at = holds.iter().position(|h| h.is_some_and(|h| h.instance == address))?;
        let hold = holds[at]?;
        let (next, paused) = hold.phase.step(dt);
        holds[at] = next.map(|phase| Hold { phase, ..hold });
        HELD.store(held(holds), Ordering::Release);
        Some(paused)
    });
    if let Some(paused) = paused {
        // SAFETY: the tick's own instance; the pause test replayed after the hook reads it.
        unsafe { AtomicU8::from_ptr(instance.paused()).store(u8::from(paused), Ordering::Release) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_waits_then_pauses_for_the_extra_time() {
        assert_eq!(Phase::start(Skills::default()), None);
        let skills = Skills::parse((2u32 << 2) as f32).unwrap();
        let mut phase = Phase::start(skills);
        let (mut paused_s, mut waited_s, dt) = (0.0f32, 0.0f32, 1.0 / 60.0);
        while let Some(p) = phase {
            let (next, paused) = p.step(dt);
            match paused {
                true => paused_s += dt,
                false if paused_s == 0.0 => waited_s += dt,
                false => {}
            }
            phase = next;
        }
        assert!((waited_s - 2.0).abs() <= dt, "{waited_s}");
        assert!((paused_s - 5.8).abs() <= 2.0 * dt, "{paused_s}");
    }
}
