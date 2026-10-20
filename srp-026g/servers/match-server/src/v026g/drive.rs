//! The input script of a bot that moves (`srp-bot --drive`, and the match
//! server's own bot places). Stage 0 of server-driven
//! player bots (`docs/notes/player_bots.md`). **For testing only.**
//!
//! In lockstep every client simulates every player from the inputs the
//! match server relays (0xEB), and a relayed input is applied unchecked,
//! as a real player's is (0x1FAB50 → 0x2D86E0; match.md §20). So a bot
//! that sends inputs walks, turns and fires in every client's world, with
//! no change to the client. This module is the script of such a bot: what
//! it holds when, and the inputs that come of it.
//!
//! The rules an input stream keeps (`docs/notes/player_bots.md` §5):
//! - one input a tick at most, on the 50 ms grid (never the precise 0xDD),
//!   and only when the keys change or there is a turn;
//! - its tick ahead of the clock by 50 to 100 ms: one past the clients'
//!   horizon is moved or dropped, one far ahead is pulled back;
//! - a turn is a change of the view, in radians, made once at the input's
//!   tick (LIKELY), first half yaw, second pitch; under π each;
//! - never the wheel's next/previous weapon (bits 25/26: every client reads
//!   past the inventory, 0x2D8D0A), never a kit choice (0xDE), never the
//!   suicide (bit 24), never the unused bits 29, 30, 31.
//!
//! The script is fixed; the bot runs it again after a pause while the match
//! lasts. Each input sent is printed with the sum of the turns sent so far,
//! which the yaw of the bot's player in the worlds must equal.

use std::f32::consts::FRAC_PI_2;

use survarium::v026g::match_msgs::{PlayerInput, TICK_MS, WireTime};

/// The action bits of an input (`actions_mask`; builder 0x241250, action
/// table 0xA74AF0). PROVEN, `docs/notes/player_bots.md` §1.
pub mod key {
    pub const FORWARD: u32 = 0x1;
    pub const BACK: u32 = 0x2;
    pub const LEFT: u32 = 0x4;
    pub const RIGHT: u32 = 0x8;
    pub const JUMP: u32 = 0x10;
    pub const FIRE: u32 = 0x20;
    pub const RELOAD: u32 = 0x40;
    pub const AIM: u32 = 0x80;
    pub const CROUCH: u32 = 0x100;
    pub const SPRINT: u32 = 0x200;
    pub const WEAPON_1: u32 = 0x1000;
    pub const WEAPON_2: u32 = 0x2000;
    /// Quick use 1..6 (bits 14-19): the n-th occupied slot in the order
    /// 13-18, 3, 0, 1, 2, 4, 5, 6 (table 0x925FD8); press and release both
    /// act. PROVEN (0x2D8D7D).
    pub const QUICK: [u32; 6] = [0x4000, 0x8000, 0x1_0000, 0x2_0000, 0x4_0000, 0x8_0000];
    /// Use (bit 21): on the key going down only, so 0 then 1. PROVEN
    /// (0x2D88B1).
    pub const USE: u32 = 0x20_0000;
    /// Drop the carried item (bit 22). PROVEN (0x2D8E73).
    pub const DROP: u32 = 0x40_0000;
    /// Throw a grenade (bit 23): held until the throw starts. PROVEN
    /// (0x2D8E05).
    pub const GRENADE: u32 = 0x80_0000;
    /// Spot (bit 28): on the key going down. PROVEN. The stock game then
    /// waited 3 s; client patch 38 has no cooldown.
    /// (0x2D877C).
    pub const SPOT: u32 = 0x1000_0000;
    /// Bits a bot never sends: the suicide (24: a hit on the player's own
    /// head, 0x2D8BC1; client patch 34 sends it for the K key), the wheel's
    /// weapon switch (25, 26) and the unused 29, 30, 31.
    pub const NEVER: u32 = 0x0100_0000 | 0x0600_0000 | 0xE000_0000;
}

/// The input's tick is this far past the clock's present tick: 50 to
/// 100 ms ahead of the clock.
const LEAD_TICKS: u32 = 2;
/// From the bot's entry to the script's start.
const START_AFTER_MS: u32 = 2_000;
/// Between two runs of the script.
const PAUSE_MS: u32 = 5_000;

/// One part of the script: the keys held for a while, and a turn made at
/// its start.
#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub ms: u32,
    pub keys: u32,
    /// Yaw and pitch change at the part's start, radians.
    pub turn: Option<(f32, f32)>,
    pub what: &'static str,
}

const fn part(ms: u32, keys: u32, what: &'static str) -> Part {
    Part {
        ms,
        keys,
        turn: None,
        what,
    }
}

/// A key pressed for one tick (a key-down action: reload, weapon).
const fn pulse(keys: u32, what: &'static str) -> Part {
    part(TICK_MS, keys, what)
}

/// The stage 0 script: walk, turn a quarter once, walk, sprint, walk
/// crouched, fire, reload, take weapon 2, turn back. The walks are long
/// enough to measure the speeds in the worlds (one every 3 to 5 s).
pub fn stage_0() -> Vec<Part> {
    use key::*;
    vec![
        part(3_000, FORWARD, "walks forward 3 s"),
        Part {
            turn: Some((FRAC_PI_2, 0.0)),
            ..part(1_000, 0, "turns yaw +pi/2 once and stands 1 s")
        },
        part(2_000, FORWARD, "walks forward 2 s"),
        part(3_000, FORWARD | SPRINT, "sprints forward 3 s"),
        part(1_000, 0, "stands 1 s"),
        part(3_000, FORWARD | CROUCH, "walks crouched 3 s"),
        part(1_000, 0, "stands 1 s"),
        part(1_000, FIRE, "fires 1 s"),
        part(500, 0, "stands 0.5 s"),
        pulse(RELOAD, "reload (one tick)"),
        part(3_000, 0, "stands 3 s"),
        pulse(WEAPON_2, "weapon 2 (one tick)"),
        part(2_000, 0, "stands 2 s"),
        Part {
            turn: Some((-FRAC_PI_2, 0.0)),
            ..part(1_000, 0, "turns yaw -pi/2 once and stands 1 s")
        },
        pulse(WEAPON_1, "weapon 1 (one tick)"),
        part(1_000, 0, "stands 1 s"),
    ]
}

/// An input the script sends, and what it is for the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    pub input: PlayerInput,
    /// Its tick's time on the match clock.
    pub time: u32,
    /// The part of the script it starts, if it starts one.
    pub what: Option<&'static str>,
    /// The turn as sent (decoded from the halves), radians.
    pub turn: (f32, f32),
    /// The sum of the yaw turns sent so far, as the halves say.
    pub yaw_sum: f32,
}

/// The script running for one bot.
pub struct Drive {
    parts: Vec<Part>,
    /// The script's length, ms.
    length: u32,
    /// The time of the first run's start, on the grid.
    start: Option<u32>,
    /// The last tick handled (its time); ticks are handled in order once.
    handled: Option<u32>,
    /// The keys of the last input sent.
    keys: u32,
    yaw_sum: f32,
}

impl Drive {
    pub fn new(parts: Vec<Part>) -> Self {
        for part in &parts {
            assert!(
                part.ms > 0 && part.ms % TICK_MS == 0,
                "{}: whole ticks",
                part.what
            );
            assert_eq!(
                part.keys & key::NEVER,
                0,
                "{}: a bit no bot sends",
                part.what
            );
            if let Some((yaw, pitch)) = part.turn {
                assert!(
                    yaw.abs() < 3.1 && pitch.abs() < 3.1,
                    "{}: under pi",
                    part.what
                );
            }
        }
        let length = parts.iter().map(|part| part.ms).sum();
        Self {
            parts,
            length,
            start: None,
            handled: None,
            keys: 0,
            yaw_sum: 0.0,
        }
    }

    /// The bot's player entered at `clock` (its own 0xE9): the script
    /// starts [`START_AFTER_MS`] later, on the grid.
    pub fn entered(&mut self, clock: u32) {
        self.start = Some((clock + START_AFTER_MS).div_ceil(TICK_MS) * TICK_MS);
    }

    pub fn started(&self) -> bool {
        self.start.is_some()
    }

    /// The part of the script at `time`, and whether `time` is its first
    /// tick; `None` before the start and in the pauses between runs.
    fn part_at(&self, time: u32) -> Option<(&Part, bool)> {
        let start = self.start?;
        let since = time.checked_sub(start)? % (self.length + PAUSE_MS);
        let mut from = 0;
        for part in &self.parts {
            if since < from + part.ms {
                return Some((part, since == from));
            }
            from += part.ms;
        }
        None
    }

    /// The inputs due with the clock at `clock`: one for each tick up to
    /// [`LEAD_TICKS`] past the present one whose keys differ from the last
    /// sent, or that starts a turn.
    pub fn due(&mut self, clock: u32) -> Vec<Sent> {
        let Some(start) = self.start else {
            return Vec::new();
        };
        let until = (clock / TICK_MS + LEAD_TICKS) * TICK_MS;
        // A tick already behind the clock is not sent: the clients have
        // played it, or are about to.
        let first = self
            .handled
            .map_or(start, |handled| handled + TICK_MS)
            .max((clock / TICK_MS + 1) * TICK_MS)
            .max(start);
        let mut sent = Vec::new();
        let mut time = first;
        while time <= until {
            let (keys, turn, what) = match self.part_at(time) {
                Some((part, first_tick)) => (
                    part.keys,
                    part.turn.filter(|_| first_tick),
                    first_tick.then_some(part.what),
                ),
                None => (0, None, None),
            };
            if keys != self.keys || turn.is_some() {
                let (yaw, pitch) = turn.unwrap_or((0.0, 0.0));
                let (yaw, pitch) = (half_bits(yaw), half_bits(pitch));
                let mut rotation = [0; 4];
                rotation[..2].copy_from_slice(&yaw.to_le_bytes());
                rotation[2..].copy_from_slice(&pitch.to_le_bytes());
                let turn = (half_value(yaw), half_value(pitch));
                self.yaw_sum += turn.0;
                self.keys = keys;
                sent.push(Sent {
                    input: PlayerInput {
                        time: WireTime::tick(time),
                        rotation,
                        actions: keys,
                    },
                    time,
                    what,
                    turn,
                    yaw_sum: self.yaw_sum,
                });
            }
            self.handled = Some(time);
            time += TICK_MS;
        }
        sent
    }
}

/// An `f32` as an IEEE half float, rounded to the nearest (ties to even),
/// as `math::half2` would store it.
pub fn half_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let abs = value.abs();
    if abs.is_nan() {
        return sign | 0x7E00;
    }
    if abs >= 65_520.0 {
        return sign | 0x7C00;
    }
    if abs < 6.103_515_6e-5 {
        // Below the smallest normal half: steps of 2^-24.
        return sign | (abs * 16_777_216.0).round_ties_even() as u16;
    }
    // A normal half's exponent: the float's is 113 or more here.
    let exponent = ((bits >> 23) & 0xFF) + 15 - 127;
    let mantissa = bits & 0x7F_FFFF;
    let mut half = (exponent << 10) | (mantissa >> 13);
    let rest = mantissa & 0x1FFF;
    if rest > 0x1000 || (rest == 0x1000 && half & 1 == 1) {
        half += 1;
    }
    sign | half as u16
}

/// An IEEE half float's value.
pub fn half_value(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 0x1F);
    let mantissa = f32::from(bits & 0x3FF);
    match exponent {
        0 => sign * mantissa * 2_f32.powi(-24),
        31 if mantissa == 0.0 => sign * f32::INFINITY,
        31 => f32::NAN,
        _ => sign * (1.0 + mantissa / 1024.0) * 2_f32.powi(exponent - 15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_are_the_ieee_ones() {
        for (value, bits) in [
            (0.0, 0x0000),
            (1.0, 0x3C00),
            (-2.0, 0xC000),
            (0.5, 0x3800),
            (65_504.0, 0x7BFF),
            (FRAC_PI_2, 0x3E48),
            (-FRAC_PI_2, 0xBE48),
        ] {
            assert_eq!(half_bits(value), bits, "{value}");
        }
        assert_eq!(half_bits(1e6), 0x7C00);
        for bits in [0x3C00_u16, 0x3E48, 0xBE48, 0x0001, 0x03FF, 0x7BFF] {
            assert_eq!(half_bits(half_value(bits)), bits, "{bits:#06x}");
        }
    }

    fn run(drive: &mut Drive, from: u32, to: u32, every: u32) -> Vec<Sent> {
        let mut sent = Vec::new();
        let mut clock = from;
        while clock <= to {
            sent.extend(drive.due(clock));
            clock += every;
        }
        sent
    }

    #[test]
    fn the_script_sends_one_input_a_tick_ahead_of_the_clock_on_change_only() {
        let mut drive = Drive::new(stage_0());
        assert!(drive.due(10_000).is_empty(), "nothing before the entry");
        drive.entered(10_013);
        let start = 12_050;
        let mut clock = 10_013;
        let mut sent: Vec<Sent> = Vec::new();
        while clock < start + drive.length + PAUSE_MS + 1_000 {
            for input in drive.due(clock) {
                // 50 to 100 ms ahead, on the grid, never twice a tick, in
                // order.
                assert!(
                    input.time > clock && input.time <= clock + 100,
                    "{input:?} at {clock}"
                );
                assert_eq!(input.time % TICK_MS, 0);
                assert!(sent.last().is_none_or(|last| last.time < input.time));
                assert!(input.input.rotation_is_sane());
                assert_eq!(input.input.actions & key::NEVER, 0);
                sent.push(input);
            }
            clock += 7;
        }
        let first = &sent[0];
        assert_eq!((first.time, first.input.actions), (start, key::FORWARD));
        assert_eq!(first.what, Some("walks forward 3 s"));
        // The turn: once, at its part's first tick, with the keys let go.
        let turns: Vec<&Sent> = sent.iter().filter(|sent| sent.turn != (0.0, 0.0)).collect();
        assert_eq!(turns.len(), 2, "one run: a quarter and back; {turns:?}");
        assert_eq!(turns[0].time, start + 3_000);
        assert_eq!(turns[0].input.rotation, [0x48, 0x3E, 0, 0]);
        assert!((turns[0].yaw_sum - 1.570_312_5).abs() < 1e-6);
        assert_eq!(turns[1].input.rotation, [0x48, 0xBE, 0, 0]);
        assert_eq!(turns[1].yaw_sum, 0.0);
        // A pulse is one tick: the key, then its release a tick later.
        let reload = sent
            .iter()
            .position(|sent| sent.input.actions == key::RELOAD)
            .unwrap();
        assert_eq!(sent[reload + 1].time, sent[reload].time + TICK_MS);
        assert_eq!(sent[reload + 1].input.actions, 0);
        // The second run starts after the pause, from the same start.
        let again = start + drive.length + PAUSE_MS;
        assert!(
            sent.iter()
                .any(|sent| sent.time == again && sent.input.actions == key::FORWARD)
        );
    }

    #[test]
    fn a_slow_pass_skips_no_pulse_and_sends_no_tick_behind_the_clock() {
        let mut drive = Drive::new(stage_0());
        drive.entered(0);
        // Passes 40 ms apart and 120 ms apart: every key change is still
        // sent, each for a tick ahead of the clock it was sent at.
        for every in [40, 120] {
            let mut drive = Drive::new(stage_0());
            drive.entered(0);
            // Two runs: 2 s, 22.65 s, a 5 s pause, 22.65 s.
            let sent = run(&mut drive, 0, 60_000, every);
            let pulses = sent
                .iter()
                .filter(|sent| sent.input.actions & (key::RELOAD | key::WEAPON_2) != 0)
                .count();
            if every <= TICK_MS {
                assert_eq!(pulses, 2 * 2, "two runs, two pulses each");
            }
            assert!(sent.windows(2).all(|pair| pair[0].time < pair[1].time));
        }
        // A pass after a long stall sends nothing for the ticks it missed.
        let sent = drive.due(2_000);
        assert!(sent.iter().all(|sent| sent.time > 2_000));
        let sent = drive.due(9_000);
        assert!(sent.iter().all(|sent| sent.time > 9_000), "{sent:?}");
    }
}
