//! Play 22: use a consumable (`player_bots_behaviour.md` §3; the record's
//! §8.8): the medkit, the bandages and the antitoxin of the bot's own kit,
//! by one rule.
//!
//! - **What it knows:** its own health pools (`pools.rs`, the HUD's), the
//!   drugs of its kit and what it pressed lately (`stock.rs`). Nothing of
//!   the enemy's.
//! - **The need of an item:** the share of each pool the item treats that
//!   is missing, weighted by what the loss costs (`NeedWeights`: the body
//!   and the head kill, a limb slows, the pain hardly matters), the worst
//!   of them; read from the pools as they will be once the drugs it has
//!   pressed have acted ([`projected`]), so one hurt is not answered twice.
//!   A body at 0.8 has a need of 0.2.
//! - **When:** a need from `start_need` (0.15) scores, the more the deeper
//!   (the graded consideration on [`Input::Need`]); from `urgent_need`
//!   (0.2, a body under 0.8) the play takes the alarm band: a Fight-band
//!   play that is running is kept against a candidate of its band short of
//!   `team::MARGIN`, and the bot knows its health a world late (up to 2 s),
//!   so a threshold of 0.6 or 0.7 presses at a true 0.4-0.55 in a fight
//!   where the best players press at 0.65 (`pr-heal.md`). The best
//!   players press 2-3 s after the first loss, at any depth, 79 % of them
//!   with the body under 0.8, moving and firing, an enemy within 30 m in
//!   57 % (`fighting-record.md` §8.8). Which item: the one that leaves the
//!   least need, the shorter when two leave as much, so the bandages go
//!   first for a small loss and the medkit is kept for a large one.
//! - **Safety is a preference:** no stand still, no crouch, no `hold_fire`
//!   (a use changes no speed and no hand, `medkit::tick` 0x33D790). If an
//!   enemy it knows of has a line to it, none is within `enemy_m` and a
//!   cover spot is within `cover_m`, it steps there first (at most
//!   `cover_step_ms`); else it presses at once and goes on. The play ends
//!   the step after the press.
//! - **The key** is the item's own, from the kit's filled slots in the
//!   client's order (`stock::Held::quick`); an item is never pressed from an
//!   empty slot or while it is active.
//! - **Not built:** the antitoxin before a toxic zone with a full pool
//!   (the zones are `hazards.rs`'s; no kit holds an antitoxin).
//!
//! `graded: false` (the tuning) is the old rule: a medkit with the body
//! under `below`, out of every known enemy's line of sight, then still and
//! crouched for `wait_s`, fire held; an enemy seen meanwhile ends it.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::pools::{Pool, Pools};
use crate::v026g::bots::stock::{Drug, Held};
use crate::v026g::bots::tactics::SpotQuery;
use crate::v026g::bots::tuning::NeedWeights;
use crate::v026g::bots::view::{EYE_CROUCHED, EYE_STANDING};
use crate::v026g::drive::key;

pub const NUMBERS: &[(&str, f32)] = &[];

/// What a consumable does, the client's (PROVEN, `fact-health-recovery.md`
/// §2; the bandages of revision 32): nothing for `delay_s` after the press,
/// then each pool it treats gains its rate a second for `active_s`, up to
/// the pool's most.
pub struct Item {
    pub drug: Drug,
    pub delay_s: f32,
    pub active_s: f32,
    pub treats: &'static [(Pool, f32)],
}

pub const ITEMS: [Item; 3] = [
    Item {
        drug: Drug::Medkit,
        delay_s: 2.0,
        active_s: 3.0,
        treats: &[
            (Pool::Body, 1.0),
            (Pool::Head, 1.0),
            (Pool::Hands, 1.0),
            (Pool::Legs, 1.0),
        ],
    },
    Item {
        drug: Drug::Bandages,
        delay_s: 1.0,
        active_s: 1.0,
        treats: &[
            (Pool::Hands, 1.0),
            (Pool::Legs, 1.0),
            (Pool::Body, 0.35),
            (Pool::Head, 0.14),
            (Pool::Pain, 0.35),
        ],
    },
    Item {
        drug: Drug::Antitoxin,
        delay_s: 1.0,
        active_s: 10.0,
        treats: &[(Pool::Infection, 0.5)],
    },
];

pub fn item(drug: Drug) -> &'static Item {
    ITEMS
        .iter()
        .find(|item| item.drug == drug)
        .expect("an item for every drug")
}

impl Item {
    /// From the press until the effect is over, ms.
    fn window_ms(&self) -> u32 {
        ((self.delay_s + self.active_s) * 1000.0) as u32
    }
}

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            // The deeper the need, the sooner; the start is the tuning's.
            Consideration::new(
                Input::Need,
                0.0,
                1.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.3), (0.2, 0.6), (0.4, 0.85), (0.6, 1.0)],
                },
            ),
            considerations::cooldown(5.0),
        ],
        numbers: Default::default(),
    }
}

/// Medkits in the slots of its kit now.
pub fn medkits_left(cx: &Ctx) -> u32 {
    u32::from(cx.bot.pantry.in_hand(Drug::Medkit))
}

/// Whether `held` was pressed and its effect is not over.
fn busy(held: &Held, now: u32) -> bool {
    held.pressed
        .is_some_and(|at| now < at + item(held.drug).window_ms())
}

/// Its pools as they will be once every drug pressed has acted: the last
/// world's, and for each press the part of its effect after that world.
pub fn projected(cx: &Ctx) -> Pools {
    let mut pools = cx.bot.pools.unwrap_or_else(|| Pools::of_body(cx.me.health));
    let seen = cx.view.world_time;
    for held in cx.bot.pantry.held() {
        let Some(at) = held.pressed else {
            continue;
        };
        let it = item(held.drug);
        let from = (at + (it.delay_s * 1000.0) as u32).max(seen);
        let to = at + it.window_ms();
        if to > from {
            let secs = (to - from) as f32 / 1000.0;
            for &(pool, rate) in it.treats {
                pools.add(pool, rate * secs);
            }
        }
    }
    pools
}

fn weight(w: &NeedWeights, pool: Pool) -> f32 {
    match pool {
        Pool::Body => w.body,
        Pool::Head => w.head,
        Pool::Legs => w.legs,
        Pool::Hands => w.hands,
        Pool::Pain => w.pain,
        Pool::Infection => w.infection,
    }
}

/// The need `item` answers in `pools`: the worst weighted missing share of
/// the pools it treats.
pub fn need(item: &Item, pools: &Pools, w: &NeedWeights) -> f32 {
    item.treats
        .iter()
        .map(|&(pool, _)| weight(w, pool) * pools.missing(pool))
        .fold(0.0, f32::max)
}

/// The need that is left once `item` has done all it will.
fn left_after(item: &Item, pools: &Pools, w: &NeedWeights) -> f32 {
    let mut after = *pools;
    for &(pool, rate) in item.treats {
        after.add(pool, rate * item.active_s);
    }
    ITEMS
        .iter()
        .map(|it| need(it, &after, w))
        .fold(0.0, f32::max)
}

/// The item to press now and the need it answers: of the units in its slots
/// that are not active, the one that leaves the least need (the shorter
/// when two leave as much); none when no need reaches `start_need`.
pub fn choose<'a>(cx: &'a Ctx) -> Option<(&'a Held, f32)> {
    let heal = &cx.tuning.heal;
    let pools = projected(cx);
    let w = &heal.weights;
    let total = |held: &Held| {
        let it = item(held.drug);
        it.delay_s + it.active_s
    };
    cx.bot
        .pantry
        .held()
        .iter()
        .filter(|held| held.hand > 0 && !busy(held, cx.now))
        .map(|held| (held, need(item(held.drug), &pools, w)))
        .filter(|&(_, need)| need >= heal.start_need)
        .map(|(held, need)| (held, need, left_after(item(held.drug), &pools, w)))
        .min_by(|a, b| {
            if (a.2 - b.2).abs() < 0.05 {
                total(a.0).total_cmp(&total(b.0))
            } else {
                a.2.total_cmp(&b.2)
            }
        })
        .map(|(held, need, _)| (held, need))
}

/// Whether an enemy it knows of has a clear line to it (crouched), or is
/// seen now (the old rule).
pub fn in_known_sight(cx: &Ctx) -> bool {
    if !cx.memory.all_seen(cx.now).is_empty() {
        return true;
    }
    let me = cx.me.feet + Vec3::new(0.0, EYE_CROUCHED, 0.0);
    cx.memory.tracks().iter().any(|track| {
        let eye = track.at + Vec3::new(0.0, EYE_STANDING, 0.0);
        cx.tactics.sees(eye, me)
    })
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.tuning.heal.graded {
        return legacy_candidates(cx, out);
    }
    if cx.me.carrying.is_some() {
        return;
    }
    let Some((_, need)) = choose(cx) else {
        return;
    };
    if let Some(mut candidate) = cx.candidate_given(
        PlayKind::Heal,
        TargetRef::None,
        None,
        &[(Input::Need, need)],
    ) {
        if need >= cx.tuning.heal.urgent_need {
            candidate.rank = Rank::Alarm;
        }
        out.push(candidate);
    }
}

/// The old rule's candidate: the body under `below`, no enemy seen or with
/// a line to it, a medkit in the slot.
fn legacy_candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let running = matches!(cx.bot.current, Some((PlayKind::Heal, _, _)));
    if !running
        && (cx.me.health >= cx.tuning.heal.below
            || cx.me.carrying.is_some()
            || medkits_left(cx) == 0)
    {
        return;
    }
    // Seen or in a known line of sight: not here (play 10 takes it).
    if in_known_sight(cx) {
        return;
    }
    let need = 1.0 - cx.me.health;
    out.extend(cx.candidate_given(
        PlayKind::Heal,
        TargetRef::None,
        None,
        &[(Input::Need, need)],
    ));
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// A step to cover first.
    Cover { to: Vec3, until: u32 },
    /// Press the item's key.
    Use,
    /// Pressed: done at the next step.
    Pressed,
    /// The old rule: still and crouched while it works.
    Wait { until: u32 },
}

/// The spot to step to before pressing, if the preference asks for one:
/// an enemy it knows of has a line to it, none is within `enemy_m`, and a
/// spot out of every such line is within `cover_m`.
fn cover_spot(cx: &Ctx, rng: &mut Rng) -> Option<Vec3> {
    let heal = &cx.tuning.heal;
    if heal.cover_m <= 0.0 || cx.enemies_near(cx.me.feet, heal.enemy_m) > 0 {
        return None;
    }
    let me = cx.me.feet + Vec3::new(0.0, EYE_STANDING, 0.0);
    let eye = Vec3::new(0.0, EYE_STANDING, 0.0);
    let seers: Vec<Vec3> = cx
        .memory
        .tracks()
        .iter()
        .map(|track| track.at)
        .filter(|&at| cx.tactics.sees(at + eye, me))
        .collect();
    let nearest = seers.iter().copied().min_by(|a, b| {
        a.distance_xz(cx.me.feet)
            .total_cmp(&b.distance_xz(cx.me.feet))
    })?;
    let mut query = SpotQuery::ring(cx.me.feet, 1.0, heal.cover_m);
    query.hidden_from = Some(nearest);
    query.side = cx.bot.side;
    query.samples = 16;
    cx.tactics
        .spots(&query, cx.now, 3, rng)
        .into_iter()
        .filter(|spot| {
            seers
                .iter()
                .all(|&at| !cx.tactics.sees(at + eye, spot.at + eye))
        })
        .filter(|spot| {
            cx.tactics
                .clear_step_for(cx.me.feet, spot.at, cx.now, cx.bot.side, false)
        })
        .min_by(|a, b| {
            a.at.distance_xz(cx.me.feet)
                .total_cmp(&b.at.distance_xz(cx.me.feet))
        })
        .map(|spot| spot.at)
}

pub fn start(_target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    if !cx.tuning.heal.graded {
        return State::Use;
    }
    match cover_spot(cx, rng) {
        Some(to) => State::Cover {
            to,
            until: cx.now + cx.tuning.heal.cover_step_ms,
        },
        None => State::Use,
    }
}

pub fn step(state: &mut State, _target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    if !cx.tuning.heal.graded {
        return legacy_step(state, cx, rng);
    }
    if let State::Cover { to, until } = *state {
        let there = cx.me.feet.distance_xz(to) <= 0.6;
        let enemy_near = cx.enemies_near(cx.me.feet, cx.tuning.heal.enemy_m) > 0;
        if cx.now < until && !there && !enemy_near {
            return Step::Continue(Intent::to(to, Pace::Walk).within(0.5).straight());
        }
        *state = State::Use;
    }
    match state {
        State::Use => {
            let Some((held, _)) = choose(cx) else {
                return Step::Done("nothing to use");
            };
            let press = key::QUICK[held.quick];
            *state = State::Pressed;
            Step::Continue(Intent::stay().pressing(press))
        }
        _ => Step::Done("used"),
    }
}

fn legacy_step(state: &mut State, cx: &Ctx, rng: &mut Rng) -> Step {
    if !cx.memory.all_seen(cx.now).is_empty() {
        return Step::Failed("an enemy is in sight");
    }
    let still = Intent::stay().crouched().hold_fire();
    match state {
        State::Wait { until } => {
            if cx.now >= *until {
                return Step::Done("healed");
            }
            Step::Continue(still)
        }
        _ => {
            let Some(held) = cx.bot.pantry.first(Drug::Medkit) else {
                return Step::Failed("no medkit left");
            };
            let (low, high) = cx.tuning.heal.wait_s;
            let wait = rng.range(low, high);
            *state = State::Wait {
                until: cx.now + (wait * 1000.0) as u32,
            };
            Step::Continue(still.pressing(key::QUICK[held.quick]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::stock::{Reading, Readings};
    use crate::v026g::bots::view::Mode;

    fn presses(steps: &[Step]) -> Vec<u32> {
        steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(i) if i.press != 0 => Some(i.press),
                _ => None,
            })
            .collect()
    }

    /// A bot of the field at `at` with its body at `health`.
    fn hurt_bot(health: f32, at: Vec3) -> Scenario {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, at).health = health;
        s
    }

    fn heals(s: &Scenario) -> Option<Candidate> {
        s.candidates(0)
            .into_iter()
            .find(|c| c.kind == PlayKind::Heal)
    }

    #[test]
    fn the_need_is_the_worst_weighted_missing_share_of_the_pools_an_item_treats() {
        let w = NeedWeights::default();
        let medkit = item(Drug::Medkit);
        let bandages = item(Drug::Bandages);
        let antitoxin = item(Drug::Antitoxin);
        assert_eq!(need(medkit, &Pools::FULL, &w), 0.0);
        assert!((need(medkit, &Pools::of_body(0.8), &w) - 0.2).abs() < 1e-6);
        assert!((need(medkit, &Pools::of_body(0.3), &w) - 0.7).abs() < 1e-6);
        // The head is 0.4 full: a quarter gone is a quarter missing.
        let mut pools = Pools::FULL;
        pools.head = 0.3;
        assert!((need(medkit, &pools, &w) - 0.25).abs() < 1e-6);
        // A broken leg slows: half its weight; the antitoxin does not treat it.
        let mut pools = Pools::FULL;
        pools.legs = [0.0, 0.5];
        assert!((need(medkit, &pools, &w) - w.legs).abs() < 1e-6);
        assert_eq!(need(antitoxin, &pools, &w), 0.0);
        // Pain alone is hardly a need; infection counts for the antitoxin only.
        let mut pools = Pools::FULL;
        pools.pain = 0.0;
        assert!((need(bandages, &pools, &w) - w.pain).abs() < 1e-6);
        assert_eq!(need(medkit, &pools, &w), 0.0);
        pools.infection = 0.7;
        assert!((need(antitoxin, &pools, &w) - 0.3 * w.infection).abs() < 1e-6);
        assert_eq!(need(medkit, &pools, &w), 0.0);
    }

    #[test]
    fn the_trigger_is_graded_a_body_at_0_86_waits_0_84_scores_0_78_is_urgent() {
        let at = Vec3::new(30.0, 0.0, 10.0);
        assert!(heals(&hurt_bot(0.9, at)).is_none());
        assert!(heals(&hurt_bot(0.86, at)).is_none());
        let mild = heals(&hurt_bot(0.84, at)).expect("a body at 0.84");
        let urgent = heals(&hurt_bot(0.78, at)).expect("a body at 0.78");
        assert_eq!(urgent.rank, Rank::Alarm);
        let deep = heals(&hurt_bot(0.5, at)).expect("a body at 0.5");
        let deeper = heals(&hurt_bot(0.3, at)).expect("a body at 0.3");
        assert_eq!(mild.rank, Rank::Fight);
        assert_eq!(deep.rank, Rank::Alarm);
        assert!(mild.score.get() < deep.score.get(), "{mild:?} {deep:?}");
        assert!(deep.score.get() < deeper.score.get() + 1e-6);
        // Hurt seen or not, in the enemy's sight or not: it is a candidate
        // either way, the safety is a preference.
        let mut s = hurt_bot(0.5, Vec3::new(30.0, 0.0, 35.0));
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 35.0), Some(Source::Seen));
        assert!(heals(&s).is_some());
    }

    #[test]
    fn a_broken_limb_with_the_body_whole_is_a_need_and_the_pools_come_from_the_world() {
        let mut s = hurt_bot(1.0, Vec3::new(30.0, 0.0, 10.0));
        assert!(heals(&s).is_none());
        let mut pools = Pools::FULL;
        pools.legs = [0.0, 0.5];
        s.pools = Some(pools);
        let candidate = heals(&s).expect("a broken leg");
        assert_eq!(candidate.rank, Rank::Alarm, "a leg's 0.38 is over the 0.2");
        // The body from the pools when there are some, else from the view.
        s.pools = Some(Pools::of_body(0.4));
        s.view.places[0].health = 1.0;
        assert_eq!(heals(&s).unwrap().rank, Rank::Alarm);
    }

    #[test]
    fn what_it_pressed_is_counted_as_acting_so_a_hurt_is_not_answered_twice() {
        let mut s = hurt_bot(0.4, Vec3::new(30.0, 0.0, 10.0));
        let now = s.view.clock;
        assert!(heals(&s).is_some());
        // Pressed a second ago: the medkit is active, its effect counts.
        s.pantry.pressed(0, now - 1_000);
        assert!(heals(&s).is_none(), "the second medkit waits");
        // Its window over and the pools still low in the world it knows: again.
        s.view.clock = now + 10_000;
        s.view.world_time = now + 9_500;
        assert!(heals(&s).is_some());
        // The last one used: never again, in any life.
        s.pantry.pressed(0, now + 10_000);
        s.view.clock = now + 30_000;
        s.view.world_time = now + 29_500;
        assert!(heals(&s).is_none());
        s.pantry.new_life(0);
        assert!(heals(&s).is_none());
    }

    #[test]
    fn the_item_that_leaves_the_least_need_is_pressed_and_the_shorter_when_equal() {
        let mut s = hurt_bot(0.8, Vec3::new(30.0, 0.0, 10.0));
        // Bandages and a medkit in the slots: bandages cover a small loss.
        let read = |s: &mut Scenario| {
            let drug = |drug, quick| Reading {
                kit: 0,
                drug,
                quick,
                slot: 1,
                in_use: true,
                in_hand: Some(1),
                stored: 0,
            };
            s.pantry.read(
                1_000,
                &Readings {
                    kit: 0,
                    kits: 1,
                    alive: true,
                    drugs: vec![drug(Drug::Medkit, 0), drug(Drug::Bandages, 3)],
                },
            );
        };
        read(&mut s);
        let cx = s.cx(0);
        let (held, _) = choose(&cx).unwrap();
        assert_eq!(held.drug, Drug::Bandages);
        // A deep loss: the bandages leave need, the medkit does not.
        s.view.places[0].health = 0.3;
        let cx = s.cx(0);
        let (held, _) = choose(&cx).unwrap();
        assert_eq!(held.drug, Drug::Medkit);
    }

    #[test]
    fn the_key_is_the_kits_own_slot_order_and_an_empty_slot_is_never_pressed() {
        let mut s = hurt_bot(0.5, Vec3::new(30.0, 0.0, 10.0));
        // The medkit is the third filled slot of its kit.
        s.pantry.read(
            1_000,
            &Readings {
                kit: 0,
                kits: 1,
                alive: true,
                drugs: vec![Reading {
                    kit: 0,
                    drug: Drug::Medkit,
                    quick: 2,
                    slot: 2,
                    in_use: true,
                    in_hand: Some(2),
                    stored: 0,
                }],
            },
        );
        let steps = s.run(0, PlayKind::Heal, TargetRef::None, 4, 300, 3.5);
        assert_eq!(presses(&steps), [key::QUICK[2]]);
        // The same kit with the slot empty: no candidate, no press.
        s.pantry.read(
            2_000,
            &Readings {
                kit: 0,
                kits: 1,
                alive: true,
                drugs: vec![Reading {
                    kit: 0,
                    drug: Drug::Medkit,
                    quick: 2,
                    slot: 2,
                    in_use: true,
                    in_hand: Some(0),
                    stored: 0,
                }],
            },
        );
        assert!(heals(&s).is_none());
        let steps = s.run(0, PlayKind::Heal, TargetRef::None, 4, 300, 3.5);
        assert!(presses(&steps).is_empty(), "{steps:?}");
    }

    #[test]
    fn the_third_press_of_a_kit_is_refused_by_the_rule() {
        let mut s = hurt_bot(0.4, Vec3::new(30.0, 0.0, 10.0));
        let mut pressed = 0;
        for k in 0..3u32 {
            s.view.clock = 300_000 + 20_000 * k;
            s.view.world_time = s.view.clock - 500;
            if let Some(c) = heals(&s) {
                assert_eq!(c.kind, PlayKind::Heal);
                pressed += 1;
                assert!(s.pantry.pressed(0, s.view.clock));
                s.last_ran.clear();
            }
        }
        assert_eq!(pressed, 2, "a kit has two for the match");
        s.pantry.new_life(0);
        s.view.clock += 60_000;
        assert!(heals(&s).is_none());
    }

    #[test]
    fn it_neither_stops_nor_crouches_nor_holds_fire_and_the_play_ends_after_the_press() {
        let mut s = hurt_bot(0.4, Vec3::new(30.0, 0.0, 10.0));
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Seen));
        let steps = s.run(0, PlayKind::Heal, TargetRef::None, 10, 300, 3.5);
        assert_eq!(presses(&steps), [key::QUICK[0]]);
        assert!(
            steps.iter().all(|s| match s {
                Step::Continue(i) => !i.crouch && i.fire,
                _ => true,
            }),
            "{steps:?}"
        );
        // The press and the step after: no wait. (A bot in a mild zone
        // presses and goes on to walk out as it was: the play reads no
        // zone and holds the bot nowhere.)
        assert_eq!(steps.len(), 2, "{steps:?}");
        assert_eq!(steps.last(), Some(&Step::Done("used")));
    }

    #[test]
    fn a_cover_step_only_when_exposed_with_no_enemy_within_10_m_and_cover_near() {
        // The wall across x = 40 (z 0 to 25): an enemy east of it at z 27
        // has a line to a bot at (38, 26.5); a spot behind the wall is 4 m
        // off.
        let bot = Vec3::new(38.0, 0.0, 26.5);
        let mut s = hurt_bot(0.5, bot);
        s.tuning.heal.cover_m = 6.0;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 27.0), Some(Source::Heard));
        let mut rng = Rng::new(11);
        let State::Cover { to, .. } = start(TargetRef::None, &s.cx(0), &mut rng) else {
            panic!("a step to cover");
        };
        assert!(
            to.distance_xz(bot) <= 6.0 && to.distance_xz(bot) >= 1.0,
            "{to:?}"
        );
        assert!(
            !s.tactics
                .sees(Vec3::new(60.0, 1.6, 27.0), to + Vec3::new(0.0, 1.6, 0.0))
        );
        let steps = s.run(0, PlayKind::Heal, TargetRef::None, 12, 300, 3.5);
        assert_eq!(presses(&steps).len(), 1, "{steps:?}");
        assert!(
            matches!(&steps[0], Step::Continue(i) if matches!(i.go, Go::To { .. }) && !i.crouch),
            "{steps:?}"
        );
        // An enemy within 10 m: press at once.
        let mut near = hurt_bot(0.5, bot);
        near.tuning.heal.cover_m = 6.0;
        near.enemy(1, 1, Vec3::new(46.0, 0.0, 27.0), Some(Source::Seen));
        assert_eq!(start(TargetRef::None, &near.cx(0), &mut rng), State::Use);
        // Not exposed (nobody with a line to it): press at once.
        let mut hidden = hurt_bot(0.5, Vec3::new(30.0, 0.0, 10.0));
        hidden.tuning.heal.cover_m = 6.0;
        hidden.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        assert_eq!(start(TargetRef::None, &hidden.cx(0), &mut rng), State::Use);
        // No cover near: press at once.
        let mut bare = hurt_bot(0.5, bot);
        bare.tuning.heal.cover_m = 1.2;
        bare.enemy(1, 1, Vec3::new(60.0, 0.0, 27.0), Some(Source::Heard));
        assert_eq!(start(TargetRef::None, &bare.cx(0), &mut rng), State::Use);
    }

    #[test]
    fn the_same_state_gives_the_same_steps() {
        let run = || {
            let mut s = hurt_bot(0.5, Vec3::new(38.0, 0.0, 26.5));
            s.tuning.heal.cover_m = 6.0;
            s.enemy(1, 1, Vec3::new(60.0, 0.0, 27.0), Some(Source::Heard));
            s.run(0, PlayKind::Heal, TargetRef::None, 12, 300, 3.5)
        };
        assert_eq!(run(), run());
    }

    // ---- the old rule, `graded: false` ----

    #[test]
    fn the_old_rule_a_wounded_bot_out_of_sight_heals_once_and_never_in_sight() {
        // The field's wall across x = 40 (z 0 to 25) between the bot and
        // an enemy it heard.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.tuning.heal.graded = false;
        s.bot(0, 0, Vec3::new(30.0, 0.0, 10.0)).health = 0.5;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::Heal, "{best:#?}");
        let steps = s.run(0, PlayKind::Heal, TargetRef::None, 20, 300, 3.5);
        assert_eq!(presses(&steps), [key::QUICK[0]]);
        assert!(steps.iter().all(|s| match s {
            Step::Continue(i) => i.crouch && !i.fire && i.go == Go::Stay,
            _ => true,
        }));
        assert_eq!(steps.last(), Some(&Step::Done("healed")));
        // 1.5 to 3 s of waiting.
        assert!((6..=12).contains(&steps.len()), "{}", steps.len());
        // The cooldown holds it back for a while, then the kit's second
        // medkit is offered; with both gone it never is, in any life.
        s.last_ran.push((PlayKind::Heal, s.view.clock));
        s.current = None;
        s.pantry.pressed(0, s.view.clock);
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        s.view.clock += 6_000;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().any(|c| c.kind == PlayKind::Heal));
        s.pantry.pressed(0, s.view.clock);
        s.view.clock += 6_000;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        s.pantry.new_life(0);
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
    }

    #[test]
    fn the_old_rule_no_heal_in_a_known_line_of_sight_or_when_healthy() {
        // Past the wall's end: the heard enemy has a clear line.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.tuning.heal.graded = false;
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0)).health = 0.5;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 35.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        // Seen: not either.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.tuning.heal.graded = false;
        s.bot(0, 0, Vec3::new(30.0, 0.0, 10.0)).health = 0.5;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Seen));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        // Healthy: not either.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.tuning.heal.graded = false;
        s.bot(0, 0, Vec3::new(30.0, 0.0, 10.0)).health = 0.9;
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
    }
}
