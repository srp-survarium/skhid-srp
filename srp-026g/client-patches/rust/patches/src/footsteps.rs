//! Patch 101: a step first made by a re-simulation is heard
//! (docs/notes/footsteps.md; the players' B20 "footsteps randomly drop out").
//!
//! The game plays a step only the first time its tick is simulated
//! (`vostok::footstep`). Another player's input reaches a game late, so his
//! ticks are simulated again with it; a step that the prediction did not
//! have then falls at an already played time and is dropped: his steps go
//! silent whenever his movement changes under lag. 0.27d0 dropped that rule
//! (its effect keeps each foot's newest step time and plays a newer one at
//! the next frame, 0x29AFA0).
//!
//! The rule here: a step past the played time plays, as before. A
//! re-simulated step plays when its foot played nothing in the last
//! `SAME_STEP` ms before it: the same step again (its time moved by the
//! correction) stays single, a new one is heard. Nothing of the simulation:
//! the step is a sound, a decal and particles.

#![forbid(unsafe_code)]

use crate::vostok::footstep::{Pass, SimTime, Step, StepSource};
use crate::vostok::{Context, Flow, FootstepsOnStepGuardExit, GameCell};

/// Two steps of one foot closer than this are one step played twice: a foot
/// steps once a cycle, 600 ms at the quickest (the sprint clips, 18 frames),
/// and a correction moves a step by the input's lateness, under that.
const SAME_STEP: i32 = 250;
/// Sound effects remembered: three player objects a player (#148), 20 to
/// 32 players a match.
const SOURCES: usize = 128;

#[derive(Clone, Copy)]
struct Played {
    source: StepSource,
    /// Each foot's newest step played.
    last: [Option<SimTime>; 2],
}

#[derive(Clone, Copy)]
struct Steps {
    played: [Option<Played>; SOURCES],
    /// The slot a new source takes when every one is in use.
    next: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Play,
    Skip,
}

impl Steps {
    const EMPTY: Steps = Steps { played: [None; SOURCES], next: 0 };

    fn decide(&mut self, step: Step) -> Decision {
        let slot = self.slot(step.source);
        let last = &mut slot.last[step.foot.index()];
        let play = match (step.pass, *last) {
            (Pass::First, _) | (Pass::Again, None) => true,
            (Pass::Again, Some(played)) => step.time.since(played) >= SAME_STEP,
        };
        if play {
            *last = Some(step.time);
            Decision::Play
        } else {
            Decision::Skip
        }
    }

    /// The source's record; a source not seen takes a free slot, or the
    /// oldest taken (a player object gone).
    fn slot(&mut self, source: StepSource) -> &mut Played {
        let known = self.played.iter().position(|p| p.is_some_and(|p| p.source == source));
        let at = known.unwrap_or_else(|| {
            let at = self.played.iter().position(Option::is_none).unwrap_or(self.next);
            self.next = (at + 1) % SOURCES;
            self.played[at] = None;
            at
        });
        self.played[at].get_or_insert(Played { source, last: [None; 2] })
    }
}

static STEPS: GameCell<Steps> = GameCell::new(Steps::EMPTY);

/// on_player_sound_event's time test (`player_sound.step_guard`).
pub fn on_step_guard(ctx: &mut Context<'_>) -> Flow<FootstepsOnStepGuardExit> {
    let Some(step) = Step::at_guard(ctx) else {
        return Flow::Continue;
    };
    let mut steps = STEPS.get(ctx.thread());
    let decision = steps.decide(step);
    STEPS.set(ctx.thread(), steps);
    match decision {
        Decision::Play => Flow::Exit(FootstepsOnStepGuardExit::Play),
        Decision::Skip => Flow::Exit(FootstepsOnStepGuardExit::Skip),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::footstep::Foot;

    fn step(source: u32, foot: Foot, time: u32, pass: Pass) -> Step {
        Step { source: StepSource::at(source), foot, time: SimTime::at(time), pass }
    }

    #[test]
    fn first_pass_always_plays() {
        let mut s = Steps::EMPTY;
        for t in [1000, 1010, 900] {
            assert_eq!(s.decide(step(1, Foot::Left, t, Pass::First)), Decision::Play);
        }
    }

    #[test]
    fn resimulated_copy_stays_single() {
        let mut s = Steps::EMPTY;
        assert_eq!(s.decide(step(1, Foot::Left, 1000, Pass::First)), Decision::Play);
        // the same step again, at its time, moved later or earlier
        for t in [1000, 1100, 1249, 950, 400] {
            assert_eq!(s.decide(step(1, Foot::Left, t, Pass::Again)), Decision::Skip, "{t}");
        }
    }

    #[test]
    fn resimulated_new_step_plays() {
        let mut s = Steps::EMPTY;
        // never played: a player who started walking under lag
        assert_eq!(s.decide(step(1, Foot::Right, 2000, Pass::Again)), Decision::Play);
        assert_eq!(s.decide(step(1, Foot::Right, 2000, Pass::Again)), Decision::Skip);
        assert_eq!(s.decide(step(1, Foot::Right, 2600, Pass::Again)), Decision::Play);
        // the other foot and another player are their own
        assert_eq!(s.decide(step(1, Foot::Left, 2100, Pass::Again)), Decision::Play);
        assert_eq!(s.decide(step(2, Foot::Right, 2100, Pass::Again)), Decision::Play);
    }

    #[test]
    fn full_table_reuses_the_oldest() {
        let mut s = Steps::EMPTY;
        for source in 0..SOURCES as u32 + 5 {
            assert_eq!(s.decide(step(source, Foot::Left, 1000, Pass::First)), Decision::Play);
        }
        assert_eq!(s.played.iter().flatten().count(), SOURCES);
        // the newest kept: its copy stays single
        let newest = SOURCES as u32 + 4;
        assert_eq!(s.decide(step(newest, Foot::Left, 1000, Pass::Again)), Decision::Skip);
    }

    #[test]
    fn clock_wrap() {
        let mut s = Steps::EMPTY;
        assert_eq!(s.decide(step(1, Foot::Left, u32::MAX - 100, Pass::First)), Decision::Play);
        assert_eq!(s.decide(step(1, Foot::Left, 50, Pass::Again)), Decision::Skip);
        assert_eq!(s.decide(step(1, Foot::Left, 200, Pass::Again)), Decision::Play);
    }
}
