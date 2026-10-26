//! The bots in pockets (floors with no way out but the way in): living bot-worlds on a pocket's floor, the deaths
//! there, and the pocket escapes (`escape::counts`) since the start.

use super::*;

pub(super) struct Pockets {
    /// Each body on a pocket's floor at the last world; living bot-worlds on pockets' floors, deaths there.
    pocket_in: Vec<bool>,
    pocket_worlds: u32,
    pocket_deaths: u32,
    /// The pocket escapes' counts at the start.
    pocket_escapes: [u64; 7],
}

impl Pockets {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            pocket_in: vec![false; bodies],
            pocket_worlds: 0,
            pocket_deaths: 0,
            pocket_escapes: super::super::escape::counts::counts(),
        }
    }
}

impl RouteTopic for Pockets {
    fn world(&mut self, _run: &RouteRun, game: &Game, bots: &Bots) {
        let Some(tactics) = bots.tactics.as_ref() else {
            return;
        };
        for (place, body) in game.bodies.iter().enumerate() {
            if bots.bots[place].is_none() {
                continue;
            }
            let node = body
                .alive
                .then(|| tactics.world.grid.locate(body.feet, 1.0))
                .flatten();
            let pocket = node.is_some_and(|node| tactics.world.grid.in_pocket(node));
            self.pocket_worlds += u32::from(pocket);
            let was_in = std::mem::replace(&mut self.pocket_in[place], pocket);
            self.pocket_deaths += u32::from(was_in && !body.alive);
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let pockets = super::super::escape::counts::counts();
        let p: Vec<u64> = (0..7).map(|k| pockets[k] - self.pocket_escapes[k]).collect();
        line.main.push(format!(
            "pocket_s {} pocket_deaths {} pocket_tries {} pocket_worked {} pocket_failed {} pocket_gave_up {} pocket_handed {} pocket_failed_hurt {} pocket_gave_up_hurt {}",
            self.pocket_worlds, self.pocket_deaths, p[0], p[1], p[2], p[3], p[4], p[5], p[6]
        ));
    }
}
