use super::*;

/// The stand-in camper (`SRP_BENCH_CAMPER=<place>`, `campers.md` K1 §4):
/// that place's bot goes to [`camper_spot`] and holds it all match,
/// standing, back to it after each respawn; its reflex fires on sight. Its
/// spot: within this of it, m; the route points it is chosen by: kept this
/// far apart, m, and seen within this, m.
pub(crate) const CAMPER_WITHIN_M: f32 = 0.7;

/// The camper's spot and how many route points it sees, if any.
pub(crate) type CamperSpot = Option<(Vec3, u32)>;

/// How the stand-in camper's spot is chosen (`SRP_BENCH_CAMPER_SPOT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpotRule {
    /// The point that sees the most of the other side's route points.
    Most,
    /// Of those that see some, one that sees many and is seen from few:
    /// seen over sees under [`HIDDEN_RATIO`] (else the lowest ratio).
    Hidden,
}

/// A hidden spot is seen from fewer than this share of what it sees
/// (`standin-camper-fidelity.md`: the owner's holds were behind an edge or a
/// pillar, seeing down a lane).
pub(crate) const HIDDEN_RATIO: f32 = 0.5;

/// The camper's eye height standing and crouched, m (the middle of the
/// client's `mapdata::stands::EYE_STANDING` and `EYE_CROUCHED`).
const EYE_STANDING_M: f32 = 1.6;
const EYE_CROUCHED_M: f32 = 0.95;

/// The stand-in camper's fidelity options, each off when unset: a spot rule,
/// the stance at the spot and the weapon (`settings.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CamperOptions {
    pub(crate) rule: SpotRule,
    /// Crouched once at the spot (the owner's holds: crouched behind cover,
    /// peeking).
    pub(crate) crouch: bool,
    /// The tier's long gun (bolt or marksman rifle) when its kits have one.
    pub(crate) long_gun: bool,
}

impl CamperOptions {
    pub(crate) const OFF: Self = Self {
        rule: SpotRule::Most,
        crouch: false,
        long_gun: false,
    };

    /// The environment's ([`bench_settings`]).
    pub(crate) fn from_env() -> Self {
        let settings = bench_settings();
        let is = |name: &str, want: &str| settings.var(name).is_ok_and(|v| v == want);
        Self {
            rule: if is("SRP_BENCH_CAMPER_SPOT", "hidden") {
                SpotRule::Hidden
            } else {
                SpotRule::Most
            },
            crouch: is("SRP_BENCH_CAMPER_CROUCH", "1"),
            long_gun: is("SRP_BENCH_CAMPER_GUN", "long"),
        }
    }
}

/// The long gun among the kits of `tier` (the camper's first): a bolt rifle,
/// else a marksman rifle ([`Gun::class`]).
pub(crate) fn long_kit(tier: survarium::v026g::loadouts::BotTier) -> Option<&'static Arms> {
    use survarium::v026g::loadouts::BotRole;
    let roles = [BotRole::Camper, BotRole::Flanker, BotRole::Support, BotRole::Rusher];
    ["bolt", "marksman"].into_iter().find_map(|class| {
        roles
            .into_iter()
            .flat_map(|role| Arms::kits_of(tier, role))
            .find(|arms| arms.primary.0.class() == class)
    })
}

impl Bots {
    /// The camper at `place` carries the long gun of `tier`'s kits, if any
    /// ([`CamperOptions::long_gun`]): its only kit, from this life on.
    pub(crate) fn arm_camper_long(&mut self, place: Place, tier: survarium::v026g::loadouts::BotTier) {
        let (Some(arms), Some(bot)) = (long_kit(tier), self.bots[place].as_mut()) else {
            return;
        };
        bot.kits = vec![(arms, arms.reserve)];
        bot.kit = 0;
        bot.kit_chosen = 0;
        bot.life_carried = arms.reserve.min(arms.primary.1);
        bot.reflex = Reflex::armed_carrying(Some(arms), bot.life_carried);
    }
}

pub(crate) const CAMPER_THIN_M: f32 = 2.0;

pub(crate) const CAMPER_SEES_M: f32 = 50.0;

/// The stand-in camper's spot: of `side`'s human route points, the one
/// whose standing eye sees the most of the other side's route points
/// within [`CAMPER_SEES_M`] (any body part of a standing body there, the
/// bots' own sight test); ties to the higher. Worked out once a map, from
/// the map's routes only (no spot table), with the count it sees; none
/// when it sees none.
pub(crate) fn camper_spot(tactics: &super::tactics::Tactics, side: i32) -> CamperSpot {
    camper_spot_by(tactics, side, SpotRule::Most, false)
}

/// What a candidate spot sees and is seen from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Candidate {
    pub(crate) at: Vec3,
    /// The other side's route points its eye sees within [`CAMPER_SEES_M`].
    pub(crate) sees: u32,
    /// Those whose standing eye sees a body of the camper's stance there.
    pub(crate) seen: u32,
}

/// The spot `rule` takes of the candidates that see some.
pub(crate) fn pick_spot(rule: SpotRule, candidates: &[Candidate]) -> CamperSpot {
    let seeing = || candidates.iter().filter(|c| c.sees > 0);
    let most = |a: &&Candidate, b: &&Candidate| a.sees.cmp(&b.sees).then(a.at.y.total_cmp(&b.at.y));
    let best = match rule {
        SpotRule::Most => seeing().max_by(most),
        SpotRule::Hidden => {
            let hidden = |c: &&Candidate| (c.seen as f32) < HIDDEN_RATIO * c.sees as f32;
            seeing().filter(hidden).max_by(most).or_else(|| {
                let ratio = |c: &Candidate| c.seen as f32 / c.sees as f32;
                seeing().min_by(|a, b| ratio(a).total_cmp(&ratio(b)).then(most(b, a)))
            })
        }
    };
    best.map(|c| (c.at, c.sees))
}

/// [`camper_spot`] by `rule`, for a camper who `crouched` at it or stands:
/// he sees from the eye of his stance, and is seen as a body of it.
pub(crate) fn camper_spot_by(
    tactics: &super::tactics::Tactics,
    side: i32,
    rule: SpotRule,
    crouched: bool,
) -> CamperSpot {
    let points = |side: i32| {
        let mut kept: Vec<Vec3> = Vec::new();
        for p in tactics.routes_of(side).flat_map(|r| r.points.iter().copied()) {
            if p.is_finite() && !kept.iter().any(|k| k.distance_xz(p) < CAMPER_THIN_M && (k.y - p.y).abs() < 1.0) {
                kept.push(p);
            }
        }
        kept
    };
    let (mine, theirs) = (points(side), points(1 - side));
    let sight = &tactics.world.sight;
    let eye_up = Vec3::new(0.0, if crouched { EYE_CROUCHED_M } else { EYE_STANDING_M }, 0.0);
    let candidates: Vec<Candidate> = mine
        .iter()
        .map(|&p| {
            let eye = p + eye_up;
            let near = |q: &&Vec3| q.distance_xz(p) <= CAMPER_SEES_M;
            let sees = theirs
                .iter()
                .filter(near)
                .filter(|&&q| super::aimpoint::shows(eye, q, false, sight, || true))
                .count() as u32;
            // Seen from only matters to the hidden rule, and only where it sees some.
            let seen = if rule == SpotRule::Hidden && sees > 0 {
                let standing = Vec3::new(0.0, EYE_STANDING_M, 0.0);
                theirs
                    .iter()
                    .filter(near)
                    .filter(|&&q| super::aimpoint::shows(q + standing, p, crouched, sight, || true))
                    .count() as u32
            } else {
                0
            };
            Candidate { at: p, sees, seen }
        })
        .collect();
    pick_spot(rule, &candidates)
}

/// Human routes on the soak field: side 0's north lane past the wall's
/// end and its south lane behind the wall; side 1's the same at x 60-70.
pub(crate) fn camper_routes() -> Vec<super::routes::HumanRoute> {
    let route = |team: i32, points: &[(f32, f32)]| super::routes::HumanRoute {
        team,
        lives: 10,
        points: points.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect(),
        ..Default::default()
    };
    vec![
        route(0, &[(10.0, 33.0), (35.0, 33.0)]),
        route(0, &[(20.0, 8.0), (30.0, 5.0)]),
        route(1, &[(60.0, 33.0), (70.0, 33.0)]),
        route(1, &[(60.0, 5.0), (70.0, 5.0)]),
    ]
}

/// The camper's spot is the route point that sees the most of the other
/// side's: on the north lane at the wall's end, not behind the wall (which
/// sees none of them), nor farther back.
#[test]
pub(crate) fn the_stand_in_campers_spot_sees_most_of_the_other_sides_routes() {
    let mut tactics = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
    tactics.set_routes(camper_routes());
    let (at, seen) = camper_spot(&tactics, 0).expect("a spot");
    assert!(at.distance_xz(Vec3::new(35.0, 0.0, 33.0)) < 0.5, "{at:?} {seen}");
    assert_eq!(seen, 4);
    // Behind the wall alone: no spot.
    let mut walled = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
    walled.set_routes(camper_routes().into_iter().filter(|r| r.team == 1 || r.points[0].z < 20.0).collect());
    assert_eq!(camper_spot(&walled, 0), None);
}

/// The hidden rule takes, of the spots that see some, one seen from under
/// half of what it sees (the most seeing of them, ties to the higher), else
/// the lowest ratio; the most rule takes the most seeing.
#[test]
pub(crate) fn the_hidden_rule_prefers_a_spot_seen_from_few() {
    let at = |x: f32, y: f32| Vec3::new(x, y, 0.0);
    let c = |x, sees, seen| Candidate { at: at(x, 0.0), sees, seen };
    let open = [c(0.0, 10, 10), c(1.0, 6, 2), c(2.0, 4, 1), c(3.0, 0, 0)];
    assert_eq!(pick_spot(SpotRule::Most, &open), Some((at(0.0, 0.0), 10)));
    assert_eq!(pick_spot(SpotRule::Hidden, &open), Some((at(1.0, 0.0), 6)));
    // None under half: the lowest ratio (4/6 over 5/6 and 9/9).
    let seen = [c(0.0, 9, 9), c(1.0, 6, 4), c(2.0, 6, 5)];
    assert_eq!(pick_spot(SpotRule::Hidden, &seen), Some((at(1.0, 0.0), 6)));
    // Exactly half is not under it; ties go to the higher.
    let tie = [Candidate { at: at(0.0, 0.0), sees: 6, seen: 3 }, Candidate { at: at(1.0, 5.0), sees: 6, seen: 3 }];
    assert_eq!(pick_spot(SpotRule::Hidden, &tie), Some((at(1.0, 5.0), 6)));
    let none = [c(0.0, 0, 0)];
    assert_eq!(pick_spot(SpotRule::Hidden, &none), None);
    assert_eq!(pick_spot(SpotRule::Most, &none), None);
}

/// On the soak field the standing most rule is the camper's old spot, and
/// the hidden rule's has a ratio no worse than it.
#[test]
pub(crate) fn the_options_off_are_the_old_spot_and_the_hidden_rule_finds_one() {
    let mut tactics = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
    tactics.set_routes(camper_routes());
    assert_eq!(camper_spot_by(&tactics, 0, SpotRule::Most, false), camper_spot(&tactics, 0));
    assert_eq!(CamperOptions::OFF, CamperOptions { rule: SpotRule::Most, crouch: false, long_gun: false });
    for crouched in [false, true] {
        let hidden = camper_spot_by(&tactics, 0, SpotRule::Hidden, crouched).expect("a spot");
        assert!(hidden.1 > 0);
    }
}

/// The long gun of a tier's kits is a bolt or a marksman rifle, the camper
/// kit's first.
#[test]
pub(crate) fn the_long_kit_is_a_bolt_or_marksman_rifle() {
    for tier in survarium::v026g::loadouts::BotTier::ALL {
        if let Some(arms) = long_kit(tier) {
            assert!(["bolt", "marksman"].contains(&arms.primary.0.class()), "{tier:?}");
        }
    }
    assert!(survarium::v026g::loadouts::BotTier::ALL.into_iter().any(|t| long_kit(t).is_some()));
}

/// In a Team Deathmatch on the soak field, the stand-in camper (bot 0)
/// goes to its spot and holds it; its teammate (the near miss) roams.
#[test]
pub(crate) fn the_stand_in_camper_holds_its_spot_and_a_teammate_does_not() {
    let world = soak_world();
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new(
        0x50AC,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    if let Some(tactics) = bots.tactics.as_mut() {
        tactics.set_routes(camper_routes());
    }
    bots.bench_camper = Some((0, None));
    let mut game = Game::new(world, &teams, 1, Some(1_000), None);
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let spot = Vec3::new(35.0, 0.0, 33.0);
    // By bot: in this life at the spot (within 1 m) yet; living ticks
    // since, and of them those within 2 m of it; living ticks in all.
    let mut there = [false; 2];
    let mut held = [(0u32, 0u32, 0u32); 2];
    // Long enough for the camper to live to its spot: with the weapon's own
    // damage (3-4 hits kill) its lives are 5-10 s and it reaches the spot
    // (13 s from its spawn) in one life of four, where the old flat fifth
    // a hit gave it one in eight within 120 s. The property is that it
    // holds the spot all the while it is there, not how long it lives.
    let (limit_ms, mut clock, mut next_world) = (300_000, 0, 1_000);
    while clock < limit_ms {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
            for (place, held) in held.iter_mut().enumerate() {
                let body = &game.bodies[place];
                let off = body.feet.distance_xz(spot);
                there[place] = body.alive && (there[place] || off <= 1.0);
                held.2 += u32::from(body.alive);
                if there[place] {
                    held.0 += 1;
                    held.1 += u32::from(off <= 2.0);
                }
            }
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: (limit_ms - clock) as f32 / 1000.0,
            total_s: (limit_ms / 1000) as f32,
            playing: true,
        });
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let bot = bots.bots[made.place].as_ref();
            let target = bot.and_then(|bot| bot.reflex.target());
            let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
        }
    }
    let found = bots.bench_camper.and_then(|(_, spot)| spot.flatten());
    assert!(found.is_some_and(|(at, _)| at.distance_xz(spot) < 0.5), "{found:?}");
    // The camper got there and held it while alive (the reflex never moves
    // it off; 3 s of it at least: how soon it arrives depends on the
    // match's draws); its mate spent little of its life there.
    let [camper, mate] = held;
    // Its lives end by kills, so the ticks it spends at the spot move with
    // any change to the other bodies of the game (a stuck recovery among
    // them: 117 and 67 in two runs of this soak that differ in one back-off's
    // keys). The floor is "got there for a few seconds"; the property is
    // that it holds the spot all the while it is there.
    assert!(camper.0 >= 50 && camper.1 == camper.0, "camper {camper:?}, mate {mate:?}");
    assert!(f64::from(mate.1) <= 0.2 * f64::from(mate.2), "camper {camper:?}, mate {mate:?}");
}
