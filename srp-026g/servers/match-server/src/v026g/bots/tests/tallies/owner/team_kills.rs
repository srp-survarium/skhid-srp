//! The owner counts of the team kills: every teammate's death of the match by what killed it (a bullet, a grenade's blast) with
//! the killer's and the victim's state at the kill (a jump under way, the time since the body's last route jump press, its
//! speed), the distance between them, and the nearest jump link of 0.8 m or more to each within 3 m; the team hits (bullets that
//! reached a mate), and the mates a trap caught (the trap's catch is no death here). Test only; one `TEAMKILL` line a kill.
//!
//! What it is for: whether a rise in team kills follows the movement of a change (bots mid-jump on the line of fire, bunched on
//! a link) or the fight's own chances (the grenades, the traps).

use super::*;
use crate::v026g::bots::tests::standin::Kill;
use crate::v026g::bots::tests::standin_traps::Event;

/// A jump link of at least this rise is a ledge for the counts, metres.
const LEDGE_M: f32 = 0.8;
/// A body this long after its last route jump press is still counted as in that jump, ms.
const IN_JUMP_MS: u32 = 1500;
/// A jump link is near a body within this, metres.
const NEAR_LINK_M: f32 = 3.0;

#[derive(Default)]
pub(super) struct TeamKillCounts;

impl Topic for TeamKillCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("team_kills", Self::part(run, game))]
    }
}

/// A body's state at a kill: a jump under way or within [`IN_JUMP_MS`] of a route jump press (the rise of that link), its speed.
struct State {
    air: bool,
    since_press_ms: Option<u32>,
    press_rise: f32,
    speed: f32,
}

impl TeamKillCounts {
    fn state(run: &Run, place: Place, tick: u32) -> State {
        let recs = &run.recs[place];
        let rec = recs.get(run.index(tick).min(recs.len().saturating_sub(1))).copied().unwrap_or_default();
        let press = run
            .presses
            .iter()
            .filter(|p| p.place == place && p.t <= tick)
            .filter_map(|p| p.route.map(|(from, to)| (p.t, to.y - from.y)))
            .max_by_key(|&(t, _)| t);
        State {
            air: rec.air,
            since_press_ms: press.map(|(t, _)| tick - t),
            press_rise: press.map_or(0.0, |(_, rise)| rise),
            speed: rec.speed,
        }
    }

    /// The nearest Jump link of [`LEDGE_M`] or more to `feet`: its take-off's distance and its rise.
    pub(super) fn near_ledge(grid: &WalkGrid, feet: Vec3) -> Option<(f32, f32)> {
        let r = NEAR_LINK_M;
        grid.floors_in([feet.x - r, feet.z - r, feet.x + r, feet.z + r])
            .filter(|&n| (grid.position(n).y - feet.y).abs() < 2.0)
            .flat_map(|n| {
                let from = grid.position(n);
                grid.links(n)
                    .filter(|&(_, kind)| kind == mapdata::LinkKind::Jump)
                    .map(move |(b, _)| (from.distance_xz(feet), grid.position(b).y - from.y))
                    .collect::<Vec<_>>()
            })
            .filter(|&(d, rise)| d <= r && rise >= LEDGE_M)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    fn part(run: &Run, game: &Game) -> Part {
        let grid = &game.world.grid;
        let (mut frag, mut bullet) = (0u32, 0u32);
        let (mut killer_jumping, mut victim_jumping, mut either_near) = (0u32, 0u32, 0u32);
        let mut lines = Vec::new();
        for k in &game.team_kill_log {
            let (killer, victim) = (Self::state(run, k.killer, k.tick), Self::state(run, k.target, k.tick));
            let in_jump = |s: &State| s.air || s.since_press_ms.is_some_and(|ms| ms <= IN_JUMP_MS);
            let (kj, vj) = (in_jump(&killer), in_jump(&victim));
            let (kn, vn) = (Self::near_ledge(grid, k.killer_feet), Self::near_ledge(grid, k.target_feet));
            frag += u32::from(k.frag);
            bullet += u32::from(!k.frag);
            killer_jumping += u32::from(kj);
            victim_jumping += u32::from(vj);
            either_near += u32::from(kn.is_some() || vn.is_some());
            let near = |n: Option<(f32, f32)>| n.map_or("none".to_owned(), |(d, rise)| format!("{d:.1} m rise {rise:.2}"));
            let press = |s: &State| s.since_press_ms.map_or("none".to_owned(), |ms| format!("{ms} ms (rise {:.2})", s.press_rise));
            lines.push(format!(
                "TEAMKILL t={} cause={} dist={:.1} killer={} air={} speed={:.1} since_press={} near_ledge={} | victim={} air={} speed={:.1} since_press={} near_ledge={}",
                k.tick,
                if k.frag { "frag" } else { "bullet" },
                k.killer_feet.distance_xz(k.target_feet),
                k.killer,
                killer.air,
                killer.speed,
                press(&killer),
                near(kn),
                k.target,
                victim.air,
                victim.speed,
                press(&victim),
                near(vn),
            ));
        }
        let mates_caught = game.traps.events.iter().filter(|e| matches!(e, Event::Caught { kind: Kill::Mate, .. })).count();
        let kills = game.team_kill_log.len();
        let json = serde_json::json!({
            "team_kills": {
                "kills": kills,
                "bullet": bullet,
                "frag": frag,
                "team_hits": game.team_hits,
                "mates_caught_by_traps": mates_caught,
                "killer_in_jump": killer_jumping,
                "victim_in_jump": victim_jumping,
                "a_ledge_within_3_m": either_near,
            },
        });
        let mut text = format!(
            "owner-test team kills: {kills} (bullet {bullet}, frag {frag}); team hits {}; mates caught by traps {mates_caught}; killer in a jump {killer_jumping}, victim in a jump {victim_jumping}, a ledge within 3 m of either {either_near}",
            game.team_hits
        );
        for line in lines {
            text.push('\n');
            text.push_str(&line);
        }
        Part::new(json, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mate killed by a bullet and one by a blast are told apart, and the line says who, when and how far.
    #[test]
    fn a_team_kill_is_told_by_its_cause() {
        let (mut game, tally) = made_up(&[0, 0, 1]);
        game.kill(1, 0, Kill::Mate, 6000);
        game.kill_by_frag = true;
        game.kill(0, 1, Kill::Mate, 7000);
        game.kill_by_frag = false;
        game.kill(2, 0, Kill::Enemy, 8000);
        let part = TeamKillCounts::part(&tally.run, &game);
        assert!(part.text.starts_with("owner-test team kills: 2 (bullet 1, frag 1)"), "{}", part.text);
        assert_eq!(part.text.lines().filter(|l| l.starts_with("TEAMKILL")).count(), 2, "{}", part.text);
        assert!(part.text.contains("cause=frag") && part.text.contains("cause=bullet"), "{}", part.text);
    }
}
