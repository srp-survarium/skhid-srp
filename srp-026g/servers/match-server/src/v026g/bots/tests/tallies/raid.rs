use super::*;

/// One raid of a soak, from its take to the bot's next play.
#[derive(Clone, Debug, Default)]
pub(crate) struct RaidRun {
    /// The raider.
    pub(crate) place: Place,
    /// Its score at the take.
    pub(crate) score: f32,
    /// How long it ran, s.
    pub(crate) ran_s: f32,
    /// How near the eye came to their container's use box, m.
    pub(crate) nearest_m: f32,
    /// Uses pressed meanwhile.
    pub(crate) uses: u32,
    pub(crate) stole: bool,
    pub(crate) died: bool,
    /// What came next: the next play, or how it ended.
    pub(crate) next: String,
}

/// The raids of a run as they go ([`RaidRun`]), each bot's from its take
/// to its next play.
#[derive(Default)]
pub(crate) struct RaidTally {
    /// Each bot's raid under way: its start, and the run so far.
    pub(crate) raiding: Vec<Option<(u32, RaidRun)>>,
    pub(crate) runs: Vec<RaidRun>,
}

impl RaidTally {
    /// After the game's steps up to `clock`: the raiders' bodies, and the
    /// `thieves` of those steps.
    pub(crate) fn bodies(&mut self, game: &Game, thieves: &[Place], clock: u32) {
        self.raiding.resize(game.bodies.len(), None);
        for (place, raid) in self.raiding.iter_mut().enumerate() {
            let Some((since, run)) = raid else {
                continue;
            };
            let body = &game.bodies[place];
            if let Some(&(_, at)) = game.containers.iter().find(|(t, _)| *t != body.team) {
                let d = (at + USE_BOX_UP - Game::eye(body)).length();
                run.nearest_m = run.nearest_m.min(d);
            }
            run.stole |= thieves.contains(&place);
            run.died |= !body.alive;
            run.ran_s = clock.saturating_sub(*since) as f32 / 1000.0;
        }
    }

    /// After a poll: its notes, the plays now and the inputs made.
    pub(crate) fn polled(&mut self, bots: &Bots, notes: &[Note], made: &[Made]) {
        self.raiding.resize(bots.bots.len(), None);
        for note in notes {
            if let Some(Some((_, run))) = self.raiding.get_mut(note.place)
                && note.text.starts_with("ends Raid")
            {
                run.next = note.text.clone();
            }
        }
        for (place, raid) in self.raiding.iter_mut().enumerate() {
            let assignment = bots.bots[place]
                .as_ref()
                .and_then(|bot| bot.assignment.as_ref());
            let on = assignment.filter(|a| a.kind == super::plays::PlayKind::Raid);
            if raid.as_ref().map(|r| r.0) != on.map(|a| a.since) {
                if let Some((_, mut run)) = raid.take() {
                    if run.next.is_empty() {
                        run.next = assignment.map_or("none".into(), |a| format!("{:?}", a.kind));
                    }
                    self.runs.push(run);
                }
                *raid = on.map(|a| {
                    let run = RaidRun {
                        place,
                        score: a.score,
                        nearest_m: f32::INFINITY,
                        ..Default::default()
                    };
                    (a.since, run)
                });
            }
        }
        for made in made {
            if made.input.actions & key::USE != 0
                && let Some(Some((_, run))) = self.raiding.get_mut(made.place)
            {
                run.uses += 1;
            }
        }
    }

    /// The runs, those under way at the end too.
    pub(crate) fn finish(mut self) -> Vec<RaidRun> {
        for (_, mut run) in self.raiding.into_iter().flatten() {
            run.next = "the match's end".into();
            self.runs.push(run);
        }
        self.runs
    }
}

/// One line on `runs`: how many, how long, how near, the uses, the steals,
/// the deaths; and what came after them.
pub(crate) fn raid_line(runs: &[RaidRun]) -> String {
    let n = runs.len().max(1);
    let mut ran: Vec<f32> = runs.iter().map(|r| r.ran_s).collect();
    ran.sort_by(f32::total_cmp);
    let mut scores: Vec<f32> = runs.iter().map(|r| r.score).collect();
    scores.sort_by(f32::total_cmp);
    let mut next: std::collections::BTreeMap<&str, u32> = Default::default();
    for run in runs {
        let what = run.next.split(';').next().unwrap_or_default();
        *next.entry(what).or_default() += 1;
    }
    let mut next: Vec<(&str, u32)> = next.into_iter().collect();
    next.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    next.truncate(8);
    format!(
        "{} raids; score median {:.2}; ran median {:.1} s, over 10 s {}, over 30 s {}; came within 5 m {}, 2 m {}; used {}; stole {}; died on one {}; next {next:?}",
        runs.len(),
        scores.get(n / 2).copied().unwrap_or(0.0),
        ran.get(n / 2).copied().unwrap_or(0.0),
        runs.iter().filter(|r| r.ran_s > 10.0).count(),
        runs.iter().filter(|r| r.ran_s > 30.0).count(),
        runs.iter().filter(|r| r.nearest_m <= 5.0).count(),
        runs.iter().filter(|r| r.nearest_m <= 2.0).count(),
        runs.iter().filter(|r| r.uses > 0).count(),
        runs.iter().filter(|r| r.stole).count(),
        runs.iter().filter(|r| r.died).count(),
    )
}
