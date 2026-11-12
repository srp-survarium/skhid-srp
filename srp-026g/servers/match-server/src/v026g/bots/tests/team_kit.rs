//! The bots' upgrades for one team only (`SRP_BENCH_UPGRADES_ONLY=<team>`), and what the carriers' deaths say about them.
//!
//! `SRP_BOT_UPGRADES` is a process-wide switch (`loadouts::upgrades_on`): both teams' bots wear the upgrades or neither does, so a
//! head to head cannot tell whether an effect of the upgrades is the own side's (the carrier's armour, weight and stamina) or the
//! enemy's (the killer's weapon with the skills' reload and aim time, `guns::arms_of`). With this setting the team named keeps the
//! process's switch and the other team is built as if it were off: its profiles (the items with their modifications), its kits'
//! arms (the weapons the client fires), and the stand-in's stamina rates and ammo of its bodies. A thread-local scope of
//! [`KitSwitches`] does it around each place that builds one of those for a team (`Bot::arm`, the roster, the stand-in's spawn and
//! weapon switch, the first bodies); the game's own tables (`shot_reach`, the range tables by tier) keep the process's switch.

use survarium::v026g::loadouts::{KitScope, KitSwitches, new_kit};

use super::*;

/// The team that keeps the process's upgrades (`SRP_BENCH_UPGRADES_ONLY=0` or `1`); the other has none. Unset: both as the process.
fn only() -> Option<u8> {
    static ONLY: std::sync::OnceLock<Option<u8>> = std::sync::OnceLock::new();
    *ONLY.get_or_init(|| match bench_settings().var("SRP_BENCH_UPGRADES_ONLY").as_deref() {
        Err(_) | Ok("both") => None,
        Ok("0") => Some(0),
        Ok("1") => Some(1),
        Ok(other) => panic!("SRP_BENCH_UPGRADES_ONLY={other:?}: 0, 1 or both"),
    })
}

/// Whether the bots of `team` have the upgrades in this run: the process's switch, unless the other team is the one that keeps them.
pub(crate) fn has_upgrades(team: u8) -> bool {
    survarium::v026g::loadouts::KitSwitches::current().upgrades && only().is_none_or(|kept| kept == team & 1)
}

/// The kit switches `team`'s things are built under until the guard is dropped: the upgrades off for the team that does not keep
/// them, nothing changed otherwise. Hold it in a variable (`let _kit = scope(team);`).
#[must_use = "the switches are the scope's only while it is held"]
pub(crate) fn scope(team: u8) -> Option<KitScope> {
    (!has_upgrades(team)).then(|| KitSwitches { kit: new_kit(), upgrades: false }.scope())
}

/// The carriers' deaths by the upgrade state of the killer's team and of the carrier's, with the carrier's bar and weight cost at
/// the death: a mechanism for "carriers killed" (C14: own side or enemy side).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CarrierDeaths {
    /// `[killer has upgrades][carrier has upgrades]`.
    pub(crate) by_state: [[u32; 2]; 2],
    /// Sums over the deaths: the carrier's stamina bar, the deaths with the low flag set, the weight cost `t` of his kit, and the
    /// deaths of a carrier whose kit is overburdened (cannot sprint at all).
    pub(crate) bar: f32,
    pub(crate) low: u32,
    pub(crate) weight_cost: f32,
    pub(crate) overburdened: u32,
}

impl CarrierDeaths {
    /// One death: `killer` shot the carrier `body`.
    pub(crate) fn add(&mut self, killer_team: u8, body: &Body) {
        self.by_state[usize::from(has_upgrades(killer_team))][usize::from(has_upgrades(body.team))] += 1;
        self.bar += body.stamina.value();
        self.low += u32::from(body.stamina.low());
        self.weight_cost += body.stamina.weight_cost();
        self.overburdened += u32::from(!body.stamina.can_sprint_by_weight());
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        let n: u32 = self.by_state.iter().flatten().sum();
        let mean = |x: f32| if n == 0 { 0.0 } else { x / n as f32 };
        serde_json::json!({
            "killer_up_carrier_up": self.by_state[1][1],
            "killer_up_carrier_plain": self.by_state[1][0],
            "killer_plain_carrier_up": self.by_state[0][1],
            "killer_plain_carrier_plain": self.by_state[0][0],
            "deaths": n,
            "bar_sum": self.bar,
            "bar_mean": mean(self.bar),
            "low_flag": self.low,
            "weight_cost_sum": self.weight_cost,
            "weight_cost_mean": mean(self.weight_cost),
            "overburdened": self.overburdened,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_set_both_teams_have_what_the_process_has() {
        // The environment of the tests does not set it.
        assert_eq!(only(), None);
        let process = KitSwitches::current().upgrades;
        assert_eq!((has_upgrades(0), has_upgrades(1)), (process, process));
        assert_eq!(scope(0).is_none(), process);
    }

    #[test]
    fn a_carrier_death_is_counted_by_the_two_upgrade_states() {
        let mut deaths = CarrierDeaths::default();
        let game = Game::new(soak_world(), &[0, 1], 1, None, None);
        deaths.add(0, &game.bodies[1]);
        let process = KitSwitches::current().upgrades;
        let json = deaths.json();
        assert_eq!(json["deaths"], 1);
        assert_eq!(json[if process { "killer_up_carrier_up" } else { "killer_plain_carrier_plain" }], 1);
    }
}
