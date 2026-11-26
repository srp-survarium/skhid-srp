//! The per-match protocol state machine of a host child: it turns the
//! relay's frames (`Df`, `WantDe`, `Close`) into steps of the match's world and answers with `Tick` / `Outcome` /
//! `Stats` / `De` / `Final` / `Diverged` / `Fault`. `Create` / `Ready` are handled by the caller (they need the
//! level and the fork); this module never touches exe memory itself: the world is behind [`Sim`], so the state machine
//! is unit-tested with a fake world.
//!
//! Rules:
//! * the world runs in client mode: a late input rolls the world back, so a value is **final** only for ticks at
//!   least `final_lag_ms` (300) behind the newest stepped DF (late inputs reach at most 255 ms back);
//! * `Outcome` is latched once, at the first final tick whose rule state is 4 or 5; the per-player numbers are
//!   read when it latches;
//! * `Stats` every `stats_every_ms` (5000) of world time; it doubles as the heartbeat;
//! * `Diverged` once, at the first input `change_input` refused or moved (a client could have done otherwise);
//! * `Tick` for every stepped DF: the E0 bytes right after the step, the moment a client hashes the tick;
//! * `De` answers `WantDe` with the full serialization of the newest final item;
//! * `Close` ends the session with `Final` (the numbers at `t_last`); the fin DFs are never sent to the host.

use crate::world::{Results, Step, TickHash};
use world_host_proto::{De, Diverged, FromHost, Outcome, PlayerStats, Stats, Tick, ToHost, KIND_PVE, MAX_PLAYERS};

/// What the session needs from a match world.
pub trait Sim {
    fn step(&mut self, df: &[u8]) -> Result<Step, String>;
    fn results(&self) -> Results;
    fn tick_hash(&self, t: u32) -> Option<TickHash>;
    /// (item time, full serialization) of the newest item at or before `t`
    fn full_item(&self, t: u32) -> Option<(u32, Vec<u8>)>;
    /// the resync DE body of tick `t` (item t, full = 1, plus the animation-history tail), checked
    fn de(&self, t: u32) -> Result<Vec<u8>, String>;
    /// the statistics object's match end (`S.vt[0x110]`), once, at `Close`; false = not run
    fn match_end(&mut self) -> bool;
    /// the deaths caught since the last call (kills.rs); Err = deaths were lost (the log must not be sent)
    fn kills(&mut self) -> Result<Vec<world_host_proto::KillRecord>, usize>;
    /// what each human used (profiles spawned with, their weapons, the stacks' use); None = not known (no DF stepped)
    fn used(&mut self) -> Option<Vec<world_host_proto::PlayerUse>>;
    /// each slot's alive flag, health and artefact row after the present step (protocol 8, vitals.rs)
    fn vitals(&self) -> Vec<crate::vitals::Sample> {
        Vec::new()
    }
}

impl Sim for crate::world::MatchWorld {
    fn step(&mut self, df: &[u8]) -> Result<Step, String> {
        self.step_df(df)
    }
    fn results(&self) -> Results {
        crate::world::MatchWorld::results(self)
    }
    fn tick_hash(&self, t: u32) -> Option<TickHash> {
        crate::world::MatchWorld::tick_hash(self, t)
    }
    fn full_item(&self, t: u32) -> Option<(u32, Vec<u8>)> {
        crate::world::MatchWorld::full_item(self, t)
    }
    fn de(&self, t: u32) -> Result<Vec<u8>, String> {
        crate::world::MatchWorld::de(self, t).map(|d| d.bytes)
    }
    fn match_end(&mut self) -> bool {
        crate::world::MatchWorld::match_end(self)
    }
    fn kills(&mut self) -> Result<Vec<world_host_proto::KillRecord>, usize> {
        crate::kills::drain()
    }
    fn used(&mut self) -> Option<Vec<world_host_proto::PlayerUse>> {
        crate::world::MatchWorld::used(self)
    }
    fn vitals(&self) -> Vec<crate::vitals::Sample> {
        crate::world::MatchWorld::vitals(self)
    }
}

#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// a tick is final this long behind the newest stepped DF
    pub final_lag_ms: u32,
    pub stats_every_ms: u32,
    /// send `Tick` frames (on for every live match; the relay uses them only with its E0 switch)
    pub tick_frames: bool,
    /// the match's tick (`Create.tick_ms`): after stepping DF t the `Tick` is item t - tick, as a client hashes tick t
    /// only after it handled DF t + tick
    pub tick_ms: u32,
    /// protocol 6 (`Create.version >= 6`): send the `Used` frame before `Final`
    pub used_frame: bool,
    /// protocol 8 (`Create.version >= 8`): the `Kills` assists and the `Final` vitals trailers (health sampled per step)
    pub v8: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig { final_lag_ms: 300, stats_every_ms: 5000, tick_frames: true, tick_ms: 50, used_frame: false, v8: false }
    }
}

/// What happened with one frame.
#[derive(Debug, Default)]
pub struct Reply {
    pub out: Vec<FromHost>,
    /// the session is over (after `Close` or a fault): the child exits
    pub done: bool,
    /// the E0 bytes of the stepped tick (for the per-match tick file)
    pub tick: Option<TickHash>,
}

#[derive(Debug)]
pub struct Session {
    cfg: SessionConfig,
    kind: u8,
    /// world time of the first DF that entered a player (the mission clock's start)
    enter_t: Option<u32>,
    /// first stepped DF whose rule state is 4 / 5, not yet final
    pending_end: Option<u32>,
    outcome_sent: bool,
    next_stats: Option<u32>,
    diverged_sent: bool,
    pub last_t: Option<u32>,
    pub rejected: u64,
    pub steps: u64,
    /// the match's deaths (protocol 3 `Kills`, sent before `Final`)
    pub kills: crate::kills::KillLog,
    /// protocol 8: the players' health over the match (vitals.rs)
    pub vitals: crate::vitals::VitalsLog,
}

fn stats_of(r: &Results) -> Vec<PlayerStats> {
    r.players
        .iter()
        .take(MAX_PLAYERS)
        .map(|p| PlayerStats { slot: p.slot, kills: p.kills, deaths: p.deaths, score: p.score, chests: p.chests, keys_spent: p.keys_spent.min(0xFFFF) as u16, gold_spent: p.gold_spent })
        .collect()
}

impl Session {
    pub fn new(kind: u8, cfg: SessionConfig) -> Session {
        Session {
            cfg,
            kind,
            enter_t: None,
            pending_end: None,
            outcome_sent: false,
            next_stats: None,
            diverged_sent: false,
            last_t: None,
            rejected: 0,
            steps: 0,
            kills: Default::default(),
            vitals: Default::default(),
        }
    }

    fn outcome(&self, t_end: u32, r: &Results) -> Outcome {
        let state = r.state.min(255) as u8;
        Outcome {
            t_end,
            state,
            winner_team: r.winner,
            pve_success: self.kind == KIND_PVE && (state == 4 || state == 5) && r.winner == 0,
            mission_ms: self.enter_t.map_or(0, |e| t_end.saturating_sub(e)),
            players: stats_of(r),
            team_score: r.team_score,
            details: r.details.iter().take(MAX_PLAYERS).copied().collect(),
        }
    }

    /// `Outcome`'s detail rows carry no badges: the counts are unfinished before the match end.
    fn without_badges(mut o: Outcome) -> Outcome {
        for d in &mut o.details {
            d.badges = [[0; 3]; world_host_proto::BADGES];
        }
        o
    }

    /// The newest tick that no input can change any more (None before it exists).
    fn final_tick(&self) -> Option<u32> {
        self.last_t.and_then(|t| t.checked_sub(self.cfg.final_lag_ms))
    }

    pub fn on_frame<S: Sim>(&mut self, sim: &mut S, msg: ToHost) -> Reply {
        let mut rep = Reply::default();
        match msg {
            ToHost::Create(_) => {
                rep.out.push(FromHost::Fault { reason: "a second Create on one match connection".into() });
                rep.done = true;
            }
            ToHost::Df(df) => {
                let st = match sim.step(&df.bytes) {
                    Ok(st) => st,
                    Err(e) => {
                        rep.out.push(FromHost::Fault { reason: format!("DF {}: {e}", df.t).chars().take(256).collect() });
                        rep.done = true;
                        return rep;
                    }
                };
                if st.t != df.t {
                    rep.out.push(FromHost::Fault { reason: format!("Df frame time {} but the DF says {}", df.t, st.t) });
                    rep.done = true;
                    return rep;
                }
                self.steps += 1;
                self.last_t = Some(st.t);
                // the kill log: a late input makes the world simulate again from its time (kills.rs)
                if let Some(l) = st.late_from {
                    self.kills.rollback(l);
                    self.vitals.rollback(l);
                }
                if self.cfg.v8 {
                    self.vitals.add(st.t, &sim.vitals());
                }
                match sim.kills() {
                    Ok(v) => self.kills.add(v),
                    Err(n) => {
                        if !self.kills.broken {
                            crate::hlog!("kill log: {n} death(s) lost (ring full): no Kills frame for this match");
                        }
                        self.kills.broken = true;
                    }
                }
                if self.enter_t.is_none() && st.inserted[0] > 0 {
                    self.enter_t = Some(st.t);
                }
                self.rejected += st.rejected as u64;
                if st.rejected > 0 && !self.diverged_sent {
                    self.diverged_sent = true;
                    rep.out.push(FromHost::Diverged(Diverged { t: st.t, retimed: 0, merged: self.rejected.min(u32::MAX as u64) as u32 }));
                }
                // the end: pending from the first step in state 4 / 5, cancelled if a rollback takes it back, latched
                // once it is final
                if !self.outcome_sent {
                    if st.state == 4 || st.state == 5 {
                        self.pending_end.get_or_insert(st.t);
                    } else {
                        self.pending_end = None;
                    }
                    if let (Some(e), Some(f)) = (self.pending_end, self.final_tick()) {
                        if e <= f {
                            let r = sim.results();
                            rep.out.push(FromHost::Outcome(Self::without_badges(self.outcome(e, &r))));
                            self.outcome_sent = true;
                        }
                    }
                }
                let due = *self.next_stats.get_or_insert(st.t + self.cfg.stats_every_ms);
                if st.t >= due {
                    let r = sim.results();
                    rep.out.push(FromHost::Stats(Stats { t: st.t, players: stats_of(&r), team_score: r.team_score }));
                    self.next_stats = Some(st.t + self.cfg.stats_every_ms);
                }
                // the wire: item t - tick (Tick.t = that item's time, on the tick grid); diagnosis (ticks.tsv): item t
                if self.cfg.tick_frames && st.t >= self.cfg.tick_ms {
                    if let Some(h) = sim.tick_hash(st.t - self.cfg.tick_ms) {
                        let m = h.mask;
                        rep.out.push(FromHost::Tick(Tick { t: h.t, mask: [m as u8, (m >> 8) as u8, (m >> 16) as u8], hashes: h.hash, g: h.g }));
                    }
                }
                rep.tick = sim.tick_hash(st.t);
            }
            // the newest final tick's DE: a refused DE is a Fault for this request
            ToHost::WantDe { request_id } => match self.final_tick() {
                Some(f) => match sim.de(f) {
                    Ok(item) => rep.out.push(FromHost::De(De { request_id, time: f, item })),
                    Err(e) => rep.out.push(FromHost::Fault { reason: format!("WantDe {request_id}: {e}").chars().take(256).collect() }),
                },
                None => rep.out.push(FromHost::Fault { reason: format!("WantDe {request_id}: no final item yet") }),
            },
            ToHost::Close { t_last } => {
                // the counters, then the statistics object's match end once and the detail rows after it (badge
                // counts closed, the four medals, the end inactivity check)
                let r = sim.results();
                // the kill log up to the last DF, before Final (an empty frame = no death; none = no valid log)
                if !self.kills.broken {
                    let mut all = self.kills.upto(t_last);
                    if !self.cfg.v8 {
                        // the assists trailer is protocol 8
                        all.iter_mut().for_each(|k| k.assists = None);
                    }
                    crate::hlog!("kill log: {} death(s) up to {t_last}", all.len());
                    if all.is_empty() {
                        rep.out.push(FromHost::Kills(world_host_proto::Kills { t_upto: t_last, records: vec![] }));
                    }
                    for c in all.chunks(world_host_proto::MAX_KILLS) {
                        rep.out.push(FromHost::Kills(world_host_proto::Kills { t_upto: t_last, records: c.to_vec() }));
                    }
                }
                // protocol 6: what each human used, read at t_last (before the match end, which only touches the
                // statistics object)
                if self.cfg.used_frame {
                    match sim.used() {
                        Some(players) => {
                            for p in &players {
                                crate::hlog!("used: slot {} profiles {:#x} weapons {:?} stacks {:?}", p.slot, p.profiles, p.weapons, p.stacks);
                            }
                            rep.out.push(FromHost::Used(world_host_proto::Used { t: t_last, players: players.into_iter().take(MAX_PLAYERS).collect() }));
                        }
                        None => crate::hlog!("used: no DF was stepped: no Used frame"),
                    }
                }
                let ended = sim.match_end();
                let after = sim.results();
                let mut o = self.outcome(t_last, &r);
                o.details = after.details.iter().take(MAX_PLAYERS).copied().collect();
                if self.cfg.v8 {
                    let v = self.vitals.finish(t_last);
                    for d in o.details.iter_mut() {
                        d.vitals = v.get(usize::from(d.slot)).copied();
                    }
                    for d in o.details.iter().filter_map(|d| d.vitals.map(|v| (d.slot, v))) {
                        crate::hlog!("vitals: slot {} regenerated {} % in his best life, lowest health at an artefact {}", d.0, d.1.regen_best, d.1.artefact_hp_min);
                    }
                }
                if !ended {
                    o = Self::without_badges(o);
                }
                rep.out.push(FromHost::Final(o));
                rep.done = true;
            }
        }
        rep
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::PlayerNumbers;
    use world_host_proto::{Df, KIND_PVP};

    /// A fake world: each DF is `u32 t, u8 state, u8 enter, u8 rejected`.
    #[derive(Default)]
    struct Fake {
        state: u32,
        t: u32,
        kills: u16,
        /// the match end ran (badge 23 appears, the inactive flag is set); `end_fails` makes it fail
        ended: bool,
        end_fails: bool,
        end_calls: u32,
        /// deaths the next `kills()` returns (Err(n) = lost)
        caught: Vec<world_host_proto::KillRecord>,
        lost: usize,
    }
    impl Sim for Fake {
        fn step(&mut self, df: &[u8]) -> Result<Step, String> {
            if df.len() != 7 && df.len() != 11 {
                return Err("bad fake DF".into());
            }
            self.t = u32::from_le_bytes(df[..4].try_into().unwrap());
            self.state = df[4] as u32;
            self.kills += 1;
            let mut st = Step { t: self.t, state: self.state, rejected: df[6] as u32, ..Default::default() };
            st.inserted[0] = df[5] as u32;
            // 4 more bytes: a late input at that time (the world simulates again from there)
            if df.len() == 11 {
                st.late_from = Some(u32::from_le_bytes(df[7..].try_into().unwrap()));
            }
            Ok(st)
        }
        fn kills(&mut self) -> Result<Vec<world_host_proto::KillRecord>, usize> {
            if self.lost > 0 {
                return Err(self.lost);
            }
            Ok(std::mem::take(&mut self.caught))
        }
        fn used(&mut self) -> Option<Vec<world_host_proto::PlayerUse>> {
            (self.t != 0).then(|| vec![world_host_proto::PlayerUse { slot: 0, profiles: 1, weapons: vec![107], stacks: vec![world_host_proto::UsedStack { uid: 100_028, dict: 760, used: 2 }] }])
        }
        fn results(&self) -> Results {
            let mut d = world_host_proto::PlayerDetail { slot: 0, best_streak: 4, ..Default::default() };
            d.events[0] = self.kills;
            d.badges[5] = [1, 0, 0];
            if self.ended {
                d.badges[23] = [1, 0, 0];
                d.inactive = true;
            }
            Results {
                time: self.t,
                state: self.state,
                winner: 0,
                team_score: [self.t / 1000, 3],
                players: vec![PlayerNumbers { slot: 0, kills: self.kills, deaths: 1, score: 7, chests: 2, keys_spent: 1, gold_spent: 0 }],
                details: vec![d],
            }
        }
        fn match_end(&mut self) -> bool {
            self.end_calls += 1;
            self.ended = !self.end_fails;
            self.ended
        }
        fn tick_hash(&self, t: u32) -> Option<TickHash> {
            Some(TickHash { t, mask: 1, hash: vec![0xAB], g: [1, 2, 3] })
        }
        fn full_item(&self, t: u32) -> Option<(u32, Vec<u8>)> {
            Some((t, vec![9; 4]))
        }
        fn de(&self, t: u32) -> Result<Vec<u8>, String> {
            if t % 50 != 0 {
                return Err("off the grid".into());
            }
            let mut b = t.to_le_bytes().to_vec();
            b.push(0);
            Ok(b)
        }
    }

    fn df(t: u32, state: u8, enter: u8, rejected: u8) -> ToHost {
        let mut b = t.to_le_bytes().to_vec();
        b.extend([state, enter, rejected]);
        ToHost::Df(Df { t, bytes: b })
    }

    fn run(s: &mut Session, w: &mut Fake, frames: Vec<ToHost>) -> Vec<FromHost> {
        frames.into_iter().flat_map(|f| s.on_frame(w, f).out).collect()
    }

    #[test]
    fn outcome_latches_once_when_final_with_mission_time() {
        let mut s = Session::new(KIND_PVE, SessionConfig::default());
        let mut w = Fake::default();
        let mut fr = vec![df(850, 3, 0, 0), df(900, 3, 1, 0)];
        // state 5 from 1000 on; final once a DF >= 1300 was stepped
        for t in (950..=1400).step_by(50) {
            fr.push(df(t, if t >= 1000 { 5 } else { 3 }, 0, 0));
        }
        let out = run(&mut s, &mut w, fr);
        let outcomes: Vec<&Outcome> = out.iter().filter_map(|m| if let FromHost::Outcome(o) = m { Some(o) } else { None }).collect();
        assert_eq!(outcomes.len(), 1);
        let o = outcomes[0];
        assert_eq!(o.t_end, 1000);
        assert_eq!(o.state, 5);
        assert!(o.pve_success);
        assert_eq!(o.mission_ms, 100); // enter DF 900
        assert_eq!(o.players[0].chests, 2);
        // the detail rows ride along; no badges before the match end
        assert_eq!(o.details.len(), 1);
        assert_eq!(o.details[0].best_streak, 4);
        assert_eq!(o.details[0].events[0], o.players[0].kills);
        assert_eq!(o.details[0].badge_mask(), 0);
        assert_eq!(w.end_calls, 0);
    }

    #[test]
    fn the_kill_log_follows_rollbacks_and_goes_before_final() {
        let k = |t, victim| world_host_proto::KillRecord { t, victim, killer: 0, weapon_category: 13, ..Default::default() };
        let late = |t: u32, from: u32| {
            let mut b = t.to_le_bytes().to_vec();
            b.extend([3, 0, 0]);
            b.extend(from.to_le_bytes());
            ToHost::Df(Df { t, bytes: b })
        };
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        w.caught = vec![k(1000, 1), k(1100, 2)];
        s.on_frame(&mut w, df(1100, 3, 0, 0));
        // a late input at 1050: the world simulates 1050.. again; victim 2 now dies at 1150, victim 3 at 1200
        w.caught = vec![k(1150, 2), k(1200, 3)];
        s.on_frame(&mut w, late(1200, 1050));
        assert_eq!(s.kills.upto(u32::MAX).iter().map(|r| (r.t, r.victim)).collect::<Vec<_>>(), vec![(1000, 1), (1150, 2), (1200, 3)]);
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1200 });
        let [FromHost::Kills(kf), FromHost::Final(_)] = &rep.out[..] else { panic!("{:?}", rep.out) };
        assert_eq!((kf.t_upto, kf.records.len()), (1200, 3));
        // no death: an empty Kills frame; lost deaths: none at all
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        s.on_frame(&mut w, df(1000, 3, 0, 0));
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1000 });
        assert!(matches!(&rep.out[..], [FromHost::Kills(k), FromHost::Final(_)] if k.records.is_empty()));
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake { lost: 2, ..Default::default() };
        s.on_frame(&mut w, df(1000, 3, 0, 0));
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1000 });
        assert!(matches!(&rep.out[..], [FromHost::Final(_)]));
    }

    #[test]
    fn final_reads_the_details_after_one_match_end() {
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        run(&mut s, &mut w, vec![df(1000, 3, 0, 0), df(1400, 5, 0, 0)]);
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1400 });
        assert_eq!(w.end_calls, 1);
        let [FromHost::Kills(_), FromHost::Final(o)] = &rep.out[..] else { panic!("{:?}", rep.out) };
        assert_eq!(o.details[0].badges[5], [1, 0, 0]);
        assert_eq!(o.details[0].badges[23], [1, 0, 0], "medal picked by the match end");
        assert!(o.details[0].inactive, "the end inactivity check");
        // a failed match end: no badges at all (unfinished counts are not sent)
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake { end_fails: true, ..Default::default() };
        run(&mut s, &mut w, vec![df(1000, 3, 0, 0)]);
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1000 });
        let [FromHost::Kills(_), FromHost::Final(o)] = &rep.out[..] else { panic!() };
        assert_eq!(o.details[0].badge_mask(), 0);
        assert_eq!(o.details[0].best_streak, 4);
    }

    /// Protocol 6: `Used` goes after `Kills` and before `Final`, only when the `Create` asked for it.
    #[test]
    fn used_goes_before_final_only_for_protocol_6() {
        let mut s = Session::new(KIND_PVP, SessionConfig { used_frame: true, ..Default::default() });
        let mut w = Fake::default();
        run(&mut s, &mut w, vec![df(1000, 3, 0, 0), df(1400, 5, 0, 0)]);
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1400 });
        let [FromHost::Kills(_), FromHost::Used(u), FromHost::Final(_)] = &rep.out[..] else { panic!("{:?}", rep.out) };
        assert_eq!(u.t, 1400);
        assert_eq!((u.players[0].weapons.clone(), u.players[0].stacks[0].used), (vec![107], 2));
        // version 5: no Used
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        run(&mut s, &mut w, vec![df(1000, 3, 0, 0)]);
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1000 });
        assert!(matches!(&rep.out[..], [FromHost::Kills(_), FromHost::Final(_)]));
    }

    /// Protocol 8: the kill records keep the host's assists and every `Final` row carries vitals; below 8 neither.
    #[test]
    fn assists_and_vitals_go_out_only_for_protocol_8() {
        let k = |t| world_host_proto::KillRecord { t, victim: 1, killer: 0, assists: Some(0b100), ..Default::default() };
        for v8 in [true, false] {
            let mut s = Session::new(KIND_PVP, SessionConfig { v8, ..Default::default() });
            let mut w = Fake { caught: vec![k(1000)], ..Default::default() };
            run(&mut s, &mut w, vec![df(1000, 3, 0, 0)]);
            let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1000 });
            let [FromHost::Kills(kf), FromHost::Final(o)] = &rep.out[..] else { panic!("{:?}", rep.out) };
            assert_eq!(kf.records[0].assists, if v8 { Some(0b100) } else { None });
            assert!(o.details.iter().all(|d| d.vitals.is_some() == v8));
            if v8 {
                assert_eq!(o.details[0].vitals, Some(world_host_proto::Vitals { regen_best: 0, artefact_hp_min: 0xFF }), "no samples");
            }
        }
    }

    #[test]
    fn a_rolled_back_end_is_not_latched() {
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        let out = run(&mut s, &mut w, vec![df(1000, 5, 0, 0), df(1050, 3, 0, 0), df(1400, 3, 0, 0)]);
        assert!(!out.iter().any(|m| matches!(m, FromHost::Outcome(_))));
    }

    #[test]
    fn stats_every_five_seconds_and_diverged_once() {
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        let fr: Vec<ToHost> = (0..=220).map(|i| df(1000 + i * 50, 3, 0, u8::from(i == 3 || i == 7))).collect();
        let out = run(&mut s, &mut w, fr);
        assert_eq!(out.iter().filter(|m| matches!(m, FromHost::Stats(_))).count(), 2); // at 6000 and 11000
        // the team values come from the world's read-out (the fake: [t / 1000, 3])
        let ts: Vec<[u32; 2]> = out.iter().filter_map(|m| if let FromHost::Stats(x) = m { Some(x.team_score) } else { None }).collect();
        assert_eq!(ts, vec![[6, 3], [11, 3]]);
        // the paid-respawn spends ride in the player rows (the fake: 1 key)
        assert!(out.iter().all(|m| !matches!(m, FromHost::Stats(x) if x.players[0].keys_spent != 1 || x.players[0].gold_spent != 0)));
        let d: Vec<&Diverged> = out.iter().filter_map(|m| if let FromHost::Diverged(d) = m { Some(d) } else { None }).collect();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].t, 1150);
        assert_eq!(s.rejected, 2);
    }

    #[test]
    fn ticks_only_with_tick_frames_but_always_reported() {
        let mut w = Fake::default();
        let mut s = Session::new(KIND_PVP, SessionConfig { tick_frames: false, ..Default::default() });
        let rep = s.on_frame(&mut w, df(1000, 3, 0, 0));
        assert!(rep.tick.as_ref().is_some_and(|h| h.t == 1000), "ticks.tsv keeps item t");
        assert!(!rep.out.iter().any(|m| matches!(m, FromHost::Tick(_))));
        // the default: on the wire, item t - tick after DF t
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let rep = s.on_frame(&mut w, df(1000, 3, 0, 0));
        assert!(matches!(&rep.out[..], [FromHost::Tick(t)] if t.t == 950 && t.mask == [1, 0, 0] && t.hashes == vec![0xAB]));
    }

    #[test]
    fn want_de_needs_a_final_tick_and_close_sends_final() {
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        assert!(matches!(&s.on_frame(&mut w, ToHost::WantDe { request_id: 1 }).out[..], [FromHost::Fault { .. }]));
        run(&mut s, &mut w, vec![df(1000, 3, 0, 0), df(1400, 3, 0, 0)]);
        let rep = s.on_frame(&mut w, ToHost::WantDe { request_id: 2 });
        assert!(matches!(&rep.out[..], [FromHost::De(d)] if d.request_id == 2 && d.time == 1100 && d.item == [0x4C, 0x04, 0, 0, 0]));
        let rep = s.on_frame(&mut w, ToHost::Close { t_last: 1400 });
        assert!(rep.done);
        assert!(matches!(&rep.out[..], [FromHost::Kills(_), FromHost::Final(o)] if o.t_end == 1400 && o.state == 3));
    }

    #[test]
    fn a_bad_df_or_a_time_mismatch_is_a_fault() {
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut w = Fake::default();
        let rep = s.on_frame(&mut w, ToHost::Df(Df { t: 5, bytes: vec![1] }));
        assert!(rep.done && matches!(&rep.out[..], [FromHost::Fault { .. }]));
        let mut s = Session::new(KIND_PVP, SessionConfig::default());
        let mut b = 1000u32.to_le_bytes().to_vec();
        b.extend([3, 0, 0]);
        let rep = s.on_frame(&mut w, ToHost::Df(Df { t: 1050, bytes: b }));
        assert!(rep.done && matches!(&rep.out[..], [FromHost::Fault { reason }] if reason.contains("1050")));
    }
}
