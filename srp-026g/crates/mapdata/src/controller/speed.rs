//! The client's ground speed: how far the walk vector may go a tick (A313; read from the 0.26g exe and its configs, notes in
//! `private/grid/controller/dis/notes-speed.md` and `notes-walkvector.md`).
//!
//! The body moves by the animation's root motion; `base_player::movement_speed` 0x2DBB50 scales the clip's playback to the
//! state's desired speed times the movement factor `F`, never under the state's minimal speed, and
//! `set_physics_controller_walk_vector` 0x2D9060 caps the flat step at `max_speed` 0x2DC010 (the walk's or the sprint's
//! forward speed times `F`). `F = max(0, 1 + S4)`, `S4` the sum of the movement-speed modifiers (index 4): skill 208, the
//! weapon in hands, the carried weight, each leg's wound, the stagger of recent hits, a carried battery, an artefact. Armour
//! (0 in every live config), stamina, drugs, slope and health in general do not change it (PROVEN by their absence in the
//! code). The main branch's client patch 94 adds a low-health factor ("adrenalin"); it is not in the 0.26g exe this reads.

/// How the body stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    Stand,
    Crouch,
}

/// Which way the keys move it against its facing: a forward diagonal counts as forward, a back diagonal as back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Forward,
    Side,
    Back,
}

/// The movement state: sprinting is forward and standing only (the sprint gates of `base_player::can_sprint` 0x2DBF40 are the
/// caller's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Move { stance: Stance, heading: Heading, aiming: bool },
    Sprint,
}

/// One leg's wound (the leg's own speed node, 0x32A9C0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Leg {
    #[default]
    Sound,
    /// Under 20 % of its health: -0.1.
    Hurt,
    /// At 0: -0.2.
    Broken,
}

impl Leg {
    fn term(self) -> f32 {
        match self {
            Leg::Sound => 0.0,
            Leg::Hurt => -0.1,
            Leg::Broken => -0.2,
        }
    }
}

/// What the modifier sum `S4` is made of, for one body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Modifiers {
    /// Skill 208 levels (+0.01 a level).
    pub skill_208: u8,
    /// The weapon in hands' own `movement_speed_modifier` (+0.05 / 0 / -0.05 in the live configs).
    pub weapon: f32,
    /// Skill 112 (a weapon value of 0 or less is not added).
    pub skill_112: bool,
    /// The carried weight, kg, and what raises the weight cap: skill 209 levels (+2 kg each) and items' kg.
    pub weight_kg: f32,
    pub skill_209: u8,
    pub item_capacity_kg: f32,
    /// Skill 202 levels (-0.05 a level on the weight's slowdown).
    pub skill_202: u8,
    /// Skill 214: no weight slowdown (and no sprint).
    pub skill_214: bool,
    pub legs: [Leg; 2],
    /// The stagger of hits younger than 0.5 s, as a positive amount (capped at 0.5).
    pub stagger: f32,
    /// A battery carried (-0.05).
    pub battery: bool,
    /// The artefact "spring": its passive (+0.02) and active (+0.1 for 10 s) parts.
    pub spring_passive: bool,
    pub spring_active: bool,
    /// The aiming modifier sum `S20` (skill 102 is +0.01 a level, and has no effect: aiming takes `min(F, F + S20)`).
    pub aim_sum: f32,
}

/// `min_carried_weight` and `max_carried_weight` of `default.player`, kg, and the weight's full slowdown.
const WEIGHT_FREE_KG: f32 = 20.0;
const WEIGHT_CAP_KG: f32 = 30.0;
const WEIGHT_SLOWDOWN: f32 = -0.2;

impl Modifiers {
    /// The weight's node 0x329B20: `(1 - t) * -m + m`, `t` the weight's share between free and cap (clamped; a NaN share is 1),
    /// `m = -0.2 * max(0, 1 - 0.05 * skill 202)`; 0 with skill 214.
    pub fn weight_term(&self) -> f32 {
        if self.skill_214 {
            return 0.0;
        }
        let cap = WEIGHT_CAP_KG + f32::from(self.skill_209) * 2.0 + self.item_capacity_kg;
        let mut t = (self.weight_kg - WEIGHT_FREE_KG) / (cap - WEIGHT_FREE_KG);
        // comiss/jc: a NaN share passes this test and becomes 1 at the next.
        t = if 0.0 < t || t.is_nan() { t } else { 0.0 };
        t = if 1.0 >= t { t } else { 1.0 };
        let m = WEIGHT_SLOWDOWN * (1.0 + f32::from(self.skill_202) * -0.05).max(0.0);
        (1.0 - t) * (-m) + m
    }

    /// `S4`, the sum of the movement-speed modifiers.
    pub fn sum(&self) -> f32 {
        let weapon = if self.skill_112 && self.weapon <= 0.0 { 0.0 } else { self.weapon };
        f32::from(self.skill_208) * 0.01
            + weapon
            + self.weight_term()
            + self.legs[0].term()
            + self.legs[1].term()
            - self.stagger.clamp(0.0, 0.5)
            + if self.battery { -0.05 } else { 0.0 }
            + if self.spring_passive { 0.02 } else { 0.0 }
            + if self.spring_active { 0.1 } else { 0.0 }
    }

    /// `F = max(0, 1 + S4)`.
    pub fn factor(&self) -> f32 {
        (1.0 + self.sum()).max(0.0)
    }
}

/// `default.player`'s `movement_speed`: (desired, minimal) m/s by state (PROVEN config).
fn desired_minimal(gait: Gait) -> (f32, f32) {
    match gait {
        Gait::Sprint => (5.0, 1.25),
        Gait::Move { stance, heading, aiming } => match (stance, aiming, heading) {
            (Stance::Stand, false, Heading::Forward) => (3.5, 0.875),
            (Stance::Stand, false, Heading::Side) => (2.5, 0.625),
            (Stance::Stand, false, Heading::Back) => (3.0, 0.75),
            (Stance::Stand, true, Heading::Forward) => (3.0, 0.75),
            (Stance::Stand, true, Heading::Side) => (2.0, 0.5),
            (Stance::Stand, true, Heading::Back) => (2.5, 0.625),
            (Stance::Crouch, false, Heading::Forward) => (2.5, 0.625),
            (Stance::Crouch, false, Heading::Side) => (2.0, 0.5),
            (Stance::Crouch, false, Heading::Back) => (2.0, 0.5),
            (Stance::Crouch, true, Heading::Forward) => (2.0, 0.5),
            (Stance::Crouch, true, Heading::Side) => (1.5, 0.375),
            (Stance::Crouch, true, Heading::Back) => (1.5, 0.375),
        },
    }
}

/// `max_speed` 0x2DC010: the flat step's cap, m/s.
pub fn cap(gait: Gait, f: f32) -> f32 {
    let (d, m) = desired_minimal(if gait == Gait::Sprint {
        Gait::Sprint
    } else {
        Gait::Move { stance: Stance::Stand, heading: Heading::Forward, aiming: false }
    });
    (d * f).max(m)
}

/// The ground speed of `gait` for a body with `modifiers`, m/s: the state's desired speed times `F` (aimed: times
/// `min(F, max(0, F + S20))`), never under its minimal speed, never over the cap. The animation's blend between states (0.3 s)
/// is not in it.
pub fn ground_speed(gait: Gait, modifiers: &Modifiers) -> f32 {
    let f = modifiers.factor();
    let (desired, minimal) = desired_minimal(gait);
    let scale = match gait {
        Gait::Move { aiming: true, .. } => f.min((f + modifiers.aim_sum).max(0.0)),
        _ => f,
    };
    (desired * scale).max(minimal).min(cap(gait, f))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: Gait = Gait::Move { stance: Stance::Stand, heading: Heading::Forward, aiming: false };

    /// The bare body's speeds are the config's desired ones; a +0.05 weapon makes the sprint 5.25 (the 5.2-5.3 m/s seen in
    /// recordings); skill 102 changes nothing.
    #[test]
    fn the_speeds_are_the_clients() {
        let bare = Modifiers::default();
        assert_eq!(ground_speed(RUN, &bare), 3.5);
        assert_eq!(ground_speed(Gait::Sprint, &bare), 5.0);
        let crouch_side = Gait::Move { stance: Stance::Crouch, heading: Heading::Side, aiming: false };
        assert_eq!(ground_speed(crouch_side, &bare), 2.0);
        let smg = Modifiers { weapon: 0.05, ..bare };
        assert!((ground_speed(Gait::Sprint, &smg) - 5.25).abs() < 1e-5);
        let aimed = Gait::Move { stance: Stance::Stand, heading: Heading::Forward, aiming: true };
        let skill_102 = Modifiers { aim_sum: 0.05, ..bare };
        assert_eq!(ground_speed(aimed, &skill_102), ground_speed(aimed, &bare));
    }

    /// The weight: nothing up to 20 kg, -0.2 at the 30 kg cap, half way at 25; none with skill 214; legs and hits add.
    #[test]
    fn weight_legs_and_hits_slow_the_body() {
        let at = |kg: f32| Modifiers { weight_kg: kg, ..Modifiers::default() };
        assert_eq!(at(18.0).weight_term(), 0.0);
        assert!((at(25.0).weight_term() + 0.1).abs() < 1e-6);
        assert!((at(40.0).weight_term() + 0.2).abs() < 1e-6);
        assert_eq!(Modifiers { skill_214: true, ..at(40.0) }.weight_term(), 0.0);
        let hurt = Modifiers { legs: [Leg::Hurt, Leg::Broken], stagger: 0.7, ..Modifiers::default() };
        assert!((hurt.factor() - (1.0 - 0.3 - 0.5)).abs() < 1e-6);
        // The cap: a fast body is held at the run's forward speed times F.
        assert_eq!(cap(RUN, 1.0), 3.5);
    }
}
