//! The bots on damage zones' floors, from the worlds: living bot-worlds on a zone's floors while it is on, and
//! the entries onto zones' floors, and of them those back onto the set left within `stuck::ZONE_AGAIN_MS`.

use super::*;

pub(super) struct Zones {
    /// Each body's zone set at the last world (0: none) and the set it last left, when.
    zone_in: Vec<u16>,
    zone_left: Vec<Option<(u16, u32)>>,
    /// Living bot-worlds on a zone's floors while it is on.
    zone_on: u32,
    /// Entries onto zones' floors, and of them back onto the set left within `stuck::ZONE_AGAIN_MS`.
    zone_entries: (u32, u32),
}

impl Zones {
    pub(super) fn new(bodies: usize) -> Self {
        Self { zone_in: vec![0; bodies], zone_left: vec![None; bodies], zone_on: 0, zone_entries: (0, 0) }
    }
}

impl RouteTopic for Zones {
    fn world(&mut self, _run: &RouteRun, game: &Game, bots: &Bots) {
        let Some(tactics) = bots.tactics.as_ref() else {
            return;
        };
        let (hazards, time) = (&tactics.hazards, game.tick);
        for (place, body) in game.bodies.iter().enumerate() {
            if bots.bots[place].is_none() {
                continue;
            }
            let node = body
                .alive
                .then(|| tactics.world.grid.locate(body.feet, 1.0))
                .flatten();
            let set = node.map_or(0, |node| hazards.set_of(node));
            let on = node
                .and_then(|node| hazards.zone_of(node))
                .is_some_and(|zone| zone.activity.active(time));
            self.zone_on += u32::from(on);
            let was = std::mem::replace(&mut self.zone_in[place], set);
            if was == 0 && set != 0 {
                self.zone_entries.0 += 1;
                let again = self.zone_left[place].is_some_and(|(left, at)| {
                    left == set && time.saturating_sub(at) <= super::super::stuck::ZONE_AGAIN_MS
                });
                self.zone_entries.1 += u32::from(again);
            } else if was != 0 && set == 0 && body.alive {
                self.zone_left[place] = Some((was, time));
            }
            if !body.alive {
                self.zone_left[place] = None;
            }
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!(
            "zone_on_s {} zone_entries {} zone_again {}",
            self.zone_on, self.zone_entries.0, self.zone_entries.1
        ));
    }
}
