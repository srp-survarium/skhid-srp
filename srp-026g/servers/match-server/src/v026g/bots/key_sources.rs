//! [keys] Where each change of a bot's walk keys and crouch key comes from: the stage of the drive tick that last set them
//! (`drive::drive_tick`). A debug trace that decides nothing.
//!
//! A tick's keys start as one origin's (the motor's pick of the eight-way set, another motor walk, an unstick replay) and pass
//! the drive's stages in their order (the side step, the step apart, the queue, the lip guard, the own-trap guard, the stance,
//! the reflex's keys, lining up, the riser, the jump, the carry, the legs). [`Tick`] notes the last stage that changed the walk
//! set (FORWARD, BACK, LEFT, RIGHT) and the last that changed the crouch key. [`Trail`] compares the sent keys with the last
//! tick's: a change of the walk set is counted by the owner of the set before and after it (a stage's own change, or a
//! hand-over: the lip guard let go and the motor's set came back), and so is a crouch key held for one tick only.
//! A chatter is the owner's recording measure: the walk set changing 6 or more times within 2 s; each is counted once for
//! every stage that owned a set in it.
//!
//! The counts are summed over every bot of the server and written at the match end ([`line`]); with the bot's trace on, each
//! chatter and each one-tick crouch is a trace record too ([`Trail::sent`]).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::v026g::drive::key;

/// The walk keys.
pub const WALK: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
/// A chatter: this many changes of the walk set …
pub const CHATTER_CHANGES: usize = 6;
/// … within this long, ms (the recording measure of the owner's matches, `h3_rec_measures_v3.py`).
pub const CHATTER_MS: u32 = 2_000;

/// The drive tick's sources of keys, in the order they act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    /// The unstick replay's keys (`drive::unstick_replay`).
    Unstick,
    /// The motor's eight-way set nearest the way to go (`Motor::pick_keys`).
    MotorPick,
    /// Another motor walk: a jump's line (forward only), a step by keys, a landing's cancel, a stand.
    MotorOther,
    /// The side step or the weave of an aimed walk.
    SideStep,
    /// The step apart from a teammate inside the body, or its ease.
    Apart,
    /// The queue behind a teammate.
    Queue,
    /// The lip guard's swap or brake.
    LipGuard,
    /// The own-trap guard.
    OwnTrap,
    /// The play's crouch or sneak.
    Stance,
    /// The reflex's keys (its crouch to fire, its sights) and the play's held keys and presses.
    Reflex,
    /// Lining up for a jump: the crouch let go.
    LiningUp,
    /// The riser's walk.
    Riser,
    /// The jump's press and hold (a press from a crouch holds it).
    Jump,
    /// The reflex's keys at a shot's clip end.
    Sent,
    /// The carry of a route jump through its clip.
    Carry,
    /// The legs: the creep of both legs broken and the legs' gate.
    Legs,
}

const STAGES: usize = Stage::Legs as usize + 1;

impl Stage {
    const ALL: [Stage; STAGES] = [
        Stage::Unstick,
        Stage::MotorPick,
        Stage::MotorOther,
        Stage::SideStep,
        Stage::Apart,
        Stage::Queue,
        Stage::LipGuard,
        Stage::OwnTrap,
        Stage::Stance,
        Stage::Reflex,
        Stage::LiningUp,
        Stage::Riser,
        Stage::Jump,
        Stage::Sent,
        Stage::Carry,
        Stage::Legs,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Stage::Unstick => "unstick",
            Stage::MotorPick => "motor pick",
            Stage::MotorOther => "motor other",
            Stage::SideStep => "side step",
            Stage::Apart => "apart",
            Stage::Queue => "queue",
            Stage::LipGuard => "lip guard",
            Stage::OwnTrap => "own trap",
            Stage::Stance => "stance",
            Stage::Reflex => "reflex",
            Stage::LiningUp => "lining up",
            Stage::Riser => "riser",
            Stage::Jump => "jump",
            Stage::Sent => "clip end",
            Stage::Carry => "carry",
            Stage::Legs => "legs",
        }
    }
}

/// One tick's keys as the stages pass them on.
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    /// The keys after the last stage noted, and the origin's; none before the origin is noted.
    at: Option<(u32, Stage)>,
    walk_by: Stage,
    crouch_by: Stage,
}

impl Default for Tick {
    fn default() -> Self {
        Self { at: None, walk_by: Stage::MotorOther, crouch_by: Stage::MotorOther }
    }
}

impl Tick {
    /// The tick's keys as their origin made them.
    pub fn begin(&mut self, origin: Stage, keys: u32) {
        *self = Self { at: Some((keys, origin)), walk_by: origin, crouch_by: origin };
    }

    /// The keys after `stage`: it owns the walk set or the crouch key if it changed them.
    pub fn after(&mut self, stage: Stage, keys: u32) {
        let Some((before, origin)) = self.at else {
            return;
        };
        if keys & WALK != before & WALK {
            self.walk_by = stage;
        }
        if keys & key::CROUCH != before & key::CROUCH {
            self.crouch_by = stage;
        }
        self.at = Some((keys, origin));
    }

    /// Begun (an origin noted) or begun now by `origin` with `keys`.
    pub fn begun_or(&mut self, origin: Stage, keys: u32) {
        if self.at.is_none() {
            self.begin(origin, keys);
        }
    }
}

/// One bot's sent keys over the ticks.
#[derive(Clone, Debug)]
pub struct Trail {
    sent: u32,
    walk_by: Stage,
    /// The walk set's changes within [`CHATTER_MS`]: their time and their owners before and after.
    recent: VecDeque<(u32, Stage, Stage)>,
    /// The crouch key held since this tick, by this stage, for this many ticks.
    crouch: Option<(u32, Stage, u32)>,
    /// The last tick's time: a gap (dead, not driven) starts the trail again.
    last: Option<u32>,
}

impl Default for Trail {
    fn default() -> Self {
        Self { sent: 0, walk_by: Stage::MotorOther, recent: VecDeque::new(), crouch: None, last: None }
    }
}

/// A drive tick is 50 ms; a longer gap than this, ms, is not the next tick.
const NEXT_TICK_MS: u32 = 120;

impl Trail {
    /// The keys sent this tick (`tick` the stages' owners): counted, and for a traced bot the records of a chatter or a one-tick
    /// crouch.
    pub fn sent(&mut self, tick: &Tick, keys: u32, time: u32, firing: bool, place: usize, traced: Option<&mut Vec<serde_json::Value>>) {
        let next = self.last.is_some_and(|last| time.wrapping_sub(last) <= NEXT_TICK_MS);
        self.last = Some(time);
        if !next {
            *self = Self { sent: keys, walk_by: tick.walk_by, last: Some(time), ..Self::default() };
            self.crouch = (keys & key::CROUCH != 0).then_some((time, tick.crouch_by, 1));
            return;
        }
        let mut records = Vec::new();
        if keys & WALK != self.sent & WALK {
            count_change(self.walk_by, tick.walk_by, firing);
            self.recent.push_back((time, self.walk_by, tick.walk_by));
            while self.recent.front().is_some_and(|&(at, _, _)| time.wrapping_sub(at) > CHATTER_MS) {
                self.recent.pop_front();
            }
            if self.recent.len() >= CHATTER_CHANGES {
                let mut owners = [false; STAGES];
                for &(_, before, after) in &self.recent {
                    owners[before as usize] = true;
                    owners[after as usize] = true;
                }
                count_chatter(&owners, firing);
                let words: Vec<&str> = Stage::ALL.iter().filter(|stage| owners[**stage as usize]).map(|stage| stage.word()).collect();
                let changes: Vec<String> = self.recent.iter().map(|&(at, before, after)| format!("{at} {}>{}", before.word(), after.word())).collect();
                records.push(serde_json::json!({ "t": time, "bot": place, "key_chatter": words, "changes": changes, "firing": firing }));
                self.recent.clear();
            }
        }
        self.walk_by = tick.walk_by;
        let crouched = keys & key::CROUCH != 0;
        self.crouch = match (self.crouch, crouched) {
            (None, true) => Some((time, tick.crouch_by, 1)),
            (Some((at, by, ticks)), true) => Some((at, by, ticks + 1)),
            (Some((at, by, 1)), false) => {
                count_blip(by);
                records.push(serde_json::json!({ "t": at, "bot": place, "crouch_blip": by.word(), "keys": format!("{:#x}>{:#x}", self.sent & 0x3ff, keys & 0x3ff) }));
                None
            }
            (Some(_), false) | (None, false) => None,
        };
        self.sent = keys;
        if let Some(traced) = traced {
            traced.extend(records);
        }
    }
}

/// The walk set's changes since the last match end, by the owner before and after, not firing.
static CHANGES: [AtomicU64; STAGES * STAGES] = [const { AtomicU64::new(0) }; STAGES * STAGES];
/// The changes with the trigger held.
static CHANGES_FIRING: AtomicU64 = AtomicU64::new(0);
/// Chatters not firing, and for each stage those it owned a set in.
static CHATTERS: AtomicU64 = AtomicU64::new(0);
static CHATTER_BY: [AtomicU64; STAGES] = [const { AtomicU64::new(0) }; STAGES];
static CHATTERS_FIRING: AtomicU64 = AtomicU64::new(0);
/// One-tick crouches by the stage that set the key.
static BLIPS: [AtomicU64; STAGES] = [const { AtomicU64::new(0) }; STAGES];
/// The motor's pick: sets changed after the hold (the nearest set moved on), at once (the way swung over
/// `motor::KEY_FORCE_RAD`), and picks begun with no set held.
static PICKS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

fn count_change(before: Stage, after: Stage, firing: bool) {
    if firing {
        CHANGES_FIRING.fetch_add(1, Relaxed);
    } else {
        CHANGES[before as usize * STAGES + after as usize].fetch_add(1, Relaxed);
    }
}

fn count_chatter(owners: &[bool; STAGES], firing: bool) {
    if firing {
        CHATTERS_FIRING.fetch_add(1, Relaxed);
        return;
    }
    CHATTERS.fetch_add(1, Relaxed);
    for (stage, owned) in owners.iter().enumerate() {
        if *owned {
            CHATTER_BY[stage].fetch_add(1, Relaxed);
        }
    }
}

fn count_blip(by: Stage) {
    BLIPS[by as usize].fetch_add(1, Relaxed);
}

/// How the motor's pick changed its set ([`pick`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// The nearest set moved on after the set was held its ticks.
    AfterHold,
    /// The way swung too far from the held set: let go at once.
    Forced,
    /// No set held (a walk begun, a gap): the nearest set.
    Fresh,
}

/// A change of the motor's picked set.
pub fn pick(how: Pick) {
    PICKS[how as usize].fetch_add(1, Relaxed);
}

/// The match-end lines, and the counts zero again.
pub fn lines() -> Vec<String> {
    let changes: Vec<u64> = CHANGES.iter().map(|count| count.swap(0, Relaxed)).collect();
    let total: u64 = changes.iter().sum();
    let mut pairs: Vec<(u64, usize)> = changes.iter().copied().enumerate().filter(|&(_, n)| n > 0).map(|(i, n)| (n, i)).collect();
    pairs.sort_by(|a, b| b.cmp(a));
    let pairs: Vec<String> = pairs
        .iter()
        .map(|&(n, i)| {
            let (before, after) = (Stage::ALL[i / STAGES], Stage::ALL[i % STAGES]);
            if before == after { format!("{} {n}", before.word()) } else { format!("{}>{} {n}", before.word(), after.word()) }
        })
        .collect();
    let firing = CHANGES_FIRING.swap(0, Relaxed);
    let chatters = CHATTERS.swap(0, Relaxed);
    let chatter_by: Vec<String> = Stage::ALL
        .iter()
        .filter_map(|stage| {
            let n = CHATTER_BY[*stage as usize].swap(0, Relaxed);
            (n > 0).then(|| format!("{} {n}", stage.word()))
        })
        .collect();
    let chatters_firing = CHATTERS_FIRING.swap(0, Relaxed);
    let blips: Vec<String> = Stage::ALL
        .iter()
        .filter_map(|stage| {
            let n = BLIPS[*stage as usize].swap(0, Relaxed);
            (n > 0).then(|| format!("{} {n}", stage.word()))
        })
        .collect();
    let [after_hold, forced, fresh] = [0, 1, 2].map(|i| PICKS[i].swap(0, Relaxed));
    vec![
        format!(
            "walk key changes since the last match end (every match on this server), not firing: {total}, by owner before>after: {}; firing: {firing}; the motor's pick changed its set {after_hold} after the hold, {forced} at once (the way swung), {fresh} with none held",
            pairs.join(", ")
        ),
        format!(
            "walk key chatters ({CHATTER_CHANGES}+ changes in {CHATTER_MS} ms) since the last match end (every match on this server), not firing: {chatters}, a stage owning a set in it: {}; firing: {chatters_firing}",
            chatter_by.join(", ")
        ),
        format!("crouch keys held one tick since the last match end (every match on this server), by the stage that set it: {}", blips.join(", ")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_owns_what_it_changed_and_a_hand_over_is_counted_by_both_owners() {
        let mut trail = Trail::default();
        let mut traced = Vec::new();
        let mut tick = Tick::default();
        // Walking forward by the motor's pick for a few ticks.
        for t in 0..3 {
            tick.begin(Stage::MotorPick, key::FORWARD | key::LEFT);
            trail.sent(&tick, key::FORWARD | key::LEFT, t * 50, false, 1, Some(&mut traced));
        }
        // The lip guard takes the side key away and adds the crouch for one tick.
        tick.begin(Stage::MotorPick, key::FORWARD | key::LEFT);
        tick.after(Stage::SideStep, key::FORWARD | key::LEFT);
        tick.after(Stage::LipGuard, key::FORWARD | key::CROUCH);
        assert_eq!((tick.walk_by, tick.crouch_by), (Stage::LipGuard, Stage::LipGuard));
        trail.sent(&tick, key::FORWARD | key::CROUCH, 150, false, 1, Some(&mut traced));
        // The motor's set comes back.
        tick.begin(Stage::MotorPick, key::FORWARD | key::LEFT);
        trail.sent(&tick, key::FORWARD | key::LEFT, 200, false, 1, Some(&mut traced));
        assert_eq!(traced.len(), 1, "{traced:?}");
        assert_eq!(traced[0]["crouch_blip"], "lip guard");
        assert_eq!(trail.recent.iter().map(|&(_, b, a)| (b, a)).collect::<Vec<_>>(), [(Stage::MotorPick, Stage::LipGuard), (Stage::LipGuard, Stage::MotorPick)]);
    }

    #[test]
    fn six_changes_in_two_seconds_are_a_chatter_and_a_gap_starts_again() {
        let mut trail = Trail::default();
        let mut traced = Vec::new();
        let mut tick = Tick::default();
        let sets = [key::FORWARD, key::FORWARD | key::LEFT];
        for i in 0..7u32 {
            tick.begin(Stage::MotorPick, sets[i as usize % 2]);
            trail.sent(&tick, sets[i as usize % 2], 1_000 + i * 100, false, 3, Some(&mut traced));
        }
        assert_eq!(traced.len(), 1, "{traced:?}");
        assert_eq!(traced[0]["key_chatter"], serde_json::json!(["motor pick"]));
        // A gap: no change is counted across it.
        tick.begin(Stage::MotorPick, key::BACK);
        trail.sent(&tick, key::BACK, 10_000, false, 3, Some(&mut traced));
        assert!(trail.recent.is_empty());
    }
}
