//! The rules of docs/notes/research_mode.md, case by case (the first
//! build's test_research_rule_026g.c).

use super::*;

/// 0.27d's parameters, units A, B, C: a 5 m cylinder, a 4 x 2 x 3 box
/// turned a quarter, a 6 m sphere.
fn params_027d() -> Params {
    let mut p = Params::ZERO;
    p.capture_count = 8;
    p.capture_ms = [20000, 18000, 15000, 14899, 14800, 14699, 14600, 14500];
    p.owned_count = 3;
    p.points_rate = [75 * RATE_MS, 150 * RATE_MS, 350 * RATE_MS];
    p.target = 100 * MICRO;
    p.award = 100;
    p.reach = 0.4;
    p.half_height = 0.9;
    p.zone_count = 3;
    for z in 0..3 {
        let d = &mut p.zones[z];
        d.cp_id = z as u32;
        d.shape_count = 1;
        d.position[0] = 100.0 * z as f32;
        let s = &mut d.shapes[0];
        s.centre[0] = 100.0 * z as f32;
        s.kind = [ShapeKind::Cylinder, ShapeKind::Box, ShapeKind::Sphere][z];
        for i in 0..3 {
            s.axes[i][i] = 1.0;
        }
        if z == 1 {
            // turned 90 degrees about y: local x is world z
            s.axes = [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]];
        }
        s.half = [[5.0, 4.0, 6.0][z], if z == 1 { 2.0 } else { 1.8 }, if z == 1 { 3.0 } else { 5.0 }];
    }
    assert!(prepare(&mut p, 500));
    p
}

/// The rule and the players of a test: every place entered, nobody of a
/// team at first.
struct Game {
    p: Params,
    s: State,
    players: [Option<Body>; PLAYERS],
    time: u32,
}

impl Game {
    fn new(p: Params) -> Game {
        let mut s = State::ZERO;
        reset(&mut s, &p);
        for i in 0..PLAYERS {
            entered(&mut s, Place::new(i as u8).unwrap());
        }
        Game { p, s, players: [None; PLAYERS], time: 0 }
    }

    fn place(&mut self, i: usize, team: Team, zone: i32) {
        let x = if zone < 0 { 1000.0 } else { 100.0 * zone as f32 };
        self.players[i] = Some(Body { team, feet: [x, 0.5, 0.0], half_height: 0.9 });
    }

    fn away(&mut self, i: usize) {
        if let Some(body) = &mut self.players[i] {
            body.feet[0] = 1000.0;
        }
    }

    /// Ticks of 50 ms for `ms`; the awards of all of them.
    fn run(&mut self, ms: u32) -> std::vec::Vec<Award> {
        let mut all = std::vec::Vec::new();
        let mut t = 0;
        while t < ms {
            self.time += 50;
            let mut awards = Awards::new();
            tick(&mut self.s, &self.p, &self.players, 50, self.time, &mut awards);
            all.extend_from_slice(awards.as_slice());
            t += 50;
        }
        all
    }

    fn zone(&self, z: usize) -> &Zone {
        &self.s.zones[z]
    }

    fn own(&mut self, z: usize, team: Team) {
        let zone = &mut self.s.zones[z];
        zone.owner = Some(team);
        zone.capturing = Some(team);
        zone.progress = FULL;
    }
}

extern crate std;

fn place(i: u8) -> Place {
    Place::new(i).unwrap()
}

#[test]
fn inside_a_unit() {
    let p = params_027d();
    // The capsule stands on the feet: 1.8 m tall standing (0.9 x 2), 0.4 m
    // round. The cylinder of unit A: radius 5, y -1.8..1.8.
    assert!(inside(&p, 0, &[5.3, 1.0, 0.0], 0.9));
    assert!(!inside(&p, 0, &[5.5, 0.0, 0.0], 0.9));
    // feet just under its top: in; just over it (a floor above): out
    assert!(inside(&p, 0, &[0.0, 1.75, 0.0], 0.9));
    assert!(!inside(&p, 0, &[0.0, 1.85, 0.0], 0.9));
    // below: a standing head reaches up into it, a crouched one (0.55) not
    assert!(inside(&p, 0, &[0.0, -3.2, 0.0], 0.9));
    assert!(!inside(&p, 0, &[0.0, -3.2, 0.0], 0.55));
    assert!(!inside(&p, 0, &[0.0, -3.7, 0.0], 0.9));
    // the edge of the capsule's bottom: round, not square
    assert!(inside(&p, 0, &[5.1, 1.75, 0.0], 0.9));
    assert!(!inside(&p, 0, &[5.3, 1.75, 0.0], 0.9));
    // the box: 4 m along world z, 3 m along world x; its corners are round
    // to the capsule (0.3 and 0.3 out: 0.42 m away)
    assert!(inside(&p, 1, &[100.0, 0.0, 4.3], 0.9));
    assert!(inside(&p, 1, &[103.3, 0.0, 0.0], 0.9));
    assert!(!inside(&p, 1, &[103.5, 0.0, 0.0], 0.9));
    assert!(!inside(&p, 1, &[103.3, 0.0, 4.3], 0.9));
    // the sphere: radius 6 at y 0
    assert!(inside(&p, 2, &[200.0, 5.9, 0.0], 0.9));
    assert!(!inside(&p, 2, &[200.0, 6.1, 0.0], 0.9));
    assert!(inside(&p, 2, &[206.3, 0.0, 0.0], 0.9));
}

#[test]
fn capture_alone() {
    let mut g = Game::new(params_027d());
    g.place(3, Team::Zero, 0);
    let all = g.run(19950);
    assert!(all.is_empty() && g.zone(0).owner.is_none() && g.zone(0).capturing == Some(Team::Zero));
    let all = g.run(100);
    assert_eq!(g.zone(0).owner, Some(Team::Zero));
    assert_eq!(all.len(), 1);
    assert!(all[0].kind == AwardKind::Capture && all[0].player == place(3) && all[0].amount == 100);
    assert!(g.zone(0).count[0] == 1 && g.zone(0).present[0] == Roster(1 << 3));
}

#[test]
fn capture_by_two() {
    let mut g = Game::new(params_027d());
    g.place(0, Team::One, 0);
    g.place(1, Team::One, 0);
    g.run(17950);
    assert!(g.zone(0).owner.is_none());
    let all = g.run(100);
    assert_eq!(g.zone(0).owner, Some(Team::One));
    assert!(all.len() == 2 && all[0].amount == 50 && all[1].amount == 50);
}

#[test]
fn contested() {
    let mut g = Game::new(params_027d());
    g.place(0, Team::Zero, 0);
    g.run(5000);
    let progress = g.zone(0).progress;
    g.place(1, Team::One, 0);
    g.run(5000);
    assert!(g.zone(0).progress == progress && g.zone(0).capturing == Some(Team::Zero));
}

#[test]
fn fall_back() {
    let mut g = Game::new(params_027d());
    g.place(0, Team::Zero, 0);
    g.run(10000); // half
    assert!(g.zone(0).progress > FULL / 2 - FULL / 100 && g.zone(0).progress <= FULL / 2);
    g.away(0);
    g.run(10000); // half speed: a quarter back
    assert!(g.zone(0).progress > FULL / 4 - FULL / 100 && g.zone(0).progress < FULL / 4 + FULL / 100);
    g.run(30000);
    assert_eq!(g.zone(0).progress, 0);
}

#[test]
fn drain_then_take() {
    let mut g = Game::new(params_027d());
    g.place(0, Team::Zero, 0);
    g.run(20050);
    assert_eq!(g.zone(0).owner, Some(Team::Zero));
    g.away(0);
    g.place(5, Team::One, 0);
    let all = g.run(19950);
    assert!(all.is_empty() && g.zone(0).owner == Some(Team::Zero) && g.zone(0).progress < FULL / 100);
    let all = g.run(100);
    assert!(g.zone(0).owner.is_none() && g.zone(0).capturing == Some(Team::One));
    assert_eq!(all.len(), 1);
    assert!(all[0].kind == AwardKind::Neutralise && all[0].player == place(5) && all[0].amount == 100);
    let all = g.run(20000);
    assert!(g.zone(0).owner == Some(Team::One) && all.len() == 1);
    assert!(all[0].kind == AwardKind::Capture && all[0].amount == 100);
}

#[test]
fn owner_refills() {
    let mut g = Game::new(params_027d());
    g.place(0, Team::Zero, 0);
    g.run(20050);
    g.away(0);
    g.place(1, Team::One, 0);
    g.run(5000);
    g.away(1);
    let drained = g.zone(0).progress;
    g.run(30000); // owned: nothing falls or fills with nobody
    assert!(g.zone(0).owner == Some(Team::Zero) && g.zone(0).progress == drained);
    g.place(0, Team::Zero, 0);
    g.run(5050);
    assert!(g.zone(0).owner == Some(Team::Zero) && g.zone(0).progress == FULL);
}

#[test]
fn water_fill() {
    let mut g = Game::new(params_027d());
    g.place(0, Team::Zero, 0);
    // alone to a third, then a second player comes: 1/3 of the capture is left at 2 players
    g.run(6000);
    let left = FULL - g.zone(0).progress;
    g.place(1, Team::Zero, 0);
    let all = g.run(20000);
    assert!(g.zone(0).owner == Some(Team::Zero) && all.len() == 2);
    // the second's mark is what was left when it came; the first one's the whole capture
    let second = 100.0 * f64::from(left) / f64::from(FULL) / 2.0;
    let first = 100.0 - second;
    assert!(all[0].player == place(0) && all[0].amount == (first + 0.5) as u32);
    assert!(all[1].player == place(1) && all[1].amount == (second + 0.5) as u32);
}

#[test]
fn points_and_end() {
    let mut g = Game::new(params_027d());
    // team 1 owns all three units
    for z in 0..3 {
        g.own(z, Team::One);
    }
    g.run(100000);
    assert!(g.s.points[1] == 100000 * 350 && g.s.score[1] == 35 && g.s.score[0] == 0);
    assert_eq!(leader(&g.s), Some(Team::One));
    g.run(185700); // 285.7 s
    assert_eq!(g.s.end, EndTime::NONE);
    g.run(50); // 285.75 s: 100 points
    assert!(g.s.points[1] == g.p.target && g.s.score[1] == 100 && g.s.end.get() == Some(g.time + 3000));
    assert!(!completed(&g.s, g.time + 2999) && completed(&g.s, g.time + 3000));
    let end = g.s.end;
    g.run(5000);
    assert_eq!(g.s.end, end);
    // nothing moves after the end: a unit stays as it was
    g.place(0, Team::Zero, 0);
    let before = g.zone(0).progress;
    g.run(5000);
    assert!(g.zone(0).progress == before && g.zone(0).owner == Some(Team::One));
}

#[test]
fn one_unit_rate() {
    let mut g = Game::new(params_027d());
    g.own(1, Team::Zero);
    g.run(60000);
    assert!(g.s.points[0] == 60000 * 75 && g.s.score[0] == 5 && leader(&g.s) == Some(Team::Zero));
    g.own(2, Team::One);
    g.run(60000);
    assert!(g.s.score[0] == 9 && g.s.score[1] == 5);
}

/// skhid's rates (the owner, 2026-10-22: 1.25 x 0.27d's): resource_per_s
/// 0.09375, 0.1875, 0.4375, whole numbers per RATE_MS (375, 750, 1750).
/// 100 points in 17:46.8, 8:53.4 and 3:48.6: 50 ms ticks of 4687.5, 9375 and
/// 21875 millionths, the first floored to 4687.
#[test]
fn skhid_rates() {
    let mut p = params_027d();
    p.points_rate = [375, 750, 1750];
    assert!(prepare(&mut p, 500));
    let ms = [1066800, 533350, 228600]; // the tick 100 is reached on
    for owned in 1..=3 {
        let mut g = Game::new(p);
        for z in 0..owned {
            g.own(z, Team::Zero);
        }
        g.run(ms[owned - 1] - 50);
        assert_eq!(g.s.end, EndTime::NONE);
        g.run(50);
        assert!(g.s.points[0] == p.target && g.s.end.get() == Some(g.time + 3000));
    }
}

/// Both at 100 shown, one at the goal: not a draw (the review of #297).
#[test]
fn leader_at_the_goal() {
    let mut g = Game::new(params_027d());
    g.s.points[0] = g.p.target - 3000; // 99.997: shows 100
    g.s.points[1] = g.p.target - 500000; // 99.5: shows 100 too
    for z in 0..3 {
        g.own(z, Team::Zero);
    }
    g.run(50);
    assert!(g.s.points[0] == g.p.target && g.s.score == [100, 100] && g.s.end.get().is_some());
    assert_eq!(leader(&g.s), Some(Team::Zero));
    // before the goal, equal shown scores are a draw
    let mut t = Game::new(params_027d());
    t.s.points = [37400000, 37200000];
    t.s.score = [37, 37];
    assert_eq!(leader(&t.s), None);
}

#[test]
fn kill_invader() {
    let mut g = Game::new(params_027d());
    g.place(2, Team::One, 0);
    g.run(1000);
    let a = kill(&g.s, place(7), place(2), Team::Zero, Team::One).unwrap();
    assert!(a.kind == AwardKind::KillInvader && a.player == place(7) && a.amount == 1);
    assert_eq!(kill(&g.s, place(7), place(3), Team::Zero, Team::One), None); // not inside
    assert_eq!(kill(&g.s, place(8), place(2), Team::One, Team::One), None); // a teammate
    assert_eq!(kill(&g.s, place(2), place(2), Team::One, Team::One), None); // himself
    g.run(20000);
    assert_eq!(g.zone(0).owner, Some(Team::One));
    assert_eq!(kill(&g.s, place(7), place(2), Team::Zero, Team::One), None); // inside his own unit
}

#[test]
fn left_and_dead() {
    let mut g = Game::new(params_027d());
    g.place(4, Team::Zero, 0);
    g.run(1000);
    assert_eq!(g.zone(0).count[0], 1);
    left(&mut g.s, place(4));
    assert!(g.zone(0).count[0] == 0 && !g.s.entered.has(4));
    g.run(1000);
    assert_eq!(g.zone(0).count[0], 0); // a place that left is not read
    entered(&mut g.s, place(4));
    g.players[4] = None; // dead
    g.run(1000);
    assert_eq!(g.zone(0).count[0], 0);
}

#[test]
fn wire() {
    let mut g = Game::new(params_027d());
    g.place(1, Team::Zero, 0);
    g.place(2, Team::One, 1);
    g.run(7000);
    let mut bytes = [0u8; 1024];
    assert_eq!(serialized_size(&g.s), 21 + 3 * 96);
    assert_eq!(write(&g.s, &mut bytes), 309);
    assert_eq!(bytes[20], 3);
    let mut back = State::ZERO;
    assert_eq!(read(&mut back, &g.p, &bytes[..309]), Some(309));
    assert_eq!(back, g.s);
    // a tick from the loaded state gives the same as from the saved one
    let (mut a1, mut a2) = (Awards::new(), Awards::new());
    tick(&mut g.s, &g.p, &g.players, 50, g.time + 50, &mut a1);
    tick(&mut back, &g.p, &g.players, 50, g.time + 50, &mut a2);
    assert_eq!(back, g.s);
    let mut bad = bytes;
    bad[21 + 4] = 3; // a team 3
    assert_eq!(read(&mut back, &g.p, &bad[..309]), None);
    let mut bad = bytes;
    bad[21 + 6] = 2; // a count unlike its mask
    assert_eq!(read(&mut back, &g.p, &bad[..309]), None);
    assert_eq!(read(&mut back, &g.p, &bytes[..308]), None);
}

#[test]
fn prepared() {
    let mut p = params_027d();
    assert!(p.capture_rate[0] == FULL / 20000 && p.restore_rate == FULL / 20000 / 2);
    p.zone_count = 4;
    assert!(!prepare(&mut p, 500));
    let mut p = params_027d();
    p.capture_ms[2] = 0;
    assert!(!prepare(&mut p, 500));
}

#[test]
fn capture_time_remaining() {
    let mut g = Game::new(params_027d());
    assert_eq!(ms_left(&g.s, &g.p, 0), 0);
    g.place(3, Team::Zero, 0);
    g.run(50);
    assert_eq!(ms_left(&g.s, &g.p, 0), 19950);
    g.place(4, Team::Zero, 0);
    g.run(50);
    assert_eq!(ms_left(&g.s, &g.p, 0), (FULL - g.zone(0).progress).div_ceil(g.p.capture_rate[1]));
    g.place(5, Team::One, 0);
    g.run(50);
    assert_eq!(ms_left(&g.s, &g.p, 0), 0); // contested
    g.away(5);
    g.away(4);
    g.run(20000);
    assert_eq!(g.zone(0).owner, Some(Team::Zero));
    assert_eq!(ms_left(&g.s, &g.p, 0), 0);
    g.away(3);
    g.place(5, Team::One, 0);
    g.run(50);
    assert_eq!(ms_left(&g.s, &g.p, 0), 19950); // drain ownership first
    g.run(20000);
    assert_eq!(g.zone(0).capturing, Some(Team::One));
    assert_eq!(g.zone(0).owner, None);
    assert_eq!(ms_left(&g.s, &g.p, 0), (FULL - g.zone(0).progress).div_ceil(g.p.capture_rate[0]));

    let mut g = Game::new(params_027d());
    g.s.zones[1].count[1] = 1;
    assert_eq!(ms_left(&g.s, &g.p, 1), 20000);
    assert_eq!(ms_left(&g.s, &g.p, 3), 0);
    g.s.end = EndTime::from_raw(1000);
    assert_eq!(ms_left(&g.s, &g.p, 1), 0);
}

/// The wire's team bytes are `Option<Team>`'s: 2 is nobody.
#[test]
fn team_bytes() {
    assert_eq!(team_byte(None), NOBODY);
    assert_eq!(team_byte(Some(Team::One)), 1);
    assert_eq!(parse_team_byte(2), Ok(None));
    assert_eq!(parse_team_byte(3), Err(()));
}
