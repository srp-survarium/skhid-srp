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
    /// [A395a] The body's distance off its route (m) at each world it walks the route on a floor of the margin's band
    /// only (`Hazards::in_band`): the motor's lateral error there, which `HazardTuning::band_drift_m` is measured from.
    band_off: Vec<f32>,
    /// [A395a] Living bot-worlds on a floor a zone on hurts by the client's contact test (`Hazards::hits_at`): the same
    /// count under either marking, where `zone_on` counts the marking's refused floors.
    hurt_on: u32,
    /// [A395a] Each body's carry at the last world, and the batteries picked up: all, and from a floor of the margin's
    /// band (`Hazards::in_band`).
    carrying: Vec<bool>,
    takes: (u32, u32),
}

impl Zones {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            zone_in: vec![0; bodies],
            zone_left: vec![None; bodies],
            zone_on: 0,
            zone_entries: (0, 0),
            band_off: Vec::new(),
            hurt_on: 0,
            carrying: vec![false; bodies],
            takes: (0, 0),
        }
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
            let set = node.map_or(0, |node| hazards.set_of(node).baked());
            let on = node
                .and_then(|node| hazards.zone_of(node))
                .is_some_and(|zone| zone.activity.active(time));
            self.zone_on += u32::from(on);
            self.hurt_on += u32::from(node.is_some_and(|node| hazards.hits_at(node, time)));
            let carries = body.alive && body.carrying.is_some();
            if carries && !std::mem::replace(&mut self.carrying[place], carries) {
                self.takes.0 += 1;
                self.takes.1 += u32::from(node.is_some_and(|node| hazards.in_band(node)));
            }
            self.carrying[place] = carries;
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
            if let (Some(node), Some(bot)) = (node, bots.bots[place].as_ref())
                && bot.bench_walk != 0
                && hazards.in_band(node)
                && let Some((route, _)) = bot.route.as_ref()
            {
                let (_, off) = route.clone().progress(body.feet);
                if off.is_finite() {
                    self.band_off.push(off);
                }
            }
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!(
            "zone_on_s {} zone_entries {} zone_again {}",
            self.zone_on, self.zone_entries.0, self.zone_entries.1
        ));
        let mut off = std::mem::take(&mut self.band_off);
        off.sort_by(f32::total_cmp);
        let at = |q: f32| off.get(((off.len() as f32 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
        let mean = if off.is_empty() { 0.0 } else { off.iter().sum::<f32>() / off.len() as f32 };
        line.main.push(format!(
            "band_off n {} mean_m {mean:.3} p50_m {:.3} p90_m {:.3} sum_m {:.1}",
            off.len(),
            at(0.5),
            at(0.9),
            off.iter().sum::<f32>()
        ));
        line.main.push(format!("zone_hurt_s {} takes {} band_takes {}", self.hurt_on, self.takes.0, self.takes.1));
    }
}
