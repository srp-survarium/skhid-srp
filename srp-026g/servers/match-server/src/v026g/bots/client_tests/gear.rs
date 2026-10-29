//! Client-test steps of the gear package (PR #707 the live grenade list and the pockets, PR #708 the trap press; `fx-gear-client-tests.md` G1, P6-1,
//! G3, T2; `an-gear-moves.md` 6.1-6.4).
//!
//! The grenade and trap counters of the match-end log lines are in every frame (`Frame::counters`): a step reads what a try changed as the
//! difference of the first and the last frame. The owner throws and the enemy supports throw as the match gives them; the runner sets no kit,
//! so the owner's kit and key are told by him (ASK OWNER in the sheet) and checked here against what the list made of his presses.
//!
//! Not here: P6-3 (a rusher pressing G with no grenade: needs a scripted press of the key near a held mate at a place, which a script cannot
//! be given without the map), P6-2 (a support's second and third life: the pocket store, `Supplies`, is not in the frame; it needs
//! `in_hand`/`stored` per life), and T1 (the trap's landing against the placer's feet: the `trap_promise` records are trace lines, not
//! notes or counters).

use super::*;
use crate::v026g::bots::grenade_threat::AvoidStats;
use crate::v026g::bots::live_grenades::Whose;

const VECTOR_LABORATORY: u8 = 7;
const COLOGNE: u8 = 8;

/// A counter step: `pick` reads N numbers of the frame's counters; a name's value is the last frame's number less the first frame's.
struct Delta<const N: usize> {
    pick: fn(&Counters) -> [f64; N],
    first: Option<[f64; N]>,
    last: Option<[f64; N]>,
}

impl<const N: usize> Delta<N> {
    fn new(pick: fn(&Counters) -> [f64; N]) -> Self {
        Self { pick, first: None, last: None }
    }
}

impl<const N: usize> std::fmt::Debug for Delta<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Delta {{ first: {:?}, last: {:?} }}", self.first, self.last)
    }
}

impl<const N: usize> CustomMeter for Delta<N> {
    fn sample(&mut self, frame: &Frame) {
        let now = (self.pick)(&frame.counters);
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    fn value(&self, n: usize) -> Option<f64> {
        let (first, last) = (self.first?, self.last?);
        (n < N).then(|| last[n] - first[n])
    }
}

fn sum3(n: [u32; 3]) -> f64 {
    f64::from(n.iter().sum::<u32>())
}

/// G1: what the list made of the enemy supports' throws: `frags_listed`, `launched`, `assumed` (listed from a bot's key with its pockets read
/// and no grenade left: a phantom), `unplaced` (let go and never placed).
fn pick_enemy(c: &Counters) -> [f64; 4] {
    [f64::from(c.throws.frag), f64::from(c.throws.launched), f64::from(c.throws.assumed), f64::from(c.throws.unplaced)]
}

/// P6-1: the owner's own throws: `human_frags` (listed as a human's), `human_quick` (by a quick-use key), `human_other_kit` (from a kit other than the
/// first), `human_empty` (his key found no grenade in the kit in use), `smokes` (smoke grenades seen and left out).
fn pick_own(c: &Counters) -> [f64; 5] {
    [
        f64::from(c.throws.human_frag),
        f64::from(c.throws.human_quick),
        f64::from(c.throws.human_other_kit),
        f64::from(c.throws.human_empty),
        f64::from(c.throws.smoke),
    ]
}

/// G3: what the avoidance of each team's bots made of the owner's grenades (`Whose::Human`): the owner team's (`mate_*`) and the other's
/// (`enemy_*`): noticed, stepped, escaped (out of the radius at the blast) and caught (in it), and for the enemy the `no way out`.
fn pick_avoid(c: &Counters) -> [f64; 8] {
    let human = Whose::Human.index();
    let (mate, enemy): (&AvoidStats, &AvoidStats) = (&c.avoid[0][human], &c.avoid[1][human]);
    [
        sum3(mate.noticed),
        sum3(mate.stepped),
        sum3(mate.escaped),
        sum3(mate.caught),
        sum3(enemy.noticed),
        sum3(enemy.stepped),
        sum3(enemy.no_way),
        sum3(enemy.caught),
    ]
}

/// T2: `traps_fired` (traps of the teams that went off) and `own_team_caught` (of them, with a broken-legged body of the same team near).
fn pick_traps(c: &Counters) -> [f64; 2] {
    [f64::from(c.traps.fired), f64::from(c.traps.own_team_caught)]
}

fn enemy_meter(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(Delta::<4>::new(pick_enemy))
}

fn own_meter(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(Delta::<5>::new(pick_own))
}

fn avoid_meter(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(Delta::<8>::new(pick_avoid))
}

fn traps_meter(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(Delta::<2>::new(pick_traps))
}

pub fn steps() -> Vec<Step> {
    let vector = || Group::new(VECTOR_LABORATORY, TEAM_DEATHMATCH).of_size(6);
    vec![
        // G1 / an-gear-moves 6.1: the enemy supports (places 4 and 12) throw; every throw is listed, none is assumed.
        Step::new(
            "gear-enemy-grenades",
            vector(),
            "Stay alive for two minutes in view of the enemy supports and watch for their grenades; take cover when you see one.",
        )
        .start(&[Cond::After(Span::s(5))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["frags_listed", "launched", "assumed", "unplaced"], make: enemy_meter })
        .measure(Measure::OwnerDeaths)
        .bar("frags_listed", Test::AtLeast(1.0))
        .bar("assumed", Test::AtMost(0.0))
        .budget_s(120)
        .arm_s(180)
        .tries(1),
        // P6-1 / an-gear-moves 6.2: the owner's kit and key. Kit 2: G, then the frag's number key.
        Step::new(
            "gear-own-throw",
            vector(),
            "Spawn in kit 2 (pick it on the death screen). Throw a grenade with G at a bot, then one with the frag's number key.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["human_frags", "human_quick", "human_other_kit", "human_empty", "smokes"], make: own_meter })
        // Both kit-2 throws listed: two human frags, one by a quick-use key, two from another kit than the first.
        .bar("human_frags", Test::AtLeast(2.0))
        .bar("human_quick", Test::AtLeast(1.0))
        .bar("human_other_kit", Test::AtLeast(2.0))
        .budget_s(120)
        .arm_s(180)
        .tries(1),
        // G3 / an-gear-moves 6.3: the bots step out of the owner's grenade.
        Step::new(
            "gear-avoid-own-grenade",
            vector(),
            "Throw a grenade at the feet of a mate bot standing still, then one at an enemy bot within 10 m that can see it.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom {
            names: &["mate_noticed", "mate_stepped", "mate_escaped", "mate_caught", "enemy_noticed", "enemy_stepped", "enemy_no_way", "enemy_caught"],
            make: avoid_meter,
        })
        .bar("mate_noticed", Test::AtLeast(1.0))
        .bar("mate_caught", Test::AtMost(0.0))
        .budget_s(150)
        .arm_s(180)
        .tries(1),
        // T2 / an-gear-moves 6.4: own and mate catches with real routes. Cologne, batteries. No bar: the placements are in the trace.
        Step::new(
            "gear-traps-own-catch",
            Group::new(COLOGNE, BATTERIES).of_size(6),
            "Stay away from the enemy's spawn exits and watch their traps for three minutes; do not walk over a trap.",
        )
        .start(&[Cond::After(Span::s(5))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["traps_fired", "own_team_caught"], make: traps_meter })
        .budget_s(180)
        .arm_s(180)
        .tries(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::dry;
    use super::*;

    fn frame(ms: u32, counters: Counters) -> Frame {
        let mut f = dry::frame_at(ms, dry::body(0, 0, mapdata::Vec3::ZERO));
        f.counters = counters;
        f
    }

    fn start() -> TryStart {
        TryStart { started: At(0), kills: 0, deaths: 0 }
    }

    /// A step reads the change over its try: counters that stood at 3 and 4 when it began and at 5 and 4 at the end read 2 and 0.
    #[test]
    fn a_counter_step_reads_the_change_over_the_try() {
        let mut meter = enemy_meter(&start());
        let mut c = Counters::default();
        c.throws.frag = 3;
        c.throws.launched = 4;
        meter.sample(&frame(0, c.clone()));
        c.throws.frag = 5;
        c.throws.assumed = 1;
        meter.sample(&frame(60_000, c));
        let v = |n| meter.value(n);
        assert_eq!((v(0), v(1), v(2), v(3)), (Some(2.0), Some(0.0), Some(1.0), Some(0.0)));
        assert_eq!(v(4), None);
        assert!(enemy_meter(&start()).value(0).is_none());
    }

    /// The avoidance step splits the owner's grenades by the team of the bot that dodged them.
    #[test]
    fn the_avoid_meter_splits_by_team() {
        let mut meter = avoid_meter(&start());
        let mut c = Counters::default();
        meter.sample(&frame(0, c.clone()));
        c.avoid[0][Whose::Human.index()].noticed = [1, 0, 0];
        c.avoid[0][Whose::Human.index()].stepped = [1, 0, 0];
        c.avoid[0][Whose::Human.index()].escaped = [1, 0, 0];
        c.avoid[1][Whose::Human.index()].noticed = [0, 1, 0];
        c.avoid[1][Whose::Human.index()].no_way = [0, 0, 1];
        c.avoid[1][Whose::Bot.index()].caught = [5, 5, 5];
        meter.sample(&frame(30_000, c));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2), v(3)), (1.0, 1.0, 1.0, 0.0));
        assert_eq!((v(4), v(5), v(6), v(7)), (1.0, 0.0, 1.0, 0.0), "a bot's own grenade is not counted");
    }

    /// The own-throw step through the real runner: two human frags, one of them by a quick key, both from another kit: PASS.
    #[test]
    fn dry_run_the_own_throw_step_passes_with_the_kit_two_throws() {
        let step = steps().into_iter().find(|s| s.id == "gear-own-throw").unwrap().tries(1);
        let (results, said) = dry::run(step, 150, |ms| {
            let mut c = Counters::default();
            if ms >= 20_000 {
                c.throws.human_frag = 1;
                c.throws.human_other_kit = 1;
            }
            if ms >= 40_000 {
                c.throws.human_frag = 2;
                c.throws.human_quick = 1;
                c.throws.human_other_kit = 2;
            }
            frame(ms, c)
        });
        assert!(results.contains("gear-own-throw") && results.contains("\tPASS\t") && results.contains("human_frags=2"), "{results}");
        assert!(said.iter().any(|l| l.contains("kit 2")), "{said:?}");
    }
}
