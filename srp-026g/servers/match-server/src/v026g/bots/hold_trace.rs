//! The bench's trace of how the hold plays (Crossfire, Cut, HoldExit) end:
//! by the play itself (`Bot::end_play`, with its reason) or by the commander
//! giving the bot another play (`commander`), by the play's age, the
//! incumbent's and the replacer's rank and score, and how far the bot still
//! had to go. Counts only; the bench prints [`line`].

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::plays::{PlayKind, Rank};
use mapdata::Vec3;

thread_local! {
    static TRACE: RefCell<BTreeMap<String, (u64, f64)>> = const { RefCell::new(BTreeMap::new()) };
}

thread_local! {
    static FEET: std::cell::Cell<Vec3> = const { std::cell::Cell::new(Vec3::ZERO) };
}

/// Where the bot being thought for stands (for [`ended`]'s distance).
pub fn at(feet: Vec3) {
    FEET.with(|f| f.set(feet));
}

pub fn feet() -> Vec3 {
    FEET.with(|f| f.get())
}

fn held(kind: PlayKind) -> bool {
    matches!(kind, PlayKind::Crossfire | PlayKind::Cut | PlayKind::HoldExit)
}

fn age_bucket(age_ms: u32) -> &'static str {
    match age_ms / 1000 {
        0..=2 => "a<3s",
        3..=5 => "b<6s",
        6..=9 => "c<10s",
        10..=19 => "d<20s",
        _ => "e>=20s",
    }
}

fn dist_bucket(dist: Option<f32>) -> &'static str {
    match dist {
        None => "no goal",
        Some(d) if d <= 2.0 => "there",
        Some(d) if d <= 8.0 => "<8m",
        Some(d) if d <= 20.0 => "<20m",
        Some(d) if d <= 40.0 => "<40m",
        Some(_) => ">40m",
    }
}

fn add(key: String, value: f64) {
    TRACE.with(|t| {
        let mut t = t.borrow_mut();
        let e = t.entry(key).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += value;
    });
}

/// A hold play ended itself (or no way to its goal).
pub fn ended(kind: PlayKind, age_ms: u32, why: &str, dist: Option<f32>) {
    if held(kind) {
        add(format!("end {kind:?} {} {why} @{}", age_bucket(age_ms), dist_bucket(dist)), 0.0);
    }
}

/// The commander handed a bot running a hold play another play.
#[allow(clippy::too_many_arguments)]
pub fn switched(
    kind: PlayKind,
    age_ms: u32,
    incumbent: Option<(Rank, f32)>,
    new_kind: PlayKind,
    new_score: f32,
    reason: &str,
    dist: Option<f32>,
) {
    if !held(kind) {
        return;
    }
    let (inc, delta) = match incumbent {
        Some((rank, score)) => (format!("cand {rank:?}"), f64::from(new_score - score)),
        None => ("no candidate".to_string(), 0.0),
    };
    add(
        format!("sw {kind:?} {} [{reason}] {inc} -> {new_kind:?} @{}", age_bucket(age_ms), dist_bucket(dist)),
        delta,
    );
}

/// The counts so far: `key: [n, mean score gain of the replacer]`.
pub fn line() -> String {
    TRACE.with(|t| {
        let t = t.borrow();
        let rows: Vec<String> = t
            .iter()
            .map(|(k, (n, sum))| format!("{k:?}: [{n}, {:.3}]", sum / *n as f64))
            .collect();
        format!("{{{}}}", rows.join(", "))
    })
}
