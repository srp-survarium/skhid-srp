//! The runner's reach into the bots: the poll hands it the frame, a think asks it what a scripted bot does.

use super::frame::{Body, BotView, Frame, Heard, Mind, Owner, Sight};
use super::run::Act;
use super::step::{Fight, Gait};
use super::time::At;
use super::Link;
use crate::v026g::bots::control::Driver;
use crate::v026g::bots::plays::{Go, Intent, Look, Pace};
use crate::v026g::bots::view::{Clock, MatchView, PlaceView};
use crate::v026g::bots::{Bot, Bots, Note};
use crate::v026g::drive::key;

fn body_of(p: &PlaceView) -> Body {
    Body {
        place: p.place,
        team: p.team,
        alive: p.alive,
        feet: p.feet,
        health: p.health,
        kills: p.kills,
        deaths: p.deaths,
        crouched: p.crouched(),
        keys: p.keys,
        yaw: p.yaw,
        pitch: p.pitch,
        speed: p.speed(),
    }
}

impl Bots {
    /// The match as the runner sees it: the bodies of the view, each bot's reflex, the batteries, what was said.
    fn client_test_frame(&self, clock: &Clock, view: &MatchView, notes: &[Note], pools: Option<crate::v026g::bots::pools::Pools>) -> Frame {
        let sight = self.tactics.as_ref().map(|tactics| Sight(tactics.world.sight.clone()));
        let owner = view.places.iter().find(|p| p.present && !p.bot).map(|p| Owner { body: body_of(p), pools });
        let bots = view
            .places
            .iter()
            .filter(|p| p.present && p.bot)
            .filter_map(|p| {
                let bot = self.bots.get(p.place)?.as_ref()?;
                Some(BotView {
                    body: body_of(p),
                    role: bot.role,
                    mind: Mind {
                        target: bot.reflex.target(),
                        gates: bot.reflex.gates(),
                        legs: bot.bench_legs_why,
                    },
                })
            })
            .collect();
        Frame {
            clock: At(view.clock),
            playing: clock.playing,
            owner,
            bots,
            batteries: view.batteries.clone(),
            heard: notes
                .iter()
                .map(|n| Heard {
                    place: n.place,
                    time: At(n.time),
                    text: n.text.clone(),
                })
                .collect(),
            sight,
        }
    }

    /// The end of a poll: the runner sees the match as it stands and what the bots said in it, and the bots it scripts are the
    /// script's from the next tick on (never a think late: this is where [`Bot::control`] is set).
    pub(in crate::v026g::bots) fn client_tests_tick(&mut self, clock: &Clock, view: &MatchView, notes: &[Note]) {
        let Some(mut link) = self.client_tests.take() else {
            return;
        };
        let frame = self.client_test_frame(clock, view, notes, link.owner_pools());
        link.tick(&frame);
        for bot in self.bots.iter_mut().flatten() {
            bot.control = match (bot.control, link.drive(bot.place)) {
                (Driver::Own | Driver::Scripted(_), Some(drive)) => Driver::Scripted(drive.grip),
                (Driver::Scripted(_), None) => Driver::Own,
                (other, _) => other,
            };
            // The script's intent is the body's from the next tick, not from the bot's next due think (200-400 ms).
            if let (Driver::Scripted(_), Some(me)) = (bot.control, view.place(bot.place).filter(|p| p.alive)) {
                script_bot(Some(&link), bot, me);
            }
        }
        self.client_tests = Some(link);
    }

    /// The lines the runner has for the owner since the last call.
    pub fn client_test_says(&mut self) -> Vec<String> {
        self.client_tests.as_mut().map(Link::take_says).unwrap_or_default()
    }
}

/// A think of `bot`: when the runner scripts it, its intent is the script's and its play is skipped (true). Otherwise it
/// plays as ever.
pub fn script_bot(link: Option<&Link>, bot: &mut Bot, me: &PlaceView) -> bool {
    let Some(drive) = link.and_then(|link| link.drive(bot.place)) else {
        return false;
    };
    let fire = matches!(drive.grip.fight, Fight::Free | Fight::Full);
    bot.intent = match drive.act {
        Act::Stand { crouch } => Intent {
            crouch,
            fire,
            ..Intent::stay()
        },
        Act::Face { look, crouch } => Intent {
            look: Look::At(look),
            crouch,
            fire,
            ..Intent::stay()
        },
        Act::Go { at, within_m, gait, crouch } => Intent {
            go: Go::To { at, within_m },
            pace: match gait {
                Gait::Walk => Pace::Walk,
                Gait::Run => Pace::Run,
            },
            // Crouched once there, if it is to be.
            crouch: crouch && me.feet.distance_xz(at) <= within_m,
            fire,
            ..Intent::stay()
        },
        Act::Throw { look, held } => Intent {
            look: Look::At(look),
            hold: if held { key::GRENADE } else { 0 },
            fire,
            ..Intent::stay()
        },
    };
    true
}
