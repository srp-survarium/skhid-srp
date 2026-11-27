//! Patch 190: the hi-Z occlusion no longer drops the light of a place for a
//! frame (renderer-026g.md 5.7; the owner's London (v0.42) report of
//! 2026-11-22: "lighting changes when I step", "a light pops up").
//!
//! The hi-Z answer is a frame or more late and tested against an older
//! depth, so while the camera moves it calls things occluded that are in
//! sight. Patch 189's log of four minutes in London: 5,080 drops of a
//! probe or ambient light, a quarter of them for one frame.
//!
//! - An environment probe or ambient light is never left out for it: one
//!   hidden costs its volume's draws, one dropped in sight relights the
//!   surfaces under it.
//! - A point or spot light is left out once the hi-Z has called it occluded
//!   for `OCCLUDED_ANSWERS` answers in a row: a hidden one may render a
//!   shadow map, so the saving stays, without the drops of a frame or two.

#![forbid(unsafe_code)]

use crate::vostok::occlusion::{self, Answer, LightAnswer, LightId, QueryIndex};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// Occluded answers in a row after which a light is left out.
const OCCLUDED_ANSWERS: u8 = 4;

/// Lights kept in an occluded streak (a London view has up to 48 in view).
const KEPT: usize = 128;

/// A light the hi-Z has called occluded for `answers` answers in a row, the
/// last of them in pass `pass` of occlusion_culling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Streak {
    light: LightId,
    answers: u8,
    pass: u32,
}

/// The lights in an occluded streak. A streak goes on only from the pass
/// before: a light that missed one was out of view, and its first answer
/// back is another object's. Full, the streak of the oldest pass gives way.
struct Streaks {
    kept: [Option<Streak>; KEPT],
    pass: u32,
    /// The index of the last light answered: a pass starts at one not above it.
    last: Option<QueryIndex>,
}

impl Streaks {
    const NONE: Streaks = Streaks { kept: [None; KEPT], pass: 0, last: None };

    /// The answer the frame acts on, for the hi-Z's answer of a light.
    fn answer(&mut self, light: LightId, answer: Answer, index: QueryIndex) -> Answer {
        if self.last.is_none_or(|last| index <= last) {
            self.pass = self.pass.wrapping_add(1);
        }
        self.last = Some(index);
        let found = self.kept.iter().position(|kept| kept.is_some_and(|s| s.light == light));
        let before = found.and_then(|slot| self.kept[slot]).filter(|s| s.pass.wrapping_add(1) == self.pass);
        match answer {
            Answer::Visible => {
                if let Some(slot) = found {
                    self.kept[slot] = None;
                }
                Answer::Visible
            }
            Answer::Occluded => {
                let slot = found.unwrap_or_else(|| self.free_or_oldest());
                let answers = before.map_or(0, |s| s.answers).saturating_add(1).min(OCCLUDED_ANSWERS);
                self.kept[slot] = Some(Streak { light, answers, pass: self.pass });
                if answers >= OCCLUDED_ANSWERS { Answer::Occluded } else { Answer::Visible }
            }
        }
    }

    fn free_or_oldest(&self) -> usize {
        let now = self.pass;
        self.kept.iter().position(Option::is_none).unwrap_or_else(|| {
            (0..KEPT).max_by_key(|&i| self.kept[i].map_or(0, |s| now.wrapping_sub(s.pass))).unwrap_or(0)
        })
    }
}

static STREAKS: Lock<Streaks> = Lock::new(Streaks::NONE);

/// `hiz.probe_answer`: an environment probe is stored visible.
pub fn on_probe_answer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    occlusion::store(ctx, Answer::Visible);
    Flow::Continue
}

/// `hiz.ambient_light_answer`: an ambient light is stored visible.
pub fn on_ambient_light_answer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    occlusion::store(ctx, Answer::Visible);
    Flow::Continue
}

/// `hiz.light_answer`: a light is stored occluded only at the end of its
/// streak's `OCCLUDED_ANSWERS`.
pub fn on_light_answer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(LightAnswer { light, answer, index }) = LightAnswer::at(ctx) else {
        return Flow::Continue;
    };
    let acted = STREAKS.with(ctx.thread(), |streaks| streaks.answer(light, answer, index));
    occlusion::store(ctx, acted);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: LightId = LightId::test(0x1000);
    const B: LightId = LightId::test(0x2000);

    /// One pass per call: A at index 5, B at 6.
    fn pass(streaks: &mut Streaks, a: Answer, b: Option<Answer>) -> Answer {
        let acted = streaks.answer(A, a, QueryIndex(5));
        if let Some(b) = b {
            streaks.answer(B, b, QueryIndex(6));
        }
        acted
    }

    #[test]
    fn a_light_is_left_out_after_its_streak() {
        let mut streaks = Streaks::NONE;
        for _ in 1..OCCLUDED_ANSWERS {
            assert_eq!(pass(&mut streaks, Answer::Occluded, None), Answer::Visible);
        }
        assert_eq!(pass(&mut streaks, Answer::Occluded, None), Answer::Occluded);
        assert_eq!(pass(&mut streaks, Answer::Occluded, None), Answer::Occluded);
        assert_eq!(pass(&mut streaks, Answer::Visible, None), Answer::Visible);
        assert_eq!(pass(&mut streaks, Answer::Occluded, None), Answer::Visible);
    }

    #[test]
    fn a_visible_answer_ends_only_its_own_streak() {
        let mut streaks = Streaks::NONE;
        for _ in 1..OCCLUDED_ANSWERS {
            pass(&mut streaks, Answer::Occluded, Some(Answer::Visible));
        }
        assert_eq!(pass(&mut streaks, Answer::Occluded, Some(Answer::Visible)), Answer::Occluded);
    }

    #[test]
    fn a_light_back_in_view_starts_a_new_streak() {
        let mut streaks = Streaks::NONE;
        for _ in 0..OCCLUDED_ANSWERS {
            pass(&mut streaks, Answer::Occluded, Some(Answer::Visible));
        }
        // A out of view for a pass: only B answers.
        streaks.answer(B, Answer::Visible, QueryIndex(5));
        assert_eq!(pass(&mut streaks, Answer::Occluded, None), Answer::Visible);
    }

    #[test]
    fn full_the_oldest_streak_gives_way() {
        let mut streaks = Streaks::NONE;
        for i in 0..KEPT as u32 {
            streaks.answer(LightId::test(0x10_0000 + i), Answer::Occluded, QueryIndex(0));
        }
        let oldest = LightId::test(0x10_0000);
        streaks.answer(A, Answer::Occluded, QueryIndex(0));
        assert!(streaks.kept.iter().flatten().any(|s| s.light == A));
        assert!(!streaks.kept.iter().flatten().any(|s| s.light == oldest));
    }
}
