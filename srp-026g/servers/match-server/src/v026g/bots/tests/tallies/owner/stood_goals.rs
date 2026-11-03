//! A278 (the owner, 2026-11-03): why the stronger team stands with a goal and no route. A93 (`traces-20261028/fx-zones-a93-sweep.md`):
//! by side 1.27 % of the ON team's bot ticks against 1.08 % of the equal-aim OFF team's, in batteries only. Each tick a bot stands so
//! (the bench's `stood with a goal and no route`: alive, farther from its goal than its radius, no route and no straight step; the
//! same read as [`StoodRead::of`]) is counted by its team, why it stands ([`Stood`]: the reflex has its legs, a play just taken, nothing),
//! its play and the play's target, and the goal's [`GoalClass`]: read once per (bot, goal) episode from its own side's rules
//! (`Tactics::allowed`, `Tactics::spawn_ok`), with the bot's objective flag as its route takes it. Nothing decides by these counts.

use super::*;
use crate::v026g::bots::path;

/// A goal moved this far is a new episode, m.
const MOVED_M: f32 = 1.0;
/// How near a floor the bot's feet and the goal must be to be read, m (the bench's own read of a goal with no way).
const FEET_R: f32 = 1.5;
const GOAL_R: f32 = 3.0;

/// What stands between a bot and its goal by its own side's rules, read once an episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GoalClass {
    /// Its feet or the goal on no floor.
    NoFloor,
    /// The goal's floor is in the other side's spawn area, which its side may not walk (not bound for their base).
    GoalInSpawn,
    /// The goal's floor is shut now by another rule (a hazard on now).
    GoalShut,
    /// The goal's floor is allowed and a way is open on the grid, but none over the floors its side may walk.
    WayCut,
    /// No way at all on the grid (another floor, a pocket).
    NoWay,
    /// An allowed way exists: the stand is its own state (a search to come, a play just taken, the legs).
    Routable,
}

impl GoalClass {
    fn name(self) -> &'static str {
        match self {
            Self::NoFloor => "no_floor",
            Self::GoalInSpawn => "goal_in_spawn",
            Self::GoalShut => "goal_shut",
            Self::WayCut => "way_cut",
            Self::NoWay => "no_way",
            Self::Routable => "routable",
        }
    }

    /// The class of `goal` for `bot` at `feet`, time `now`.
    fn of(bot: &Bot, tactics: &crate::v026g::bots::tactics::Tactics, grid: &mapdata::WalkGrid, feet: Vec3, goal: Vec3, now: u32) -> Self {
        let (Some(from), Some(to)) = (grid.locate(feet, FEET_R), grid.locate(goal, GOAL_R)) else {
            return Self::NoFloor;
        };
        let objective = bot.intent.objective;
        if !tactics.spawn_ok(to, bot.side, objective) {
            return Self::GoalInSpawn;
        }
        if !tactics.allowed(to, now, bot.side, objective) {
            return Self::GoalShut;
        }
        if path::plan_where(grid, from, to, |n| tactics.allowed(n, now, bot.side, objective)).is_some() {
            return Self::Routable;
        }
        if path::plan(grid, from, to).is_some() { Self::WayCut } else { Self::NoWay }
    }
}

/// The name of a play's target kind (`Battery(4)` is `Battery`).
fn target_kind(bot: &Bot) -> String {
    bot.assignment.as_ref().map_or_else(
        || "none".to_owned(),
        |a| format!("{:?}", a.target).split(['(', ' ', '{']).next().unwrap_or_default().to_owned(),
    )
}

/// A stand this many ticks long or more is kept in detail (2 s).
const LONG_TICKS: u32 = 40;
/// At most this many long stands a team are kept a match.
const LONG_KEPT: usize = 40;
/// A route that ended this long ago or less is the stand's state, ms.
const ENDED_MS: u32 = 1_000;

/// The bot's route state at a stood tick, from its own bench fields: shut in its pocket, starved of a search, a search for this goal
/// failed lately (its no-way kept), its last route ended lately (and how), or none of these.
fn route_state(bot: &Bot, goal: Vec3, now: u32) -> String {
    if bot.bench_shut_in {
        return "shut_in".to_owned();
    }
    if bot.bench_starved_polls > 0 {
        return "starved".to_owned();
    }
    if bot.no_way.is_some_and(|(at, until)| now < until && at.distance_xz(goal) <= MOVED_M) {
        return "no_way_kept".to_owned();
    }
    match bot.route_end {
        Some((end, at)) if now.saturating_sub(at) <= ENDED_MS => format!("ended_{end:?}"),
        _ => "other".to_owned(),
    }
}

/// One place's stand: its goal and the goal's class, its first tick, its length, where it stood, and its key at the first tick.
#[derive(Clone, Debug)]
struct Episode {
    goal: Vec3,
    class: GoalClass,
    from: u32,
    ticks: u32,
    feet: Vec3,
    key: String,
    team: usize,
}

#[derive(Default)]
pub(super) struct StoodGoals {
    /// Each place's stand under way.
    episode: Vec<Option<Episode>>,
    /// By team: ticks by "why|play|target|class|state", and episodes by "play|target|class".
    ticks: [BTreeMap<String, u64>; 2],
    episodes: [BTreeMap<String, u64>; 2],
    /// By team: the bot ticks with a goal (the bench's `goal_ticks`).
    goal_ticks: [u64; 2],
    /// By team: the stands of [`LONG_TICKS`] or more (at most [`LONG_KEPT`]).
    long: [Vec<serde_json::Value>; 2],
}

impl StoodGoals {
    pub(super) fn new(n: usize) -> Self {
        Self { episode: vec![None; n], ..Self::default() }
    }

    /// The stand of `place` ends: kept in detail when long.
    fn close(&mut self, place: Place) {
        let Some(ep) = self.episode.get_mut(place).and_then(Option::take) else {
            return;
        };
        if ep.ticks >= LONG_TICKS && self.long[ep.team].len() < LONG_KEPT {
            self.long[ep.team].push(serde_json::json!({
                "place": place, "from": ep.from, "ticks": ep.ticks, "key": ep.key,
                "feet": [(ep.feet.x * 10.0).round() / 10.0, (ep.feet.y * 10.0).round() / 10.0, (ep.feet.z * 10.0).round() / 10.0],
                "goal": [(ep.goal.x * 10.0).round() / 10.0, (ep.goal.y * 10.0).round() / 10.0, (ep.goal.z * 10.0).round() / 10.0],
            }));
        }
    }
}

impl Topic for StoodGoals {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, game: &Game) {
        let Some(tactics) = bots.tactics() else {
            return;
        };
        let grid = &game.world.grid;
        if self.episode.len() < bots.bots.len() {
            self.episode.resize(bots.bots.len(), None);
        }
        for bot in bots.bots.iter().flatten() {
            let place = bot.place;
            let Some(body) = game.bodies.get(place).filter(|b| b.alive) else {
                self.close(place);
                continue;
            };
            let Some(Go::To { at, within_m }) = bot.bench_go else {
                self.close(place);
                continue;
            };
            if body.feet.distance_xz(at) <= within_m {
                self.close(place);
                continue;
            }
            let team = usize::from(body.team).min(1);
            self.goal_ticks[team] += 1;
            let Some(read) = StoodRead::of(bot) else {
                self.close(place);
                continue;
            };
            let play = bot.assignment.as_ref().map_or_else(|| "none".to_owned(), |a| format!("{:?}", a.kind));
            let state = route_state(bot, at, game.tick);
            if self.episode[place].as_ref().is_some_and(|ep| ep.goal.distance_xz(at) > MOVED_M) {
                self.close(place);
            }
            if self.episode[place].is_none() {
                let class = GoalClass::of(bot, tactics, grid, body.feet, at, game.tick);
                *self.episodes[team].entry(format!("{play}|{}|{}", target_kind(bot), class.name())).or_default() += 1;
                self.episode[place] = Some(Episode {
                    goal: at,
                    class,
                    from: game.tick,
                    ticks: 0,
                    feet: body.feet,
                    key: format!("{}|{play}|{}|{}|{state}", read.why.name(), target_kind(bot), class.name()),
                    team,
                });
            }
            let Some(ep) = self.episode[place].as_mut() else {
                continue;
            };
            ep.ticks += 1;
            let class = ep.class;
            *self.ticks[team].entry(format!("{}|{play}|{}|{}|{state}", read.why.name(), target_kind(bot), class.name())).or_default() += 1;
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        for place in 0..self.episode.len() {
            self.close(place);
        }
        let mut json = serde_json::Map::new();
        let mut lines = vec![
            "owner-test stood goals (A278: each tick a bot stood with a goal and no route, by why|play|target|goal class|route state; episodes by \
             play|target|class; stands of 2 s or more in detail)"
                .to_owned(),
        ];
        for team in 0..2 {
            let top = |m: &BTreeMap<String, u64>| {
                let mut v: Vec<(&String, &u64)> = m.iter().collect();
                v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                v.iter().take(8).map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")
            };
            lines.push(format!(
                "  team {team}: goal ticks {}, stood {}; top {}; episodes {}",
                self.goal_ticks[team],
                self.ticks[team].values().sum::<u64>(),
                top(&self.ticks[team]),
                top(&self.episodes[team])
            ));
            json.insert(
                format!("team{team}"),
                serde_json::json!({
                    "goal_ticks": self.goal_ticks[team], "ticks": self.ticks[team], "episodes": self.episodes[team],
                    "long": self.long[team],
                }),
            );
        }
        vec![("stood_goals", Part::new(serde_json::json!({ "stood_goals": json }), lines.join("\n")))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_kind_is_its_variant_name() {
        assert_eq!("Battery(4)".split(['(', ' ', '{']).next(), Some("Battery"));
        assert_eq!(GoalClass::GoalInSpawn.name(), "goal_in_spawn");
    }
}
