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

use core::ffi::CStr;
use core::marker::PhantomData;
use core::ptr::null;

use super::config::ConfigValue;
use super::context::Context;
use super::hook::Thread;
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

impl BusScene {
    /// # Safety
    /// `scene` holds a reference that is now the bus's, forever.
    pub(super) unsafe fn of(scene: raw::SoundScene) -> BusScene {
        BusScene(scene)
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
