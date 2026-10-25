use super::*;

/// How far forward the bots go, as the #343 check's `advance.py` counts it
/// from a match's worlds: each living bot's progress along the line from
/// its side's spawns (0) to the enemy's (1), once a world; kills and deaths
/// by where the bot stood; and the spot plays' runs (HoldExit, Crossfire,
/// Cut) that reached their spot, read each tick.
pub(crate) struct PushTally {
    /// Each map side's spawns' middle.
    pub(crate) spawns: Vec<(i32, Vec3)>,
    /// Per team: living bot-worlds, of them past half way, past 70 %.
    pub(crate) worlds: [[u32; 3]; 2],
    /// Per team: kills and deaths, all and past half way.
    pub(crate) kills: [[u32; 2]; 2],
    pub(crate) deaths: [[u32; 2]; 2],
    /// Each place's progress at the last world.
    pub(crate) last: Vec<Option<f32>>,
    /// Each bot's spot-play run: (kind, since, reached its spot).
    pub(crate) runs: Vec<Option<(usize, u32, bool)>>,
    /// Per spot play (HoldExit, Crossfire, Cut) and team: runs, of them
    /// reached.
    pub(crate) spot: [[[u32; 2]; 2]; 3],
    /// Roam runs ended "the route turned bad".
    pub(crate) turned_bad: u32,
    /// Each place's last play taken ("Kind Target"), while it runs it.
    pub(crate) last_take: Vec<Option<String>>,
    /// A slot handed from one teammate to another: a bot takes the play
    /// and target a teammate was running, and the teammate no longer runs
    /// it.
    pub(crate) handoffs: u32,
}

impl PushTally {
    pub(crate) fn start(world: &World, bodies: usize) -> Self {
        let mut spawns: Vec<(i32, Vec3, u32)> = Vec::new();
        for s in &world.map.spawns {
            match spawns.iter_mut().find(|e| e.0 == s.team) {
                Some(e) => {
                    e.1 = e.1 + s.position;
                    e.2 += 1;
                }
                None => spawns.push((s.team, s.position, 1)),
            }
        }
        PushTally {
            spawns: spawns
                .into_iter()
                .map(|(team, sum, n)| (team, sum * (1.0 / n as f32)))
                .collect(),
            worlds: [[0; 3]; 2],
            kills: [[0; 2]; 2],
            deaths: [[0; 2]; 2],
            last: vec![None; bodies],
            runs: vec![None; bodies],
            spot: [[[0; 2]; 2]; 3],
            turned_bad: 0,
            last_take: vec![None; bodies],
            handoffs: 0,
        }
    }

    /// The commander's notes, after its poll: Roam's "the route turned
    /// bad" ends, and slot hand-offs.
    pub(crate) fn note(&mut self, note: &Note, bots: &Bots) {
        if note.text.starts_with("ends ") {
            self.turned_bad += u32::from(note.text == "ends Roam: the route turned bad");
            if let Some(last) = self.last_take.get_mut(note.place) {
                *last = None;
            }
            return;
        }
        let Some(rest) = note.text.strip_prefix("takes ") else {
            return;
        };
        let what = rest.split(" (score").next().unwrap_or(rest).to_owned();
        let Some(Some(me)) = bots.bots.get(note.place) else {
            return;
        };
        // Plays with no target take no slot.
        if !what.ends_with(" None") {
            for other in bots.bots.iter().flatten() {
                let running = |a: &Assignment| format!("{:?} {:?}", a.kind, a.target) == what;
                if other.place != note.place
                    && other.team == me.team
                    && self.last_take[other.place].as_deref() == Some(what.as_str())
                    && !other.assignment.as_ref().is_some_and(running)
                {
                    self.handoffs += 1;
                    self.last_take[other.place] = None;
                    break;
                }
            }
        }
        self.last_take[note.place] = Some(what);
    }

    /// A bot's progress from its side's spawns to the enemy's.
    pub(crate) fn progress(&self, side: Option<i32>, at: Vec3) -> Option<f32> {
        let side = side?;
        let own = self.spawns.iter().find(|s| s.0 == side)?.1;
        let enemy = self.spawns.iter().find(|s| s.0 != side)?.1;
        let (ax, az) = (enemy.x - own.x, enemy.z - own.z);
        let len2 = ax * ax + az * az;
        (len2 > 1.0).then(|| ((at.x - own.x) * ax + (at.z - own.z) * az) / len2)
    }

    pub(crate) fn world(&mut self, game: &Game, bots: &Bots) {
        for bot in bots.bots.iter().flatten() {
            let body = &game.bodies[bot.place];
            let p = body
                .alive
                .then(|| self.progress(bot.side, body.feet))
                .flatten();
            self.last[bot.place] = p;
            if let Some(p) = p {
                let w = &mut self.worlds[usize::from(bot.team & 1)];
                w[0] += 1;
                w[1] += u32::from(p > 0.5);
                w[2] += u32::from(p > 0.7);
            }
        }
    }

    /// A place's kills and deaths this tick.
    pub(crate) fn stats(&mut self, place: usize, team: u8, kills: u32, deaths: u32) {
        let forward = u32::from(self.last[place].is_some_and(|p| p > 0.5));
        let t = usize::from(team & 1);
        self.kills[t][0] += kills;
        self.kills[t][1] += kills * forward;
        self.deaths[t][0] += deaths;
        self.deaths[t][1] += deaths * forward;
    }

    pub(crate) fn tick(&mut self, bots: &Bots) {
        use super::plays::{PlayKind, Running, crossfire, cut, hold_exit};
        for bot in bots.bots.iter().flatten() {
            let run = bot.assignment.as_ref().and_then(|a| {
                let kind = match a.kind {
                    PlayKind::HoldExit => 0,
                    PlayKind::Crossfire => 1,
                    PlayKind::Cut => 2,
                    _ => return None,
                };
                Some((kind, a.since))
            });
            let held = matches!(
                bot.running,
                Some(Running::HoldExit(hold_exit::State::Hold { .. }))
                    | Some(Running::Crossfire(crossfire::State::Hold { .. }))
                    | Some(Running::Cut(cut::State::Hold { .. }))
            );
            let t = usize::from(bot.team & 1);
            match (&mut self.runs[bot.place], run) {
                (Some((kind, since, reached)), Some((k, s))) if *kind == k && *since == s => {
                    *reached |= held;
                }
                (slot, run) => {
                    if let Some((kind, _, reached)) = slot.take() {
                        self.spot[kind][t][0] += 1;
                        self.spot[kind][t][1] += u32::from(reached);
                    }
                    *slot = run.map(|(k, s)| (k, s, held));
                }
            }
        }
    }

    /// The line the bench prints (one `push:` line, `key value` pairs, team
    /// 0's then team 1's).
    pub(crate) fn line(mut self, bots: &Bots) -> String {
        for bot in bots.bots.iter().flatten() {
            if let Some((kind, _, reached)) = self.runs[bot.place].take() {
                let t = usize::from(bot.team & 1);
                self.spot[kind][t][0] += 1;
                self.spot[kind][t][1] += u32::from(reached);
            }
        }
        let mut out = String::new();
        for t in 0..2 {
            let w = self.worlds[t];
            let s = |k: usize| self.spot[k][t];
            out += &format!(
                "worlds{t} {} half{t} {} past70_{t} {} kills{t} {} kills_fwd{t} {} deaths{t} {} deaths_fwd{t} {} hold_exit{t} {} hold_exit_at{t} {} crossfire{t} {} crossfire_at{t} {} cut{t} {} cut_at{t} {} ",
                w[0],
                w[1],
                w[2],
                self.kills[t][0],
                self.kills[t][1],
                self.deaths[t][0],
                self.deaths[t][1],
                s(0)[0],
                s(0)[1],
                s(1)[0],
                s(1)[1],
                s(2)[0],
                s(2)[1],
            );
        }
        out += &format!("turned_bad {} handoffs {}", self.turned_bad, self.handoffs);
        out
    }
}
