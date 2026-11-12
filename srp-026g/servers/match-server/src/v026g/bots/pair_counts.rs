//! The bench's counts of the pair and route-start rules (`plays::buddy`, `plays::roam`): how often each rule, when its switch is on,
//! changed what a bot did. Counts only; the bench prints [`line`].

use std::cell::Cell;

thread_local! {
    static COUNTS: Cell<[u64; 3]> = const { Cell::new([0; 3]) };
}

fn add(k: usize) {
    COUNTS.with(|c| {
        let mut v = c.get();
        v[k] += 1;
        c.set(v);
    });
}

/// A leader walked on at a walk for a follower behind it (`roam.leader_eases`), a step.
pub fn eased() {
    add(0);
}

/// A follower was told its leader had just respawned (`buddy.respawn_rule`), an answer.
pub fn respawned() {
    add(1);
}

/// A route run started elsewhere than the old rule would have started it (`roam.join_ahead`).
pub fn joined_elsewhere() {
    add(2);
}

/// `eased N respawned N joined_elsewhere N`, so far on this thread.
pub fn line() -> String {
    let [a, b, c] = COUNTS.with(|c| c.get());
    format!("eased {a} respawned {b} joined_elsewhere {c}")
}
