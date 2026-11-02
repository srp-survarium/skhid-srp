use super::*;
use crate::v026g::bots::guns::WeaponClass;

/// The aim at skill 0 (weakest) and 1 (best); `skill` mixes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AimTuning {
    /// The aim's error when it settles, degrees: a floor drawn every tick.
    pub error_deg: (f32, f32),
    /// The aim's slow wander about a tracked target: its spread (standard
    /// deviation, yaw; the pitch's is half), degrees, and how fast it
    /// comes back, seconds (a mean-reverting walk).
    pub wander_deg: (f32, f32),
    pub wander_s: f32,
    /// With `combat.settle` on ([`super::super::aim_settle`]): the wander's spread after a long time on the target, degrees,
    /// and the time constant of its settling, seconds. GUESS: skill 1 settles to nothing in a second, skill 0 to half of its
    /// wander in 3 s.
    pub settled_wander_deg: (f32, f32),
    pub settle_tau_s: (f32, f32),
    /// The time to bring the view onto a target, seconds (the motor's
    /// turn time while aiming).
    pub settle_s: (f32, f32),
    /// The noticing delay times this.
    pub notice: (f32, f32),
    /// Fire only within this of the target's direction, degrees.
    pub fire_within_deg: f32,
    /// Bursts of an automatic weapon whose class has no rows in
    /// `combat.fire` (`FireTuning::smg`, `rifle`): held, ms; and the pause
    /// after any burst, ms (the best players, 6+ matches of 2026-10-16..22:
    /// 150 / 300-350 / 600-800 ms, `player_bots_weapons.md` §3).
    pub burst_ms: (u32, u32),
    pub pause_ms: (u32, u32),
    /// Farther than this a bot does not shoot, metres; with a bolt action
    /// `combat.fire.bolt_range_m`.
    pub range_m: f32,
    /// From noticing an enemy to the first shot, ms (drawn ±15 % per
    /// target).
    pub reaction_ms: (f32, f32),
    /// [A243] How the skill mixes the aim's angles (`error_deg`, `wander_deg`,
    /// `settled_wander_deg`) and the class factors ([`ClassAim`]).
    pub skill_curve: SkillCurve,
    /// [A243] Each gun class's factors on the aim's angles at skill 0 and at
    /// skill 1: each class has its own bar (the owner, 2026-11-01).
    pub class_aim: ClassAim,
    /// The reflex's other numbers (plays 5, 7, 11, 19, 20, 21, reloads),
    /// the same at every skill.
    pub combat: super::super::reflex::CombatTuning,
    /// No shot through a teammate ([`super::super::friends`]).
    pub friends: super::super::friends::FriendlyFireTuning,
}

impl Default for AimTuning {
    fn default() -> Self {
        Self {
            // Skill 1 is the best quarter of the top players of
            // `TOP-PLAYERS.md` (the owner, 2026-10-23; it was their
            // median): a tighter floor and wander, a quicker settle,
            // reaction and noticing. The chain from a sighting to the first
            // shot at skill 1 is then ~400-450 ms (their q1 400 ms).
            error_deg: (6.0, 0.9),
            wander_deg: (3.0, 0.8),
            wander_s: 0.4,
            settled_wander_deg: (1.5, 0.0),
            settle_tau_s: (3.0, 1.0),
            settle_s: (0.35, 0.10),
            notice: (1.5, 0.6),
            fire_within_deg: 4.0,
            burst_ms: (500, 1_000),
            pause_ms: (150, 700),
            range_m: 80.0,
            reaction_ms: (450.0, 120.0),
            skill_curve: SkillCurve::default(),
            class_aim: ClassAim::default(),
            combat: super::super::reflex::CombatTuning::default(),
            friends: super::super::friends::FriendlyFireTuning::default(),
        }
    }
}

impl AimTuning {
    /// An aim angle (`range`: at skill 0, at skill 1) at `skill` with a gun of
    /// `class`: the curve's mix times the class's factor.
    pub fn angle(&self, range: (f32, f32), skill: f32, class: WeaponClass) -> f32 {
        self.skill_curve.mix(range, skill) * self.class_factor(skill, class)
    }

    /// The class's factor at `skill` ([`ClassAim`], mixed by the curve on each half). It also scales a scoped gun's settle
    /// bound (`CombatTuning::settle_k`): the shot waits until the error it expects fits that bound, so a factor on the angles
    /// alone would only lengthen the wait; on both, it is the error at the shot that the factor scales, for every class.
    pub fn class_factor(&self, skill: f32, class: WeaponClass) -> f32 {
        self.skill_curve.mix_halves(self.class_aim.of(class), skill)
    }
}

/// [A243] How the skill mixes an aim angle between skill 0 and skill 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillCurve {
    /// A straight line in degrees (before A243; `all_off.json` keeps it). The
    /// hit rate it gives is convex in the skill: the angle falls by the same
    /// degrees each step and the hits climb steeply only as it nears the body's
    /// angular size (A242, rifles + SMGs at the stand-in human: 12.8 / 14.0 /
    /// 14.6 / 20.1 / 23.2 % at 0.2 / 0.4 / 0.6 / 0.8 / 1.0).
    Linear,
    /// The same factor each step (`a0 * (a1 / a0)^skill`), so the middle skills
    /// aim tighter than the straight line and the hit rate runs nearer the
    /// owner's straight line of hit rates; an end at 0 falls back to the line.
    #[default]
    Geometric,
}

impl SkillCurve {
    pub fn mix(self, range: (f32, f32), skill: f32) -> f32 {
        match self {
            Self::Geometric if range.0 > 0.0 && range.1 > 0.0 => {
                range.0 * (range.1 / range.0).powf(skill.clamp(0.0, 1.0))
            }
            Self::Linear | Self::Geometric => super::super::signals::mix(range, skill),
        }
    }

    /// A value given at skill 0, 0.5 and 1: the curve's mix on each half. A middle on the curve between the ends (the
    /// geometric one's `sqrt(low * high)`) gives the same values as [`Self::mix`] of the ends.
    pub fn mix_halves(self, (low, middle, high): (f32, f32, f32), skill: f32) -> f32 {
        let skill = skill.clamp(0.0, 1.0);
        if skill <= 0.5 {
            self.mix((low, middle), skill * 2.0)
        } else {
            self.mix((middle, high), skill * 2.0 - 1.0)
        }
    }
}

/// [A243] Factors on the aim's angles by the gun's class, at skill 0, 0.5 and
/// 1 (1.0: the shared numbers, [`ClassAim::same`], `all_off.json`'s), mixed by
/// the skill curve on each half ([`SkillCurve::mix_halves`]). The class is the
/// bots' own (`CombatTuning::class_of`).
///
/// The defaults put each class on the owner's line (2026-11-01): skill 0 the
/// class's live median, skill 1 its bar, a straight line of hit rates between,
/// per shot on the target's line at 0-65 m (bolts 0-105 m; a shotgun per
/// pellet), never over the bar. Fitted on the bench's skill ladder with the
/// geometric curve, `combat.zoom_error` all and `combat.settle_k_low` 2.0, its
/// rates weighed to the live humans' shots by distance and target speed
/// (`a243-read.py`; the live shares and the bench's rates in the A243 PR):
/// rifles, SMGs, MGs 10.2 -> 18-20 %; shotgun ~11 -> ~20 %; marksman 18.6 ->
/// 30-32 %; bolt 31.1 -> 38-40 %. The middle point is there because two ends
/// alone bend the line by up to 1.2 points, beyond the bench's run-to-run
/// spread (automatics over it at 0.6, marksman over at 0.2-0.6, shotgun under
/// at 0.4-0.6). Pistols have no bench kit and take the automatics' factors
/// (their bar, 10.7 -> up to 25 %, is not measured). The bolt's skill-1 factor
/// is over 1 because its scope zooms the error by 0.25 (`combat.zoom_error`): at
/// 1.0 its aim leaves within his chest's half-width on 95 % of its shots and it
/// hits ~43 %, over the bar; the bench's skill-1 sweep (factor 1.1 / 2.0 / 3.0 /
/// 4.5: 40.1 / 35.4 / 32.4 / 26.9 %) puts 1.4 in the bar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClassAim {
    pub pistol: (f32, f32, f32),
    pub smg: (f32, f32, f32),
    pub rifle: (f32, f32, f32),
    pub mg: (f32, f32, f32),
    pub shotgun: (f32, f32, f32),
    pub marksman: (f32, f32, f32),
    pub bolt: (f32, f32, f32),
}

impl Default for ClassAim {
    fn default() -> Self {
        let automatic = (0.40, 0.77, 1.32);
        Self {
            pistol: automatic,
            smg: automatic,
            rifle: automatic,
            mg: automatic,
            shotgun: (0.94, 1.04, 1.30),
            marksman: (1.18, 1.24, 0.72),
            bolt: (0.74, 0.97, 1.4),
        }
    }
}

impl ClassAim {
    /// Every class on the shared numbers (before A243).
    pub fn same() -> Self {
        let same = (1.0, 1.0, 1.0);
        Self { pistol: same, smg: same, rifle: same, mg: same, shotgun: same, marksman: same, bolt: same }
    }

    /// The class's factors, to change.
    pub fn of_mut(&mut self, class: WeaponClass) -> &mut (f32, f32, f32) {
        match class {
            WeaponClass::Pistol => &mut self.pistol,
            WeaponClass::Smg => &mut self.smg,
            WeaponClass::Rifle => &mut self.rifle,
            WeaponClass::MachineGun => &mut self.mg,
            WeaponClass::Shotgun => &mut self.shotgun,
            WeaponClass::Marksman => &mut self.marksman,
            WeaponClass::Bolt => &mut self.bolt,
        }
    }

    pub fn of(&self, class: WeaponClass) -> (f32, f32, f32) {
        match class {
            WeaponClass::Pistol => self.pistol,
            WeaponClass::Smg => self.smg,
            WeaponClass::Rifle => self.rifle,
            WeaponClass::MachineGun => self.mg,
            WeaponClass::Shotgun => self.shotgun,
            WeaponClass::Marksman => self.marksman,
            WeaponClass::Bolt => self.bolt,
        }
    }
}

/// [A246, A266] The bench's calibration of each gun class's aim, from A243 (PR #1089, 2026-11-02; the geometric curve,
/// `combat.zoom_error` all, `combat.settle_k_low` 2.0; every rate a shot on the target's line, weighed to the live humans'
/// shots by distance band and target speed): per class ([`ClassCalibration`]) the owner's cap, the skill-1 sweep (factor
/// at skill 1 -> rate at skill 1) and the line the bench measured at the shipped factors (rate at skill 0, 0.2, ..., 1).
/// The admin page sets a class's skill-1 target through it and shows the expected rate at any skill (`tuning/live.rs`);
/// nothing the bots do reads it. The automatics (rifles, SMGs, machine guns) share one; pistols have no bench kit, so none.
/// Bench-measured estimates, not live measurements; a rerun of A243's ladders replaces them, and a wider range is a higher
/// cap with sweep points above it, not new code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AimCalibration {
    /// Where the numbers come from, as the page shows it.
    pub source: String,
    pub automatics: ClassCalibration,
    pub shotgun: ClassCalibration,
    pub marksman: ClassCalibration,
    pub bolt: ClassCalibration,
}

impl Default for AimCalibration {
    fn default() -> Self {
        let points = |points: &[(f32, f32)]| Calibration(points.to_vec());
        // A243's steps of the skill, and each class's rates at them ("after", weighed; two tiers' rows: their mean).
        let ladder = |rates: [f32; 6]| [0.0, 0.2, 0.4, 0.6, 0.8, 1.0].into_iter().zip(rates).collect();
        Self {
            source: "PR #1089 (A243's skill-1 sweep and per-class ladders, weighed to the live shots, 2026-11-02; \
                     tiers pooled, the sweep made non-increasing)"
                .to_owned(),
            // 13-15: 10.0 11.9 14.1 15.7 17.7 18.4; 9-10: 10.1 12.0 13.9 15.8 18.4 18.8.
            automatics: ClassCalibration {
                cap: 20.0,
                points: points(&[(1.0, 21.5), (1.1, 21.4), (1.25, 18.9), (1.32, 18.53), (1.6, 16.5), (2.0, 13.6)]),
                ladder: ladder([10.05, 11.95, 14.0, 15.75, 18.05, 18.6]),
                ladder_factors: (0.40, 0.77, 1.32),
            },
            // Per pellet. 13-15: 11.1 12.9 14.1 16.6 18.3 19.2; 9-10: 11.1 13.0 15.1 17.1 18.4 18.4.
            shotgun: ClassCalibration {
                cap: 20.0,
                points: points(&[
                    (0.8, 21.4),
                    (1.0, 20.4),
                    (1.1, 20.4),
                    (1.25, 19.45),
                    (1.3, 18.8),
                    (1.35, 18.8),
                    (1.6, 17.77),
                    (2.0, 15.4),
                ]),
                ladder: ladder([11.1, 12.95, 14.6, 16.85, 18.35, 18.8]),
                ladder_factors: (0.94, 1.04, 1.30),
            },
            // 13-15 only.
            marksman: ClassCalibration {
                cap: 32.0,
                points: points(&[(0.72, 29.52), (0.75, 29.52), (0.8, 29.52), (0.85, 29.3), (1.0, 28.75)]),
                ladder: ladder([18.7, 19.8, 22.3, 25.2, 28.4, 28.9]),
                ladder_factors: (1.18, 1.24, 0.72),
            },
            // 9-10 only; its spreads are 1.4-5.3 points (1.5-2 k shots a step), hence the dip at 0.2.
            bolt: ClassCalibration {
                cap: 40.0,
                points: points(&[(1.0, 42.4), (1.1, 40.1), (1.4, 39.1), (2.0, 35.4), (3.0, 32.4), (4.5, 26.9)]),
                ladder: ladder([32.4, 29.2, 33.4, 35.2, 35.2, 39.1]),
                ladder_factors: (0.74, 0.97, 1.4),
            },
        }
    }
}

impl Check for AimCalibration {
    /// Every class's calibration in order ([`ClassCalibration::is_ordered`]).
    fn check(&self) -> Result<(), String> {
        let classes = [&self.automatics, &self.shotgun, &self.marksman, &self.bolt];
        check(classes.iter().all(|class| class.is_ordered()), "calibration")
    }
}

impl AimCalibration {
    /// The calibration of `class`'s guns; `None` for a class with none (pistols).
    pub fn of(&self, class: WeaponClass) -> Option<&ClassCalibration> {
        match class {
            WeaponClass::Pistol => None,
            WeaponClass::Smg | WeaponClass::Rifle | WeaponClass::MachineGun => Some(&self.automatics),
            WeaponClass::Shotgun => Some(&self.shotgun),
            WeaponClass::Marksman => Some(&self.marksman),
            WeaponClass::Bolt => Some(&self.bolt),
        }
    }
}

/// One class's calibration (see [`AimCalibration`]).
///
/// The rate at skill 1 for a skill-1 factor is the sweep's, put on the ladder ([`ClassCalibration::at_skill_1`]: the
/// two measure the same point, and the ladder's end is the line's). The expected rate at a lower skill follows the
/// class's line ([`Line::at`]): the measured ladder, straight between its steps, with the skill-1 rate's change from the
/// ladder's end added on the upper half by the dial's own weight of its skill-1 point there, `2 * skill - 1` (it mixes
/// each half's two points by it): none of it under 0.5, all of it at 1. It holds for the factors and the curve the
/// ladder was measured with ([`ClassCalibration::line`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassCalibration {
    /// The owner's bar at skill 1, %: the most a target is set to.
    pub cap: f32,
    /// The skill-1 sweep: the factor at skill 1 -> the rate at skill 1.
    pub points: Calibration,
    /// The line measured at `ladder_factors` with the geometric curve: (skill, rate %), the skill rising from 0 to 1.
    pub ladder: Vec<(f32, f32)>,
    /// The class factors (skill 0, 0.5, 1) the ladder was measured with.
    pub ladder_factors: (f32, f32, f32),
}

impl ClassCalibration {
    /// The ladder's rate at `skill` (0 to 1), on the straight lines between its steps.
    fn on_ladder(&self, skill: f32) -> f32 {
        let skill = skill.clamp(0.0, 1.0);
        self.ladder
            .windows(2)
            .find_map(|pair| {
                let ((s0, r0), (s1, r1)) = (pair[0], pair[1]);
                (s0..=s1).contains(&skill).then(|| r0 + (r1 - r0) * (skill - s0) / (s1 - s0))
            })
            .unwrap_or(f32::NAN)
    }

    /// The ladder's end less the sweep's rate at the ladder's skill-1 factor: what puts a sweep's rate on the line.
    fn offset(&self) -> f32 {
        self.on_ladder(1.0) - self.points.rate_at(self.ladder_factors.2).unwrap_or(f32::NAN)
    }

    /// The rate at skill 1 with the skill-1 factor `high`; `None` outside the sweep.
    pub fn at_skill_1(&self, high: f32) -> Option<f32> {
        Some(self.points.rate_at(high)? + self.offset())
    }

    /// The skill-1 factor whose rate at skill 1 is `rate` (the inverse of [`ClassCalibration::at_skill_1`]; on a flat
    /// stretch of the sweep, its first factor); `None` outside the sweep.
    pub fn factor_for(&self, rate: f32) -> Option<f32> {
        self.points.factor_for(rate - self.offset())
    }

    /// The rates at skill 1 the sweep reaches: (lowest, highest).
    pub fn rates(&self) -> Option<(f32, f32)> {
        let (low, high) = self.points.rates()?;
        Some((low + self.offset(), high + self.offset()))
    }

    /// The class's line with `factors` (skill 0, 0.5, 1) mixed by `curve`: `None` unless the ladder was measured so (the
    /// geometric curve, its skill-0 and 0.5 factors) and the skill-1 factor is within the sweep.
    pub fn line(&self, curve: SkillCurve, (low, middle, high): (f32, f32, f32)) -> Option<Line<'_>> {
        let same = |a: f32, b: f32| (a - b).abs() <= 1e-5;
        let (ladder_low, ladder_middle, _) = self.ladder_factors;
        if curve != SkillCurve::Geometric || !same(low, ladder_low) || !same(middle, ladder_middle) {
            return None;
        }
        let shift = self.at_skill_1(high)? - self.on_ladder(1.0);
        Some(Line { calibration: self, shift })
    }

    /// A cap within 0-100 %, the sweep in order ([`Calibration::is_ordered`]) and reaching the ladder's skill-1 factor,
    /// the ladder's skills rising from 0 to 1 with rates within 0-100 %, its factors finite and over 0.
    fn is_ordered(&self) -> bool {
        let ladder = &self.ladder;
        let (low, middle, high) = self.ladder_factors;
        self.cap > 0.0
            && self.cap <= 100.0
            && self.points.is_ordered()
            && self.points.rate_at(high).is_some()
            && ladder.len() >= 2
            && ladder.first().is_some_and(|&(skill, _)| skill == 0.0)
            && ladder.last().is_some_and(|&(skill, _)| skill == 1.0)
            && ladder.windows(2).all(|pair| pair[0].0 < pair[1].0)
            && ladder.iter().all(|&(_, rate)| (0.0..=100.0).contains(&rate))
            && [low, middle, high].iter().all(|f| f.is_finite() && *f > 0.0)
    }
}

/// A class's expected line at its factors ([`ClassCalibration::line`]).
#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    calibration: &'a ClassCalibration,
    /// The skill-1 rate less the ladder's end.
    shift: f32,
}

impl Line<'_> {
    /// The expected rate at `skill` (0 to 1), %.
    pub fn at(&self, skill: f32) -> f32 {
        let weight = (2.0 * skill - 1.0).clamp(0.0, 1.0);
        self.calibration.on_ladder(skill) + weight * self.shift
    }
}

/// One class's points: (the factor at skill 1, the hit rate at skill 1 %), the factor rising, the rate never rising and
/// falling from the first point to the last ([`Calibration::is_ordered`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Calibration(pub Vec<(f32, f32)>);

impl Calibration {
    /// The rate at `factor`, on the straight lines between the points; `None` outside them.
    pub fn rate_at(&self, factor: f32) -> Option<f32> {
        self.0.windows(2).find_map(|pair| {
            let ((f0, r0), (f1, r1)) = (pair[0], pair[1]);
            (f0..=f1).contains(&factor).then(|| r0 + (r1 - r0) * (factor - f0) / (f1 - f0))
        })
    }

    /// The smallest factor whose rate is `rate`: the inverse of [`Calibration::rate_at`] (on a flat stretch, its first
    /// point); `None` outside the points' rates.
    pub fn factor_for(&self, rate: f32) -> Option<f32> {
        self.0.windows(2).find_map(|pair| {
            let ((f0, r0), (f1, r1)) = (pair[0], pair[1]);
            if !(r1..=r0).contains(&rate) {
                None
            } else if r0 == r1 {
                Some(f0)
            } else {
                Some(f0 + (f1 - f0) * (r0 - rate) / (r0 - r1))
            }
        })
    }

    /// The rates the points span: the last point's and the first's.
    pub fn rates(&self) -> Option<(f32, f32)> {
        Some((self.0.last()?.1, self.0.first()?.1))
    }

    /// Two points or more, each finite with a factor over 0 and a rate within 0-100 %; the factors rising, the rates
    /// never rising, and the last under the first.
    fn is_ordered(&self) -> bool {
        let points = &self.0;
        points.len() >= 2
            && points.iter().all(|&(f, r)| f.is_finite() && f > 0.0 && (0.0..=100.0).contains(&r))
            && points.windows(2).all(|pair| pair[0].0 < pair[1].0 && pair[0].1 >= pair[1].1)
            && points[points.len() - 1].1 < points[0].1
    }
}

/// Self-balancing (play 23): the bots' K/D against humans keeps them fun.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkillTuning {
    /// The start and the floor of a tier with no row in `tiers`.
    pub start: f32,
    pub min: f32,
    /// The top of the scale: no tier's ceiling is over it, and a tier with
    /// no row in `tiers` has it as its ceiling.
    pub max: f32,
    /// [A244] By the bots' tier ([`BotTier`]), its bracket: where a team's
    /// skill starts, and the floor and the ceiling the self-balancing keeps
    /// it between (the owner, 2026-11-01: players of level 0-5 skill 0.0-0.6
    /// from 0.3; 6-10 0.2-0.8 from 0.5; 11-15 0.3-0.9 from 0.6). On A243's
    /// line a skill is a hit rate (automatics 10.2 % at 0 to 18-20 % at 1),
    /// so a ceiling caps how well a tier's bots shoot. Before A244 (and
    /// `all_off.json`): 4-5 from 0.6, floor 0.3; 9-10 from 0.75, floor 0.5;
    /// 13-15 from 0.85, floor 0.65; one ceiling, `max`, for every tier.
    pub tiers: BTreeMap<BotTier, TierSkill>,
    /// One step's size before damping (`step / min(steps, 10)`).
    pub step: f32,
    /// The dead band of K/D against humans.
    pub kd_low: f32,
    pub kd_high: f32,
    /// How often it is looked at, and over how long, seconds.
    pub every_s: u32,
    pub window_s: u32,
    /// Fewer kills and deaths than this in the window say nothing.
    pub min_events: u32,
}

impl Default for SkillTuning {
    fn default() -> Self {
        Self {
            start: 0.7,
            min: 0.0,
            max: 1.0,
            tiers: [
                (BotTier::Low, TierSkill { start: 0.3, min: 0.0, max: 0.6 }),
                (BotTier::Mid, TierSkill { start: 0.5, min: 0.2, max: 0.8 }),
                (BotTier::High, TierSkill { start: 0.6, min: 0.3, max: 0.9 }),
            ]
            .into_iter()
            .collect(),
            step: 0.15,
            kd_low: 0.67,
            kd_high: 1.5,
            every_s: 30,
            window_s: 300,
            min_events: 4,
        }
    }
}

impl SkillTuning {
    /// A team of bots of `tier`'s bracket: the tier's row, else `start`,
    /// `min` and `max`; the ceiling at most `max`, the floor at most the
    /// ceiling, the start between them.
    pub fn of_tier(&self, tier: BotTier) -> TierSkill {
        let row = self.tiers.get(&tier).copied().unwrap_or(TierSkill {
            start: self.start,
            min: self.min,
            max: self.max,
        });
        let max = row.max.min(self.max);
        let min = row.min.min(max);
        TierSkill {
            start: row.start.clamp(min, max),
            min,
            max,
        }
    }
}

/// A tier's bracket of the skill: where it starts, its floor and its
/// ceiling (A244).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierSkill {
    pub start: f32,
    pub min: f32,
    pub max: f32,
}

impl Check for SkillTuning {
    /// The skill's ranges in order.
    fn check(&self) -> Result<(), String> {
        check(
            self.min <= self.start && self.start <= self.max,
            "skill.start",
        )?;
        for (tier, row) in &self.tiers {
            check(
                (0.0..=row.start).contains(&row.min) && row.start <= row.max && row.max <= self.max,
                &format!("skill.tiers.{tier:?}"),
            )?;
        }
        check(self.kd_low < self.kd_high, "skill.kd_*")?;
        Ok(())
    }
}

impl Check for AimTuning {
    /// The aim's ranges in order (bursts, pauses, the combat strafe and recoil).
    fn check(&self) -> Result<(), String> {
        check(self.burst_ms.0 <= self.burst_ms.1, "aim.burst_ms")?;
        check(self.pause_ms.0 <= self.pause_ms.1, "aim.pause_ms")?;
        check(self.wander_s > 0.0, "aim.wander_s")?;
        check(
            self.settle_tau_s.0 > 0.0
                && self.settle_tau_s.1 > 0.0
                && self.settled_wander_deg.0 >= 0.0
                && self.settled_wander_deg.1 >= 0.0
                && self.combat.settle_k > 0.0
                && self.combat.settle_k_low > 0.0,
            "aim.settle_*",
        )?;
        let c = &self.class_aim;
        check(
            [c.pistol, c.smg, c.rifle, c.mg, c.shotgun, c.marksman, c.bolt]
                .iter()
                .all(|&(low, middle, high)| [low, middle, high].iter().all(|f| f.is_finite() && *f > 0.0)),
            "aim.class_aim",
        )?;
        // Every class's error floor and wander shrink at every step of the skill, so its hit rate can only rise with it (the
        // owner's rule). The settled wander is left out: its skill-1 end is 0, so the curve mixes it in a straight line and a
        // rising factor lifts it by a hair at the lowest skills (automatics 0.60 -> 0.62 deg from 0 to 0.2), under the floor's
        // fall there (2.40 -> 2.13 deg).
        let classes = [
            WeaponClass::Pistol,
            WeaponClass::Smg,
            WeaponClass::Rifle,
            WeaponClass::MachineGun,
            WeaponClass::Shotgun,
            WeaponClass::Marksman,
            WeaponClass::Bolt,
        ];
        check(
            classes.iter().all(|&class| {
                [self.error_deg, self.wander_deg].iter().all(|&range| {
                    let steps: Vec<f32> = (0..=10).map(|i| self.angle(range, i as f32 / 10.0, class)).collect();
                    steps.windows(2).all(|w| w[1] <= w[0])
                })
            }),
            "aim.class_aim (an error floor or wander grows with the skill)",
        )?;
        let c = &self.combat;
        check(
            c.strafe_run_ms.0 > 0 && c.strafe_run_ms.0 <= c.strafe_run_ms.1,
            "aim.combat.strafe_run_ms",
        )?;
        check(
            c.strafe_still_ms.0 <= c.strafe_still_ms.1,
            "aim.combat.strafe_still_ms",
        )?;
        check(
            (0.0..=1.0).contains(&c.recoil_pull)
                && (0.0..=c.recoil_pull).contains(&c.recoil_pull_low),
            "aim.combat.recoil_pull",
        )?;
        check(c.worth.valid(), "aim.combat.worth")?;
        check(c.band_cross_deg.is_none_or(|deg| (0.0..90.0).contains(&deg)), "aim.combat.band_cross_deg")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [A246] The calibration maps a rate to a factor and back on its straight lines: the factor rises as the rate falls, a
    /// flat stretch gives its first factor, and a rate or factor outside the points has none. A table out of order is
    /// refused.
    #[test]
    fn the_calibration_maps_a_rate_to_its_factor_and_back() {
        let near = |got: Option<f32>, want: f32| got.is_some_and(|got| (got - want).abs() < 1e-4);
        let calibration = AimCalibration::default();
        assert_eq!(calibration.check(), Ok(()), "the shipped calibration is in order");
        let automatics = calibration.of(WeaponClass::Rifle).unwrap();
        assert_eq!(calibration.of(WeaponClass::Smg), Some(automatics));
        assert_eq!(calibration.of(WeaponClass::Pistol), None);
        // The sweep: at a point and between two.
        let points = &automatics.points;
        assert!(near(points.factor_for(18.9), 1.25));
        assert!(near(points.rate_at(1.25), 18.9));
        let twenty = points.factor_for(20.0).unwrap();
        assert!((1.1..1.25).contains(&twenty), "{twenty}");
        assert!(near(points.rate_at(twenty), 20.0));
        // Monotone: a higher rate never takes a wider factor.
        let rates: Vec<f32> = (136..=215).map(|tenth| tenth as f32 / 10.0).collect();
        let factors: Vec<f32> = rates.iter().map(|&rate| points.factor_for(rate).unwrap()).collect();
        assert!(factors.windows(2).all(|pair| pair[1] <= pair[0]), "{factors:?}");
        // Outside the points.
        assert_eq!(points.factor_for(21.6), None);
        assert_eq!(points.factor_for(13.5), None);
        assert_eq!(points.rate_at(0.9), None);
        assert_eq!(points.rate_at(2.1), None);
        // A flat stretch (the marksman's scope): its first factor.
        assert_eq!(calibration.marksman.points.factor_for(29.52), Some(0.72));
        assert!(near(calibration.shotgun.points.factor_for(20.4), 1.0));
        // On the line: the shipped skill-1 factor gives the ladder's end, and a rate maps to its factor and back.
        assert!(near(automatics.at_skill_1(1.32), 18.6));
        assert!(near(automatics.factor_for(18.6), 1.32));
        let nineteen = automatics.factor_for(19.0).unwrap();
        assert!(near(automatics.at_skill_1(nineteen), 19.0));
        assert!(near(calibration.marksman.rates().map(|(_, top)| top), 28.9));
        // Every class ships on the factors its ladder was measured with.
        let shipped = ClassAim::default();
        for class in [WeaponClass::Smg, WeaponClass::Rifle, WeaponClass::MachineGun, WeaponClass::Shotgun, WeaponClass::Marksman, WeaponClass::Bolt] {
            assert_eq!(calibration.of(class).map(|class| class.ladder_factors), Some(shipped.of(class)), "{class:?}");
        }
        // Out of order: refused.
        for bad in [
            vec![(1.0, 20.0)],
            vec![(1.0, 20.0), (1.2, 21.0)],
            vec![(1.2, 20.0), (1.0, 18.0)],
            vec![(1.0, 20.0), (1.2, 20.0)],
            vec![(0.0, 20.0), (1.2, 18.0)],
            vec![(1.0, 120.0), (1.2, 18.0)],
            vec![(1.5, 40.0), (2.0, 35.0)],
        ] {
            let mut calibration = AimCalibration::default();
            calibration.bolt.points = Calibration(bad.clone());
            assert_eq!(calibration.check(), Err("calibration".to_owned()), "{bad:?}");
        }
        // A ladder not from 0 to 1, a skill twice, a cap over 100: refused.
        let mut short = AimCalibration::default();
        short.bolt.ladder.pop();
        let mut twice = AimCalibration::default();
        twice.shotgun.ladder[2].0 = 0.2;
        let over = AimCalibration { marksman: ClassCalibration { cap: 120.0, ..AimCalibration::default().marksman }, ..AimCalibration::default() };
        for bad in [short, twice, over] {
            assert_eq!(bad.check(), Err("calibration".to_owned()), "{bad:?}");
        }
    }

    /// [A266] The expected line is the bench's ladder at the factors it was measured with; a skill-1 rate set otherwise
    /// moves its upper half by the dial's weight there, and nothing at 0.5 and under.
    #[test]
    fn the_line_is_the_ladder_at_its_factors_and_moves_its_upper_half_with_skill_1() {
        let calibration = AimCalibration::default();
        let automatics = &calibration.automatics;
        let line = |high: f32| automatics.line(SkillCurve::Geometric, (0.40, 0.77, high)).unwrap();
        let shipped = line(1.32);
        for &(skill, rate) in &automatics.ladder {
            assert!((shipped.at(skill) - rate).abs() < 1e-4, "{skill}: {}", shipped.at(skill));
        }
        assert!((shipped.at(0.3) - (11.95 + 14.0) / 2.0).abs() < 1e-4);
        // 19 % at skill 1: +0.4 there, +0.24 at 0.8 (its weight 0.6), none at 0.5 and under.
        let nineteen = line(automatics.factor_for(19.0).unwrap());
        assert!((nineteen.at(1.0) - 19.0).abs() < 1e-3);
        assert!((nineteen.at(0.8) - shipped.at(0.8) - 0.24).abs() < 1e-3);
        for skill in [0.0, 0.2, 0.4, 0.5] {
            assert_eq!(nineteen.at(skill), shipped.at(skill), "{skill}");
        }
        // The straight curve, other factors at skill 0 or 0.5 (`all_off.json`'s), a skill-1 factor outside the sweep: none.
        assert!(automatics.line(SkillCurve::Linear, (0.40, 0.77, 1.32)).is_none());
        assert!(automatics.line(SkillCurve::Geometric, (1.0, 1.0, 1.0)).is_none());
        assert!(automatics.line(SkillCurve::Geometric, (0.40, 0.8, 1.32)).is_none());
        assert!(automatics.line(SkillCurve::Geometric, (0.40, 0.77, 2.5)).is_none());
    }

    #[test]
    fn the_geometric_curve_keeps_the_ends_and_tightens_the_middle() {
        let error = (6.0, 0.9);
        for curve in [SkillCurve::Linear, SkillCurve::Geometric] {
            assert!((curve.mix(error, 0.0) - 6.0).abs() < 1e-5);
            assert!((curve.mix(error, 1.0) - 0.9).abs() < 1e-5);
        }
        // Halfway: the line's 3.45 degrees, the same factor twice 2.32 (6 x sqrt(0.15)).
        assert!((SkillCurve::Linear.mix(error, 0.5) - 3.45).abs() < 1e-4);
        assert!((SkillCurve::Geometric.mix(error, 0.5) - 6.0 * 0.15f32.sqrt()).abs() < 1e-4);
        // An end at 0 (the settled wander at skill 1): the straight line.
        assert!((SkillCurve::Geometric.mix((1.5, 0.0), 0.5) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn a_class_factor_scales_only_its_own_class() {
        let aim = AimTuning { class_aim: ClassAim { bolt: (2.0, 1.0, 0.5), ..ClassAim::same() }, ..AimTuning::default() };
        assert!((aim.angle(aim.error_deg, 1.0, WeaponClass::Rifle) - 0.9).abs() < 1e-5);
        assert!((aim.angle(aim.error_deg, 1.0, WeaponClass::Bolt) - 0.45).abs() < 1e-5);
        assert!((aim.angle(aim.error_deg, 0.0, WeaponClass::Bolt) - 12.0).abs() < 1e-5);
        assert!(aim.check().is_ok());
        let bad = AimTuning { class_aim: ClassAim { pistol: (0.0, 1.0, 1.0), ..ClassAim::same() }, ..AimTuning::default() };
        assert!(bad.check().is_err());
        // A factor that grows faster than the aim tightens makes the angle grow with the skill: refused.
        let growing = AimTuning { class_aim: ClassAim { rifle: (0.5, 1.0, 4.0), ..ClassAim::same() }, ..AimTuning::default() };
        assert!(growing.check().is_err());
    }

    #[test]
    fn the_middle_factor_bends_each_half_and_on_the_curve_changes_nothing() {
        let curve = SkillCurve::Geometric;
        // A middle on the geometric curve between the ends: the same as the two ends alone at every skill.
        let (low, high) = (0.4f32, 1.32f32);
        for i in 0..=10 {
            let s = i as f32 / 10.0;
            assert!((curve.mix_halves((low, (low * high).sqrt(), high), s) - curve.mix((low, high), s)).abs() < 1e-5, "{s}");
        }
        // Off the curve: the ends and the middle are kept, each half its own geometric mix.
        assert!((curve.mix_halves((0.4, 0.8, 1.6), 0.0) - 0.4).abs() < 1e-6);
        assert!((curve.mix_halves((0.4, 0.8, 1.6), 0.5) - 0.8).abs() < 1e-6);
        assert!((curve.mix_halves((0.4, 0.8, 1.6), 0.25) - 0.4 * 2f32.sqrt()).abs() < 1e-5);
        assert!((curve.mix_halves((0.4, 0.8, 1.6), 0.75) - 0.8 * 2f32.sqrt()).abs() < 1e-5);
        assert!((curve.mix_halves((0.4, 0.8, 1.6), 1.0) - 1.6).abs() < 1e-6);
    }

    #[test]
    fn the_default_class_factors_are_valid_and_keep_every_class_rising_with_skill() {
        let aim = AimTuning::default();
        assert!(aim.check().is_ok());
        let classes = [
            WeaponClass::Pistol,
            WeaponClass::Smg,
            WeaponClass::Rifle,
            WeaponClass::MachineGun,
            WeaponClass::Shotgun,
            WeaponClass::Marksman,
            WeaponClass::Bolt,
        ];
        // Each class's error angle falls at every step, so its hit rate can only rise with the skill (the owner's rule).
        for class in classes {
            let steps: Vec<f32> = (0..=10).map(|i| aim.angle(aim.error_deg, i as f32 / 10.0, class)).collect();
            assert!(steps.windows(2).all(|w| w[1] < w[0]), "{class:?}: {steps:?}");
        }
    }
}
