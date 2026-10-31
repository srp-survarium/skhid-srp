use super::*;

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
            combat: super::super::reflex::CombatTuning::default(),
            friends: super::super::friends::FriendlyFireTuning::default(),
        }
    }
}

/// Self-balancing (play 23): the bots' K/D against humans keeps them fun.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkillTuning {
    /// The start and the floor of a tier with no row in `tiers`.
    pub start: f32,
    pub min: f32,
    /// The most, every tier (the owner's word, 2026-10-23: no tier has its
    /// own most).
    pub max: f32,
    /// By the bots' tier ([`BotTier`]): where a team's skill starts and
    /// the floor the self-balancing keeps it above (the owner, 2026-10-23:
    /// 4-5 from 0.6, floor 0.3; 9-10 from 0.75, floor 0.5; 13-15 from
    /// 0.85, floor 0.65).
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
                (BotTier::Low, (0.6, 0.3)),
                (BotTier::Mid, (0.75, 0.5)),
                (BotTier::High, (0.85, 0.65)),
            ]
            .into_iter()
            .map(|(tier, (start, min))| (tier, TierSkill { start, min }))
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
    /// Where a team of bots of `tier` starts, and its floor: the tier's
    /// row, else `start` and `min`; the start within the floor and `max`.
    pub fn of_tier(&self, tier: BotTier) -> TierSkill {
        let row = self.tiers.get(&tier).copied().unwrap_or(TierSkill {
            start: self.start,
            min: self.min,
        });
        let min = row.min.min(self.max);
        TierSkill {
            start: row.start.clamp(min, self.max),
            min,
        }
    }
}

/// A tier's start and floor of the skill.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierSkill {
    pub start: f32,
    pub min: f32,
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
                (0.0..=row.start).contains(&row.min) && row.start <= self.max,
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
                && self.combat.settle_k > 0.0,
            "aim.settle_*",
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
        Ok(())
    }
}
