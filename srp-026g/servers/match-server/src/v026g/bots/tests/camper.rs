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

/// Which team the stand-in camper stands on (`SRP_BENCH_CAMPER_SIDE`). The place `SRP_BENCH_CAMPER` names is on team 0 for a place under
/// the team size, so the camper was on team 0 in every match, and in a by-side head to head the one team that meets him is always the
/// same one: a by-side split of a camper cell is lopsided by construction (the pooled A+B cancels it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CamperSide {
    /// The team of the place named.
    #[default]
    Given,
    /// The same role's place on the other team ([`mirror_place`]).
    Mirror,
    /// The given place on an even seed, its mirror on an odd one: the cells of a run, and of both its arms, split between the teams.
    BySeed,
}

impl CamperSide {
    /// Unset: [`Self::Given`]; `mirror`, `seed`; anything else is refused.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("given") => Ok(Self::Given),
            Some("mirror") => Ok(Self::Mirror),
            Some("seed") => Ok(Self::BySeed),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_SIDE={other:?}: given, mirror or seed")),
        }
    }

    /// The place the camper takes in the match of `seed` with `per_team` bots a side, from the place named.
    pub(crate) fn place(self, named: Place, per_team: usize, seed: u32) -> Place {
        match self {
            Self::Given => named,
            Self::Mirror => mirror_place(named, per_team),
            Self::BySeed if seed % 2 == 1 => mirror_place(named, per_team),
            Self::BySeed => named,
        }
    }
}

/// The place of the same role on the other team: the places of team 0 are `0..per_team`, those of team 1 the next `per_team`, and a team's
/// k-th place has the k-th role (`loadouts::bot_roles`).
pub(crate) fn mirror_place(place: Place, per_team: usize) -> Place {
    if place < per_team { place + per_team } else { place - per_team }
}

/// The stand-in camper's fidelity options, each off when unset: a spot rule,
/// the stance at the spot and the weapon (`settings.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CamperOptions {
    pub(crate) rule: SpotRule,
    /// Crouched once at the spot (the owner's holds: crouched behind cover,
    /// peeking).
    pub(crate) crouch: bool,
    /// The tier's long gun (bolt or marksman rifle) when its kits have one.
    pub(crate) long_gun: bool,
    /// Works an area, peeks, relocates and leaves, as a human camper does ([`MoveOptions`]); off: stands on its spot all match.
    pub(crate) moving: Option<MoveOptions>,
    /// Where the spot is chosen ([`CamperAt`]).
    pub(crate) at: CamperAt,
    /// Where he looks at his spot, and whether scoped ([`Watch`]).
    pub(crate) watch: Watch,
    /// Which team he stands on ([`CamperSide`]).
    pub(crate) side: CamperSide,
    /// Where each of his lives starts ([`CamperStart`]).
    pub(crate) start: CamperStart,
    /// Where the standing camper goes after a death ([`ReturnRule`]).
    pub(crate) returns: ReturnRule,
}

/// Where the standing camper (no [`MoveOptions`]) goes after a death (`SRP_BENCH_CAMPER_RETURN`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReturnRule {
    /// Back to the same point at once, every life: the old stand-in.
    #[default]
    Always,
    /// As the live campers do after a death (`camper_move::LiveReturn`): back to the very point he held now and then, to a
    /// point near it more often, else to another spot.
    Live,
}

impl ReturnRule {
    /// Unset or `always`: [`Self::Always`]; `live`; anything else is refused.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("always") => Ok(Self::Always),
            Some("live") => Ok(Self::Live),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_RETURN={other:?}: always or live")),
        }
    }
}

impl CamperOptions {
    pub(crate) const OFF: Self = Self {
        rule: SpotRule::Most,
        crouch: false,
        long_gun: false,
        moving: None,
        at: CamperAt::Free,
        watch: Watch::Around,
        side: CamperSide::Given,
        start: CamperStart::Spawn,
        returns: ReturnRule::Always,
    };

    /// The environment's ([`bench_settings`]); a value it does not know refuses the run.
    pub(crate) fn from_env() -> Self {
        let settings = bench_settings();
        Self::parse(&|name| settings.var(name).ok()).unwrap_or_else(|e| panic!("{e}"))
    }

    /// The options `get` (a setting by name) gives; a value that is not one of the choices is an error, not the default.
    pub(crate) fn parse(get: &dyn Fn(&str) -> Option<String>) -> Result<Self, String> {
        let rule = match get("SRP_BENCH_CAMPER_SPOT").as_deref() {
            None | Some("most") => SpotRule::Most,
            Some("hidden") => SpotRule::Hidden,
            Some(other) => return Err(format!("SRP_BENCH_CAMPER_SPOT={other:?}: most or hidden")),
        };
        let crouch = match get("SRP_BENCH_CAMPER_CROUCH").as_deref() {
            None | Some("0") => false,
            Some("1") => true,
            Some(other) => return Err(format!("SRP_BENCH_CAMPER_CROUCH={other:?}: 1 or unset")),
        };
        let long_gun = match get("SRP_BENCH_CAMPER_GUN").as_deref() {
            None => false,
            Some("long") => true,
            Some(other) => return Err(format!("SRP_BENCH_CAMPER_GUN={other:?}: long or unset")),
        };
        Ok(Self {
            rule,
            crouch,
            long_gun,
            moving: MoveOptions::parse(
                get("SRP_BENCH_CAMPER_MOVE").as_deref(),
                get("SRP_BENCH_CAMPER_DWELL_S").as_deref(),
                get("SRP_BENCH_CAMPER_LEAVE").as_deref(),
            )?,
            at: CamperAt::parse(get("SRP_BENCH_CAMPER_AT").as_deref())?,
            watch: Watch::parse(get("SRP_BENCH_CAMPER_WATCH").as_deref())?,
            side: CamperSide::parse(get("SRP_BENCH_CAMPER_SIDE").as_deref())?,
            start: CamperStart::parse(get("SRP_BENCH_CAMPER_START").as_deref(), get("SRP_BENCH_CAMPER_ARRIVE").as_deref())?,
            returns: ReturnRule::parse(get("SRP_BENCH_CAMPER_RETURN").as_deref())?,
        })
    }
}

/// The long gun among the kits of `tier` (the camper's first): a bolt rifle,
/// else a marksman rifle ([`Gun::class`]).
pub(crate) fn long_kit(tier: survarium::v026g::loadouts::BotTier) -> Option<&'static Arms> {
    use survarium::v026g::loadouts::{BotEquipment, BotRole, RusherKits};
    let equipment = BotEquipment::of(tier, RusherKits::default());
    let roles = [BotRole::Camper, BotRole::Flanker, BotRole::Support, BotRole::Rusher];
    ["bolt", "marksman"].into_iter().find_map(|class| {
        roles
            .into_iter()
            .flat_map(|role| Arms::kits_of(equipment, role))
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
        // His weapon as the match knows it: live gives a human the weapon of his kit (`game/driven.rs` `weapon_of`), so
        // the long gun, not his place's role's (a flanker's SMG at place 0), or no one is ever outranged by him.
        let by_data = self.match_wide_tuning().aim.combat.fire.weapon_class_by_data;
        if let Some(known) = self.features.weapons.get_mut(place) {
            *known = Some(super::reflex::WeaponKind::of_class(arms.primary.0.class_for(by_data)));
        }
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
    pick_spot(rule, &camper_candidates(tactics, side, rule, crouched))
}

/// Every route point of `side` with what it sees and, for the hidden rule, what sees it ([`camper_spot_by`] picks one).
pub(crate) fn camper_candidates(
    tactics: &super::tactics::Tactics,
    side: i32,
    rule: SpotRule,
    crouched: bool,
) -> Vec<Candidate> {
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
    // The eye of the stance is the game's (`view::eye_height`).
    let eye_up = super::view::still_eye(Vec3::ZERO, crouched);
    mine.iter()
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
                let standing = super::view::still_eye(Vec3::ZERO, false);
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
        .collect()
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

/// The options are parsed once: a value that is none of the choices is refused, and nothing set is the options off.
#[test]
pub(crate) fn the_camper_options_refuse_an_unknown_value() {
    let with = |pairs: &'static [(&'static str, &'static str)]| {
        CamperOptions::parse(&move |name| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v).to_string()))
    };
    assert_eq!(with(&[]), Ok(CamperOptions::OFF));
    let all = with(&[
        ("SRP_BENCH_CAMPER_SPOT", "hidden"),
        ("SRP_BENCH_CAMPER_CROUCH", "1"),
        ("SRP_BENCH_CAMPER_GUN", "long"),
        ("SRP_BENCH_CAMPER_MOVE", "1"),
        ("SRP_BENCH_CAMPER_AT", "container"),
        ("SRP_BENCH_CAMPER_WATCH", "lane"),
        ("SRP_BENCH_CAMPER_SIDE", "seed"),
        ("SRP_BENCH_CAMPER_START", "spot"),
        ("SRP_BENCH_CAMPER_ARRIVE", "live"),
        ("SRP_BENCH_CAMPER_RETURN", "live"),
    ])
    .expect("known values");
    assert_eq!((all.rule, all.crouch, all.long_gun, all.at), (SpotRule::Hidden, true, true, CamperAt::Container));
    assert!(all.moving.is_some());
    assert_eq!(all.watch, Watch::Lane);
    assert_eq!(all.side, CamperSide::BySeed);
    assert_eq!(all.start, CamperStart::Spot(Arrival::LiveWalk));
    assert_eq!(all.returns, ReturnRule::Live);
    let leaving = with(&[("SRP_BENCH_CAMPER_MOVE", "live"), ("SRP_BENCH_CAMPER_LEAVE", "live")]).expect("known values");
    assert_eq!(leaving.moving, Some(MoveOptions::Live(LiveLeave::PerKill)));
    for bad in [
        ("SRP_BENCH_CAMPER_SPOT", "hiden"),
        ("SRP_BENCH_CAMPER_CROUCH", "yes"),
        ("SRP_BENCH_CAMPER_GUN", "short"),
        ("SRP_BENCH_CAMPER_AT", "batery"),
        ("SRP_BENCH_CAMPER_WATCH", "lanes"),
        ("SRP_BENCH_CAMPER_SIDE", "team1"),
        ("SRP_BENCH_CAMPER_START", "home"),
        ("SRP_BENCH_CAMPER_ARRIVE", "walk"),
        ("SRP_BENCH_CAMPER_ARRIVE", "live"),
        ("SRP_BENCH_CAMPER_LEAVE", "live"),
        ("SRP_BENCH_CAMPER_RETURN", "sometimes"),
    ] {
        let pairs: &'static [(&'static str, &'static str)] = Box::leak(Box::new([bad]));
        assert!(with(pairs).is_err(), "{bad:?}");
    }
}

/// On the soak field the standing most rule is the camper's old spot, and
/// the hidden rule's has a ratio no worse than it.
#[test]
pub(crate) fn the_options_off_are_the_old_spot_and_the_hidden_rule_finds_one() {
    let mut tactics = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
    tactics.set_routes(camper_routes());
    assert_eq!(camper_spot_by(&tactics, 0, SpotRule::Most, false), camper_spot(&tactics, 0));
    assert_eq!(CamperOptions::OFF, CamperOptions { rule: SpotRule::Most, crouch: false, long_gun: false, moving: None, at: CamperAt::Free, watch: Watch::Around, side: CamperSide::Given, start: CamperStart::Spawn, returns: ReturnRule::Always });
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

/// The stand-in armed with a bolt is known to the match as a sniper, as live knows a human by his kit: the place's role
/// (place 0, a flanker: an SMG) would leave every enemy unaware that he outranges them (h2h-camper-20261030T1200: no
/// anti-sniper smoke, outranged check never on him).
#[test]
pub(crate) fn the_long_gunned_stand_in_is_known_as_a_sniper() {
    use crate::v026g::bots::reflex::WeaponKind;
    use survarium::v026g::loadouts::BotTier;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x5A1F,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, true],
        &[None, None],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.set_weapons(vec![Some(WeaponKind::Smg), Some(WeaponKind::Rifle)]);
    let tier = BotTier::ALL.into_iter().find(|&t| long_kit(t).is_some_and(|a| a.primary.0.class() == "bolt")).expect("a tier with a bolt");
    bots.arm_camper_long(0, tier);
    assert_eq!(bots.features.weapons, vec![Some(WeaponKind::Sniper), Some(WeaponKind::Rifle)]);
}

/// 300 s of a Team Deathmatch on the soak field with the game's seed `seed` and the bots' `tuning`: for the
/// stand-in camper (bot 0) and its teammate (the near miss) the living ticks they were at the camper's spot in
/// the life they reached it in, of them those within 2 m of it, and their living ticks in all.
fn stand_in_camper_run(seed: u64, tuning: Tuning) -> [(u32, u32, u32); 2] {
    stand_in_camper_with(seed, tuning, CamperOptions::OFF).0
}

/// [`stand_in_camper_run`] for a camper with `options`, and the ticks its intent held the scope key (`key::AIM`) while it was within 1 m of its spot (the reflex scopes for a target itself, which is not the intent).
fn stand_in_camper_with(seed: u64, tuning: Tuning, options: CamperOptions) -> ([(u32, u32, u32); 2], u32) {
    let world = soak_world();
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new(
        0x50AC,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    if let Some(tactics) = bots.tactics.as_mut() {
        tactics.set_routes(camper_routes());
    }
    bots.bench_camper = Some(BenchCamper::new(0, options, seed as u32, Arc::new(human_tuning())));
    let mut game = Game::new(world, &teams, seed, Some(1_000), None);
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let spot = Vec3::new(35.0, 0.0, 33.0);
    // By bot: in this life at the spot (within 1 m) yet; living ticks
    // since, and of them those within 2 m of it; living ticks in all.
    let mut there = [false; 2];
    let mut held = [(0u32, 0u32, 0u32); 2];
    let mut scoped = 0u32;
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
            if made.place == 0 && game.bodies[0].alive && game.bodies[0].feet.distance_xz(spot) <= 1.0 && bot.is_some_and(|bot| bot.intent.hold & key::AIM != 0) {
                scoped += 1;
            }
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
        }
    }
    let found = bots.bench_camper.as_ref().and_then(BenchCamper::first_spot);
    assert!(found.is_some_and(|(at, _)| at.distance_xz(spot) < 0.5), "{found:?}");
    (held, scoped)
}

/// With `SRP_BENCH_CAMPER_WATCH=lane` the camper is scoped (the scope key held) a good part of the time it is at its spot; without, it
/// never does.
#[test]
pub(crate) fn the_watching_camper_holds_the_scope_at_its_spot_and_the_old_one_does_not() {
    let (held, scoped) = stand_in_camper_with(1, Tuning::default(), CamperOptions { watch: Watch::Lane, ..CamperOptions::OFF });
    assert!(held[0].0 >= 50 && f64::from(held[0].1) >= 0.9 * f64::from(held[0].0), "{held:?}");
    assert!(scoped >= held[0].0 / 4, "scoped {scoped} of {} ticks at the spot", held[0].0);
    let (_, never) = stand_in_camper_with(1, Tuning::default(), CamperOptions::OFF);
    assert!(never < scoped / 4, "the old one scoped {never} against {scoped}");
}

/// In a Team Deathmatch on the soak field, the stand-in camper (bot 0)
/// goes to its spot and holds it; its teammate (the near miss) roams.
#[test]
pub(crate) fn the_stand_in_camper_holds_its_spot_and_a_teammate_does_not() {
    let held = stand_in_camper_run(1, Tuning::default());
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

/// The same with the sight refresh on. The time at the spot is what a life lasts there against the enemy that
/// sees it, which varies from seed to seed (in runs of 240 s, from 7 to 930 ticks of 50 ms; over 16 seeds 6,321
/// ticks before the refresh and 5,928 after, a difference the seeds' own spread explains), so what is asked for
/// is the sum of four seeds, and that the camper holds its spot all the while it is there (bar a push by a body).
#[test]
pub(crate) fn with_the_sight_refresh_the_stand_in_camper_holds_its_spot_and_a_teammate_does_not() {
    let (mut camper, mut mate) = ((0, 0, 0), (0, 0, 0));
    for seed in 1..=4 {
        let [c, m] = stand_in_camper_run(seed, super::sight::refreshed_tuning());
        camper = (camper.0 + c.0, camper.1 + c.1, camper.2 + c.2);
        mate = (mate.0 + m.0, mate.1 + m.1, mate.2 + m.2);
    }
    assert!(camper.0 >= 100, "camper {camper:?}, mate {mate:?}");
    assert!(f64::from(camper.1) >= 0.95 * f64::from(camper.0), "camper {camper:?}, mate {mate:?}");
    assert!(f64::from(mate.1) <= 0.2 * f64::from(mate.2), "camper {camper:?}, mate {mate:?}");
}

/// The camper's team: given (the place named), its mirror on the other team, or by the seed's parity; the mirror of a mirror is the place.
#[test]
pub(crate) fn the_camper_side_picks_the_team_of_the_place() {
    let per_team = 10;
    assert_eq!(mirror_place(1, per_team), 11);
    assert_eq!(mirror_place(11, per_team), 1);
    assert_eq!(mirror_place(mirror_place(4, per_team), per_team), 4);
    assert_eq!(CamperSide::Given.place(1, per_team, 3), 1);
    assert_eq!(CamperSide::Mirror.place(1, per_team, 2), 11);
    assert_eq!((CamperSide::BySeed.place(1, per_team, 2), CamperSide::BySeed.place(1, per_team, 3)), (1, 11));
    // The same role: a team's k-th place has the k-th role.
    let roles = survarium::v026g::loadouts::bot_roles(&(0..2 * per_team).map(|p| u8::from(p >= per_team)).collect::<Vec<u8>>());
    assert_eq!(roles[1], roles[mirror_place(1, per_team)]);
    assert_eq!(CamperSide::parse(None), Ok(CamperSide::Given));
    assert!(CamperSide::parse(Some("team1")).is_err());
}
