use super::*;
use crate::v026g::bots::reflex::{CombatTuning, WeaponKind};

/// What a bot did in the seconds before an episode (a stuck one, a missed
/// jump, a press that freed nothing), read from the record of every tick
/// ([`Rec`]), so the bench says why the bots are where they are stuck and
/// not only how many (`fact-x1-movement.md`: the logs could not).
pub(crate) const PRE_S: usize = 5;

/// ... as ticks.
pub(crate) const PRE_TICKS: usize = PRE_S * TPS;

/// The ticks of the [`PRE_S`] seconds before record `ri` (fewer at the
/// start of the match): any fire key; a target farther than the top of the
/// weapon's preferred band ([`band_top_m`]); the body standing throughout
/// (alive, under [`STOOD_MPS`]; not in an empty window).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pre {
    pub(crate) fired: bool,
    pub(crate) past_band: bool,
    pub(crate) standing: bool,
}

impl Pre {
    pub(crate) fn before(recs: &[Rec], ri: usize) -> Self {
        let window = &recs[ri.saturating_sub(PRE_TICKS).min(recs.len())..ri.min(recs.len())];
        Self {
            fired: window.iter().any(|r| r.alive && r.keys & key::FIRE != 0),
            past_band: window.iter().any(|r| r.alive && r.past_band),
            standing: !window.is_empty() && window.iter().all(|r| r.alive && r.speed < STOOD_MPS),
        }
    }
}

/// Episodes of one kind: by a key (a cause, the kind of a press) their
/// number and how many had fired, had a target past the band, stood, in the
/// [`PRE_S`] seconds before; and by the bot's play at the start.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreCounts {
    pub(crate) by: BTreeMap<&'static str, [u32; 4]>,
    pub(crate) plays: BTreeMap<String, u32>,
}

impl PreCounts {
    /// The episode of `place` at record `ri`, under `key`.
    pub(crate) fn add(&mut self, tally: &OwnerTally, key: &'static str, place: Place, ri: usize) {
        let recs = &tally.recs[place];
        let pre = Pre::before(recs, ri);
        let c = self.by.entry(key).or_default();
        c[0] += 1;
        c[1] += u32::from(pre.fired);
        c[2] += u32::from(pre.past_band);
        c[3] += u32::from(pre.standing);
        let play = recs.get(ri).map_or("none", |r| tally.plays[usize::from(r.play)].as_str());
        *self.plays.entry(play.to_owned()).or_default() += 1;
    }

    /// `{key: [episodes, fired, past band, standing]}`.
    pub(crate) fn by_json(&self) -> serde_json::Value {
        serde_json::json!(self.by)
    }

    /// The counts of every key together, `[episodes, fired, past band,
    /// standing]`.
    pub(crate) fn all(&self) -> [u32; 4] {
        self.by.values().fold([0; 4], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2], a[3] + c[3]])
    }

    /// For the owner-test line: "n (fired f, past band p, standing s)".
    pub(crate) fn text(&self) -> String {
        let [n, f, p, s] = self.all();
        format!("{n} (fired {f}, target past the band {p}, standing {s})")
    }
}

/// The top of the preferred band of a weapon class (`CombatTuning::band`,
/// the defaults: the bench's own tuning is not the yardstick), m.
pub(crate) fn band_top_m(class: &str) -> f32 {
    static TUNING: std::sync::OnceLock<CombatTuning> = std::sync::OnceLock::new();
    let kind = match class {
        "pistol" => WeaponKind::Pistol,
        "smg" => WeaponKind::Smg,
        "shotgun" => WeaponKind::Shotgun,
        "marksman" | "bolt" => WeaponKind::Sniper,
        _ => WeaponKind::Rifle,
    };
    TUNING.get_or_init(CombatTuning::default).band(kind).1
}

/// Whether the body's target is a living enemy farther than the top of its
/// weapon's preferred band: the bot would close in or hold fire, by its
/// play. (No range of X1's: the tip has none.)
pub(crate) fn target_past_band(game: &Game, place: Place, body: &super::super::Body) -> bool {
    body.alive
        && body
            .target
            .and_then(|t| enemy_of(game, place, body.team, t))
            .is_some_and(|him| him.feet.distance_xz(body.feet) > band_top_m(body.gun.class()))
}

#[test]
pub(crate) fn the_five_seconds_before_are_fire_a_far_target_and_standing() {
    let rec = |fire: bool, far: bool, speed: f32| Rec {
        alive: true,
        keys: if fire { key::FIRE } else { 0 },
        past_band: far,
        speed,
        ..Rec::default()
    };
    let quiet = |n: usize| vec![rec(false, false, 0.0); n];
    let mut recs = quiet(300);
    // Nothing before the first tick: all false, not "standing".
    assert_eq!(Pre::before(&recs, 0), Pre::default());
    // Standing throughout, nothing else.
    assert_eq!(Pre::before(&recs, 250), Pre { fired: false, past_band: false, standing: true });
    // A shot 4 s before counts, one 6 s before does not.
    recs[250 - 4 * TPS] = rec(true, false, 0.0);
    recs[250 - 6 * TPS] = rec(true, true, 0.0);
    assert_eq!(Pre::before(&recs, 250), Pre { fired: true, past_band: false, standing: true });
    assert!(Pre::before(&recs, 250 - TPS).past_band, "6 s before 250 is in the window of 250 - 1 s");
    // A tick of walking spoils standing; a dead tick too.
    recs[200] = rec(false, true, 2.0);
    assert_eq!(Pre::before(&recs, 250), Pre { fired: true, past_band: true, standing: false });
    recs[200] = Rec { alive: false, ..rec(true, true, 0.0) };
    assert_eq!(Pre::before(&recs, 250), Pre { fired: true, past_band: false, standing: false });
}

#[test]
pub(crate) fn the_band_top_follows_the_weapon_class() {
    let tuning = CombatTuning::default();
    assert_eq!(band_top_m("rifle"), tuning.band_rifle.1);
    assert_eq!(band_top_m("mg"), tuning.band_rifle.1);
    assert_eq!(band_top_m("smg"), tuning.band_smg.1);
    assert_eq!(band_top_m("shotgun"), tuning.band_shotgun.1);
    assert_eq!(band_top_m("pistol"), tuning.band_pistol.1);
    assert_eq!(band_top_m("marksman"), tuning.band_sniper.1);
    assert_eq!(band_top_m("bolt"), tuning.band_sniper.1);
}

#[test]
pub(crate) fn a_stuck_episode_is_counted_with_what_the_bot_did_before_it() {
    // Enemy 1 stands 90-100 m on (past every band), enemy 4 10 m from bot 3.
    // 5 s, then 5 s of pushing against a wall without moving, then off.
    // Bot 0 walks firing at the far one; bot 2 stands with no key, the far
    // one its target; bot 3 walks with the near one for its target.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 1]);
    play(&mut game, &mut tally, 260, |k, game| {
        for (place, z) in [(0, 10.0), (2, 20.0), (3, 30.0)] {
            let walks = place != 2;
            let (x, keys, step) = match k {
                0..100 if walks => (10.0 + 0.2 * k as f32, key::FORWARD | if place == 0 { key::FIRE } else { 0 }, 0.2),
                0..100 => (29.8, 0, 0.0),
                100..200 => (29.8, key::FORWARD, 0.0),
                _ => (29.8 + 0.2 * (k - 199) as f32, key::FORWARD, 0.2),
            };
            put(game, place, x, z, keys);
            game.bodies[place].step_m = step;
        }
        put(game, 1, 30.0, 110.0, 0);
        put(game, 4, 30.0, 40.0, 0);
        game.bodies[0].target = Some(1);
        game.bodies[2].target = Some(1);
        game.bodies[3].target = Some(4);
        vec![bot_now(), None, bot_now(), bot_now(), None]
    });
    let (_, json) = tally.finish(&game);
    let by = json["stuck_pre_5s"].as_object().expect("stuck_pre_5s");
    let all: Vec<u64> = (0..4)
        .map(|i| by.values().map(|c| c[i].as_u64().unwrap()).sum())
        .collect();
    // Three pushes; bot 0 fired and had a far target; bot 2 had one and
    // stood; bot 3 did none of the three.
    assert_eq!(all, vec![3, 1, 2, 1], "{json}");
    assert_eq!(json["stuck_play"]["Roam"], 3);
}

#[test]
pub(crate) fn a_missed_jump_and_a_press_that_freed_nothing_are_counted_with_what_the_bot_did_before() {
    let grid = {
        let triangles = super::super::ledge_triangles(0.8, mapdata::WalkerBehaviour::BySlope);
        WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default())
    };
    // Enemy 2 far from bots 0 and 1. Bot 0 stands firing at it 3 s, then at
    // the ledge's foot pushes forward (a missed jump); bot 1 stands 3 s too,
    // with no key, no target, then does the same and presses a jump 1.5 s in
    // (no missed jump); bot 3 hops at 3 s after firing at the far enemy and
    // stands (a press that frees nothing).
    let (mut game, mut tally) = made_up(&[0, 0, 1, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        let lead = k < 60;
        put(game, 0, if lead { 2.0 } else { 9.6 }, 3.0, if lead { key::FIRE } else { key::FORWARD });
        let keys1 = match k {
            0..60 => 0,
            90 => key::FORWARD | key::JUMP,
            _ => key::FORWARD,
        };
        put(game, 1, if lead { 2.0 } else { 9.6 }, 7.0, keys1);
        put(game, 2, 12.0, 100.0, 0);
        put(game, 3, 2.0, 5.0, if k < 60 { key::FIRE } else if k == 60 { key::JUMP } else { 0 });
        for body in &mut game.bodies {
            body.yaw = -std::f32::consts::FRAC_PI_2;
        }
        game.bodies[0].target = Some(2);
        game.bodies[3].target = Some(2);
        let mut nows = vec![bot_now(), bot_now(), None, bot_now()];
        if k == 60 {
            nows[3].as_mut().unwrap().jump = Some(("hop", None));
        }
        nows
    });
    let presses = tally.jumps.presses.clone();
    let (stalls, starts) = tally.stall_scan(0, &grid, &presses);
    assert_eq!((stalls[0], starts.len()), (1, 1));
    let pre = Pre::before(&tally.recs[0], starts[0]);
    assert_eq!(pre, Pre { fired: true, past_band: true, standing: true });
    assert!(tally.stall_scan(1, &grid, &presses).1.is_empty());
    // The hop that freed nothing (bot 3) is in the jumps part's counts.
    let (_, json) = tally.finish(&game);
    assert_eq!(json["not_freed_pre_5s"]["hop"], serde_json::json!([1, 1, 1, 1]), "{json}");
    assert_eq!(json["not_freed_play"]["Roam"], 1);
}
