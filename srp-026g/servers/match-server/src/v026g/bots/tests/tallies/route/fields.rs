//! The bench's anomaly fields (`standin_fields.rs`, `SRP_BENCH_LIVE_FIELDS`) and what the bots did about them: the
//! live zones' target counts, next to the baked zones' (`zones.rs`). All zero when the generator is off.
//!
//! - `field_seen`, `field_lag_ms_p50` (with `field_lag_n`): fields made; from a field's making to the world that first
//!   listed it;
//! - `field_entries_on` / `field_entries_charging`: bot entries onto a field's floors while it hurts / while it
//!   charges; `field_caught_in`: bots inside when it began to hurt;
//! - `field_bot_s_on`: bot-seconds on a hurting field's floors; `field_escape_s_p50`: from a bot's first tick on a
//!   hurting field to its first off its floors;
//! - `field_deaths`, `field_hp`: deaths and health (of 1.0 a body) the fields took;
//! - `field_held_s`: field-seconds spent holding (nobody takes an artefact in a bot match); `field_shut_share`
//!   (max, mean): the share of the grid's floors under a field.

use super::*;

use survarium::v026g::match_msgs::anomaly_fields::FieldId;

use super::super::super::standin_fields::FieldStats;

#[derive(Default)]
pub(super) struct Fields {
    /// Each body's field at the last tick: which, and whether it hurt.
    was: Vec<Option<(FieldId, bool)>>,
    /// Since when each body has been on a hurting field.
    on_since: Vec<Option<u32>>,
    entries_on: u32,
    entries_charging: u32,
    caught_in: u32,
    bot_ms_on: u64,
    escape_ms: Vec<u32>,
    /// The generator's side, read at the end of the match ([`RouteTopic::finish`]).
    stats: FieldStats,
}

impl Fields {
    pub(super) fn new(bodies: usize) -> Self {
        Self { was: vec![None; bodies], on_since: vec![None; bodies], ..Self::default() }
    }
}

fn p50(values: &[u32]) -> u32 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len() / 2).copied().unwrap_or(0)
}

impl RouteTopic for Fields {
    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        if at.bots.bots[at.place].is_none() {
            return;
        }
        let now = at.field;
        let was = std::mem::replace(&mut self.was[at.place], now);
        match (was, now) {
            (_, Some((id, hurts))) if was.is_none_or(|(w, _)| w != id) => {
                if hurts {
                    self.entries_on += 1;
                } else {
                    self.entries_charging += 1;
                }
            }
            (Some((w, false)), Some((id, true))) if w == id => self.caught_in += 1,
            _ => {}
        }
        let hurting = now.is_some_and(|(_, hurts)| hurts);
        if hurting {
            self.bot_ms_on += u64::from(TICK_MS);
            self.on_since[at.place].get_or_insert(at.time);
        } else if let Some(since) = self.on_since[at.place].take() {
            self.escape_ms.push(at.time.saturating_sub(since));
        }
    }

    fn finish(&mut self, game: &Game) {
        self.stats = game.fields.stats.clone();
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let stats = &self.stats;
        let (shut_max, shut_mean) = (stats.shut.0, if stats.shut.2 > 0 { stats.shut.1 / f64::from(stats.shut.2) } else { 0.0 });
        line.tail.push(format!(
            "field_seen {} field_lag_ms_p50 {} field_lag_n {} field_deaths {} field_hp {:.2} field_held_s {} field_shut_share_max {:.3} field_shut_share_mean {:.3}",
            stats.made,
            p50(&stats.lag_ms),
            stats.lag_ms.len(),
            stats.deaths,
            stats.hp,
            stats.held_ms / 1000,
            shut_max,
            shut_mean
        ));
        line.tail.push(format!(
            "field_entries_on {} field_entries_charging {} field_caught_in {} field_bot_s_on {} field_escape_s_p50 {:.1} field_escape_n {}",
            self.entries_on,
            self.entries_charging,
            self.caught_in,
            self.bot_ms_on / 1000,
            f64::from(p50(&self.escape_ms)) / 1000.0,
            self.escape_ms.len()
        ));
    }
}
