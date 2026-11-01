//! A two-clip reload (patch 90; docs/notes/tactical-reload.md): 0.26g's
//! state type 314, `pistol_weapon_core_reload_state`, which the pistols, the
//! SVD, the SKS, the SOCOM 16, the Kedr, the Kiparis and every two-clip port
//! reload with. Its hands clips are by view, crouch and magazine: the
//! magazine-not-empty (tactical) clip in slot 0, the empty one in slot 2
//! (fields_026g.tsv `hands_animations`). get_weapon_lexeme (0x30FE90) plays
//! the one of `ammo_in_magazine == 0`, as retail's does.
//!
//! The game gives both clips one speed, the tactical clip's length over
//! `parameters/reload_time` (the ctor 0x30FC50), so the empty reload lasts
//! the clips' length ratio longer. Retail 0.69d0 times them apart: the empty
//! clip in `reload_time`, the tactical one in `tactical_reload_time`
//! (pistol_weapon_core_reload_state's ctor 0x2003906D0 and its lexeme
//! 0x200390B60). Here a state whose config has both is timed so; one
//! without `tactical_reload_time` keeps the game's.

use core::ffi::CStr;

use super::config::ConfigValue;
use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;

/// How long a clip must last, in seconds: finite and above zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seconds(f32);

impl Seconds {
    pub fn parse(seconds: f32) -> Option<Seconds> {
        (seconds.is_finite() && seconds > 0.0).then_some(Seconds(seconds))
    }
}

/// A clip's speed: its length over the time it must last; finite and
/// above zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeScale(f32);

impl TimeScale {
    pub fn parse(scale: f32) -> Option<TimeScale> {
        (scale.is_finite() && scale > 0.0).then_some(TimeScale(scale))
    }
}

/// The two times a weapon's config gives its reload, as retail's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReloadTimes {
    /// `parameters/reload_time`: the empty magazine's reload.
    pub empty: Seconds,
    /// `parameters/tactical_reload_time`: with rounds left in it.
    pub tactical: Seconds,
}

/// How a state's two clips are timed. The state holds it in
/// `tactical_time_scale` (0 for `Native`); it is read only through this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReloadTiming {
    /// The game's: `time_scale` (the tactical clip's length over
    /// `reload_time`) for both clips.
    Native,
    /// Retail's: `time_scale` is the empty clip's (its length over
    /// `reload_time`), `tactical` the tactical clip's (its length over
    /// `tactical_reload_time`).
    Split { tactical: TimeScale },
}

impl ReloadTiming {
    fn parse(stored: f32) -> ReloadTiming {
        TimeScale::parse(stored).map_or(ReloadTiming::Native, |tactical| ReloadTiming::Split { tactical })
    }

    fn stored(self) -> f32 {
        match self {
            ReloadTiming::Native => 0.0,
            ReloadTiming::Split { tactical } => tactical.0,
        }
    }
}

/// The magazine as a reload starts: which clip it plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Magazine {
    /// Rounds left in it: the tactical clip.
    RoundsLeft,
    /// None: the empty clip.
    Empty,
}

impl Magazine {
    /// get_weapon_lexeme's own test (0x30FEA5: `cmp [weapon+5E5h],0`).
    fn of(rounds: u8) -> Magazine {
        if rounds == 0 { Magazine::Empty } else { Magazine::RoundsLeft }
    }
}

/// The hands clips' slots (fields_026g.tsv `hands_animations`): view 0,
/// standing; the ctor's speed is the tactical one's.
const TACTICAL_CLIP: usize = 0;
const EMPTY_CLIP: usize = 2;

/// A two-clip reload state its cook just made (`pistol_reload.constructed`):
/// its clips and the game's speed are set, the creation params still alive.
pub struct ReloadCook<'t> {
    state: raw::PistolWeaponCoreReloadState,
    params: raw::WeaponStateCreationParams,
    _thread: &'t Thread,
}

impl<'t> ReloadCook<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ReloadCook<'t>> {
        use raw::site::pistol_reload_constructed as site;
        // SAFETY: the context is this site's: the state the handler ctor
        // builds and the creation params it was given (its frame), both
        // alive for the cook's call, on its thread.
        unsafe {
            let state = site::state(ctx)?;
            let params = site::frame(ctx)?.params()?;
            Some(ReloadCook { state, params, _thread: ctx.thread() })
        }
    }

    /// The weapon config's reload times (its modifications applied, as the
    /// game read `reload_time` from it): None when it has no
    /// `tactical_reload_time`, or either is no time.
    pub fn times(&self) -> Option<ReloadTimes> {
        // SAFETY: the params and the config they hold are alive for the
        // cook's call; the config is loaded and relocated.
        unsafe {
            let empty = Seconds::parse(read(self.params.reload_time()))?;
            let root = self.params.config()?.root()?;
            let parameters = ConfigValue::read(root).ok()?.table().ok()?.get(c"parameters").ok()??.table().ok()?;
            let tactical = parameters.get(TACTICAL_RELOAD_TIME).ok()??.number().ok()?;
            Some(ReloadTimes { empty, tactical: Seconds::parse(tactical)? })
        }
    }

    /// Times the clips: with `times`, the empty clip in `empty` and the
    /// tactical one in `tactical` (each its length over its time, as the
    /// game's ctor computes its one speed); without, the game's. Either
    /// clip missing, or a speed that is none, keeps the game's.
    pub fn time(&self, times: Option<ReloadTimes>) -> ReloadTiming {
        let split = times.and_then(|times| Some((self.speed(EMPTY_CLIP, times.empty)?, self.speed(TACTICAL_CLIP, times.tactical)?)));
        let timing = match split {
            Some((empty, tactical)) => {
                // SAFETY: the state is the one the cook builds, alive and
                // this thread's; time_scale is read only by get_weapon_lexeme.
                unsafe { self.state.time_scale().write_volatile(empty.0) };
                ReloadTiming::Split { tactical }
            }
            None => ReloadTiming::Native,
        };
        // SAFETY: as above; tactical_time_scale is ours (the class's tail
        // padding, fields_026g.tsv), read only by ReloadLexeme.
        unsafe { self.state.tactical_time_scale().write_volatile(timing.stored()) };
        timing
    }

    /// Clip `slot`'s speed to last `time`, by the game's own
    /// computed_show_animation_time_scale (0x30D750).
    fn speed(&self, slot: usize, time: Seconds) -> Option<TimeScale> {
        // SAFETY: the slot is one of the 8 the ctor filled; its clip is
        // loaded (the cook's subresources are ready) and the engine function
        // only reads it, taking and dropping a count.
        unsafe {
            let clip = self.state.hands_animations().at(slot);
            if read(clip.resource()) == 0 {
                return None;
            }
            TimeScale::parse(raw::engine_animation_time_scale(Some(clip), time.0))
        }
    }
}

const TACTICAL_RELOAD_TIME: &CStr = c"tactical_reload_time";

/// A two-clip reload's hands clip being made (`pistol_reload.lexeme_time_scale`):
/// its speed is apply_modifier's value in the frame, not yet modified.
pub struct ReloadLexeme<'t> {
    state: raw::PistolWeaponCoreReloadState,
    frame: raw::ReloadLexemeFrame,
    _thread: &'t Thread,
}

impl<'t> ReloadLexeme<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ReloadLexeme<'t>> {
        use raw::site::pistol_reload_lexeme_time_scale as site;
        // SAFETY: the context is this site's: the state get_weapon_lexeme
        // runs for and its own frame, alive while it runs, on its thread.
        unsafe { Some(ReloadLexeme { state: site::state(ctx)?, frame: site::frame(ctx)?, _thread: ctx.thread() }) }
    }

    pub fn timing(&self) -> ReloadTiming {
        // SAFETY: the state is alive; its every construction went through
        // ReloadCook::time, which wrote the field.
        ReloadTiming::parse(unsafe { read(self.state.tactical_time_scale()) })
    }

    /// The weapon's magazine as get_weapon_lexeme tests it; None without a
    /// weapon (the game then reads none either).
    pub fn magazine(&self) -> Option<Magazine> {
        // SAFETY: the state's weapon is alive while its state plays.
        unsafe { Some(Magazine::of(read(self.state.weapon()?.ammo_in_magazine()))) }
    }

    /// The hands clip's speed before the player's reload modifier.
    pub fn set_time_scale(&self, scale: TimeScale) {
        // SAFETY: the frame's slot is apply_modifier's value argument,
        // get_weapon_lexeme's to write until the call.
        unsafe { self.frame.time_scale().write_volatile(scale.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_speeds_are_positive_and_finite() {
        assert_eq!(Seconds::parse(2.2), Some(Seconds(2.2)));
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(Seconds::parse(bad), None);
            assert_eq!(TimeScale::parse(bad), None);
        }
    }

    #[test]
    fn the_stored_timing_reads_back() {
        let split = ReloadTiming::Split { tactical: TimeScale(1.75) };
        assert_eq!(ReloadTiming::parse(split.stored()), split);
        assert_eq!(ReloadTiming::parse(ReloadTiming::Native.stored()), ReloadTiming::Native);
        assert_eq!(ReloadTiming::parse(f32::NAN), ReloadTiming::Native);
    }

    #[test]
    fn the_magazine_is_the_games_test() {
        assert_eq!(Magazine::of(0), Magazine::Empty);
        assert_eq!(Magazine::of(1), Magazine::RoundsLeft);
        assert_eq!(Magazine::of(30), Magazine::RoundsLeft);
    }
}
