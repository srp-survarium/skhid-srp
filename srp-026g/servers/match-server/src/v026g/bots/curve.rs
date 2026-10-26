//! Utility scoring (`docs/notes/player_bots_research.md` B1 point 3, B3):
//! a play's score is a few considerations multiplied. Each consideration
//! takes one input (a number from the situation), clamps it between its
//! bookends, maps it to `[0, 1]` and passes it through a response curve.
//! The product stops at the first 0, and is compensated for the number of
//! considerations, so a play with many of them is not sunk by the count
//! alone (Mark and Lewis's make-up factor).
//!
//! The inputs a consideration may read are [`Input`]; their values come
//! from the play's context (`plays::Ctx::input`). Which considerations a
//! play has, their bookends and curves, are tuning (`tuning.rs`): a play
//! can be tied to a match signal or a team modifier without code.

use serde::{Deserialize, Serialize};

/// A number in `[0, 1]`, made only by clamping.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize)]
pub struct Score(f32);

impl Score {
    pub const ZERO: Self = Self(0.0);
    pub const ONE: Self = Self(1.0);

    pub fn new(value: f32) -> Self {
        Self(if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        })
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 <= 0.0
    }
}

/// A response curve over `x` in `[0, 1]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Curve {
    Linear,
    /// `x^k`.
    Poly {
        k: f32,
    },
    /// `1 / (1 + e^(-k (x - mid)))`, rescaled to 0 and 1 at the ends.
    Logistic {
        k: f32,
        mid: f32,
    },
    /// Straight lines between points `(x, y)` sorted by x.
    Piecewise {
        points: Vec<(f32, f32)>,
    },
    /// 0 below `at`, 1 from it.
    Step {
        at: f32,
    },
}

impl Curve {
    pub fn eval(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Self::Linear => x,
            Self::Poly { k } => x.powf(*k),
            Self::Logistic { k, mid } => {
                let f = |x: f32| 1.0 / (1.0 + (-k * (x - mid)).exp());
                let (low, high) = (f(0.0), f(1.0));
                if (high - low).abs() < 1e-6 {
                    0.5
                } else {
                    (f(x) - low) / (high - low)
                }
            }
            Self::Piecewise { points } => {
                let Some(first) = points.first() else {
                    return x;
                };
                if x <= first.0 {
                    return first.1;
                }
                for pair in points.windows(2) {
                    let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                    if x <= x1 {
                        let t = if x1 > x0 { (x - x0) / (x1 - x0) } else { 1.0 };
                        return y0 + (y1 - y0) * t;
                    }
                }
                points.last().map_or(x, |last| last.1)
            }
            Self::Step { at } => f32::from(u8::from(x >= *at)),
        }
    }
}

/// What a consideration reads. The units are the input's own (seconds,
/// metres, batteries, `[0, 1]`); the bookends bring them to `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    /// Seconds left in the match.
    TimeLeft,
    /// Our points minus theirs (batteries, or tickets / 10 in Team
    /// Deathmatch).
    ScoreGap,
    /// The path length from the bot to the candidate's target, metres.
    PathDist,
    /// The bot's health, `[0, 1]`.
    Health,
    /// The share of its kit's rounds for a life that it holds, `[0, 1]`.
    Ammo,
    /// How much of the pools a consumable treats is missing, weighted by
    /// what the loss costs, `[0, 1]` (`plays/heal.rs`).
    Need,
    /// Enemies known within 25 m of the target.
    EnemiesNearTarget,
    /// Teammates (bots and humans) within 25 m of the target.
    FriendsNearTarget,
    /// Enemies dead (waiting to respawn), as a share of the enemy team.
    EnemiesDead,
    /// The team's modifiers (`signals::Mods`), `[0, 1]`.
    Aggression,
    Risk,
    ObjectiveUrgency,
    /// The bot's personality's weight for the play's family, `[0, 2]`.
    Personality,
    /// Seconds since this bot last ran this play (large if never).
    SinceLast,
    /// Seconds this bot has run its present play (0 for another play).
    SinceStarted,
    /// 1 when the candidate is the bot's present play and target.
    Current,
    /// Seconds since a battery of ours was taken (large if none).
    SinceTheft,
    /// How far behind on points the team is, late in the match, `[0, 1]`
    /// (`signals::MatchSignals::behind_late`).
    BehindLate,
    /// Seconds left, fewer the farther behind the team is
    /// (`signals::MatchSignals::time_left_behind`): a team far behind
    /// reads the match's end as nearer.
    TimeLeftBehind,
}

/// One consideration: `curve((clamp(input, lo, hi) - lo) / (hi - lo))`,
/// inverted if asked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consideration {
    pub input: Input,
    pub lo: f32,
    pub hi: f32,
    pub curve: Curve,
    #[serde(default)]
    pub invert: bool,
}

impl Consideration {
    pub fn new(input: Input, lo: f32, hi: f32, curve: Curve) -> Self {
        Self {
            input,
            lo,
            hi,
            curve,
            invert: false,
        }
    }

    pub fn inverted(self) -> Self {
        Self {
            invert: true,
            ..self
        }
    }

    pub fn eval(&self, value: f32) -> Score {
        let span = self.hi - self.lo;
        let x = if span.abs() < 1e-9 {
            f32::from(u8::from(value >= self.hi))
        } else {
            ((value - self.lo) / span).clamp(0.0, 1.0)
        };
        let y = self.curve.eval(x);
        Score::new(if self.invert { 1.0 - y } else { y })
    }
}

/// Why a score is what it is: each consideration's input value and result,
/// for the trace.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Explain {
    pub parts: Vec<(Input, f32, f32)>,
}

/// The considerations multiplied, with compensation; 0 at the first 0.
/// `value(input)` is the input's present value.
pub fn score(
    considerations: &[Consideration],
    mut value: impl FnMut(Input) -> f32,
) -> (Score, Explain) {
    let mut explain = Explain::default();
    let mut product = 1.0_f32;
    for consideration in considerations {
        let input = value(consideration.input);
        let result = consideration.eval(input);
        explain
            .parts
            .push((consideration.input, input, result.get()));
        if result.is_zero() {
            return (Score::ZERO, explain);
        }
        product *= result.get();
    }
    (Score::new(made_up(product, considerations.len())), explain)
}

/// The product of `n` considerations' results with the make-up factor:
/// each consideration's loss is partly given back, so the count of
/// considerations does not decide alone.
fn made_up(mut product: f32, n: usize) -> f32 {
    if n > 0 {
        let make_up = 1.0 - 1.0 / n as f32;
        product += (1.0 - product) * make_up * product;
    }
    product
}

impl Explain {
    /// By how much the score would change were some results others: the
    /// made-up product with `with(input, result)` for each result, over the
    /// one with the results as they are. 1 when the parts stop at a zero (the
    /// score is 0 then) or come to 0.
    pub fn rescaled(&self, mut with: impl FnMut(Input, f32) -> f32) -> f32 {
        let n = self.parts.len();
        let (mut now, mut then) = (1.0_f32, 1.0_f32);
        for &(input, _, result) in &self.parts {
            now *= result;
            then *= with(input, result);
        }
        let now = made_up(now, n);
        if now <= 0.0 || self.parts.iter().any(|p| p.2 <= 0.0) {
            return 1.0;
        }
        made_up(then, n) / now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_are_monotone_and_keep_their_ends() {
        let curves = [
            Curve::Linear,
            Curve::Poly { k: 2.0 },
            Curve::Poly { k: 0.5 },
            Curve::Logistic { k: 10.0, mid: 0.5 },
            Curve::Piecewise {
                points: vec![(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)],
            },
        ];
        for curve in &curves {
            assert!(curve.eval(0.0).abs() < 1e-5, "{curve:?}");
            assert!((curve.eval(1.0) - 1.0).abs() < 1e-5, "{curve:?}");
            let mut last = -1.0;
            for i in 0..=100 {
                let y = curve.eval(i as f32 / 100.0);
                assert!(y >= last - 1e-6, "{curve:?}");
                last = y;
            }
        }
        assert_eq!(Curve::Step { at: 0.3 }.eval(0.29), 0.0);
        assert_eq!(Curve::Step { at: 0.3 }.eval(0.3), 1.0);
    }

    #[test]
    fn a_score_stops_at_zero_and_is_compensated() {
        let near = Consideration::new(Input::PathDist, 0.0, 100.0, Curve::Linear).inverted();
        let health = Consideration::new(Input::Health, 0.2, 1.0, Curve::Linear);
        let (score, explain) = score(&[near.clone(), health.clone()], |input| match input {
            Input::PathDist => 50.0,
            _ => 1.0,
        });
        // 0.5 x 1 = 0.5, made up by (1 - 0.5) x 0.5 x 0.5.
        assert!((score.get() - 0.625).abs() < 1e-5, "{score:?}");
        assert_eq!(explain.parts.len(), 2);
        let (zero, explain) = score_of(&[health, near], 0.1);
        assert!(zero.is_zero());
        assert_eq!(explain.parts.len(), 1, "stopped at the first 0");
    }

    fn score_of(considerations: &[Consideration], health: f32) -> (Score, Explain) {
        score(considerations, |input| match input {
            Input::Health => health,
            _ => 10.0,
        })
    }

    #[test]
    fn considerations_read_back_from_json() {
        let json = r#"{"input":"time_left","lo":0,"hi":240,"curve":{"piecewise":{"points":[[0,1],[0.5,1],[1,0]]}},"invert":false}"#;
        let consideration: Consideration = serde_json::from_str(json).unwrap();
        assert_eq!(consideration.input, Input::TimeLeft);
        assert!((consideration.eval(60.0).get() - 1.0).abs() < 1e-5);
        assert!(consideration.eval(240.0).is_zero());
        assert!(
            serde_json::from_str::<Consideration>(
                r#"{"input":"health","lo":0,"hi":1,"curve":"linear","oops":1}"#
            )
            .is_err()
        );
    }
}
