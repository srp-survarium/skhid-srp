//! The one upload allowance of an updater's run
//! (`srp-026g/docs/notes/torrent-downloads.md`, "The upload cap"): every
//! swarm and the rendezvous' torrent take their bytes from it, in every
//! engine of the run, so all the sharing together stays within
//! [`CAP`] bytes a second. The launcher's `1E` may lower it, never raise it.
//!
//! A token bucket: it fills at the rate, up to a second of it (four blocks
//! at the least), and starts full. Bytes sent outside a swarm (the
//! rendezvous' torrent, a few hundred KB at once) may leave it owing: then
//! nothing goes until it is paid back.

use crate::torrent::BLOCK;

/// The fixed cap of all sharing: 1 MiB a second ("1 MB/s").
pub const CAP: u64 = 1 << 20;

#[derive(Debug, Clone)]
pub struct Allowance {
    /// Bytes a second: [`CAP`], or less as the launcher asked.
    rate: u64,
    /// Bytes that may go now; below zero: owed.
    tokens: i64,
    /// The clock (milliseconds) it was filled to.
    at: u64,
    /// Every byte sent in the run, from every swarm and the rendezvous.
    sent: u64,
}

fn as_tokens(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

impl Allowance {
    /// At [`CAP`], full, at the clock `now`.
    pub fn new(now: u64) -> Self {
        let mut allowance = Self {
            rate: CAP,
            tokens: 0,
            at: now,
            sent: 0,
        };
        allowance.tokens = as_tokens(allowance.burst());
        allowance
    }

    fn burst(&self) -> u64 {
        self.rate.max(u64::from(BLOCK) * 4)
    }

    /// The launcher's upload limit (`1E`, bytes a second; `None`: none):
    /// the rate is the lower of it and [`CAP`].
    pub fn lower(&mut self, asked: Option<u64>) {
        self.rate = asked.filter(|&asked| asked > 0).map_or(CAP, |asked| asked.min(CAP));
        self.tokens = self.tokens.min(as_tokens(self.burst()));
    }

    /// Bytes a second it allows.
    pub fn rate(&self) -> u64 {
        self.rate
    }

    /// Bytes sent in the run so far.
    pub fn sent(&self) -> u64 {
        self.sent
    }

    /// Fills it for the time passed since the last fill.
    pub fn tick(&mut self, now: u64) {
        let elapsed = now.saturating_sub(self.at);
        // Filled by whole bytes: what a short tick would round away waits.
        let more = self.rate.saturating_mul(elapsed) / 1000;
        if more == 0 {
            return;
        }
        self.at = now;
        self.tokens = self.tokens.saturating_add(as_tokens(more)).min(as_tokens(self.burst()));
    }

    /// Whether `bytes` may go now.
    pub fn room(&self, bytes: u32) -> bool {
        self.tokens >= i64::from(bytes)
    }

    /// Whether nothing is owed: something may be given outside a swarm.
    pub fn clear(&self) -> bool {
        self.tokens >= 0
    }

    /// `bytes` went to a peer: out of the allowance, owed when more than it
    /// had.
    pub fn spend(&mut self, bytes: u64) {
        self.tokens = self.tokens.saturating_sub(as_tokens(bytes));
        self.sent = self.sent.saturating_add(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_full_and_fills_at_the_cap() {
        let mut allowance = Allowance::new(1000);
        assert_eq!(allowance.rate(), CAP);
        assert!(allowance.room(BLOCK));
        allowance.spend(CAP);
        assert!(!allowance.room(1));
        assert!(allowance.clear());
        allowance.tick(1500);
        assert!(allowance.room((CAP / 2) as u32) && !allowance.room((CAP / 2) as u32 + 1));
        // Never more than a second of it.
        allowance.tick(100_000);
        assert!(allowance.room(CAP as u32) && !allowance.room(CAP as u32 + 1));
        assert_eq!(allowance.sent(), CAP);
    }

    #[test]
    fn the_launcher_may_lower_it_only() {
        let mut allowance = Allowance::new(0);
        allowance.lower(Some(CAP * 10));
        assert_eq!(allowance.rate(), CAP);
        allowance.lower(Some(1000));
        assert_eq!(allowance.rate(), 1000);
        // Lowered, it holds no more than its burst (four blocks).
        assert!(!allowance.room(BLOCK * 4 + 1));
        allowance.lower(None);
        assert_eq!(allowance.rate(), CAP);
        allowance.lower(Some(0));
        assert_eq!(allowance.rate(), CAP);
    }

    #[test]
    fn what_goes_outside_a_swarm_is_owed() {
        let mut allowance = Allowance::new(0);
        allowance.spend(CAP * 3);
        assert!(!allowance.clear());
        allowance.tick(1000);
        assert!(!allowance.clear());
        allowance.tick(2000);
        assert!(allowance.clear() && !allowance.room(1));
    }
}
