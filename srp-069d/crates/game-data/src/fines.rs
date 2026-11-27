//! The leaving-match fine: `gameplay/fines/leaving_match_fine.options` (volume 00, PROVEN data): `charges.money` 1000,
//! `next_fine_multiplier` 1, `ban_time` (seconds of queue block by the number of leaves that day). The inactivity fine
//! (`inactivity_fine.options`) has the same charges plus `give_fine_time_seconds` 300 / `first_warning_time_seconds` 180;
//! it needs a server-side inactivity source and is not applied. Rules: docs/notes/online.md (leave / AFK penalty).

/// `charges.money`.
pub const LEAVING_FINE_MONEY: u32 = 1000;
/// `charges.next_fine_multiplier` (1: the fine does not grow with repeats).
pub const NEXT_FINE_MULTIPLIER: u32 = 1;
/// `charges.ban_time[*].amount`: the queue block of the 1st, 2nd, ... leave of a day (the last entry for any later one).
pub const BAN_TIME_SECS: [u32; 10] = [0, 180, 180, 300, 300, 480, 480, 600, 600, 900];

/// Silver fine of a leave: `money` x the player's equipment tier (1..=5; OFFICIAL 0.56b "depends only on the maximum
/// equipment tier"; the product is a CHOICE that fits the player reports of ~2000 at a low and ~5000 at a high tier) x
/// `next_fine_multiplier`^(earlier leaves today).
pub fn leaving_fine(tier: u8, earlier_today: u32) -> u32 {
    let tier = u32::from(tier.clamp(1, 5));
    let mult = NEXT_FINE_MULTIPLIER.saturating_pow(earlier_today.min(16));
    LEAVING_FINE_MONEY.saturating_mul(tier).saturating_mul(mult)
}

/// Queue block of the leave that follows `earlier_today` leaves.
pub fn ban_secs(earlier_today: u32) -> u32 {
    BAN_TIME_SECS[(earlier_today as usize).min(BAN_TIME_SECS.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fine_by_tier_and_the_ban_ladder() {
        assert_eq!((leaving_fine(0, 0), leaving_fine(2, 0), leaving_fine(5, 3), leaving_fine(9, 0)), (1000, 2000, 5000, 5000));
        assert_eq!((ban_secs(0), ban_secs(1), ban_secs(3), ban_secs(9), ban_secs(50)), (0, 180, 300, 900, 900));
    }
}
