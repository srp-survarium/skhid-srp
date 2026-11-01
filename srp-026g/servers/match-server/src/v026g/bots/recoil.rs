//! A weapon's kick ("recoil") as the client moves it: the server keeps it
//! for each bot's own weapon, to know where the bot's view is while it
//! fires and to turn so that the view, kick and all, stays on its aim.
//!
//! Read in the 0.26g client (`survarium.exe` build 2777; tags as in the
//! notes):
//! - **The kick moves the view**, and the bullet with it: the view's pitch
//!   is `clamp(m_look_pitch + recoil.y, -1, 1)` (0x2E0F60); the camera and
//!   the bullet take one matrix (0x1F3480 to 0x2422B0; `instant_fire`
//!   0x2E23A0). There is no lift of the shot over a still view. PROVEN.
//! - **A downward turn is taken off the kick first** (0x2E0FB0): with a
//!   shot fired since the weapon was taken in hand, a pitch turn `d < 0`
//!   and `y > 0`, `c = min(-d, y)`: `y -= c` and the base pitch
//!   (`m_look_pitch`, what a world saves and the inputs add to) moves by
//!   `d + c`. So a turn moves the view by exactly `d`; it only splits the
//!   view between the base pitch and the kick. PROVEN, and seen in three
//!   recorded matches (the base pitch above the turns' sum while firing,
//!   never by more than the downward turns).
//! - **A shot** (0x331C50): `y += T (1 - max(y, 0)^p)` and `x += (R - L)
//!   (1 - |x|^p)`, with `T`, `R`, `L` uniform in the weapon's config ranges
//!   (degrees, `T / 63.735`, `R` and `L / 21.48`: [`TOP_DEG_PER_UNIT`],
//!   [`SIDE_DEG_PER_UNIT`]), `p` its `recoil_power`, each times
//!   `max(0, 1 + recoil_modifier)` (0 for the bots' kit), clamped to ±1.
//!   The kick does not jump: the shot sets a speed that takes it to the
//!   new value over the shot interval `60 / rounds_per_minute` (0x331F82).
//!   PROVEN.
//! - **The weapon's tick** (0x331A50) moves the kick at that speed until
//!   the interval since the last shot is over, and for the rest of the
//!   tick a PID controller (0x3321E0, the config's gains: Kp 5, Ki 10 or
//!   15, Kd 0) pulls it back to 0, undershooting a little; a shot zeroes
//!   the controller's error and integral. The weapon ticks before the
//!   tick's shot (0x2DAAD1 before 0x2DAADB), so the controller also runs
//!   inside a burst. PROVEN.
//! - Aiming down the sights and crouching change none of it (0x331A50 and
//!   0x331C50 read neither). PROVEN. Taking the weapon in hand (0x2E2C21)
//!   and a reload (0x310E52) reset it (0x332140). PROVEN; when in the
//!   reload it happens is not traced (the bot resets as it starts one).
//!
//! What the module assumes:
//! - The bot predicts with each range's middle; the client draws from the
//!   weapon's own generator (seeded with the player's time as he takes the
//!   weapon, 0x2E2C1E), which the server does not rebuild.
//! - An automatic weapon fires at the tick its trigger goes down and every
//!   shot interval after while it is held, each shot in the tick its time
//!   falls in; a one-round weapon at the tick of the press ([`Cadence`];
//!   LIKELY: the fire state reads `rounds_per_minute` at 0x30B336, the shot
//!   is the fire clip's own event).
//! - Within a tick: the weapon's update, then the input's turn, then the
//!   tick's shots. The update before the shots is PROVEN; where the turn
//!   falls against them is not traced.
//! - The pitch's unit: the bots take it for a radian. The client's is
//!   LIKELY 60-64° (its aim animation is sampled at `(pitch + 1) / 2`, and
//!   a unit of kick is 63.735° of the config), UNKNOWN; left as it is.
//! - The side kick `x` is kept but turns nothing: it drives an animation
//!   (`recoil_horiz`) whose turn of the view is UNKNOWN, and its middle is
//!   0 for every bot weapon. The shake (`camerashake`) is not kept.

use survarium::v026g::match_msgs::TICK_MS;

/// The config's degrees of vertical kick in a unit of pitch: the console
/// variable `vertical_recoil_animation_half_range` (0xA707C8).
pub const TOP_DEG_PER_UNIT: f32 = 63.735;
/// The same of the side kick, `horizontal_recoil_animation_half_range`
/// (0xA707C4).
pub const SIDE_DEG_PER_UNIT: f32 = 21.48;

/// One weapon's `recoil` block and rate of fire
/// (`gameplay/items/weapons/wpn_<name>.options`, parsed at 0x331840).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kick {
    /// The item's name (`items_dict`).
    pub item: &'static str,
    /// `rounds_per_minute`: the shot interval is `60 / rpm` s.
    pub rpm: u32,
    /// `min/max_top_recoil`, `min/max_left_recoil`, `min/max_right_recoil`:
    /// the config's degrees.
    pub top_deg: (f32, f32),
    pub left_deg: (f32, f32),
    pub right_deg: (f32, f32),
    /// `recoil_power`.
    pub power: f32,
    /// `compensation_proportional_gain`, `_integral_gain`,
    /// `_derivational_gain`.
    pub gains: (f32, f32, f32),
}

fn within(range: (f32, f32), u: f32) -> f32 {
    range.0 + (range.1 - range.0) * u
}

impl Kick {
    /// The shot interval, seconds.
    pub fn interval_s(&self) -> f32 {
        60.0 / self.rpm as f32
    }

    /// The vertical kick of a shot drawn at `u` of its range (0.5: the
    /// middle), pitch units.
    pub fn top(&self, u: f32) -> f32 {
        within(self.top_deg, u) / TOP_DEG_PER_UNIT
    }

    /// The side kick of a shot, `R - L`, drawn at `r` and `l` of their
    /// ranges.
    pub fn side(&self, r: f32, l: f32) -> f32 {
        (within(self.right_deg, r) - within(self.left_deg, l)) / SIDE_DEG_PER_UNIT
    }
}

/// Every weapon's kick, read from the game data (`survarium::v026g::weapons`,
/// the configs' `recoil` blocks and rates of fire): a weapon a patch adds or
/// rebalances kicks by its own numbers. PROVEN (the configs).
pub fn kicks() -> &'static [Kick] {
    static ALL: std::sync::OnceLock<Vec<Kick>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| kicks_of(survarium::v026g::weapons::all()))
}

/// The kicks of the weapon table `weapons`: what [`kicks`] makes of the
/// process's own, and a test makes of a patched copy's.
pub fn kicks_of(weapons: &[survarium::v026g::weapons::WeaponParams]) -> Vec<Kick> {
    weapons
        .iter()
        .map(|w| Kick {
            item: w.name,
            rpm: w.rounds_per_minute,
            top_deg: w.top_recoil,
            left_deg: w.left_recoil,
            right_deg: w.right_recoil,
            power: w.recoil_power,
            gains: w.recoil_gains,
        })
        .collect()
}

/// The weapon whose kick a weapon the data does not know takes: the
/// AK-74M. Reached by [`Recoil::default`] (a reflex with no arms yet,
/// `Reflex::default`, until `Reflex::in_hand` gives it its weapon's) and by
/// an item that is not in the table; a `Gun` of the same table always has
/// its own kick.
pub const DEFAULT_KICK: &str = "ak_74m";

/// The kick of `item` among `all`: [`DEFAULT_KICK`]'s for one not there,
/// and the table's first when a patch has taken the AK-74M out too.
pub fn kick_in<'a>(all: &'a [Kick], item: &str) -> &'a Kick {
    all.iter()
        .find(|k| k.item == item)
        .or_else(|| all.iter().find(|k| k.item == DEFAULT_KICK))
        .or_else(|| all.first())
        .expect("the game data has a weapon")
}

/// The kick of the weapon item `item`; the AK-74M's for one not known.
pub fn kick_of(item: &str) -> &'static Kick {
    kick_in(kicks(), item)
}

/// The weapons taken in hand that have no kick of their own and took [`DEFAULT_KICK`]'s, each once
/// ([`Recoil::in_hand`]).
static NO_KICK: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// `item` has no kick of its own: said in the log the first time, so a weapon the data lacks is never silent.
fn no_kick(item: &'static str) {
    let mut seen = NO_KICK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !seen.contains(&item) {
        seen.push(item);
        log::warn!("bots: the weapon {item} has no kick in the game data; it kicks as the {DEFAULT_KICK}");
    }
}

/// The weapons taken in hand so far that had no kick of their own.
pub fn weapons_without_kick() -> Vec<&'static str> {
    NO_KICK.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// One weapon's kick as the client keeps it (0x331FF0's fields).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recoil {
    /// The weapon in hand the model is for: the kick's own item, or one the
    /// data has no kick for, which kicks as [`DEFAULT_KICK`] ([`Self::in_hand`]).
    held: &'static str,
    kick: &'static Kick,
    /// The side and the vertical kick, pitch units.
    at: [f32; 2],
    /// Their speeds, a second.
    speed: [f32; 2],
    /// The controller's last error and its integral.
    error: [f32; 2],
    integral: [f32; 2],
    /// The last shot, ms; none since the weapon was taken in hand or
    /// reset.
    last_shot: Option<f32>,
    /// The last weapon tick, ms.
    ticked: Option<u32>,
}

impl Default for Recoil {
    fn default() -> Self {
        Self::new(kick_of(DEFAULT_KICK))
    }
}

impl Recoil {
    /// The weapon of `kick` just taken in hand.
    pub fn new(kick: &'static Kick) -> Self {
        Self::of(kick.item, kick)
    }

    /// The weapon item `item` just taken in hand: its own kick, or
    /// [`DEFAULT_KICK`]'s for one the data has none for ([`kick_of`]; logged
    /// once). The model stays `item`'s ([`Self::held`]), so the weapon in hand
    /// keeps it from tick to tick either way.
    pub fn in_hand(item: &'static str) -> Self {
        let kick = kick_of(item);
        if kick.item != item {
            no_kick(item);
        }
        Self::of(item, kick)
    }

    fn of(held: &'static str, kick: &'static Kick) -> Self {
        Self {
            held,
            kick,
            at: [0.0; 2],
            speed: [0.0; 2],
            error: [0.0; 2],
            integral: [0.0; 2],
            last_shot: None,
            ticked: None,
        }
    }

    /// The weapon in hand the model is for.
    pub fn held(&self) -> &'static str {
        self.held
    }

    pub fn kick(&self) -> &'static Kick {
        self.kick
    }

    /// The vertical kick: the view is the base pitch plus this (pitch
    /// units).
    pub fn y(&self) -> f32 {
        self.at[1]
    }

    /// The side kick.
    pub fn x(&self) -> f32 {
        self.at[0]
    }

    /// The view's pitch over the base pitch `base`.
    pub fn view(&self, base: f32) -> f32 {
        (base + self.at[1]).clamp(-1.0, 1.0)
    }

    /// A reload: all back to 0, as no shot fired (0x332140).
    pub fn reset(&mut self) {
        *self = Self {
            ticked: self.ticked,
            ..Self::of(self.held, self.kick)
        };
    }

    /// The weapon's ticks up to `now`, ms, one every [`TICK_MS`] from the
    /// last (the first call only sets the clock).
    pub fn update(&mut self, now: u32) {
        let Some(mut t) = self.ticked else {
            self.ticked = Some(now);
            return;
        };
        while t < now {
            let next = (t + TICK_MS).min(now);
            self.step(t, next);
            t = next;
        }
        self.ticked = Some(t);
    }

    /// One weapon tick from `from` to `to`, ms: at the shot's speed until
    /// its interval is over, the controller for the rest.
    fn step(&mut self, from: u32, to: u32) {
        let dt = (to - from) as f32 / 1000.0;
        let interval_ms = self.kick.interval_s() * 1000.0;
        let ramp = self.last_shot.map_or(0.0, |shot| {
            ((shot + interval_ms - from as f32) / 1000.0).clamp(0.0, dt)
        });
        let rest = dt - ramp;
        let (kp, ki, kd) = self.kick.gains;
        for i in 0..2 {
            self.at[i] += self.speed[i] * ramp;
            if rest > 0.0 {
                let e = self.at[i];
                self.integral[i] += e * rest;
                let de = (e - self.error[i]) / rest;
                self.error[i] = e;
                self.speed[i] = -(kp * e + ki * self.integral[i] + kd * de);
                self.at[i] += self.speed[i] * rest;
            }
            self.at[i] = self.at[i].clamp(-1.0, 1.0);
        }
    }

    /// A shot at `at_ms` kicking `top` up and `side` across (pitch units,
    /// [`Kick::top`] and [`Kick::side`]): the kick heads for its new value
    /// over the shot interval.
    pub fn shot(&mut self, at_ms: f32, top: f32, side: f32) {
        self.last_shot = Some(at_ms);
        self.error = [0.0; 2];
        self.integral = [0.0; 2];
        let p = self.kick.power;
        let [x, y] = self.at;
        let to = [
            (x + side * (1.0 - x.abs().powf(p))).clamp(-1.0, 1.0),
            (y + top * (1.0 - y.max(0.0).powf(p))).clamp(-1.0, 1.0),
        ];
        let interval = self.kick.interval_s();
        self.speed = [(to[0] - x) / interval, (to[1] - y) / interval];
    }

    /// [`Self::shot`] with the middle of each range, as the bot predicts.
    pub fn mean_shot(&mut self, at_ms: f32) {
        let (top, side) = (self.kick.top(0.5), self.kick.side(0.5, 0.5));
        self.shot(at_ms, top, side);
    }

    /// A pitch turn `d`: the part of it the kick takes (the base pitch
    /// moves by `d` and this).
    pub fn turn(&mut self, d: f32) -> f32 {
        if self.last_shot.is_none() || d >= 0.0 || self.at[1] <= 0.0 {
            return 0.0;
        }
        let taken = (-d).min(self.at[1]);
        self.at[1] -= taken;
        taken
    }
}

/// When a trigger fires, tick by tick: an automatic weapon at the tick it
/// goes down and every shot interval after while it is held (never sooner
/// than an interval after the last shot); any other once a press, at the
/// press.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cadence {
    next: Option<f32>,
    last: Option<f32>,
    down: bool,
}

impl Cadence {
    /// The tick `now` with the trigger `held`: each shot's time, ms, to
    /// `shot`.
    pub fn tick(
        &mut self,
        now: u32,
        held: bool,
        auto: bool,
        interval_ms: f32,
        mut shot: impl FnMut(f32),
    ) {
        let now_ms = now as f32;
        if auto && held {
            let earliest = self.last.map_or(now_ms, |last| last + interval_ms);
            let mut next = self.next.unwrap_or(now_ms.max(earliest));
            while next <= now_ms + 1e-3 {
                shot(next);
                self.last = Some(next);
                next += interval_ms;
            }
            self.next = Some(next);
        } else {
            self.next = None;
            if held && !self.down {
                shot(now_ms);
                self.last = Some(now_ms);
            }
        }
        self.down = held;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns::guns;

    #[test]
    fn every_bot_weapon_has_its_kick_at_its_rate_of_fire() {
        for gun in guns() {
            let kick = kick_of(gun.item);
            assert_eq!(kick.item, gun.item);
            assert_eq!(kick.rpm, gun.rpm, "{}", gun.item);
        }
        assert_eq!(kick_of("pkm").item, "ak_74m");
        // The middles: the AK-74U's 2.1° up, nothing to either side.
        let ak = kick_of("ak_74u");
        assert!((ak.top(0.5) - 2.1 / 63.735).abs() < 1e-7);
        assert_eq!(ak.side(0.5, 0.5), 0.0);
    }

    /// The burst of the research's simulation of the client's code
    /// (`player_bots_behaviour.md` §5): AK-74M, 12 shots with the middles,
    /// the first on a tick, no turn; the view's rise in the config's
    /// degrees (units × 63.735) at 100 ... 1100 ms, then 200, 400 and
    /// 700 ms after the trigger is let go.
    #[test]
    fn an_ak_74m_burst_rises_and_comes_back_as_the_clients_code_does() {
        let mut recoil = Recoil::new(kick_of("ak_74m"));
        let mut cadence = Cadence::default();
        let interval_ms = recoil.kick().interval_s() * 1000.0;
        let mut shots = 0;
        let mut rise = Vec::new();
        for now in (0..=1_800).step_by(50) {
            recoil.update(now);
            rise.push((now, recoil.y() * TOP_DEG_PER_UNIT));
            let mut fired = Vec::new();
            cadence.tick(now, now <= 1_100, true, interval_ms, |t| fired.push(t));
            for t in fired {
                recoil.mean_shot(t);
                shots += 1;
            }
        }
        assert_eq!(shots, 12);
        let table = [
            (100, 1.8),
            (200, 3.2),
            (300, 4.1),
            (500, 4.4),
            (700, 5.7),
            (900, 7.5),
            (1_100, 7.3),
            (1_300, 1.5),
            (1_500, -0.9),
            (1_800, -1.4),
        ];
        for (at, want) in table {
            let got = rise.iter().find(|(t, _)| *t == at).unwrap().1;
            assert!((got - want).abs() <= 0.05, "{at} ms: {got} against {want}");
        }
    }

    #[test]
    fn the_controller_halves_the_kick_in_a_tenth_and_undershoots() {
        // From a kick of 1 with no shot under way: by hand, 1 - 0.05 (5 +
        // 10 × 0.05) = 0.725, then 0.725 - 0.05 (5 × 0.725 + 10 × 0.08625)
        // = 0.5006.
        let mut recoil = Recoil::new(kick_of("ak_74u"));
        recoil.update(0);
        recoil.at[1] = 1.0;
        recoil.update(50);
        assert!((recoil.y() - 0.725).abs() < 1e-5, "{}", recoil.y());
        recoil.update(100);
        assert!((recoil.y() - 0.500_6).abs() < 1e-4, "{}", recoil.y());
        // Below 0 by 19 % (Ki 10) and 25 % (Ki 15) at most, then back.
        for (item, under) in [("ak_74u", -0.194), ("uzi", -0.246)] {
            let mut recoil = Recoil::new(kick_of(item));
            recoil.update(0);
            recoil.at[1] = 1.0;
            let mut lowest: f32 = 1.0;
            for now in (50..=3_000).step_by(50) {
                recoil.update(now);
                lowest = lowest.min(recoil.y());
            }
            assert!((lowest - under).abs() < 2e-3, "{item}: {lowest}");
            assert!(recoil.y().abs() < 0.02, "{item}: {}", recoil.y());
        }
    }

    #[test]
    fn a_bolt_action_kicks_over_its_shot_interval_and_comes_back_below() {
        // The Mosin: 3° over 375 ms, back by ~0.3 s later, then 0.74° under.
        let mut recoil = Recoil::new(kick_of("mosin_nagant"));
        recoil.update(0);
        recoil.mean_shot(0.0);
        let mut rise = Vec::new();
        for now in (50..=1_000).step_by(50) {
            recoil.update(now);
            rise.push(recoil.y() * TOP_DEG_PER_UNIT);
        }
        // 3° / 375 ms: 0.4° a tick.
        assert!((rise[0] - 0.4).abs() < 1e-3 && (rise[6] - 2.8).abs() < 1e-3);
        // At 0 between 600 and 650 ms.
        assert!(rise[11] > 0.0 && rise[12] < 0.0, "{rise:?}");
        let lowest = rise.iter().copied().fold(f32::INFINITY, f32::min);
        assert!((lowest + 0.74).abs() < 0.01, "{lowest}");
        // No side kick at all.
        assert_eq!(recoil.x(), 0.0);
    }

    #[test]
    fn a_downward_turn_is_taken_off_the_kick_and_moves_the_view_by_itself() {
        let mut recoil = Recoil::new(kick_of("ak_74u"));
        recoil.update(0);
        // No shot since the weapon was taken in hand: nothing is taken.
        recoil.at[1] = 0.05;
        assert_eq!(recoil.turn(-0.02), 0.0);
        recoil.mean_shot(0.0);
        let mut base = 0.1;
        let view = recoil.view(base);
        // Down 0.02: all of it off the kick, the base stays.
        let taken = recoil.turn(-0.02);
        base += -0.02 + taken;
        assert!((taken - 0.02).abs() < 1e-7 && (base - 0.1f32).abs() < 1e-7);
        assert!((recoil.view(base) - (view - 0.02)).abs() < 1e-6);
        // Down 0.1: the kick's 0.03 left, the base the rest.
        let taken = recoil.turn(-0.1);
        base += -0.1 + taken;
        assert!((taken - 0.03).abs() < 1e-6 && recoil.y() == 0.0);
        assert!((recoil.view(base) - (view - 0.12)).abs() < 1e-6);
        // Up, or with no kick above 0: the base takes all.
        assert_eq!(recoil.turn(0.05), 0.0);
        assert_eq!(recoil.turn(-0.05), 0.0);
        // A reload: no kick and no shot.
        recoil.at[1] = 0.04;
        recoil.reset();
        assert_eq!((recoil.y(), recoil.turn(-0.01)), (0.0, 0.0));
    }

    #[test]
    fn the_cadence_fires_an_automatic_weapon_at_its_rate_and_another_at_each_press() {
        let mut cadence = Cadence::default();
        let mut shots = Vec::new();
        // 600 a minute held for 0..=300 ms: 0, 100, 200, 300.
        for now in (0..=400).step_by(50) {
            cadence.tick(now, now <= 300, true, 100.0, |t| shots.push(t));
        }
        assert_eq!(shots, vec![0.0, 100.0, 200.0, 300.0]);
        // Let go and pressed again at once: not before an interval after
        // the last shot.
        let mut cadence = Cadence::default();
        shots.clear();
        for (now, held) in [(0, true), (50, false), (100, true), (150, true)] {
            cadence.tick(now, held, true, 150.0, |t| shots.push(t));
        }
        assert_eq!(shots, vec![0.0, 150.0]);
        // One round a press, held two ticks: one shot.
        let mut cadence = Cadence::default();
        shots.clear();
        for (now, held) in [(0, true), (50, true), (100, false), (150, true)] {
            cadence.tick(now, held, false, 200.0, |t| shots.push(t));
        }
        assert_eq!(shots, vec![0.0, 150.0]);
    }
}
