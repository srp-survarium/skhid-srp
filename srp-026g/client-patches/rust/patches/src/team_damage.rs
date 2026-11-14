//! Patch 63: team damage turned back on its dealer past a threshold, as
//! 0.28a2, 0.28d0 and 0.31a2 had it (docs/notes/team-damage.md; the hooks
//! are `team_damage.toml`'s, the game's side vostok::team_damage).
//!
//! - **The rule:** srp's block after the match's options holds tag 1, the
//!   three numbers (`srp_validate::team_damage::Rule`), in a public match of
//!   two teams only: the server decides it with the anti-AFK's own test (not
//!   in a private room, against bots or in a free-for-all). Without it the
//!   patch does nothing.
//! - **The counts:** what each place's hits took off other places' players,
//!   all of it and of it its teammates' (the pain a hit took, 1 a full bar:
//!   hit_impl's loss for the player's on_hit, 0x2D9EA9); only a hit with a
//!   source, of another place than its victim's (0.28a2's 0x5AA41B,
//!   0x5AA62F: not a fall, an anomaly, one's own grenade). They never go
//!   down: per match.
//! - **The turn:** before a teammate's hit is dealt (hit_impl past its tests
//!   of a dead and a dying victim), its dealer is tested as 0.28a2's
//!   0x5AA850 does: teammates' / all > `max_ratio` and teammates' >
//!   `max_amount`. Past both the victim takes nothing and the dealer takes
//!   the same hit, its damage times `return_ratio`, through his own
//!   hit_impl: his armour, his place, so it is counted for nobody and turned
//!   no more, and a death by it is a suicide.
//! - **The world:** the counts are the world's: each world saved has them
//!   after its effects (the record, when any place dealt any), each world
//!   loaded has its own (none without the record), so a rollback, a late
//!   joiner's world and a replay's seek count as the games that played.

#![forbid(unsafe_code)]

use srp_validate::team_damage::{Counts, Dealt, Rule, TAG};

use crate::vostok::parse::Reader;
use crate::vostok::team_damage::{EffectsReader, EffectsWriter, HitStart, PainLost};
use crate::vostok::types::Place;
use crate::vostok::{Context, Flow, GameCell, NoExit, OptionsRead, TeamDamageOnHitStartExit};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";

/// The match's team damage, from its options' read to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Match {
    /// No rule in the options: nothing is counted or turned.
    Off,
    /// The rule, and what each place dealt in the world being simulated.
    On { rule: Rule, counts: Counts },
}

/// What changes the match's team damage.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// A match's options were read (a match's or a replay's).
    Options(Option<Rule>),
    /// A hit of `place` took `lost` off another place's player.
    Lost { place: Place, teammate: bool, lost: f32 },
    /// A world was loaded with these counts.
    Loaded(Counts),
}

fn step(state: Match, event: Event) -> Match {
    match (state, event) {
        (_, Event::Options(rule)) => rule.map_or(Match::Off, |rule| Match::On { rule, counts: Counts::NONE }),
        (Match::Off, Event::Lost { .. } | Event::Loaded(_)) => Match::Off,
        (Match::On { rule, counts }, Event::Lost { place, teammate, lost }) => {
            let dealt = counts.get(place.index());
            // A sum that is no number is never one: the counts stay.
            let counts = dealt.with(lost, teammate).map_or(counts, |dealt| counts.with(place.index(), dealt));
            Match::On { rule, counts }
        }
        (Match::On { rule, .. }, Event::Loaded(counts)) => Match::On { rule, counts },
    }
}

static MATCH: GameCell<Match> = GameCell::new(Match::Off);

/// The rule in srp's block (the bytes after the options' squads): its
/// first entry, tag 1, when it is a rule.
pub fn rule_in(mut rest: Reader) -> Option<Rule> {
    if rest.array::<4>() != Some(SRPX) {
        return None;
    }
    let (tag, len) = (rest.u8()?, rest.u8()?);
    let entry = rest.take(usize::from(len))?;
    if tag == TAG { Rule::of_entry(entry) } else { None }
}

/// Whether a place that dealt `dealt` has its hits on teammates turned on
/// it (0.28a2's 0x5AA850).
fn turned(rule: Rule, dealt: Dealt) -> bool {
    dealt.team() > rule.max_amount() && dealt.team() / dealt.all() > rule.max_ratio()
}

/// hit_impl past its tests of a dead and a dying victim.
pub fn on_hit_start(ctx: &mut Context) -> Flow<TeamDamageOnHitStartExit> {
    let Match::On { rule, counts } = MATCH.get(ctx.thread()) else { return Flow::Continue };
    let Some(hit) = HitStart::at(ctx) else { return Flow::Continue };
    let Some(dealer) = hit.dealer() else { return Flow::Continue };
    if !dealer.ally() || !turned(rule, counts.get(dealer.place().index())) {
        return Flow::Continue;
    }
    hit.turn(dealer, rule.return_ratio());
    Flow::Exit(TeamDamageOnHitStartExit::Skipped)
}

/// hit_impl once the hit took pain.
pub fn on_pain_lost(ctx: &mut Context) -> Flow<NoExit> {
    if matches!(MATCH.get(ctx.thread()), Match::On { .. })
        && let Some(PainLost { dealer, lost }) = PainLost::at(ctx)
    {
        let event = Event::Lost { place: dealer.place(), teammate: dealer.ally(), lost };
        drop(dealer);
        MATCH.update(ctx.thread(), |state| step(state, event));
    }
    Flow::Continue
}

/// game_world_core::serialize after the effects: the counts' record.
pub fn on_world_save(ctx: &mut Context) -> Flow<NoExit> {
    let Match::On { counts, .. } = MATCH.get(ctx.thread()) else { return Flow::Continue };
    let mut record = [0; Counts::MAX_RECORD];
    let len = counts.record(&mut record);
    if let (Some(bytes), Some(mut writer)) = (record.get(..len).filter(|bytes| !bytes.is_empty()), EffectsWriter::at(ctx)) {
        writer.append(bytes);
    }
    Flow::Continue
}

/// game_world_core::deserialize after the effects: the world's counts.
pub fn on_world_load(ctx: &mut Context) -> Flow<NoExit> {
    if !matches!(MATCH.get(ctx.thread()), Match::On { .. }) {
        return Flow::Continue;
    }
    let counts = EffectsReader::at(ctx)
        .and_then(|mut reader| {
            reader.decode(|rest| {
                let bytes = rest.remaining();
                let counts = Counts::read(bytes, Place::COUNT).ok()?;
                rest.take(bytes.len())?;
                Some(counts)
            })
        })
        .unwrap_or(Counts::NONE);
    MATCH.update(ctx.thread(), |state| step(state, Event::Loaded(counts)));
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    let rule = OptionsRead::at_static_info(ctx).and_then(|read| rule_in(read.rest()));
    MATCH.update(ctx.thread(), |state| step(state, Event::Options(rule)));
    Flow::Continue
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    let rule = OptionsRead::at_replay(ctx).and_then(|read| rule_in(read.rest()));
    MATCH.update(ctx.thread(), |state| step(state, Event::Options(rule)));
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(bytes: &[u8]) -> Option<Rule> {
        rule_in(Reader::new(bytes))
    }

    fn lost(place: u32, teammate: bool, lost: f32) -> Event {
        Event::Lost { place: Place::new(place).unwrap(), teammate, lost }
    }

    #[test]
    fn the_rule_is_the_blocks_first_entry() {
        let entry = [&b"SRPX\x01\x0c"[..], &Rule::ORIGINAL.entry()].concat();
        assert_eq!(rule(&entry), Some(Rule::ORIGINAL));
        assert_eq!(rule(&[&entry[..], b"\x02\x01\x01"].concat()), Some(Rule::ORIGINAL));
        assert_eq!(rule(b"SRPX\x02\x01\x01"), None);
        assert_eq!(rule(b""), None);
        assert_eq!(rule(&entry[..entry.len() - 1]), None);
    }

    #[test]
    fn past_both_thresholds_a_dealers_hits_on_teammates_turn() {
        let rule = Rule::ORIGINAL;
        let dealt = |all: f32, team: f32| Dealt::of_bits(all.to_bits(), team.to_bits()).unwrap();
        assert!(!turned(rule, Dealt::NONE));
        // Teammates' 3.0 is not more than 3.
        assert!(!turned(rule, dealt(3.0, 3.0)));
        assert!(turned(rule, dealt(3.5, 3.5)));
        // 67 % of all is not more than 0.67; 4 of 5 is.
        assert!(!turned(rule, dealt(6.0, 4.0)));
        assert!(turned(rule, dealt(5.0, 4.0)));
    }

    #[test]
    fn the_counts_follow_the_match_and_its_worlds() {
        let mut state = step(Match::On { rule: Rule::ORIGINAL, counts: Counts::NONE }, Event::Options(None));
        assert_eq!(state, Match::Off);
        state = step(state, lost(1, true, 1.0));
        assert_eq!(state, Match::Off);
        state = step(state, Event::Options(Some(Rule::ORIGINAL)));
        for event in [lost(1, true, 2.0), lost(1, false, 0.5), lost(3, true, 1.0)] {
            state = step(state, event);
        }
        let Match::On { counts, .. } = state else { panic!("on") };
        assert_eq!((counts.get(1).all(), counts.get(1).team()), (2.5, 2.0));
        assert_eq!((counts.get(3).all(), counts.get(3).team()), (1.0, 1.0));
        // A world loaded has its own counts: an earlier one's, or none.
        let earlier = Counts::NONE.with(1, Dealt::NONE.with(2.0, true).unwrap());
        assert_eq!(step(state, Event::Loaded(earlier)), Match::On { rule: Rule::ORIGINAL, counts: earlier });
        // A new match's options start afresh.
        assert_eq!(
            step(state, Event::Options(Some(Rule::ORIGINAL))),
            Match::On { rule: Rule::ORIGINAL, counts: Counts::NONE }
        );
    }
}
