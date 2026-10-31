use super::*;

/// A dead enemy carrier's look spot is made only from a sense of him this
/// recent, ms.
pub(super) const LOOK_SENSED_MS: u32 = 30_000;

/// After a theft whose thief dropped the battery alive ([`Bots::theft_drop`]):
/// the team looks round its container at this many spots this far off it
/// (each looked at within `battery_near_m`, 3 m: the ring 1-7 m off),
/// while the theft is this recent (ms); the looks go after
/// [`THEFT_LOOK_KEEP_MS`].
pub(super) const THEFT_RING_POINTS: usize = 6;

pub(super) const THEFT_RING_M: f32 = 4.0;

pub(super) const THEFT_RING_MS: u32 = 20_000;

pub(super) const THEFT_LOOK_KEEP_MS: u32 = 30_000;

impl Bots {
    /// The rest of a trusted world: deaths become danger spots and duels,
    /// a battery of ours carried off is a theft.
    ///
    /// A lying battery's position is taken as the world gives it: the
    /// worlds are in the map's frame (PROVEN, 0x4da3b51d: the batteries at
    /// 50 s lie on level_08's spawner spots to 0.01 m). The bots once
    /// guessed a mirrored z until a battery far from z = 0 voted otherwise;
    /// on level_05, whose spawners lie at z -0.5 to -1, they took the mirror
    /// image all match and never took those batteries.
    pub fn facts(&mut self, facts: WorldFacts) -> Vec<Note> {
        let mut notes = Vec::new();
        let time = facts.time;
        let at = |clock| Clock {
            clock,
            time_left_s: 0.0,
            total_s: 0.0,
            playing: true,
        };
        let before = self.tracker.view(&at(time), &self.tunings.match_wide().kin, None);
        let new_deaths = self.tracker.facts(facts);
        // Who the view believes carries the batteries, and why (`view`).
        for text in self.tracker.take_notes() {
            notes.push(Note {
                place: usize::MAX,
                time,
                text,
            });
        }
        let after = self.tracker.view(&at(time), &self.tunings.match_wide().kin, None);
        let kills_by = |bot: bool| {
            after
                .places
                .iter()
                .zip(&before.places)
                .any(|(a, b)| a.bot == bot && a.kills > b.kills)
        };
        let (human_kills, bot_kills) = (kills_by(false), kills_by(true));
        for (a, b) in after.places.iter().zip(&before.places) {
            if a.deaths <= b.deaths {
                continue;
            }
            let team = usize::from(a.team & 1);
            // The kill feed's spot: where the body was last reckoned alive
            // (the world showing the death has it without feet, and was
            // fixed first: `before` has none). A body never placed gives
            // no spot.
            if let Some(death) = new_deaths.iter().find(|d| d.place == a.place) {
                self.teams[team].knowledge.danger.push(DangerSpot {
                    at: death.at,
                    time,
                    victim: a.place,
                });
                // And a loss for the spawners' search: the spot the bot
                // was sent to look at, if it was.
                let spawner = self
                    .bots
                    .iter()
                    .flatten()
                    .find(|b| b.place == a.place)
                    .and_then(|b| b.assignment.as_ref())
                    .and_then(|a| match (a.kind, a.target) {
                        (plays::PlayKind::Collect, plays::TargetRef::Spawner(i)) => Some(i),
                        _ => None,
                    });
                self.teams[team].knowledge.losses.push(Loss {
                    at: death.at,
                    time,
                    spawner,
                });
            }
            // [A145] Where an enemy that held a place died: the killing team's held spots of his, and the places its killers'
            // own senses had him staying at. Taken before his death takes the spots' sense.
            if let Some(defeated) = self.tunings.of(1 - team as u8).traps.at_defeated {
                use super::know::{Defeated, DefeatedKind};
                let winners = 1 - team;
                let keep_ms = (defeated.keep_s * 1000.0) as u32;
                let mut held: Vec<(Vec3, DefeatedKind)> = self.teams[winners]
                    .features
                    .threat
                    .spots
                    .held_of(a.place, time, usize::from(defeated.min_kills))
                    .into_iter()
                    .map(|at| (at, DefeatedKind::Spot))
                    .collect();
                let camp_ms = self.tunings.of(winners as u8).aim.combat.camp_ms;
                for killer in after
                    .places
                    .iter()
                    .zip(&before.places)
                    .filter(|(now, was)| usize::from(now.team & 1) == winners && now.kills > was.kills)
                    .map(|(now, _)| now.place)
                {
                    if let Some(at) = self
                        .bots
                        .iter()
                        .flatten()
                        .find(|b| b.place == killer)
                        .and_then(|b| b.reflex.camp_of(a.place, camp_ms))
                    {
                        held.push((at, DefeatedKind::Camp));
                    }
                }
                for (at, kind) in held {
                    let record = Defeated { at, time, shooter: a.place, kind };
                    if self.teams[winners].knowledge.defeat(record, keep_ms) {
                        #[cfg(test)]
                        super::traps::defeated::count_team(
                            winners as u8,
                            match kind {
                                DefeatedKind::Spot => super::traps::defeated::NOTED_SPOT,
                                DefeatedKind::Camp => super::traps::defeated::NOTED_CAMP,
                            },
                        );
                        notes.push(Note {
                            place: usize::MAX,
                            time,
                            text: format!(
                                "team {winners}: defeated holder {} at ({:.1}, {:.1}, {:.1}): {}",
                                a.place,
                                at.x,
                                at.y,
                                at.z,
                                match kind {
                                    DefeatedKind::Spot => "a held threat spot",
                                    DefeatedKind::Camp => "a camp",
                                }
                            ),
                        });
                    }
                }
            }
            // Threat spots (K1): the killer the team sensed lately, if the
            // kill feed gave one (one enemy's kills rose), is where this
            // death adds to a spot; a dead shooter's spots keep half.
            // A team's spots are kept by its own tuning: the dead's enemies drop the dead shooter's, the dead's team
            // adds where its killer was.
            if self.tunings.of(1 - team as u8).threat.spots {
                self.teams[1 - team].features.threat.spots.shooter_died(a.place, time);
            }
            // [threat.breach, traps.at_cleared] The killing team's kill feed: the enemy it killed, and its killer when one's
            // kills rose.
            let winners = self.tunings.of(1 - team as u8);
            if winners.threat.breach.is_some() || winners.traps.at_cleared.is_some() {
                let killers: Vec<Place> = after
                    .places
                    .iter()
                    .zip(&before.places)
                    .filter(|(now, was)| usize::from(now.team & 1) != team && now.kills > was.kills)
                    .map(|(now, _)| now.place)
                    .collect();
                let kills = &mut self.teams[1 - team].features.threat.kills;
                kills.retain(|k| time.saturating_sub(k.time) <= threat::DEATH_KEPT_MS);
                kills.push(super::breach::Kill {
                    victim: a.place,
                    killer: match killers[..] {
                        [killer] => Some(killer),
                        _ => None,
                    },
                    time,
                });
            }
            if self.tunings.of(team as u8).threat.spots {
                let killers: Vec<Place> = after
                    .places
                    .iter()
                    .zip(&before.places)
                    .filter(|(now, was)| usize::from(now.team & 1) != team && now.kills > was.kills)
                    .map(|(now, _)| now.place)
                    .collect();
                let deaths = &mut self.teams[team].features.threat.deaths;
                deaths.retain(|d| time.saturating_sub(d.time) <= threat::DEATH_KEPT_MS);
                deaths.push(threat::Death {
                    victim: a.place,
                    killer: match killers[..] {
                        [killer] => Some(killer),
                        _ => None,
                    },
                    time,
                });
                if let [killer] = killers[..]
                    && let Some(death) = new_deaths.iter().find(|d| d.place == a.place)
                    && let Some((_, sensed, at)) = self.last_sensed(killer, team, &[])
                    && sensed + threat::SENSED_MS >= time
                {
                    self.teams[team].features.threat.spots.died(death.at, killer, at, sensed, time, threat::SpotRules::of(&self.tunings.of(team as u8).threat));
                }
            }
            // An enemy known to carry a battery died: his team's HUD shows
            // where it fell, ours does not. The team looks where it last
            // sensed him itself (never the thief cheat's place).
            if !self.tunings.of(1 - team as u8).cheats.batteries {
                self.lost_carrier(a.place, 1 - team, (&before, &after), time, &mut notes);
            }
            // Self-balancing counts duels between bots and humans.
            if a.bot && human_kills {
                self.teams[team].skill.duel(Duel {
                    time,
                    bot_won: false,
                });
            }
            if !a.bot && bot_kills {
                self.teams[1 - team].skill.duel(Duel {
                    time,
                    bot_won: true,
                });
            }
            for bot in self.bots.iter_mut().flatten() {
                bot.memory.forget(a.place);
            }
            // Nor do the team's callouts of him bring him back (match
            // 0x5402a730: 115 of 125 bot lines 1.5-5 s after a death still
            // knew the dead enemy, told).
            for state in &mut self.teams {
                state.knowledge.forget_callouts(a.place);
            }
        }
        // With the battery cheat off, what each team's HUD shows of the
        // batteries.
        for team in 0..2u8 {
            if self.tunings.of(team).cheats.batteries {
                continue;
            }
            let knowledge = &mut self.teams[usize::from(team)].knowledge;
            knowledge.hud_update(team, &before, &after, time);
            knowledge.settle_looks(team, &after);
            for text in knowledge.battery_notes.drain(..) {
                let text = format!("team {team}: {text}");
                notes.push(Note {
                    place: usize::MAX,
                    time,
                    text,
                });
            }
        }
        // A battery of ours taken from home.
        for (battery, (now, then)) in after.batteries.iter().zip(&before.batteries).enumerate() {
            if let (BatteryState::Carried(thief), BatteryState::Home(id)) = (now, then)
                && let Some(container) = after.containers.iter().find(|c| c.id == *id)
            {
                let team = usize::from(container.team & 1);
                self.teams[team].knowledge.thefts_match += 1;
                self.teams[team].knowledge.thefts.push(Theft {
                    battery,
                    at: container.at,
                    time,
                    thief: *thief,
                });
                notes.push(Note {
                    place: usize::MAX,
                    time,
                    text: format!("team {team}'s battery {battery} is stolen"),
                });
            }
        }
        // A battery of ours stolen and dropped by its thief alive, or (in
        // one world) taken from home and dropped: the team looks for it.
        for (battery, (now, then)) in after.batteries.iter().zip(&before.batteries).enumerate()
        {
            if !matches!(now, BatteryState::Free(_)) {
                continue;
            }
            match *then {
                BatteryState::Carried(Some(thief))
                    if after.place(thief).is_some_and(|p| p.alive) =>
                {
                    let Some(thief_team) = after.place(thief).map(|p| usize::from(p.team & 1))
                    else {
                        continue;
                    };
                    let team = 1 - thief_team;
                    let Some(theft) = self.teams[team]
                        .knowledge
                        .thefts
                        .iter()
                        .rev()
                        .find(|t| t.battery == battery && t.thief.is_none_or(|p| p == thief))
                        .copied()
                    else {
                        continue;
                    };
                    self.theft_drop(
                        team,
                        battery,
                        theft.at,
                        theft.time,
                        Some(thief),
                        time,
                        &mut notes,
                    );
                }
                BatteryState::Home(id) => {
                    let Some(container) = after.containers.iter().find(|c| c.id == id) else {
                        continue;
                    };
                    let team = usize::from(container.team & 1);
                    self.theft_drop(team, battery, container.at, time, None, time, &mut notes);
                }
                _ => {}
            }
        }
        notes
    }

    /// The enemy at `who` died, and team `team` (his enemies) knew he
    /// carried a battery: it stole from us (the HUD's theft), or a bot of
    /// the team saw him carrying it. The battery fell where he died, shown
    /// on his team's HUD only (`TeamKnowledge::hud_update`), and a nook may
    /// hide it from sight (match 0x15aa5b99: 126 s 3.6 m from our
    /// container). The team gets a spot to look at: where it last sensed
    /// him itself, the killer's own sight first, else the newest sight,
    /// hearing or callout of him (never the thief cheat's place).
    pub(super) fn lost_carrier(
        &mut self,
        who: Place,
        team: usize,
        (before, after): (&MatchView, &MatchView),
        time: u32,
        notes: &mut Vec<Note>,
    ) {
        let knowledge = &self.teams[team].knowledge;
        // His battery: carried by him, or (a world showed him dead first,
        // and the carrier went with it) one carried by nobody known that
        // he was known to carry, lying now or still carried.
        let battery = (0..after.batteries.len()).find(|&b| {
            let was = before.batteries.get(b).copied();
            let carried = was == Some(BatteryState::Carried(Some(who)));
            let fell = was == Some(BatteryState::Carried(None))
                && matches!(
                    after.batteries[b],
                    BatteryState::Free(_) | BatteryState::Carried(None)
                );
            (carried || fell) && knowledge.knew_carrying(who, b)
        });
        let Some(battery) = battery else {
            return;
        };
        if knowledge.battery(battery).is_some() {
            return;
        }
        let killers: Vec<Place> = after
            .places
            .iter()
            .zip(&before.places)
            .filter(|(a, b)| usize::from(a.team & 1) == team && a.kills > b.kills)
            .map(|(a, _)| a.place)
            .collect();
        let Some((killer, sensed, at)) = self.last_sensed(who, team, &killers) else {
            return;
        };
        if sensed + LOOK_SENSED_MS < time {
            return;
        }
        self.teams[team].knowledge.add_look(LookSpot {
            battery,
            at,
            time,
            until: time + know::LOOK_KEEP_MS,
            theft: false,
        });
        notes.push(Note {
            place: usize::MAX,
            time,
            text: format!(
                "team {team} looks for battery {battery} where its carrier died ({}, {:.1} s before)",
                if killer { "the killer's sight" } else { "last sensed" },
                time.saturating_sub(sensed) as f32 / 1000.0
            ),
        });
    }

    /// Where team `team` last sensed the enemy at `who` itself: the sight
    /// of one of `killers` first, else the newest sight, hearing or
    /// callout of him (never the thief cheat's place). (Whether a killer's
    /// sight, when, where.)
    pub(super) fn last_sensed(&self, who: Place, team: usize, killers: &[Place]) -> Option<(bool, u32, Vec3)> {
        let knowledge = &self.teams[team].knowledge;
        // (the killer's sight, when, where)
        let mut best: Option<(bool, u32, Vec3)> = None;
        let mut take = |killer: bool, sensed: u32, at: Vec3| {
            if best.is_none_or(|(k, t, _)| (killer, sensed) > (k, t)) {
                best = Some((killer, sensed, at));
            }
        };
        for bot in self
            .bots
            .iter()
            .flatten()
            .filter(|b| usize::from(b.team & 1) == team)
        {
            if let Some(track) = bot.memory.tracks().iter().find(|t| t.who == who)
                && let Some((at, sensed)) = track.own
            {
                let killer = killers.contains(&bot.place) && track.source == Source::Seen;
                take(killer, sensed, at);
            }
        }
        for seen in knowledge.carriers_seen.iter().filter(|s| s.who == who) {
            take(false, seen.time, seen.at);
        }
        best
    }

    /// F2 of the analysis of the owner's matches 0x652d/0x1a09 of
    /// 2026-10-16: a battery of team `team` stolen at `theft_time` from its
    /// container at `container` lies now, and its thief (`thief`, if the
    /// team can tell) lives: he dropped it (the owner took batteries out
    /// and dropped them 2-5 m off; they lay unknown 23-40 s). The HUD gave
    /// the theft and the container's count; the drop is on his team's HUD
    /// only. Unless the team knows where it lies, it looks round its
    /// container (a ring of [`THEFT_RING_POINTS`] spots [`THEFT_RING_M`]
    /// off it, while the theft is [`THEFT_RING_MS`] recent) and where it
    /// last sensed him itself, if that is off the ring; one bot does it
    /// (Collect), and the spots go after [`THEFT_LOOK_KEEP_MS`].
    #[allow(clippy::too_many_arguments)]
    pub(super) fn theft_drop(
        &mut self,
        team: usize,
        battery: usize,
        container: Vec3,
        theft_time: u32,
        thief: Option<Place>,
        time: u32,
        notes: &mut Vec<Note>,
    ) {
        if self.tunings.of(team as u8).cheats.batteries || self.teams[team].knowledge.battery(battery).is_some() {
            return;
        }
        let mut spots = Vec::new();
        let ring = time.saturating_sub(theft_time) <= THEFT_RING_MS;
        if ring {
            spots.extend((0..THEFT_RING_POINTS).map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / THEFT_RING_POINTS as f32;
                container + Vec3::new(angle.cos(), 0.0, angle.sin()) * THEFT_RING_M
            }));
        }
        let sensed = thief
            .and_then(|who| self.last_sensed(who, team, &[]))
            .filter(|&(_, sensed, _)| sensed + LOOK_SENSED_MS >= time)
            .map(|(_, sensed, at)| (sensed, at));
        let far = THEFT_RING_M + self.tunings.of(team as u8).sense.battery_near_m;
        if let Some((_, at)) = sensed
            && (!ring || at.distance_xz(container) > far)
        {
            spots.push(at);
        }
        if spots.is_empty() {
            return;
        }
        self.teams[team].knowledge.add_theft_looks(
            battery,
            &spots,
            time,
            time + THEFT_LOOK_KEEP_MS,
        );
        notes.push(Note {
            place: usize::MAX,
            time,
            text: format!(
                "team {team} looks for battery {battery}, stolen {:.1} s before, its thief no longer carrying it ({}{})",
                time.saturating_sub(theft_time) as f32 / 1000.0,
                if ring { "round the container" } else { "" },
                match sensed {
                    Some((sensed, _)) if spots.len() > THEFT_RING_POINTS || !ring => format!(
                        "{}where last sensed {:.1} s before",
                        if ring { ", " } else { "" },
                        time.saturating_sub(sensed) as f32 / 1000.0
                    ),
                    _ => String::new(),
                }
            ),
        });
    }
}
