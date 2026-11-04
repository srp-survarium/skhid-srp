//! [A290] The pre-aiming stand-in human (`SRP_BENCH_HUMAN_PLAY=hunter`, with `SRP_BENCH_HUMAN=<place>`): the opponent the bots'
//! fight fixes are judged against. A287 (`traces-20261028/fx-fights-a287-owner-beats-bots.md`, the owner's 8 local PvB matches
//! of 2026-11-03) found that he wins because he comes at the bots ready and they are not: 65 % of the bots he killed died
//! within about 1 s of his first hit on them, against 22-34 % for the old stand-in (A230), and he fires a median 135 ms after
//! a line to the bot opens. He does not camp: 71 % of his kills came within 10 s of his moving 10 m or more.
//!
//! So the hunter, between fights, goes at the nearest enemy he knows (seen, heard, told by his team, as the bots know: their
//! own [`nearest_known`]), runs while far, and inside [`HUNT_WALK_M`] walks with his sights up on that enemy's place: when the
//! bot shows there his view is already on it. In a fight (his reflex has a target he sees) he fights as the stand-in human
//! does, his play and his reflex, with the side steps of a live player (`aim.combat.stand_strafe`). With no enemy known he
//! plays as the stand-in human always did (the plays take him for a human; Raid when it is a candidate).
//!
//! His tuning is his own and fixed ([`hunter_tuning`], as the stand-in camper's), so the two runs of a head-to-head bench face
//! the same opponent and the bots' aim changes do not move him: the build's defaults with the human aim of
//! `camper_human_aim.json`, every aim pair at [`HunterAim`]'s numbers at every skill (the bots' dial does not move him).
//! Calibrated on A287 (the bench's `under_fire` by-human counts and the scoreboard; numbers in the PR).
//!
//! He rides on the stand-in camper's hooks (`BenchCamper`: `Bots::tuning_for`, the think's `drive`), so a match has the
//! hunter or a camper, never both. Bench only: nothing of the release build changes.

use super::*;
use super::camper_move::nearest_known;
use crate::v026g::bots::control::Driver;
use crate::v026g::bots::plays::{Go, Intent, Look, Pace};
use crate::v026g::bots::think::Due;

/// What the stand-in human plays (`SRP_BENCH_HUMAN_PLAY`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HumanPlay {
    /// As ever: a bot the plays take for a human, on its team's tuning.
    #[default]
    Bot,
    /// [`Hunt`]: hunts the enemies he knows, sights up, by [`hunter_tuning`].
    Hunter,
}

impl HumanPlay {
    /// Unset: [`Self::Bot`]; `hunter`; anything else is refused.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("bot") => Ok(Self::Bot),
            Some("hunter") => Ok(Self::Hunter),
            Some(other) => Err(format!("SRP_BENCH_HUMAN_PLAY={other:?}: bot or hunter")),
        }
    }

    pub(crate) fn from_env() -> Self {
        Self::parse(bench_settings().var("SRP_BENCH_HUMAN_PLAY").ok().as_deref()).unwrap_or_else(|e| panic!("{e}"))
    }
}

/// Inside this of the enemy he hunts he walks, his sights up on that enemy's place, m (A287: his kills at a median 15.5 m,
/// p75 22.1 m).
pub(crate) const HUNT_WALK_M: f32 = 25.0;

/// He stops this near the place he hunts, m: the bot is met there or not at all (A287: his kills at p25 9.5 m).
pub(crate) const HUNT_STOP_M: f32 = 8.0;

/// The hunter's aim, the same at every skill (each pair of the aim section at these numbers). Calibrated on A287 (the PR has
/// the bench's counts against the owner's).
#[derive(Clone, Copy, Debug)]
pub(crate) struct HunterAim {
    pub(crate) error_deg: f32,
    pub(crate) wander_deg: f32,
    pub(crate) settled_wander_deg: f32,
    pub(crate) settle_tau_s: f32,
    pub(crate) settle_s: f32,
    pub(crate) notice: f32,
    pub(crate) reaction_ms: f32,
}

/// The calibrated hunter.
pub(crate) const HUNTER_AIM: HunterAim = HunterAim {
    error_deg: 0.9,
    wander_deg: 0.8,
    settled_wander_deg: 0.0,
    settle_tau_s: 1.0,
    settle_s: 0.1,
    notice: 0.6,
    reaction_ms: 120.0,
};

/// The hunter's tuning: [`human_tuning`] with [`HUNTER_AIM`] at both ends of every aim pair and the live side steps in a fight.
pub(crate) fn hunter_tuning() -> Tuning {
    hunter_tuning_with(HUNTER_AIM)
}

pub(crate) fn hunter_tuning_with(a: HunterAim) -> Tuning {
    let mut tuning = human_tuning();
    let aim = &mut tuning.aim;
    aim.error_deg = (a.error_deg, a.error_deg);
    aim.wander_deg = (a.wander_deg, a.wander_deg);
    aim.settled_wander_deg = (a.settled_wander_deg, a.settled_wander_deg);
    aim.settle_tau_s = (a.settle_tau_s, a.settle_tau_s);
    aim.settle_s = (a.settle_s, a.settle_s);
    aim.notice = (a.notice, a.notice);
    aim.reaction_ms = (a.reaction_ms, a.reaction_ms);
    aim.combat.stand_strafe = true;
    tuning
}

/// The hunter's state: his counts (thinks by what he did).
#[derive(Clone, Debug, Default)]
pub(crate) struct Hunt {
    /// Thinks that hunted a known enemy, walking sights-up / running.
    pub(crate) sights_up: u32,
    pub(crate) running: u32,
    /// Thinks in a fight (his reflex on a target he sees): his play and reflex.
    pub(crate) fighting: u32,
    /// Thinks with no enemy known: his play.
    pub(crate) playing: u32,
}

impl Hunt {
    /// His think: true when it set his intent (hunting), false when his play and reflex go on (a fight, no enemy known).
    pub(crate) fn drive(&mut self, due: &Due, bot: &mut Bot) -> bool {
        let Due { me, now, .. } = *due;
        let tracks = bot.memory.tracks();
        let in_fight = bot
            .reflex
            .target()
            .is_some_and(|target| tracks.iter().any(|t| t.who == target && t.sight_since.is_some()));
        if in_fight {
            self.fighting += 1;
            bot.control = Driver::Own;
            return false;
        }
        let Some(at) = nearest_known(tracks, me.feet, now) else {
            self.playing += 1;
            bot.control = Driver::Own;
            return false;
        };
        let near = at.distance_xz(me.feet) <= HUNT_WALK_M;
        bot.control = Driver::BenchCamper;
        let mut intent = Intent {
            go: Go::To { at, within_m: HUNT_STOP_M },
            pace: if near { Pace::Walk } else { Pace::Run },
            look: if near { Look::At(at) } else { Look::Path },
            ..Intent::stay()
        };
        if near {
            intent.hold |= key::AIM;
            self.sights_up += 1;
        } else {
            self.running += 1;
        }
        bot.intent = intent;
        true
    }

    pub(crate) fn line(&self) -> String {
        format!(
            "thinks: sights up {} running {} fighting {} playing {}",
            self.sights_up, self.running, self.fighting, self.playing
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_human_play_setting_parses() {
        assert_eq!(HumanPlay::parse(None), Ok(HumanPlay::Bot));
        assert_eq!(HumanPlay::parse(Some("bot")), Ok(HumanPlay::Bot));
        assert_eq!(HumanPlay::parse(Some("hunter")), Ok(HumanPlay::Hunter));
        assert!(HumanPlay::parse(Some("owner")).is_err());
    }

    #[test]
    fn the_hunters_aim_is_the_same_at_every_skill_and_he_steps_aside_in_a_fight() {
        let tuning = hunter_tuning();
        let aim = &tuning.aim;
        assert_eq!(aim.error_deg.0, aim.error_deg.1);
        assert_eq!(aim.reaction_ms.0, aim.reaction_ms.1);
        assert_eq!(aim.notice.0, aim.notice.1);
        assert!(aim.combat.stand_strafe);
        // The rest of his tuning is the stand-in camper's.
        assert_eq!(tuning.sense, human_tuning().sense);
    }
}
