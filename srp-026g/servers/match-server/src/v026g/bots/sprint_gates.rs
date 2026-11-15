//! [2e, owner-14 row 15] What the client's keys cannot do, for the reckoned body ([`super::kin::SprintGates`],
//! [`super::kin::SpeedTerms::LegsStagger`]).
//!
//! The client (PROVEN, `sprint-jump-re/FINDINGS.md` 1.4 and 1.8): `base_player::can_sprint` 0x2DBF40 takes no sprint while the
//! hands' `can_sprint` (0x2E3A10) refuses (the aim transition not at exactly 0, so for the gun's `aim_time` after AIM goes up; the
//! weapon not idle: a reload) or a stagger is on (|player+0x300| > 1e-5 at 0x2DBFE5); a reload takes the sights down, so AIM held
//! through it does not slow the body. `hit_impl` 0x2D9B90 queues each hit's stopping speed; the update 0x2DBCD0 writes
//! -min(sum of the hits younger than `stagger_duration` 0.5 s, `max_stagger` 0.5) into the movement factor.
//!
//! The reckoning moved by keys and legs only: on the owner's 14 recordings 1,313 walking-class worlds over 0.2 m were a sprint
//! the client refused (911 a reload, 116 an aim-out), 2,615 AIM held through a reload, 2,086 a hit's stagger
//! (`drift-walking-rootcause.md` C, D, E; PROVEN). The bot knows its own reloads and its gun's aim time ([`OwnGates`], set by
//! the driver); every body's hits come with the worlds (`m_recent_hits`, the world's clock) ([`Stagger`]).

use crate::v026g::drive::key;

/// [Gap 5] A body's sprint and aim gates as a world saves them, every body's (a human's too): what `base_player::can_sprint`
/// 0x2DBF40 and the hands' `can_sprint` 0x2E3A10 read at the world. Read once at the world's boundary (`driven.rs`, from
/// srp-validate's typed views); the reckoning's use is [`super::body`]'s. The stamina's low flag is not here: it comes with the
/// stamina record (`stamina::Saved`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGates {
    /// The controller's `m_is_sprinting` (+204h, `bullet_character_controller::serialize` 0x7F7F0): the client's sprint state.
    pub sprinting: bool,
    pub hands: WorldHands,
}

/// [Gap 5] What the hands hold, for the sprint gate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldHands {
    /// No gun has a user (no gun's state machine saved with a user: a battery, a grenade, a swap's gap).
    NoGun,
    /// The one gun with a user.
    Gun(GunGate),
    /// More than one gun saved with a user, or an aim transition outside the client's values: the gate is not known.
    Unread,
}

/// [Gap 5] The gun in hands' gate: its state machine's state, its aim transition, the sprint clip it waits for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunGate {
    pub state: HandsState,
    pub aim: AimAt,
    /// `m_sprint_animation_to_wait_for` (+5ECh; srp-validate `InHand::sprint_animation_to_wait_for`, `None` for 6).
    pub sprint_wait: Option<u8>,
}

/// [Gap 5] The gun's state machine's state as the sprint gate tells them apart (`can_sprint` 0x2E3A10 takes idle, and melee
/// while a sprint clip is awaited).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandsState {
    Idle,
    Reloading,
    /// The melee state (srp-validate `StateData::Melee`: its index moves by gun).
    Melee,
    /// Show, hide, fire, chamber, throw.
    Busy,
}

impl HandsState {
    /// From the gun's saved state: its index ([`super::guns::GunAt::of_index`], idle 2 and reload 3 on every gun) and its
    /// data's class (melee's).
    pub fn of(in_hand: &srp_validate::world::InHand) -> Self {
        if matches!(in_hand.state(), srp_validate::world::StateData::Melee { .. }) {
            return Self::Melee;
        }
        match super::guns::GunAt::of_index(in_hand.state_index()) {
            super::guns::GunAt::Idle => Self::Idle,
            super::guns::GunAt::Reloading => Self::Reloading,
            super::guns::GunAt::Busy => Self::Busy,
        }
    }
}

/// [Gap 5] The gun's aim transition (`m_aim_progress`, weapon +5FCh): the hands' `can_sprint` needs its current value at
/// exactly 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AimAt {
    /// Settled at 0.
    Down,
    /// Under way (`m_start_transition_time_in_ms` saved).
    Moving {
        /// Its target is 1 (up), else 0.
        target_up: bool,
        /// `m_current_value`.
        current: f32,
        /// `m_start_transition_time_in_ms` RAW, on the weapon tick's clock (0x2E1C76 stores weapon +5A4h, from
        /// `weapon_core::tick` 0x2E09E0), not the world's: its link to the world's clock is not traced.
        start_weapon_ms: u32,
        /// `m_transition_time`, as saved.
        transition_time: f32,
    },
    /// Settled at 1.
    Up,
}

impl AimAt {
    /// From the saved transition: `None` for values the client does not use (a target other than 0 or 1, a settled value
    /// other than 0 or 1).
    pub fn of(aim: &srp_validate::world::Transition) -> Option<Self> {
        let (current, target) = (aim.current_value().get(), aim.target_value().get());
        match aim.start_transition_time_ms() {
            Some(start) => {
                let target_up = match target {
                    1.0 => true,
                    0.0 => false,
                    _ => return None,
                };
                Some(Self::Moving {
                    target_up,
                    current,
                    start_weapon_ms: start,
                    transition_time: aim.transition_time().get(),
                })
            }
            None => match current {
                0.0 => Some(Self::Down),
                1.0 => Some(Self::Up),
                _ => None,
            },
        }
    }
}

impl WorldHands {
    /// From the place's guns by slot (`PlaceWorld::guns`): the one saved with a user ([`Self::Unread`] for more than one).
    pub fn of(guns: &[(u8, srp_validate::world::WeaponState)]) -> Self {
        let mut held = guns.iter().filter_map(|(_, gun)| gun.in_hand());
        let Some(in_hand) = held.next() else {
            return Self::NoGun;
        };
        if held.next().is_some() {
            return Self::Unread;
        }
        match AimAt::of(&in_hand.aim_progress()) {
            Some(aim) => Self::Gun(GunGate {
                state: HandsState::of(&in_hand),
                aim,
                sprint_wait: in_hand.sprint_animation_to_wait_for().map(|s| s.get()),
            }),
            None => Self::Unread,
        }
    }
}

/// `stagger_duration`, ms (`default.player`'s `stagger_params`, PROVEN).
pub const STAGGER_MS: u32 = 500;
/// `max_stagger`: the most the hits take off the speed factor (PROVEN).
pub const STAGGER_MAX: f32 = 0.5;
/// Reloads kept, the newest last: the reckoning replays at most a few seconds of ticks.
const RELOADS_KEPT: usize = 4;
/// Hits kept, the newest last.
const HITS_KEPT: usize = 16;

/// A bot's own gates on its keys: its reloads (from the press to the magazine's end) and how long its sights take to come down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnGates {
    /// (from, until) ms, the newest last.
    reloads: Vec<(u32, u32)>,
    /// The gun in hands' `aim_time`, ms: after AIM goes up the sprint waits this long.
    aim_out_ms: u32,
    /// [A364 P10] The bot's reckoned bar's low flag: `can_sprint` 0x2DBF40 refuses the sprint under it (PROVEN,
    /// `docs/notes/sprint_state.md`).
    low_stamina: bool,
    /// [A388 (e)] A tube's reload loop the worlds show under way (`StateData::ShotgunReload`, state id 4): since this world's
    /// time, held until a world shows the gun out of it ([`Self::world_tube_reload`]).
    tube_since: Option<u32>,
}

impl OwnGates {
    /// A reload from `from` until `until` (ms); the same reload again changes nothing.
    pub fn reload(&mut self, from: u32, until: u32) {
        if until <= from {
            return;
        }
        if let Some(kept) = self.reloads.iter_mut().find(|kept| kept.0 == from) {
            kept.1 = until;
            return;
        }
        if self.reloads.len() >= RELOADS_KEPT && self.reloads.first().is_some_and(|oldest| from < oldest.0) {
            return;
        }
        self.reloads.push((from, until));
        self.reloads.sort_unstable_by_key(|kept| kept.0);
        if self.reloads.len() > RELOADS_KEPT {
            self.reloads.remove(0);
        }
    }

    /// [A388 (e) gap 5] A world made at `at` shows the body's gun reloading with its reload clip ending at `until` (the world's
    /// mixing tree, F55; [`super::guns::ReloadClip`]): the client's reload runs from `at` at the latest until then. A kept
    /// reload under way at `at` ends at the clip's end (the world's word over a table's); else the span is kept anew. Returns
    /// whether the gates changed.
    pub fn world_reload(&mut self, at: u32, until: u32) -> bool {
        if until <= at {
            return false;
        }
        if let Some(kept) = self.reloads.iter_mut().find(|kept| kept.0 <= at && at < kept.1) {
            let changed = kept.1 != until;
            kept.1 = until;
            return changed;
        }
        self.reload(at, until);
        true
    }

    pub fn set_aim_out_ms(&mut self, ms: u32) {
        self.aim_out_ms = ms;
    }

    pub fn set_low_stamina(&mut self, low: bool) {
        self.low_stamina = low;
    }

    pub fn aim_out_ms(&self) -> u32 {
        self.aim_out_ms
    }

    /// Whether a reload is under way at `time`.
    pub fn reloading(&self, time: u32) -> bool {
        self.tube_since.is_some_and(|since| since <= time) || self.reloads.iter().any(|&(from, until)| from <= time && time < until)
    }

    /// [A388 (e)] A world made at `at` on a tube's reload loop: `true` while the world shows the gun in it. The loop's end is
    /// not in the world (its round clips are not read), so a loop seen under way is held until a world shows it over: exact
    /// at each world, at most one world's interval long past the loop's end. Returns whether the gates changed.
    pub fn world_tube_reload(&mut self, at: u32, under_way: bool) -> bool {
        let was = self.tube_since;
        self.tube_since = match (was, under_way) {
            (Some(since), true) => Some(since),
            (None, true) => Some(at),
            (_, false) => None,
        };
        self.tube_since != was
    }

    /// The keys of a tick at `time` as the client takes them: a reload has the sights down (no AIM) and no sprint; the low
    /// stamina flag no sprint.
    pub fn taken(&self, time: u32, keys: u32) -> u32 {
        let keys = if self.reloading(time) { keys & !(key::AIM | key::SPRINT) } else { keys };
        if self.low_stamina { keys & !key::SPRINT } else { keys }
    }
}

/// A body's hits as the worlds showed them: when (the world's ms) and each hit's stopping speed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stagger {
    hits: Vec<(u32, f32)>,
}

impl Stagger {
    /// A world's `m_recent_hits`: the list replaces what an older world showed (the client keeps its own queue).
    pub fn seen(&mut self, hits: &[(u32, f32)]) {
        self.hits.clear();
        let from = hits.len().saturating_sub(HITS_KEPT);
        self.hits.extend(hits[from..].iter().copied().filter(|&(_, power)| power.is_finite() && power > 0.0));
    }

    /// The share the hits take off the speed factor at `time`: the hits younger than [`STAGGER_MS`] summed, at most
    /// [`STAGGER_MAX`]. [F4] A hit stamped at `time` itself is not in it yet (0 < age < 500 ms): the client's stagger node
    /// takes a hit on the step after its stamp (exactness S5: 1,571 worlds where a hit of the world's own time was counted a
    /// tick early).
    pub fn share(&self, time: u32) -> f32 {
        self.hits
            .iter()
            .filter(|&&(at, _)| at < time && time < at + STAGGER_MS)
            .map(|&(_, power)| power)
            .sum::<f32>()
            .min(STAGGER_MAX)
    }

    /// Whether the client takes no sprint at `time` for a stagger.
    pub fn on(&self, time: u32) -> bool {
        self.share(time) > 1e-5
    }
}

/// [A388 (e) gap 5] The worlds that showed a body reloading with its clip's end read, and those that changed its reckoned
/// body's gates, on this server (the match-end line). Decides nothing.
pub(crate) mod world_reloads {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    static READ: AtomicU64 = AtomicU64::new(0);
    static CHANGED: AtomicU64 = AtomicU64::new(0);
    static NOT_READ: AtomicU64 = AtomicU64::new(0);
    static TUBE: AtomicU64 = AtomicU64::new(0);

    pub(crate) fn read(changed: bool) {
        READ.fetch_add(1, Relaxed);
        if changed {
            CHANGED.fetch_add(1, Relaxed);
        }
    }

    /// A world showing a body in a tube's reload loop (held to the next world, [`super::OwnGates::world_tube_reload`]).
    pub(crate) fn tube() {
        TUBE.fetch_add(1, Relaxed);
    }

    /// A body reloading in a world whose reload's end is not read (a cancelled clip, no clip in the tree).
    pub(crate) fn not_read() {
        NOT_READ.fetch_add(1, Relaxed);
    }

    /// (read, changed, not read, tube) since the last call, and zero again.
    pub(crate) fn take() -> (u64, u64, u64, u64) {
        (READ.swap(0, Relaxed), CHANGED.swap(0, Relaxed), NOT_READ.swap(0, Relaxed), TUBE.swap(0, Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [A388 (e) gap 5] A world's reload refuses the sights and the sprint from the world to the clip's end; a kept reload under
    /// way at the world ends at the clip's end; the same word again changes nothing.
    /// [A388 (e)] A tube's loop seen under way refuses the sights and the sprint from that world until a world shows it over.
    #[test]
    fn a_tubes_loop_is_held_until_a_world_shows_it_over() {
        let mut gates = OwnGates::default();
        let run = key::FORWARD | key::SPRINT | key::AIM;
        assert!(!gates.world_tube_reload(500, false));
        assert!(gates.world_tube_reload(1_000, true));
        assert_eq!(gates.taken(999, run), run, "before the world that showed it");
        assert_eq!(gates.taken(1_000, run), key::FORWARD);
        assert!(!gates.world_tube_reload(1_200, true), "the loop still under way: held from its first world");
        assert_eq!(gates.taken(5_000, run), key::FORWARD);
        assert!(gates.world_tube_reload(5_100, false));
        assert_eq!(gates.taken(5_100, run), run);
    }

    #[test]
    fn a_worlds_reload_gates_to_its_clips_end() {
        let mut gates = OwnGates::default();
        let run = key::FORWARD | key::SPRINT | key::AIM;
        assert!(gates.world_reload(1_000, 2_400));
        assert_eq!(gates.taken(1_000, run), key::FORWARD);
        assert_eq!(gates.taken(2_399, run), key::FORWARD);
        assert_eq!(gates.taken(2_400, run), run);
        assert!(!gates.world_reload(1_100, 2_400), "the next world's same clip");
        gates.reload(3_000, 6_000);
        assert!(gates.world_reload(3_500, 5_200), "the clip ends sooner than the table");
        assert_eq!(gates.taken(5_200, run), run);
        assert!(!gates.world_reload(7_000, 7_000), "an end not after the world is no reload");
    }

    #[test]
    fn a_reload_takes_the_sights_and_the_sprint() {
        let mut gates = OwnGates::default();
        gates.reload(1_000, 3_000);
        gates.reload(1_000, 3_000);
        let keys = key::FORWARD | key::SPRINT | key::AIM;
        assert_eq!(gates.taken(999, keys), keys);
        assert_eq!(gates.taken(1_000, keys), key::FORWARD);
        assert_eq!(gates.taken(2_950, keys | key::FIRE), key::FORWARD | key::FIRE);
        assert_eq!(gates.taken(3_000, keys), keys);
        for k in 0..10 {
            gates.reload(10_000 + k * 5_000, 12_000 + k * 5_000);
        }
        assert!(!gates.reloading(1_500), "old reloads are let go of");
        assert!(gates.reloading(56_000));
    }

    /// The game's rule: the hits of the last half second summed, capped at a half (one shotgun pellet's 0.8 halves the speed).
    #[test]
    fn hits_stagger_for_half_a_second_capped_at_a_half() {
        let mut stagger = Stagger::default();
        stagger.seen(&[(1_000, 0.1), (1_200, 0.25), (1_300, f32::NAN)]);
        assert_eq!(stagger.share(999), 0.0);
        assert!((stagger.share(1_100) - 0.1).abs() < 1e-6);
        assert!((stagger.share(1_250) - 0.35).abs() < 1e-6);
        assert!((stagger.share(1_550) - 0.25).abs() < 1e-6);
        assert!(!stagger.on(1_700));
        stagger.seen(&[(5_000, 0.8)]);
        assert_eq!(stagger.share(5_100), STAGGER_MAX);
        assert_eq!(stagger.share(1_250), 0.0, "a new world's list replaces the old one");
    }
}
