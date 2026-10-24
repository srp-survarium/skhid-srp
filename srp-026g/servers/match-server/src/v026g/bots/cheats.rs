//! The small cheats (`player_bots_behaviour.md` §2: "a bit, not too
//! much"), all in this one module (research B9: omniscience must not leak
//! anywhere else). Each gives only [`Source::Cheat`] percepts, which steer
//! routes and where to look; the motor cannot aim at them (`know`).
//!
//! - **The thief:** while a battery of the team is carried by an enemy,
//!   his place with an error, every few seconds.
//! - **A rough radar:** while a bot senses no enemy, the nearest enemy's
//!   place with a large error, every few seconds.
//! - **Every battery's place** is the view's (`MatchView::batteries`) and
//!   is read by the plays only when [`CheatTuning::batteries`] is on.
//! - Hearing ×1.25 and a longer memory are numbers in `sense` and `know`.

use mapdata::Vec3;

use super::know::{CheatKind, Percept, Source};
use super::rng::Rng;
use super::tuning::CheatTuning;
use super::view::{BatteryState, MatchView, Place};

/// When each cheat last spoke, per team.
#[derive(Clone, Debug, Default)]
pub struct CheatClock {
    thief: u32,
    radar: u32,
}

fn blur(at: Vec3, error: f32, rng: &mut Rng) -> Vec3 {
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let r = error * rng.unit().sqrt();
    Vec3::new(at.x + r * angle.cos(), at.y, at.z + r * angle.sin())
}

/// The cheat percepts of `team` at `view.clock`. `quiet(place)`: whether
/// the bot at `place` senses no enemy now (the radar speaks only then).
pub fn cheat_percepts(
    team: u8,
    view: &MatchView,
    tuning: &CheatTuning,
    clock: &mut CheatClock,
    near_to: Vec3,
    quiet: bool,
    rng: &mut Rng,
) -> Vec<Percept> {
    let mut out = Vec::new();
    let now = view.clock;
    if tuning.thief_every_ms > 0 && now >= clock.thief + tuning.thief_every_ms {
        let thieves: Vec<Place> = view
            .batteries
            .iter()
            .filter_map(|battery| match battery {
                BatteryState::Carried(Some(carrier)) => Some(*carrier),
                _ => None,
            })
            .filter(|&carrier| view.place(carrier).is_some_and(|p| p.team != team))
            .collect();
        if !thieves.is_empty() {
            clock.thief = now;
        }
        for thief in thieves {
            if let Some(enemy) = view.place(thief) {
                out.push(Percept {
                    who: thief,
                    at: blur(enemy.feet, tuning.thief_error_m, rng),
                    error_m: tuning.thief_error_m,
                    time: now,
                    source: Source::Cheat(CheatKind::Thief),
                    busy: false,
                });
            }
        }
    }
    if quiet && tuning.radar_every_ms > 0 && now >= clock.radar + tuning.radar_every_ms {
        clock.radar = now;
        if let Some(enemy) = view.enemies_of(team).min_by(|a, b| {
            a.feet
                .distance_xz(near_to)
                .total_cmp(&b.feet.distance_xz(near_to))
        }) {
            out.push(Percept {
                who: enemy.place,
                at: blur(enemy.feet, tuning.radar_error_m, rng),
                error_m: tuning.radar_error_m,
                time: now,
                source: Source::Cheat(CheatKind::Radar),
                busy: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::view::{Mode, PlaceView};

    #[test]
    fn the_thief_and_the_radar_speak_with_their_errors_and_never_as_sight() {
        let body = |place: Place, team: u8, feet: Vec3| PlaceView {
            place,
            team,
            bot: place == 0,
            present: true,
            alive: true,
            feet,
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            known_at: 0,
        };
        let thief_at = Vec3::new(50.0, 0.0, 50.0);
        let view = MatchView {
            clock: 10_000,
            mode: Mode::Batteries,
            time_left_s: 300.0,
            total_s: 900.0,
            points: [0, 0],
            places: vec![body(0, 0, Vec3::ZERO), body(1, 1, thief_at)],
            batteries: vec![BatteryState::Carried(Some(1))],
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 9_000,
            can_respawn: [true, true],
            next_battery: None,
        };
        let tuning = CheatTuning::default();
        let mut clock = CheatClock::default();
        let percepts = cheat_percepts(
            0,
            &view,
            &tuning,
            &mut clock,
            Vec3::ZERO,
            true,
            &mut Rng::new(2),
        );
        let kinds: Vec<Source> = percepts.iter().map(|p| p.source).collect();
        assert_eq!(
            kinds,
            [
                Source::Cheat(CheatKind::Thief),
                Source::Cheat(CheatKind::Radar)
            ]
        );
        assert!(percepts[0].at.distance_xz(thief_at) <= tuning.thief_error_m + 1e-3);
        assert!(percepts[1].at.distance_xz(thief_at) <= tuning.radar_error_m + 1e-3);
        // Not again before their periods; never for the thief's own team.
        assert!(
            cheat_percepts(
                0,
                &view,
                &tuning,
                &mut clock,
                Vec3::ZERO,
                true,
                &mut Rng::new(2)
            )
            .is_empty()
        );
        let mut other = CheatClock::default();
        let theirs = cheat_percepts(
            1,
            &view,
            &tuning,
            &mut other,
            Vec3::ZERO,
            false,
            &mut Rng::new(2),
        );
        assert!(theirs.is_empty());
    }
}
