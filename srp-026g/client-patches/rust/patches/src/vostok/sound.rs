//! The sound world as the music (patch 88) plays in it (docs/notes/music.md):
//! a sound scene of its own (the bus), kept among the world's active scenes
//! by the sound thread and given its volume there, and looping hud sounds
//! in it.
//!
//! - [`SoundUser`]: the game's sound world and its logic world user, as
//!   long as the game (read at `game.tick`);
//! - [`BusScene`]: the music's scene, made once and never let go (the
//!   sound thread ticks every active scene while the world lives);
//! - [`Emitter`], [`Voice`]: a loaded single sound and a playing hud sound,
//!   each holding a reference of its own and letting it go when dropped;
//! - [`SoundTick`]: the sound thread's tick (`sound_world.tick`), where the
//!   bus is kept active and its volume set;
//! - [`TrackPath`]: a track's resource path, read once from the mode's
//!   `ui.loading_music` (`level_loading.ui`) or made from a patch's name.
//! - [`Source`], [`VoiceVolume`] and the tagging sites: what a proxy plays
//!   (patch 94's Concentration), tagged once by the game's thread where it is
//!   made and read by the sound thread where its voice's volume is set.

use core::ffi::CStr;
use core::marker::PhantomData;
use core::ptr::null;
use core::sync::atomic::{AtomicU32, Ordering};

use super::config::ConfigValue;
use super::context::Context;
use super::hook::Thread;
use super::sync::thread_id;
use super::types::Place;
use super::world::read;
use crate::raw;

/// A hud sound's playback mode (play 0x5ABA0's first argument).
const LOOPED: u32 = 1;
/// A scene's fade state: fading in (process_fade 0x5C061).
const FADING_IN: u32 = 1;
/// The bytes of a track path, its zero too (a game path is shorter).
const PATH_ROOM: usize = 64;

/// The game's sound world and its logic world user (every game sound's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundUser {
    world: raw::SoundWorld,
    user: raw::WorldUser,
}

// SAFETY: the sound world and its world users are the engine's, made
// before the game and alive after it; a SoundUser is only kept in a patch's
// lock and used on the game's thread (the world user is the logic thread's).
unsafe impl Send for SoundUser {}

impl SoundUser {
    /// The game's, at `game.tick` (EBX the game).
    pub fn at_game_tick(ctx: &Context<'_>) -> Option<SoundUser> {
        // SAFETY: the site's EBX is the game, alive and ticking on this
        // thread; its sound world (+5350h) and the world's logic user
        // (+0B0h, get_logic_world_user 0x9F7E0) are the engine's.
        unsafe {
            let world = raw::site::game_tick::game(ctx)?.sound_world()?;
            Some(SoundUser { world, user: world.logic_user()? })
        }
    }

    /// `emitter` as a looping hud sound in `bus`: its voice, None when the
    /// scene's proxies are all taken. The voice starts at the bus's volume.
    pub fn play_looped(&self, _: &Thread, bus: BusScene, emitter: &Emitter) -> Option<Voice> {
        let mut scene = Some(bus.0);
        let mut out = None;
        // SAFETY: the game's own path for a hud sound (damage_sound_effect
        // 0x23F6A0, object_sound::insert 0x294A7B): emit_hud_sound fills
        // `out` with a new proxy holding one reference (ours, the Voice's),
        // or 0; play's mode 1 loops, with no producer and no receiver to
        // ignore. On the game's thread, the logic user's.
        unsafe {
            raw::engine_emit_hud_sound(Some(emitter.0), &mut scene, Some(self.user), &mut out);
            let proxy = out?;
            raw::engine_play_voice(Some(proxy), LOOPED, null(), null());
            Some(Voice(proxy))
        }
    }
}

/// The music's sound scene. The game never makes it current: the sound
/// thread keeps it active ([`SoundTick::keep`]). It is never let go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusScene(raw::SoundScene);

// SAFETY: the scene holds a reference of ours that is never let go, so it
// lives as long as the sound world; the game's thread emits into it and the
// sound thread ticks it, as the game's own scenes are used.
unsafe impl Send for BusScene {}

/// The bus's listener: at the origin, looking down +z, up +y. A hud sound is
/// heard the same wherever the listener is; the engine only needs one.
const ORIGIN: [f32; 3] = [0.0, 0.0, 0.0];
const FRONT: [f32; 3] = [0.0, 0.0, 1.0];
const TOP: [f32; 3] = [0.0, 1.0, 0.0];

impl BusScene {
    /// # Safety
    /// `scene` holds a reference that is now the bus's, forever.
    pub(super) unsafe fn of(scene: raw::SoundScene) -> BusScene {
        BusScene(scene)
    }

    /// Gives the bus a listener. A scene's sounds get voices only once its
    /// listener is set: its tick calls notify_listener (0x5C280, the only
    /// path to create_sound_voice) only then (0x57C9C). The game sets its
    /// own scenes' every frame; the bus's never moves, so once is enough.
    pub fn listen(&self, _: &Thread) {
        // SAFETY: the game's own call (game_world 0x1F7653, the lobby
        // 0x21D689), from the game's thread: it stores the three vectors as
        // atomic halves the sound thread reads, and sets +2FAh. The bus is
        // alive (its reference is never let go).
        unsafe { raw::engine_set_listener(Some(self.0), ORIGIN.as_ptr(), FRONT.as_ptr(), TOP.as_ptr()) }
    }
}

/// A loaded single sound, one reference of its own.
pub struct Emitter(raw::SoundEmitter);

// SAFETY: its count is atomic (the game's own takers lock xadd it); it is
// kept in a patch's lock and played and let go on the game's thread.
unsafe impl Send for Emitter {}

impl Emitter {
    /// # Safety
    /// `emitter` holds a reference that is now this Emitter's.
    pub(super) unsafe fn owning(emitter: raw::SoundEmitter) -> Emitter {
        Emitter(emitter)
    }
}

impl Drop for Emitter {
    fn drop(&mut self) {
        let mut slot = Some(self.0);
        // SAFETY: our reference, let go once, by the game's own release of
        // an unmanaged resource (0x11540: count +0D0h, deleted at 0).
        unsafe { raw::engine_release_emitter(&mut slot) }
    }
}

/// A playing hud sound (a sound_instance_proxy), one reference of its own.
/// Letting it go stops it: the proxy's free_object (0x5B390) queues its
/// destruction to the sound thread, which stops its voice, as when the game
/// lets go of a looping scene sound (object_sound::remove 0x294A90).
pub struct Voice(raw::SoundInstanceProxy);

// SAFETY: its count is not atomic (single_threading_policy): a Voice is
// kept in a patch's lock that only the game's thread takes, made and let go
// there, as the game's own proxies are.
unsafe impl Send for Voice {}

impl Drop for Voice {
    fn drop(&mut self) {
        // SAFETY: our reference, let go once on the game's thread, as
        // intrusive_ptr<sound_instance_proxy>::dec (0x56800) does: the count
        // (+28h, not atomic) down by one, the proxy's free_object at 0.
        unsafe {
            let references = self.0.references();
            *references -= 1;
            if *references == 0 {
                raw::engine_free_voice(Some(self.0));
            }
        }
    }
}

/// A scene's volume, 0 to 1 (its fade's; the engine multiplies it by the
/// general volume).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Volume(f32);

impl Volume {
    pub const SILENT: Volume = Volume(0.0);
    pub const FULL: Volume = Volume(1.0);

    /// `value` within 0..=1 (a NaN is silent).
    pub fn new(value: f32) -> Volume {
        Volume(if value >= 1.0 { 1.0 } else if value > 0.0 { value } else { 0.0 })
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// The sound thread's tick, after its orders and before its active scenes
/// tick (`sound_world.tick`).
pub struct SoundTick<'t> {
    world: raw::SoundWorld,
    elapsed: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> SoundTick<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<SoundTick<'t>> {
        // SAFETY: the site's EDI is the ticking sound world, EBX the ms
        // since its last tick, on the sound thread.
        unsafe {
            let world = raw::site::sound_world_tick::world(ctx)?;
            Some(SoundTick { world, elapsed: raw::site::sound_world_tick::elapsed(ctx), thread: PhantomData })
        }
    }

    /// The ms since the world's last tick.
    pub fn elapsed(&self) -> u32 {
        self.elapsed
    }

    /// Keeps `bus` among the active scenes at `volume`. The engine gives
    /// the scene's submix `volume x general` in this tick: the scene is left
    /// fading in, so its process_fade (0x5C060, in the scene's tick right
    /// after this) writes it, with an endless fade-in that adds nothing.
    pub fn keep(&self, bus: BusScene, volume: Volume) {
        let scene = bus.0;
        // SAFETY: on the sound thread, between its orders and its scenes'
        // ticks, the scene list is not being walked; the bus is alive (its
        // reference is never let go). add_scene_to_active (0x59FB0) appends
        // it and sets its general factor and submix volume; the caller sets
        // the active flag, as set_active_sound_scene_impl (0x5A062). The
        // game never makes the bus current, so nothing else fades it.
        unsafe {
            if *scene.active() == 0 {
                raw::engine_add_active_scene(Some(self.world), Some(scene));
                *scene.active() = 1;
            }
            *scene.volume() = volume.get();
            *scene.fade_in_time() = u32::MAX;
            *scene.fade_state() = FADING_IN;
        }
    }
}

/// A track's resource path (`music/level_01_loading`): lower-case letters,
/// digits, `_` and `/`, at most 63 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackPath {
    bytes: [u8; PATH_ROOM],
    len: u8,
}

impl TrackPath {
    /// `text` as a track path, None when it is not one.
    pub fn new(text: &CStr) -> Option<TrackPath> {
        let text = text.to_bytes();
        let fine = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'/';
        if text.is_empty() || text.len() >= PATH_ROOM || !text.iter().all(fine) {
            return None;
        }
        let mut bytes = [0; PATH_ROOM];
        bytes[..text.len()].copy_from_slice(text);
        Some(TrackPath { bytes, len: text.len() as u8 })
    }

    pub fn as_c_str(&self) -> &CStr {
        // the bytes are the text's (no zero inside, new) and a zero after
        CStr::from_bytes_with_nul(&self.bytes[..=usize::from(self.len)]).unwrap_or(c"")
    }
}

/// A mode's theme, read at `level_loading.ui` (switch_to_level_loading with
/// the mode's `ui` table): its `loading_music`, as retail's mode files have it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadingMusic {
    /// The mode names one.
    Track(TrackPath),
    /// The mode has no `loading_music` (or one that is not a track path).
    None,
}

impl LoadingMusic {
    pub fn at_level_loading(ctx: &Context<'_>) -> LoadingMusic {
        // SAFETY: the site's ESI is the mode's ui table, a node of its
        // config, which the map's descriptor holds loaded for the call.
        let table = unsafe {
            match raw::site::level_loading_ui::ui(ctx).map(|node| ConfigValue::read(node)) {
                Some(Ok(value)) => value.table(),
                _ => return LoadingMusic::None,
            }
        };
        let text = table.ok().and_then(|t| t.get(c"loading_music").ok().flatten()).and_then(|v| v.text().ok());
        text.and_then(TrackPath::new).map_or(LoadingMusic::None, LoadingMusic::Track)
    }
}

/// What a sound proxy plays, as patch 94 tags it (its +8Ch, the tail
/// padding of the proxy's pool record: fields_026g.tsv). The game's thread
/// tags a proxy where it is made, before it is played; the sound thread
/// reads the tag where its voice's volume is set. A record is cleared at
/// each allocation, so a proxy no site tags is [`Source::Untagged`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Any sound no site below tags: its volume is the game's.
    Untagged,
    /// A map object's looped sound (object_sound) or a damage zone's.
    Environment,
    /// An opponent's weapon (not its shot) or step.
    Opponent,
}

impl Source {
    fn code(self) -> u32 {
        match self {
            Source::Untagged => 0,
            Source::Environment => 1,
            Source::Opponent => 2,
        }
    }

    /// The tag's word; anything but ours is untagged.
    fn parse(code: u32) -> Source {
        match code {
            1 => Source::Environment,
            2 => Source::Opponent,
            _ => Source::Untagged,
        }
    }
}

/// The tag word of a live proxy, an atomic: the game's thread writes it,
/// the sound thread reads it.
///
/// # Safety
/// `proxy` is a live proxy record (made by the scene's pool, 144 bytes).
unsafe fn tag_of<'a>(proxy: raw::SoundInstanceProxy) -> &'a AtomicU32 {
    // SAFETY: the caller's; the word is 4-aligned in the 16-aligned record.
    unsafe { AtomicU32::from_ptr(proxy.source()) }
}

/// A proxy a tagging site has just made, not yet played.
pub struct NewProxy<'t> {
    at: raw::SoundInstanceProxy,
    thread: PhantomData<&'t Thread>,
}

impl NewProxy<'_> {
    pub fn tag(&self, source: Source) {
        // SAFETY: the site's new proxy, alive for the hook; the sound thread
        // reads the word only once a play order hands it the proxy.
        unsafe { tag_of(self.at).store(source.code(), Ordering::Release) }
    }
}

fn new_proxy<'t>(_: &'t Thread, at: Option<raw::SoundInstanceProxy>) -> Option<NewProxy<'t>> {
    at.map(|at| NewProxy { at, thread: PhantomData })
}

/// `sound_proxy.allocated`: the record new_helper has just taken, before a
/// constructor, cleared to untagged (the free list hands records back as
/// they were).
pub fn clear_allocated(ctx: &Context<'_>) {
    // SAFETY: the site's EAX is the record taken from the pool, or 0.
    if let Some(proxy) = new_proxy(ctx.thread(), unsafe { raw::site::sound_proxy_allocated::proxy(ctx) }) {
        proxy.tag(Source::Untagged);
    }
}

/// `object_sound.play`: a map object's looped spot or point sound.
pub fn object_sound<'t>(ctx: &'t Context<'_>) -> Option<NewProxy<'t>> {
    // SAFETY: the site's ESI is the object's new proxy (its +160h), alive.
    new_proxy(ctx.thread(), unsafe { raw::site::object_sound_play::proxy(ctx) })
}

/// `damage_zone.sound`: a damage zone's looped sound, if one was made.
pub fn zone_sound<'t>(ctx: &'t Context<'_>) -> Option<NewProxy<'t>> {
    // SAFETY: the site's EAX is emit_point_sound's out slot, a local of the
    // zone's frame holding the new proxy or 0.
    let proxy = unsafe { raw::site::damage_zone_sound::slot(ctx).and_then(|slot| read_proxy(slot)) };
    new_proxy(ctx.thread(), proxy)
}

/// # Safety
/// `slot` is a live out slot.
unsafe fn read_proxy(slot: raw::ProxySlot) -> Option<raw::SoundInstanceProxy> {
    // SAFETY: the caller's.
    unsafe { slot.proxy() }
}

/// What a weapon's sound is: its shot (the fire states' effects) or any
/// other (show, hide, reloads, chambering).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponSoundKind {
    Shot,
    Other,
}

/// `weapon_sound.made`: a weapon sound's new proxy (hud or point), its
/// kind and its user's place.
pub struct WeaponSound<'t> {
    pub proxy: NewProxy<'t>,
    pub kind: WeaponSoundKind,
    /// None for a place the roster has not (the user is the game's own
    /// read, 0x297E50).
    pub place: Option<Place>,
}

impl<'t> WeaponSound<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<WeaponSound<'t>> {
        use raw::site::weapon_sound_made as site;
        // SAFETY: the site's: EAX the out slot, [esp+24h] the effect, alive
        // for its callback. The effect is a member of its handler state at
        // +158h..+170h, so both words before it are inside that state; the
        // weapon's user is the game's own read (0x297E50).
        unsafe {
            let proxy = new_proxy(ctx.thread(), site::slot(ctx).and_then(|slot| read_proxy(slot)))?;
            let effect = site::frame(ctx)?.effect()?;
            let core = read(effect.core_fire_handler()) == raw::image::CORE_FIRE_SOUND_HANDLER;
            let sidearm = read(effect.sidearm_fire_handler());
            let shot = core
                || sidearm == raw::image::PISTOL_FIRE_SOUND_HANDLER
                || sidearm == raw::image::DOUBLE_FIRE_SOUND_HANDLER;
            let place = effect.weapon()?.user().and_then(|user| Place::new(u32::from(read(user.place()))));
            let kind = if shot { WeaponSoundKind::Shot } else { WeaponSoundKind::Other };
            Some(WeaponSound { proxy, kind, place })
        }
    }
}

/// The thread playing an opponent's step now (its id, 0 none): marked
/// around step_manager::on_step's two callers (`player_sound.foot_step` ..
/// `foot_stepped`, `player.landing_step` .. `landing_stepped`), so only that
/// thread's point sounds in between are the step's (a bullet task's sound
/// at the same time is not).
static STEP: AtomicU32 = AtomicU32::new(0);

/// Whose step the site plays: its player's place.
pub fn foot_step_place(ctx: &Context<'_>) -> Option<Place> {
    // SAFETY: the site's ESI is on_foot_step's effect, its player alive.
    unsafe {
        let player = raw::site::player_sound_foot_step::effect(ctx)?.player()?;
        Place::new(u32::from(read(player.place())))
    }
}

/// Whose landing the site plays: its player's place.
pub fn landing_place(ctx: &Context<'_>) -> Option<Place> {
    // SAFETY: the site's EDI is on_landing's player, alive.
    unsafe { Place::new(u32::from(read(raw::site::player_landing_step::player(ctx)?.place()))) }
}

/// This thread's next point sounds, until [`end_step`], are an opponent's step.
pub fn begin_opponent_step(_: &Thread) {
    STEP.store(thread_id(), Ordering::Relaxed);
}

/// The step's call has returned (also when it played nothing).
pub fn end_step(_: &Thread) {
    let _ = STEP.compare_exchange(thread_id(), 0, Ordering::Relaxed, Ordering::Relaxed);
}

/// Who asked emit_and_play_once for a point sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnceCaller {
    /// An opponent's step (this thread is marked).
    OpponentStep,
    /// A damage zone's one-shot (its fx, by the return address).
    DamageZone,
    Other,
}

/// `sound_emitter.once_point`: a one-shot point sound's new proxy and its
/// caller.
pub struct OncePoint<'t> {
    pub proxy: NewProxy<'t>,
    pub caller: OnceCaller,
}

impl<'t> OncePoint<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<OncePoint<'t>> {
        use raw::site::sound_emitter_once_point as site;
        // SAFETY: the site's EAX is the new proxy, EBP emit_and_play_once's
        // frame (its return address at +4).
        let (proxy, caller) = unsafe { (site::proxy(ctx), read(site::frame(ctx)?.caller())) };
        let proxy = new_proxy(ctx.thread(), proxy)?;
        let caller = if STEP.load(Ordering::Relaxed) == thread_id() {
            OnceCaller::OpponentStep
        } else if [raw::image::ZONE_SOUND_ONCE_A, raw::image::ZONE_SOUND_ONCE_B, raw::image::ZONE_SOUND_ONCE_C]
            .contains(&caller)
        {
            OnceCaller::DamageZone
        } else {
            OnceCaller::Other
        };
        Some(OncePoint { proxy, caller })
    }
}

/// `sound_voice.gain`: a voice's volume (before set_volume_impl's clamp to
/// [0, 1]) and what its proxy plays, on the sound thread.
pub struct VoiceVolume {
    pub source: Source,
    pub volume: f32,
}

impl VoiceVolume {
    pub fn at(ctx: &Context<'_>) -> Option<VoiceVolume> {
        // SAFETY: the site's EDI is the scene's propagator, its proxy alive
        // while the propagator is (the sound thread frees both).
        let proxy = unsafe { raw::site::sound_voice_gain::propagator(ctx)?.proxy()? };
        // SAFETY: as above.
        let source = Source::parse(unsafe { tag_of(proxy).load(Ordering::Acquire) });
        Some(VoiceVolume { source, volume: ctx.xmm_f32(1) })
    }

    /// The volume set_volume_impl gets (it still clamps it to [0, 1]).
    pub fn set(ctx: &mut Context<'_>, volume: f32) {
        ctx.set_xmm_f32(1, volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_paths_are_parsed_once() {
        let path = TrackPath::new(c"music/level_01_loading").expect("a track path");
        assert_eq!(path.as_c_str(), c"music/level_01_loading");
        assert_eq!(TrackPath::new(c""), None);
        assert_eq!(TrackPath::new(c"Music/x"), None);
        assert_eq!(TrackPath::new(c"music/a b"), None);
        let long = [b'a'; 64];
        let long = CStr::from_bytes_until_nul(&[&long[..], &[0]].concat()).map(TrackPath::new).ok().flatten();
        assert_eq!(long, None);
    }

    #[test]
    fn volumes_stay_within_their_range() {
        assert_eq!(Volume::new(2.0), Volume::FULL);
        assert_eq!(Volume::new(-1.0), Volume::SILENT);
        assert_eq!(Volume::new(f32::NAN), Volume::SILENT);
        assert_eq!(Volume::new(0.25).get(), 0.25);
    }
}
