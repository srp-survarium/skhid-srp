//! The runner's reach into the bots: the poll hands it the frame, a think asks it what a scripted bot does.

use super::run::Drive;
use super::step::Gait;
use super::{Frame, Link};
use crate::v026g::bots::plays::{Go, Intent, Look, Pace};
use crate::v026g::bots::view::{Clock, MatchView, PlaceView};
use crate::v026g::bots::{Bot, Bots, Note};
use crate::v026g::drive::key;

impl Bots {
    /// The end of a poll: the runner sees the match as it stands and what the bots said in it.
    pub(in crate::v026g::bots) fn client_tests_tick(&mut self, clock: &Clock, view: &MatchView, notes: &[Note]) {
        if let Some(link) = self.client_tests.as_mut() {
            link.tick(&Frame::of_view(view, clock.playing, notes));
        }
    }

    /// The lines the runner has for the owner since the last call.
    pub fn client_test_says(&mut self) -> Vec<String> {
        self.client_tests.as_mut().map(Link::take_says).unwrap_or_default()
    }
}

/// A think of `bot`: when the runner scripts it, its intent is the script's and its play is skipped (true). Otherwise it
/// plays as ever.
pub fn script_bot(link: Option<&Link>, bot: &mut Bot, me: &PlaceView) -> bool {
    let drive = link.and_then(|link| link.drive(bot.place));
    bot.scripted = drive.is_some();
    let Some(drive) = drive else {
        return false;
    };
    bot.intent = match drive {
        Drive::Idle => Intent {
            fire: false,
            ..Intent::stay()
        },
        Drive::Stand { crouch } => Intent {
            crouch,
            ..Intent::stay()
        },
        Drive::Go { at, within_m, gait, crouch } => Intent {
            go: Go::To { at, within_m },
            pace: match gait {
                Gait::Walk => Pace::Walk,
                Gait::Run => Pace::Run,
            },
            // Crouched once there, if it is to be.
            crouch: crouch && me.feet.distance_xz(at) <= within_m,
            ..Intent::stay()
        },
        Drive::Throw { look, held } => Intent {
            look: Look::At(look),
            hold: if held { key::GRENADE } else { 0 },
            ..Intent::stay()
        },
    };
    true
}
