//! The owner counts of the friend hold (a bot that would fire holds for a teammate in its line, or touching its front, and
//! steps to free the shot): by the bot's team, the ticks a bot held fire that it would otherwise have fired ("ready"), by how
//! the teammate blocked it (the line, or touch alone); the step taken (across the line, or back; taken, or the ground refused
//! it); the hold spells (a run of consecutive ready ticks of one kind), those longer than 1.5 s, and the spells after which the
//! bot fired within a second ("freed"). A spell ends when the hold ends, whatever the reason (a shot, a lost target, a mate
//! moved, a death).

use super::*;
use crate::v026g::bots::friends::BlockKind;

/// A spell longer than this is long, ms.
const LONG_MS: u32 = 1_500;

/// After a spell ends, a shot within this frees it, ms.
const FREED_MS: u32 = 1_000;

const KINDS: usize = 2;

fn index(kind: BlockKind) -> usize {
    match kind {
        BlockKind::Line => 0,
        BlockKind::Touch => 1,
    }
}

#[derive(Clone, Copy, Default)]
struct Kind {
    /// Ticks a bot held fire it was ready to fire, bot-ticks.
    ready_ticks: u64,
    /// ... of which the step was: across and taken, across and refused, back and taken, back and refused, none.
    steps: [u64; 5],
    spells: u64,
    spell_ms: u64,
    long_spells: u64,
    freed: u64,
}

/// A spell running: its kind, when it began, and when it was last seen.
#[derive(Clone, Copy)]
struct Spell {
    kind: BlockKind,
    began: u32,
    last: u32,
}

/// A spell that ended, waiting for a shot within [`FREED_MS`].
#[derive(Clone, Copy)]
struct Ended {
    kind: BlockKind,
    until: u32,
    shot_then: Option<u32>,
}

pub(super) struct FriendHoldCounts {
    by_team: [[Kind; KINDS]; 2],
    spell: Vec<Option<Spell>>,
    ended: Vec<Option<Ended>>,
}

impl FriendHoldCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { by_team: Default::default(), spell: vec![None; n], ended: vec![None; n] }
    }
}

impl Topic for FriendHoldCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, game: &Game) {
        let now = game.tick;
        for (place, bot) in bots.bots.iter().enumerate() {
            let Some(bot) = bot.as_ref().filter(|_| place < self.spell.len()) else { continue };
            let team = usize::from(bot.team & 1);
            let shot = bot.reflex.last_shot_ms();
            if let Some(ended) = self.ended[place] {
                if shot != ended.shot_then {
                    self.by_team[team][index(ended.kind)].freed += 1;
                    self.ended[place] = None;
                } else if now > ended.until {
                    self.ended[place] = None;
                }
            }
            let held = bot.reflex.held_for().filter(|h| h.ready);
            match (held, self.spell[place]) {
                (Some(h), Some(spell)) if spell.kind == h.kind => self.spell[place] = Some(Spell { last: now, ..spell }),
                (held, Some(spell)) => {
                    let k = &mut self.by_team[team][index(spell.kind)];
                    let ms = u64::from(spell.last.saturating_sub(spell.began));
                    k.spells += 1;
                    k.spell_ms += ms;
                    k.long_spells += u64::from(ms > u64::from(LONG_MS));
                    self.ended[place] = Some(Ended { kind: spell.kind, until: now + FREED_MS, shot_then: shot });
                    self.spell[place] = held.map(|h| Spell { kind: h.kind, began: now, last: now });
                }
                (Some(h), None) => self.spell[place] = Some(Spell { kind: h.kind, began: now, last: now }),
                (None, None) => {}
            }
            if let Some(h) = held {
                let k = &mut self.by_team[team][index(h.kind)];
                k.ready_ticks += 1;
                let step = match bot.bench_friend_step {
                    0 => 4,
                    s => usize::from(s) - 1,
                };
                k.steps[step] += 1;
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("friend_hold", self.part())]
    }
}

impl FriendHoldCounts {
    fn part(&self) -> Part {
        let names = ["line", "touch"];
        let team = |t: usize| {
            let kinds: serde_json::Map<String, serde_json::Value> = names
                .iter()
                .zip(&self.by_team[t])
                .map(|(name, k)| {
                    (
                        (*name).to_owned(),
                        serde_json::json!({
                            "ready_ticks": k.ready_ticks,
                            "step_across_taken": k.steps[0],
                            "step_across_refused": k.steps[1],
                            "step_back_taken": k.steps[2],
                            "step_back_refused": k.steps[3],
                            "no_step": k.steps[4],
                            "spells": k.spells,
                            "spell_ms": k.spell_ms,
                            "long_spells": k.long_spells,
                            "freed": k.freed,
                        }),
                    )
                })
                .collect();
            serde_json::Value::Object(kinds)
        };
        let json = serde_json::json!({ "friend_hold": { "by_team": { "0": team(0), "1": team(1) } } });
        let line = |t: usize| {
            names
                .iter()
                .zip(&self.by_team[t])
                .map(|(name, k)| {
                    format!(
                        "{name}: {} ready ticks, {} spells ({} long, mean {:.0} ms), {} freed by a shot in 1 s, steps across {}/{} back {}/{} (taken/refused)",
                        k.ready_ticks,
                        k.spells,
                        k.long_spells,
                        if k.spells == 0 { 0.0 } else { k.spell_ms as f64 / k.spells as f64 },
                        k.freed,
                        k.steps[0],
                        k.steps[1],
                        k.steps[2],
                        k.steps[3],
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        Part::new(json, format!("owner-test friend hold: team 0 [{}]; team 1 [{}]", line(0), line(1)))
    }
}
