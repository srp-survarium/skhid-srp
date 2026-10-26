//! The reversals by the bot's role: a count only (it steers nothing), the kit train's guard on the rusher's
//! kit. The same moves as [`walking`](super::walking)'s `reversals` (a move of 1.5 m or more with no target turning
//! back over 135° from the one before), split into the rusher's and the other roles', and of the others' those that
//! came within [`REVERSAL_NEAR_MS`] after a reversal of a rusher of their own team (a teammate reacting to it, or the
//! same cause). Their sum is `reversals`.

use super::*;

/// A bot's reversal counts as near a rusher's when it came within this many ms after a reversal of a rusher of its
/// own team.
pub(crate) const REVERSAL_NEAR_MS: u32 = 5_000;

pub(super) struct ReversalRoles {
    /// Each body at the last world: feet, alive; and its last move of 1.5 m or more.
    last: Vec<Option<(Vec3, bool)>>,
    last_move: Vec<Option<(f32, f32)>>,
    /// The rusher's reversals, the others'.
    by_role: [u32; 2],
    /// Of the others', those near a rusher's of their team ([`REVERSAL_NEAR_MS`]).
    near_rusher: u32,
    /// When a rusher of each team last reversed.
    rusher_reversed: [Option<u32>; 2],
}

impl ReversalRoles {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            last: vec![None; bodies],
            last_move: vec![None; bodies],
            by_role: [0; 2],
            near_rusher: 0,
            rusher_reversed: [None; 2],
        }
    }
}

impl RouteTopic for ReversalRoles {
    fn world(&mut self, _run: &RouteRun, game: &Game, bots: &Bots) {
        // The reversals of this world: (team, whether the bot is a rusher).
        let mut reversed: Vec<(usize, bool)> = Vec::new();
        for (place, body) in game.bodies.iter().enumerate() {
            let before = self.last[place].replace((body.feet, body.alive));
            let Some((from, true)) = before.filter(|_| body.alive) else {
                self.last_move[place] = None;
                continue;
            };
            let moved = from.distance_xz(body.feet);
            let step = (body.feet.x - from.x, body.feet.z - from.z);
            if moved >= 1.5 && body.target.is_none() {
                if let Some(back) = self.last_move[place] {
                    let cos = (step.0 * back.0 + step.1 * back.1) / (moved * back.0.hypot(back.1));
                    if cos < (135f32).to_radians().cos() {
                        let rusher = bots.bots[place]
                            .as_ref()
                            .is_some_and(|bot| bot.role == Some(survarium::v026g::loadouts::BotRole::Rusher));
                        reversed.push((usize::from(body.team.min(1)), rusher));
                    }
                }
                self.last_move[place] = Some(step);
            } else if moved >= 1.5 {
                self.last_move[place] = None;
            }
        }
        // A rusher's reversals mark its team's time first, so the others' of the same world count whatever their
        // places.
        for &(team, rusher) in &reversed {
            self.by_role[usize::from(!rusher)] += 1;
            if rusher {
                self.rusher_reversed[team] = Some(game.tick);
            }
        }
        for &(team, rusher) in &reversed {
            if !rusher
                && self.rusher_reversed[team].is_some_and(|at| game.tick.wrapping_sub(at) <= REVERSAL_NEAR_MS)
            {
                self.near_rusher += 1;
            }
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!(
            "reversals_rusher {} reversals_others {} reversals_others_near_rusher {}",
            self.by_role[0], self.by_role[1], self.near_rusher
        ));
    }
}

/// Reversals are split by the bot's role, and a teammate's of a rusher's within [`REVERSAL_NEAR_MS`] is counted as
/// near it (counts only).
#[test]
fn reversals_are_split_by_role_and_counted_near_a_rushers() {
    use crate::v026g::bots::Bots;
    use std::sync::Arc;
    let teams = [0u8, 0, 1];
    let roles = [Some("rusher"), Some("flanker"), Some("flanker")];
    let bots = Bots::new(
        1,
        survarium::v026g::maps::mode::BATTERIES,
        6,
        &teams,
        &[true; 3],
        &roles,
        Arc::new(crate::v026g::bots::Tuning::default()),
        crate::v026g::bots::Trace::none(0),
    );
    let mut game = Game::new(super::super::super::standin::soak_world(), &teams, 1, None, None);
    let mut tally = RouteTally::start(3);
    let mut topic = ReversalRoles::new(3);
    // x at each world (1 s apart). The rusher turns back at world 2, its teammate at the same world and again at
    // world 10 (8 s on); the other team's flanker turns back at world 7, with no rusher on its team.
    let x: [[f32; 13]; 3] = [
        [0., 2., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.],
        [0., 2., 0., 0., 0., 0., 0., 0., 0., 0., 2., 2., 2.],
        [0., 0., 0., 0., 0., 0., 2., 0., 0., 0., 0., 0., 0.],
    ];
    for world in 0..13 {
        game.tick = (world as u32 + 1) * 1000;
        for place in 0..3 {
            let body = &mut game.bodies[place];
            body.alive = true;
            body.feet = Vec3::new(x[place][world], 0.0, 0.0);
        }
        tally.world(&game, &bots);
        topic.world(&tally.run, &game, &bots);
    }
    assert_eq!(topic.by_role, [1, 3], "the rusher's, the others'");
    assert_eq!(topic.near_rusher, 1, "the teammate's of the same world");
    let mut line = Line::default();
    topic.line(&tally.run, &mut line);
    assert_eq!(
        line.main,
        ["reversals_rusher 1 reversals_others 3 reversals_others_near_rusher 1"]
    );
}
