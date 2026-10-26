use super::*;

/// A team's shared state: knowledge, modifiers, skill, the commander's
/// clock.
pub(super) struct TeamState {
    pub(super) knowledge: TeamKnowledge,
    pub(super) mods: Mods,
    pub(super) skill: Skill,
    pub(super) cheats: CheatClock,
    pub(super) signals: MatchSignals,
    pub(super) next_commander: u32,
    /// Goals a bot of the team proved no way to, and until when the
    /// team's plays leave them ([`TEAM_NO_WAY_KEEP_MS`]).
    pub(super) no_way_to: Vec<(Vec3, u32)>,
    /// Where its bots got stuck or fell, for the match ([`TeamSpots`]).
    pub(super) spots: TeamSpots,
    /// Where a shooter killed its bots from, and the regions his line
    /// covers now ([`threat`]).
    pub(super) threat: threat::ThreatSpots,
    pub(super) covered: Option<Arc<threat::Covered>>,
}

/// A team's threat spots at its commander tick ([`threat`]): the
/// shooters its bots sense now, spots cleared where a bot sees them
/// empty, a held spot cast at, and the regions his line covers told to
/// the team's bots when they changed.
fn threat_tick(
    tactics: &Tactics,
    bots: &mut [Option<Bot>],
    state: &mut TeamState,
    team: u8,
    view: &MatchView,
    now: u32,
    notes: &mut Vec<Note>,
) {
    if state.threat.is_empty() && state.covered.is_none() {
        return;
    }
    let eye = Vec3::new(0.0, view::eye_height(false, 0.0, 0.0), 0.0);
    let watchers: Vec<Vec3> = view.team(team).map(|p| p.feet + eye).collect();
    let mut enemies = Vec::new();
    for bot in bots.iter().flatten().filter(|b| b.team == team) {
        for track in bot.memory.tracks() {
            if let Some((at, time)) = track.own
                && now.saturating_sub(time) <= threat::NOW_MS
            {
                enemies.push(at);
                state.threat.shooter_sensed(track.who, at, time);
            }
        }
    }
    state.threat.tick(now, &watchers, &enemies, tactics);
    let mut regions = state.threat.covered_regions(now);
    regions.sort_unstable();
    regions.dedup();
    let same = match &state.covered {
        Some(covered) => covered.regions() == &regions[..],
        None => regions.is_empty(),
    };
    if same {
        return;
    }
    notes.push(Note {
        place: usize::MAX,
        time: now,
        text: format!(
            "team {team}: threat spots keep {} regions out of the routes ({} held)",
            regions.len(),
            state.threat.held(now).count()
        ),
    });
    state.covered = (!regions.is_empty())
        .then(|| Arc::new(threat::Covered::build(regions, tactics.regions().len(), tactics.region_lookup())));
    let shared = state.covered.clone().unwrap_or_default();
    for bot in bots.iter_mut().flatten().filter(|b| b.team == team) {
        bot.threat = shared.clone();
    }
}

impl Bots {
    /// A team's commander tick: signals, modifiers, skill, and a play for
    /// each living bot.
    pub(super) fn commander(&mut self, team: u8, view: &MatchView, notes: &mut Vec<Note>) {
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        // The bench's human stand-in: a human to the plays.
        #[cfg(test)]
        let stand_in;
        #[cfg(test)]
        let view = match self.bench_human {
            Some(place) => {
                stand_in = {
                    let mut view = view.clone();
                    if let Some(p) = view.places.iter_mut().find(|p| p.place == place) {
                        p.bot = false;
                    }
                    view
                };
                &stand_in
            }
            None => view,
        };
        let tuning = self.tuning_of(team);
        let now = view.clock;
        let state = &mut self.teams[usize::from(team)];
        state.next_commander = now + tuning.think.commander_ms;
        state.knowledge.forget_old(now);
        state.knowledge.settle_looks(team, view);
        // What a bot of the team proved no way to lately, the whole team
        // leaves for longer, from the proof on.
        state.no_way_to.retain(|&(_, until)| now < until);
        for bot in self.bots.iter_mut().flatten().filter(|b| b.team == team) {
            learn_no_way(&mut state.no_way_to, &bot.no_way_to, now);
            state.spots.learn(now, &bot.stuck.tell());
        }
        // Where its bots got stuck or fell, every bot keeps clear of.
        let spots = state.spots.avoided();
        for bot in self.bots.iter_mut().flatten().filter(|b| b.team == team) {
            bot.stuck.share(spots.clone());
        }
        let since_theft = state
            .knowledge
            .thefts
            .iter()
            .map(|t| now.saturating_sub(t.time) as f32 / 1000.0)
            .fold(1e6, f32::min);
        state.signals = MatchSignals::read(view, team, since_theft);
        state.mods.update(&state.signals, &tuning.mods);
        // A team that can no longer respawn (Team Deathmatch, its tickets
        // out) plays it safe.
        if !view.can_respawn[usize::from(team & 1)] {
            state.mods.aggression = state.mods.aggression.min(0.2);
        }
        if let Some((before, after, kd)) = state.skill.balance(now, &tuning.skill) {
            notes.push(Note {
                place: usize::MAX,
                time: now,
                text: format!(
                    "team {team}'s skill {before:.2} -> {after:.2} (K/D against humans {kd:.2})"
                ),
            });
        }
        // Partners for play 21: the team's bots two by two (pairs, play
        // 25); an odd one out partners its nearest living teammate bot one
        // way, so a third bot too trades its partner's death.
        let mates: Vec<Place> = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .map(|b| b.place)
            .collect();
        for pair in mates.chunks(2) {
            match pair {
                [a, b] => {
                    for (me, other) in [(*a, *b), (*b, *a)] {
                        if let Some(Some(bot)) = self.bots.get_mut(me) {
                            bot.partner = Some(other);
                            bot.paired = true;
                        }
                    }
                }
                [odd] => {
                    let alive = |place: Place| view.place(place).filter(|p| p.alive);
                    let nearest = alive(*odd).and_then(|me| {
                        mates
                            .iter()
                            .filter(|&&other| other != *odd)
                            .filter_map(|&other| alive(other))
                            .min_by(|a, b| {
                                a.feet
                                    .distance_xz(me.feet)
                                    .total_cmp(&b.feet.distance_xz(me.feet))
                            })
                            .map(|p| p.place)
                    });
                    if let Some(Some(bot)) = self.bots.get_mut(*odd) {
                        // A partner down is kept (its death is the one to
                        // trade); dead itself, or none alive: the one it
                        // had, else the previous bot.
                        let kept = bot.partner.filter(|p| mates.contains(p) && p != odd);
                        let down = kept.filter(|&p| alive(p).is_none());
                        bot.partner = down
                            .or(nearest)
                            .or(kept)
                            .or_else(|| mates.len().checked_sub(2).map(|i| mates[i]));
                        bot.paired = false;
                    }
                }
                _ => {}
            }
        }
        // The bots checking spawners' spots now, and where (no more of
        // them than batteries missing, and apart, `plays::collect`).
        let searching = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .filter_map(|b| match b.assignment.as_ref()? {
                a if a.kind == plays::PlayKind::Collect => match a.target {
                    plays::TargetRef::Spawner(index) => Some((b.place, index)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        self.teams[usize::from(team)].knowledge.searching = searching;
        // What the team knows of each enemy human, for the match (CG-12).
        let tracks = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .flat_map(|b| b.memory.tracks());
        self.teams[usize::from(team)]
            .knowledge
            .learn_humans(team, view, tracks, now);
        // Where a shooter kills from, and the regions his line covers
        // (K1).
        if tuning.threat.spots {
            threat_tick(tactics, &mut self.bots, &mut self.teams[usize::from(team)], team, view, now, notes);
        }
        let state = &self.teams[usize::from(team)];
        let mut asked: Vec<(Place, Vec<plays::Candidate>, Option<Assignment>)> = Vec::new();
        let mut blocks = Vec::new();
        // Bots with no fresh hold on a slot (`team::HOLD_FRESH_MS`): a way
        // to their goal or their play's target failed lately, or a carrier.
        let mut shaky: Vec<Place> = Vec::new();
        for bot in self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team && b.entered.is_some())
        {
            let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
                continue;
            };
            let no_way = team_no_way(&bot.no_way_to, &state.no_way_to);
            let cx = Ctx {
                now,
                view,
                me,
                bot: BotFacts {
                    personality: bot.personality(&tuning),
                    side: bot.side,
                    current: bot.assignment.as_ref().map(|a| (a.kind, a.target, a.since)),
                    last_ran: &bot.last_ran,
                    partner: bot.partner,
                    paired: bot.paired,
                    medkits_used: bot.medkits_used,
                    no_way_to: &no_way,
                    set_aside: &bot.set_aside,
                    failed_stands: &bot.failed_stands,
                    dry: bot.reflex.dry(),
                    crossing: walks_crossing(bot),
                },
                memory: &bot.memory,
                team: &state.knowledge,
                sig: &state.signals,
                mods: &state.mods,
                tactics,
                tuning: &tuning,
                memo: &self.memo,
            };
            let mut candidates = Vec::new();
            plays::candidates(&cx, &mut candidates);
            // Collect's answers read, not asked again (`take_blocks`).
            blocks.push((bot.place, plays::collect::take_blocks(&cx, &candidates)));
            #[cfg(test)]
            if self.bench_human == Some(bot.place)
                && candidates.iter().any(|c| c.kind == plays::PlayKind::Raid)
            {
                candidates.retain(|c| c.kind == plays::PlayKind::Raid);
            }
            let goal = match bot.intent.go {
                Go::To { at, .. } => Some(at),
                _ => None,
            };
            let target = bot.assignment.as_ref().and_then(|a| cx.point_of(a.target));
            if me.carrying.is_some()
                || goal_shut(&bot.intent, me, &bot.no_way_to, now)
                || goal.into_iter().chain(target).any(|at| cx.no_way_lately(at))
            {
                shaky.push(bot.place);
            }
            asked.push((bot.place, candidates, bot.assignment.clone()));
        }
        let asking = asked
            .iter()
            .map(|(place, candidates, current)| Asking {
                place: *place,
                candidates: candidates.clone(),
                current: current.as_ref(),
            })
            .collect();
        let gap = state.signals.score_gap;
        // [pockets] Bots given up in a pocket (`escape.rs`) let their slots go.
        let given_up: Vec<Place> = asked
            .iter()
            .map(|&(place, _, _)| place)
            .filter(|&place| tactics.escapes.trapped(place).is_some_and(|t| t.gave_up))
            .collect();
        for (place, assignment, changed) in team::assign_trapped(asking, now, gap, &given_up, &shaky) {
            let Some(me) = view.place(place) else {
                continue;
            };
            let Some(Some(bot)) = self.bots.get_mut(place) else {
                continue;
            };
            if !changed {
                bot.assignment = Some(assignment);
                continue;
            }
            #[cfg(test)]
            if given_up.contains(&place)
                && bot.assignment.as_ref().is_some_and(|a| a.slot.is_some())
            {
                escape::counts::handed();
            }
            // The new play starts.
            bot.give_way(assignment.kind, now);
            let no_way = team_no_way(&bot.no_way_to, &state.no_way_to);
            let cx = Ctx {
                now,
                view,
                me,
                bot: BotFacts {
                    personality: bot.personality(&tuning),
                    side: bot.side,
                    current: Some((assignment.kind, assignment.target, now)),
                    last_ran: &bot.last_ran,
                    partner: bot.partner,
                    paired: bot.paired,
                    medkits_used: bot.medkits_used,
                    no_way_to: &no_way,
                    set_aside: &bot.set_aside,
                    failed_stands: &bot.failed_stands,
                    dry: bot.reflex.dry(),
                    crossing: walks_crossing(bot),
                },
                memory: &bot.memory,
                team: &state.knowledge,
                sig: &state.signals,
                mods: &state.mods,
                tactics,
                tuning: &tuning,
                memo: &self.memo,
            };
            let mut rng = bot.rng.clone();
            let running = plays::start(
                assignment.kind,
                assignment.target,
                assignment.slot,
                &cx,
                &mut rng,
            );
            bot.rng = rng;
            bot.note(
                notes,
                now,
                format!(
                    "takes {:?} {:?} (score {:.2}: {})",
                    assignment.kind, assignment.target, assignment.score, assignment.reason
                ),
            );
            if self.trace.enabled() {
                let (mut top, left) = asked
                    .iter()
                    .find(|(p, _, _)| *p == place)
                    .map(|(_, c, was)| (c.clone(), was.clone()))
                    .unwrap_or_default();
                let bands = team::Bands::used();
                top.sort_by(|a, b| bands.better(a, b, gap < 0.0));
                let json = |c: &plays::Candidate| {
                    serde_json::json!({
                        "play": format!("{:?}", c.kind),
                        "target": format!("{:?}", c.target),
                        "rank": format!("{:?}", c.rank),
                        "score": c.score.get(),
                        "parts": c.why.parts.iter()
                            .map(|(i, v, s)| serde_json::json!([format!("{i:?}"), v, s]))
                            .collect::<Vec<_>>(),
                    })
                };
                // The play it leaves: its candidate now (null when it is
                // none), so a trace says why it was left.
                let left = left.map(|was| {
                    let now_as = top
                        .iter()
                        .find(|c| c.kind == was.kind && c.target == was.target)
                        .map(json);
                    serde_json::json!({
                        "play": format!("{:?}", was.kind),
                        "since": was.since,
                        "candidate": now_as,
                    })
                });
                let top: Vec<serde_json::Value> = top.iter().take(5).map(json).collect();
                self.trace.line(serde_json::json!({
                    "t": now, "bot": place, "team": team,
                    "pos": me.feet.to_array(), "health": me.health,
                    "takes": format!("{:?}", assignment.kind),
                    "reason": assignment.reason,
                    "left": left,
                    "top": top,
                    "mods": [state.mods.aggression, state.mods.risk, state.mods.objective_urgency],
                    "skill": state.skill.skill,
                }));
            }
            bot.running = Some(running);
            bot.end_route(RouteEnd::NewAssignment, now);
            bot.assignment = Some(assignment);
        }
        // Why known batteries left a bot's Collect candidates, when it
        // changed (`collect::note_take_blocks`).
        let knowledge = &mut self.teams[usize::from(team)].knowledge;
        for (place, blocks) in blocks {
            for text in plays::collect::note_take_blocks(knowledge, place, &blocks) {
                notes.push(Note {
                    place,
                    time: now,
                    text,
                });
            }
        }
    }
}

/// A team learns its bot's goals with no way lately (`Bot::no_way_to`,
/// kept [`NO_WAY_KEEP_MS`] from the proof): each is left by the team
/// [`TEAM_NO_WAY_KEEP_MS`] from its latest proof.
pub(super) fn learn_no_way(team: &mut Vec<(Vec3, u32)>, own: &[(Vec3, u32)], now: u32) {
    for &(goal, until) in own {
        let until = until - NO_WAY_KEEP_MS.min(until) + TEAM_NO_WAY_KEEP_MS;
        if now >= until {
            continue;
        }
        match team
            .iter_mut()
            .find(|(old, _)| old.distance_xz(goal) <= REPLAN_MOVED_M)
        {
            Some(old) => old.1 = old.1.max(until),
            None => team.push((goal, until)),
        }
    }
}

/// A bot's goals with no way lately and its team's
/// ([`TEAM_NO_WAY_KEEP_MS`]), for the commander's choice.
pub(super) fn team_no_way(own: &[(Vec3, u32)], team: &[(Vec3, u32)]) -> Vec<(Vec3, u32)> {
    own.iter().chain(team).copied().collect()
}
