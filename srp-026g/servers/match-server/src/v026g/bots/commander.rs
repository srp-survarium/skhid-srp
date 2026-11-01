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
    /// What each feature keeps of the team ([`features`]).
    pub(super) features: TeamFeatures,
}

/// A team's threat spots at its commander tick ([`threat`]): the
/// shooters its bots sense now, spots cleared where a bot sees them
/// empty, a held spot cast at, the regions his line covers told to the
/// team's bots when they changed, a note per spot event (F4), and the
/// team's jobs on held spots (K2) with what the plays read of them.
#[allow(clippy::too_many_arguments)]
fn threat_tick(
    tactics: &Tactics,
    bots: &mut [Option<Bot>],
    state: &mut TeamState,
    team: u8,
    view: &MatchView,
    now: u32,
    tuning: &Tuning,
    objective: objective::ObjectiveMap,
    camper: Option<Place>,
    notes: &mut Vec<Note>,
    weapons: &[Option<reflex::WeaponKind>],
) {
    let rules = threat::SpotRules::of(&tuning.threat);
    let kept = Kept::of(&tuning.threat);
    let threat = &mut state.features.threat;
    // A sensed sniper is a spot (#836's `sniper_lanes`, or #25b's area memory).
    let sniper_spots = tuning.threat.sniper_lanes || tuning.threat.sniper_memory.is_some();
    if !sniper_spots && threat.spots.is_empty() && threat.covered.is_none() && threat.jobs.is_empty() {
        if !state.knowledge.threat.held.is_empty() || !state.knowledge.threat.jobs.is_empty() {
            state.knowledge.threat = Arc::default();
        }
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
                threat.spots.shooter_sensed(track.who, at, time, rules);
                if sniper_spots && weapons.get(track.who).copied().flatten() == Some(reflex::WeaponKind::Sniper) {
                    let combat = &tuning.aim.combat;
                    let reach = combat.band(reflex::WeaponKind::Sniper).1 + combat.band_slack_m;
                    // How: the track's source when its last report is the own sense (else a cheat's or the HUD's came
                    // since, and the spot keeps how it knew him).
                    let how = (track.time == time).then(|| threat::Sense::of(track.source)).flatten();
                    threat.spots.sniper_sensed(track.who, at, time, how, reach, now, rules);
                }
            }
        }
    }
    // The lanes cast by the team's bullets (#705's cover rule), else by sight as it was.
    if tuning.threat.lanes_pierce {
        let eyes = threat::FireEyes { tactics, cover: tuning.cover.rule() };
        threat.spots.tick(now, &watchers, &enemies, &eyes, rules);
    } else {
        threat.spots.tick(now, &watchers, &enemies, tactics, rules);
    }
    // [smoke] The held lanes, for a smoke across one (`smoke.cross_lane`).
    if tuning.smoke.cross_lane {
        let lanes: Arc<[threat::HeldLane]> = threat.spots.lanes_kept(now, kept.smoke).into();
        for bot in bots.iter_mut().flatten().filter(|b| b.team == team) {
            bot.lanes = lanes.clone();
        }
    }
    for event in threat.spots.take_events() {
        notes.push(Note {
            place: usize::MAX,
            time: event.time,
            text: format!("team {team}: {event}"),
        });
    }
    // The jobs (K2) and the picture the plays read, every tick (the bots
    // move).
    let held = threat.spots.held_lines(now, rules);
    // [threat.concede] The held lines whose shooter's area the team conceded or tried (design-p1c §3), by id, sorted; and
    // the memory's areas left open long enough forgotten.
    let concede = match tuning.threat.concede {
        Some(_) => {
            threat.spot_memory.forget_old(now);
            let memory = &threat.spot_memory;
            let mut offers = super::concede::Offers::default();
            for line in &held {
                match memory.area_at(line.at).map(|area| memory.offers(area, now)) {
                    Some(super::concede::Offer::None) => offers.conceded.push(line.id),
                    Some(super::concede::Offer::NotLike(last)) => offers.tried.push((line.id, last)),
                    Some(super::concede::Offer::Any) | None => {}
                }
            }
            offers.conceded.sort_unstable();
            offers.tried.sort_unstable_by_key(|&(id, _)| id);
            offers
        }
        None => super::concede::Offers::default(),
    };
    if tuning.threat.job {
        let mates: Vec<threat::Mate> = bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .filter_map(|b| {
                // The bench's stand-in camper (`camper`, none live) never
                // steps its plays (it holds its spot): it is given no role,
                // else it holds one until the job ends (336 of team 0's
                // 3,109 roles, 5504b7a4).
                if camper == Some(b.place) {
                    return None;
                }
                let p = view.place(b.place).filter(|p| p.alive)?;
                // The job play it runs now, if any.
                let playing = b.assignment.as_ref().and_then(|a| match (a.kind, a.target) {
                    (plays::PlayKind::Flank, plays::TargetRef::ThreatSpot(id)) => Some((threat::Role::Flanker, id)),
                    (plays::PlayKind::Crossfire, plays::TargetRef::ThreatSpot(id)) => Some((threat::Role::Pinner, id)),
                    _ => None,
                });
                Some(threat::Mate { place: b.place, feet: p.feet, free: !objective.is_actor(p) && !b.reflex.dry(), playing })
            })
            .collect();
        let mut holds = threat::holds(&held, tuning.threat.job_kills);
        // [threat.concede] No job on a conceded area, nor a new one like the last failed try on a tried one (a Flank: the
        // same kind, so only its time differs; design-p1c §3).
        if let Some(concede) = tuning.threat.concede {
            let (memory, jobs, rules) = (&threat.spot_memory, &threat.jobs, concede.rules());
            holds.retain(|l| {
                let Some(area) = memory.area_at(l.at) else {
                    return true;
                };
                let running = jobs.iter().any(|j| j.spot == l.id);
                match memory.offers(area, now) {
                    super::concede::Offer::Any => true,
                    super::concede::Offer::None => {
                        super::concede::tally(team, super::concede::Tally::JobRefusedConceded);
                        false
                    }
                    super::concede::Offer::NotLike(last) => {
                        let next = super::concede::Try { kind: super::concede::TryKind::Job(super::concede::JobKind::Flank), bearing: last.bearing, at: now };
                        let differs = running || super::concede::differs(&last, &next, &rules);
                        if !differs {
                            super::concede::tally(team, super::concede::Tally::JobRefusedLike);
                        }
                        differs
                    }
                }
            });
        }
        // [threat.pair_target] Two holds covering one lane: a job on the one with more kills only (design-p1c §6).
        if let Some(pair) = tuning.threat.pair_target {
            holds = threat::one_of_each_pair(holds, &threat.jobs, pair.step_m);
        }
        let areas = objective.home_spots(&tactics.world.map);
        let job_team = threat::JobTeam { mates: &mates, size: view.team(team).count(), objective_areas: &areas };
        let spots = &threat.spots;
        let unheld = |id| if concede.is_conceded(id) { threat::JobEnd::Conceded } else { spots.unheld_why(id, now) };
        // A role's bot stands out of every held line, or of the holds' only (the switch off).
        let lanes = if tuning.threat.job_out_of_all_lines { &held[..] } else { &holds[..] };
        let opening = if tuning.threat.job_open_staffed { threat::Opening::Staffed } else { threat::Opening::ThenDropped };
        let events = threat::jobs_tick(&mut threat.jobs, &mut threat.rested, &holds, lanes, opening, &job_team, &unheld, now);
        threat.deaths.retain(|d| now.saturating_sub(d.time) <= threat::DEATH_KEPT_MS);
        for event in events {
            threat::count_job(event);
            let mut text = format!("team {team}: {event}");
            match event {
                threat::JobEvent::Ended { end, life_ms, shooter, .. } => {
                    threat::job_tally(team, threat::JobTally::of_end(end));
                    threat::job_life(team, life_ms);
                    // A holder down: whom to (the job outcome, by team).
                    if let threat::JobEnd::HolderDown { place, .. } = end {
                        let killer = threat.deaths.iter().rev().find(|d| d.victim == place).and_then(|d| d.killer);
                        let (by, said) = match killer {
                            Some(k) if k == shooter => (threat::JobTally::DownByShooter, "the shooter".to_string()),
                            Some(k) => (threat::JobTally::DownByOther, format!("another enemy ({k})")),
                            None => (threat::JobTally::DownUnknown, "not known".to_string()),
                        };
                        threat::job_tally(team, by);
                        text.push_str(&format!(", killed by {said}"));
                    }
                }
                threat::JobEvent::Given { spot, role, place } => {
                    threat::offers_reset(place, role);
                    // Fairness: a role on a spot whose shooter the team has
                    // not sensed for `SEEN_MS`.
                    let stale = held
                        .iter()
                        .find(|l| l.id == spot)
                        .is_some_and(|l| l.seen.is_none_or(|t| now.saturating_sub(t) > threat::SEEN_MS));
                    if stale {
                        threat::job_tally(team, threat::JobTally::GivenUnsensed);
                    }
                }
                threat::JobEvent::Passed { role, place, why: threat::PassedWhy::NoPlay, .. } => {
                    // The bench's class of a role let go unplayed: its last offer, and the play its bot runs.
                    let running = bots.iter().flatten().find(|b| b.place == place).and_then(|b| b.assignment.as_ref()).map(|a| format!("{:?}", a.kind));
                    threat::passed_unplayed(place, role, running.as_deref());
                }
                threat::JobEvent::Passed { .. } => {}
            }
            notes.push(Note {
                place: usize::MAX,
                time: now,
                text,
            });
        }
    }
    let sniper_regions = if tuning.threat.sniper_memory.is_some() { threat.spots.sniper_regions(now) } else { Vec::new() };
    // [threat.answer_sniper, A210] Play 28 answers a sniper the team remembers, not only one the bot sensed lately.
    let snipers = if tuning.threat.answer_sniper { threat.spots.sniper_spots(now) } else { Vec::new() };
    let picture = threat::Picture { held, jobs: threat.jobs.clone(), sniper_regions, snipers, concede };
    if *state.knowledge.threat != picture {
        state.knowledge.threat = Arc::new(picture);
    }
    // [smoke.spot_rules] A region a cloud hides from the spot's shooter is not his while it stands: routes cross under it
    // (the detour weighs the lanes) and no lane smoke is planned for it.
    let planning = view.smoke.planning(now, team, super::smoke::Forming::of(&tuning.smoke));
    let mut regions = if tuning.smoke.spot_rules && !planning.is_empty() {
        let (grid, map) = (&tactics.world.grid, tactics.regions());
        let (eye, chest) = (Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.0, 1.3, 0.0));
        let (covered, opened) = threat.spots.covered_regions_smoked(now, threat::KeptOut::All, |at, r| {
            planning.covers(now, &tuning.smoke, at + eye, grid.position(map.middle(r)) + chest)
        });
        // The regions opened are counted by the bench only (crossings under a cloud).
        #[cfg(test)]
        if !opened.is_empty() {
            under_cloud(bots, team, grid, map, &opened, tuning, now);
        }
        #[cfg(not(test))]
        let _ = opened;
        covered
    } else {
        // [bench] With the rules off the crossings under a cloud are counted all the same (count, don't act), so the
        // two arms of a head-to-head read alike; the covered regions are #801's.
        #[cfg(test)]
        if !planning.is_empty() {
            let (grid, map) = (&tactics.world.grid, tactics.regions());
            let (eye, chest) = (Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.0, 1.3, 0.0));
            let (_, opened) = threat.spots.covered_regions_smoked(now, threat::KeptOut::All, |at, r| {
                planning.covers(now, &tuning.smoke, at + eye, grid.position(map.middle(r)) + chest)
            });
            if !opened.is_empty() {
                under_cloud(bots, team, grid, map, &opened, tuning, now);
            }
        }
        threat.spots.covered_regions(now)
    };
    regions.sort_unstable();
    regions.dedup();
    // [threat.sniper_memory] The regions of the kill spots alone, for a consumer whose switch keeps the sensed snipers'
    // spots out of it (none with the memory off).
    let kills_regions = kept.any_kills_only().then(|| {
        let mut r = if tuning.smoke.spot_rules && !planning.is_empty() {
            let (grid, map) = (&tactics.world.grid, tactics.regions());
            let (eye, chest) = (Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.0, 1.3, 0.0));
            threat.spots.covered_regions_smoked(now, threat::KeptOut::KillsOnly, |at, r| {
                planning.covers(now, &tuning.smoke, at + eye, grid.position(map.middle(r)) + chest)
            }).0
        } else {
            threat.spots.covered_regions_kept(now, threat::KeptOut::KillsOnly)
        };
        r.sort_unstable();
        r.dedup();
        r
    });
    let same_as = |was: &Option<Arc<threat::Covered>>, regions: &[u32]| match was {
        Some(covered) => covered.regions() == regions,
        None => regions.is_empty(),
    };
    let kills_same = match &kills_regions {
        Some(r) => same_as(&threat.covered_kills, r),
        None => threat.covered_kills.is_none(),
    };
    if same_as(&threat.covered, &regions) && kills_same {
        return;
    }
    let build = |regions: Vec<u32>| {
        (!regions.is_empty()).then(|| Arc::new(threat::Covered::build(regions, tactics.regions().len(), tactics.region_lookup())))
    };
    if !kills_same {
        threat.covered_kills = kills_regions.and_then(build);
    }
    if same_as(&threat.covered, &regions) {
        kept.tell(bots, team, threat);
        return;
    }
    notes.push(Note {
        place: usize::MAX,
        time: now,
        text: format!(
            "team {team}: threat spots keep {} regions out of the routes ({} held)",
            regions.len(),
            threat.spots.held(now).count()
        ),
    });
    threat.covered = build(regions);
    kept.tell(bots, team, threat);
}

/// [threat.sniper_memory] Which held spots each consumer of the covered regions reads (`SniperMemory`'s switches; all of
/// them with the memory off).
#[derive(Clone, Copy)]
struct Kept {
    routes: threat::KeptOut,
    sprint: threat::KeptOut,
    smoke: threat::KeptOut,
}

impl Kept {
    fn of(t: &tuning::ThreatTuning) -> Self {
        let of = |on: bool| if on { threat::KeptOut::All } else { threat::KeptOut::KillsOnly };
        match t.sniper_memory {
            Some(m) => Self { routes: of(m.routes), sprint: of(m.sprint), smoke: of(m.smoke) },
            None => Self { routes: threat::KeptOut::All, sprint: threat::KeptOut::All, smoke: threat::KeptOut::All },
        }
    }

    fn any_kills_only(self) -> bool {
        [self.routes, self.sprint, self.smoke].contains(&threat::KeptOut::KillsOnly)
    }

    /// The team's bots told their covered regions, each consumer's.
    fn tell(self, bots: &mut [Option<Bot>], team: u8, threat: &threat::TeamThreat) {
        let pick = |kept| {
            match kept {
                threat::KeptOut::All => &threat.covered,
                threat::KeptOut::KillsOnly => &threat.covered_kills,
            }
            .clone()
            .unwrap_or_default()
        };
        let (routes, sprint, smoke) = (pick(self.routes), pick(self.sprint), pick(self.smoke));
        for bot in bots.iter_mut().flatten().filter(|b| b.team == team) {
            bot.threat = routes.clone();
            bot.sprint_threat = sprint.clone();
            bot.smoke_threat = smoke.clone();
        }
    }
}

/// [smoke.spot_rules, bench] The team's bots whose route ahead (`smoke.lane_ahead_m`, 2 m steps) enters a region a cloud
/// opened: crossings under an existing cloud, counted once a bot 10 s apart.
#[cfg(test)]
fn under_cloud(
    bots: &mut [Option<Bot>],
    team: u8,
    grid: &mapdata::WalkGrid,
    map: &mapdata::regions::Regions,
    opened: &[u32],
    tuning: &Tuning,
    now: u32,
) {
    for bot in bots.iter_mut().flatten().filter(|b| b.team == team) {
        let Some((route, _)) = bot.route.as_ref() else {
            continue;
        };
        let heights = bot.motor.heights();
        let start = route.walked();
        let end = (start + tuning.smoke.lane_ahead_m.get()).min(route.length());
        let enters = (1..)
            .map(|k| start + k as f32 * 2.0)
            .take_while(|&s| s <= end)
            .filter_map(|s| grid.locate(route.at(s, heights), 1.5))
            .any(|n| opened.binary_search(&map.of(n)).is_ok());
        if enters {
            super::smoke::tally::under_cloud(bot.place, bot.team, now);
        }
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
        state.refresh_traps(&tactics.world.grid, tuning.traps.ring_m);
        let field = state.features.traps.field.clone();
        for bot in self.bots.iter_mut().flatten().filter(|b| b.team == team) {
            bot.features.traps.field = field.clone();
        }
        // What a bot of the team proved no way to lately, the whole team
        // leaves for longer, from the proof on.
        state.features.stuck.no_way_to.retain(|&(_, until)| now < until);
        for bot in self.bots.iter_mut().flatten().filter(|b| b.team == team) {
            learn_no_way(&mut state.features.stuck.no_way_to, &bot.no_way_to, now);
            state.features.stuck.spots.learn(now, &bot.stuck.tell());
        }
        // Where its bots got stuck or fell, every bot keeps clear of.
        let spots = state.features.stuck.spots.avoided();
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
        // [A192] The bots running Crossfire now, and on which choke.
        let crossfiring = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .filter_map(|b| b.assignment.as_ref().filter(|a| a.kind == plays::PlayKind::Crossfire).map(|a| (b.place, a.target)))
            .collect();
        self.teams[usize::from(team)].knowledge.crossfiring = crossfiring;
        // What the team knows of each enemy human, for the match (CG-12).
        let tracks = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .flat_map(|b| b.memory.tracks());
        self.teams[usize::from(team)]
            .knowledge
            .learn_humans_as(team, view, tracks, now, self.bench_enemy_human_lanes(team));
        // Where a shooter kills from, and the regions his line covers
        // (K1).
        if tuning.threat.spots {
            // The bench's stand-in camper's place (`SRP_BENCH_CAMPER`); none live.
            #[cfg(test)]
            let camper = self.bench_camper.as_ref().map(|c| c.place());
            #[cfg(not(test))]
            let camper = None;
            threat_tick(tactics, &mut self.bots, &mut self.teams[usize::from(team)], team, view, now, &tuning, self.mode.objective(), camper, notes, &self.features.weapons);
        }
        let state = &self.teams[usize::from(team)];
        let mut asked: Vec<(Place, Vec<plays::Candidate>, Option<Assignment>)> = Vec::new();
        let mut blocks = Vec::new();
        // Bots with no fresh hold on a slot (`team::HOLD_FRESH_MS`): a way
        // to their goal or their play's target failed lately, or a carrier.
        let mut shaky: Vec<Place> = Vec::new();
        // Bots whose hold walks to its spot (`commit.hold_to_arrival`).
        let mut committed: Vec<Place> = Vec::new();
        for bot in self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team && b.entered.is_some())
        {
            let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
                continue;
            };
            let no_way = team_no_way(&bot.no_way_to, &state.features.stuck.no_way_to);
            let cx = Ctx {
                now,
                view,
                me,
                bot: BotFacts {
                    personality: bot.personality(&tuning),
                    weapon: bot.weapon(&self.features.weapons, &tuning),
                    weapons: &self.features.weapons,
                    side: bot.side,
                    current: bot.assignment.as_ref().map(|a| (a.kind, a.target, a.since)),
                    last_ran: &bot.last_ran,
                    partner: bot.partner,
                    paired: bot.paired,
                    medkits_used: bot.features.stock.medkits_used,
                    pantry: &bot.features.stock.pantry,
                    pools: bot.features.stock.pools,
                    zone_armour: &bot.features.crossing.armour,
                    no_way_to: &no_way,
                    set_aside: &bot.features.memory.set_aside,
                    failed_stands: &bot.features.memory.failed_stands,
                    dry: bot.reflex.dry(),
                    hit: bot.reflex.last_hit(),
                    ammo: bot.ammo(),
                    crossing: walks_crossing(bot),
                    held_choke: bot.held_choke,
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
            #[cfg(test)]
            if let Some(a) = bot.assignment.as_ref()
                && !candidates.iter().any(|c| c.kind == a.kind && c.target == a.target)
            {
                if a.kind == plays::PlayKind::Answer {
                    plays::answer::left_out(&cx, a.target);
                }
                let dist = match bot.intent.go {
                    Go::To { at, .. } => Some(me.feet.distance_xz(at)),
                    _ => None,
                };
                super::hold_trace::dropped(
                    a.kind,
                    a.target,
                    now.saturating_sub(a.since),
                    &cx.drop_reason(a.kind, a.target),
                    dist,
                );
            }
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
            if cx.view.objective().is_actor(me)
                || goal_shut(&bot.intent, me, &bot.no_way_to, now)
                || goal.into_iter().chain(target).any(|at| cx.no_way_lately(at))
            {
                shaky.push(bot.place);
            } else if tuning.commit.hold_to_arrival
                && bot.running.as_ref().is_some_and(plays::Running::travelling)
            {
                // A hold still walking to its spot (no way lately: not).
                committed.push(bot.place);
            }
            asked.push((bot.place, candidates, bot.assignment.clone()));
        }
        let asking = asked
            .iter()
            .map(|(place, candidates, current)| Asking {
                place: *place,
                candidates: candidates.clone(),
                current: current.as_ref(),
                committed: committed.contains(place),
            })
            .collect();
        let gap = state.signals.score_gap;
        // [pockets] Bots given up in a pocket (`escape.rs`) let their slots go.
        let given_up: Vec<Place> = asked
            .iter()
            .map(|&(place, _, _)| place)
            .filter(|&place| tactics.escapes.trapped(place).is_some_and(|t| t.gave_up))
            .collect();
        // [gang-up] The trade partners of the team's bots (not counted as a second bot on one enemy).
        let partners: Vec<(Place, Place)> = asked
            .iter()
            .filter_map(|&(place, _, _)| self.bots.get(place)?.as_ref()?.partner.map(|partner| (place, partner)))
            .collect();
        let gang = tuning.threat.gang_rule().map(|rule| team::GangUp { rule, partners: &partners, tally: &tactics.gang_tally });
        for (place, assignment, changed) in team::assign_trapped(
            asking,
            now,
            gap,
            &given_up,
            &shaky,
            team::Pass { gang: gang.as_ref(), held_in_pass: tuning.commit.held_in_pass },
        ) {
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
            #[cfg(test)]
            if let Some(old) = bot.assignment.as_ref()
                && old.kind == plays::PlayKind::Answer
            {
                let offered = asked
                    .iter()
                    .any(|(p, c, _)| *p == place && c.iter().any(|c| c.kind == old.kind && c.target == old.target));
                plays::answer::replaced(offered, assignment.kind);
            }
            #[cfg(test)]
            if let Some(old) = bot.assignment.as_ref() {
                let incumbent = asked
                    .iter()
                    .find(|(p, _, _)| *p == place)
                    .and_then(|(_, c, _)| c.iter().find(|c| c.kind == old.kind && c.target == old.target))
                    .map(|c| (c.rank, c.score.get()));
                let dist = match bot.intent.go {
                    Go::To { at, .. } => Some(me.feet.distance_xz(at)),
                    _ => None,
                };
                super::hold_trace::switched(
                    old.kind,
                    old.target,
                    now.saturating_sub(old.since),
                    incumbent,
                    assignment.kind,
                    asked
                        .iter()
                        .find(|(p, _, _)| *p == place)
                        .and_then(|(_, c, _)| {
                            c.iter().find(|c| c.kind == assignment.kind && c.target == assignment.target)
                        })
                        .map(|c| c.rank),
                    assignment.score,
                    assignment.reason,
                    dist,
                );
            }
            // The new play starts.
            bot.give_way(assignment.kind, now);
            // (Per start: a claim made by an earlier start of this pass is seen by the next.)
            let state = &self.teams[usize::from(team)];
            let no_way = team_no_way(&bot.no_way_to, &state.features.stuck.no_way_to);
            let cx = Ctx {
                now,
                view,
                me,
                bot: BotFacts {
                    personality: bot.personality(&tuning),
                    weapon: bot.weapon(&self.features.weapons, &tuning),
                    weapons: &self.features.weapons,
                    side: bot.side,
                    current: Some((assignment.kind, assignment.target, now)),
                    last_ran: &bot.last_ran,
                    partner: bot.partner,
                    paired: bot.paired,
                    medkits_used: bot.features.stock.medkits_used,
                    pantry: &bot.features.stock.pantry,
                    pools: bot.features.stock.pools,
                    zone_armour: &bot.features.crossing.armour,
                    no_way_to: &no_way,
                    set_aside: &bot.features.memory.set_aside,
                    failed_stands: &bot.features.memory.failed_stands,
                    dry: bot.reflex.dry(),
                    hit: bot.reflex.last_hit(),
                    ammo: bot.ammo(),
                    crossing: walks_crossing(bot),
                    held_choke: bot.held_choke,
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
            // A Crossfire on an enemy's last known place keeps the place
            // when the memory forgets him (`commit.crossfire_point`).
            let held_choke = match (assignment.kind, assignment.target) {
                (plays::PlayKind::Crossfire, plays::TargetRef::Place(who))
                    if tuning.commit.crossfire_point =>
                {
                    cx.point_of(assignment.target).map(|at| (who, at, now))
                }
                _ => None,
            };
            bot.rng = rng;
            bot.note(
                notes,
                now,
                format!(
                    "takes {:?} {:?} (score {:.2}: {})",
                    assignment.kind, assignment.target, assignment.score, assignment.reason
                ),
            );
            // [threat.concede] A play taken whose target lies in a conceded area: the objective's (its cost, item 13) or
            // another (a fight or roam play is refused there; what remains is the bot's own or an alarm).
            if tuning.threat.concede.is_some()
                && cx.point_of(assignment.target).is_some_and(|to| state.knowledge.threat.conceded_at(to).is_some())
            {
                let rank = plays::default_tuning(assignment.kind).rank;
                let tally = if rank == plays::Rank::Objective { super::concede::Tally::TakenObjective } else { super::concede::Tally::TakenOther };
                super::concede::tally(team, tally);
            }
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
            bot.held_choke = held_choke;
            // The route a Roam run is drawn on is claimed at once, for the draws that follow (`roam.fresh_claims`).
            let claim = running.route_claim();
            bot.running = Some(running);
            bot.end_route(RouteEnd::NewAssignment, now);
            bot.assignment = Some(assignment);
            if tuning.number(plays::PlayKind::Roam, "fresh_claims") > 0.5 {
                let knowledge = &mut self.teams[usize::from(team)].knowledge;
                match (claim, bot.side) {
                    (Some((route, drawn)), Some(side)) => {
                        knowledge.claim_route(place, side, route, drawn, now);
                        // For the log: which path drew the route, the start countdown's opening pass or the commander's pass in play.
                        let how = if self.opening_pass { "opening pass in the countdown" } else { "fresh claim in play" };
                        bot.note(notes, now, format!("draws route {route} of side {side} ({how})"));
                    }
                    _ => knowledge.release_route(place),
                }
            }
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
