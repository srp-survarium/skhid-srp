//! Patch 180: Return to Sender (422, technical-skills.md). A player whose current kit has the
//! skill throws a live enemy grenade near it back along its view, with patch 85's pickup input
//! (Use's press): instant, no animation, the grenade leaving from its eye as from a throw, its fuse
//! running on. The command (`technical_skills::throw_back`) is played in every game at set_input,
//! from the grenades' saved state alone; the kill stays the original thrower's.
//!
//! - every game (`technical.throw_back`): a command from a player with the skill takes the nearest
//!   live enemy grenade in reach and in sight and throws it;
//! - this game: the prompt ([`aim`], composed from artefact_carry's on_hud, before patch 78's and
//!   85's own targets) and Use's press sent as the command ([`on_input_end`], composed from
//!   artefact_carry's on_input_end after patch 78's chord step and before patch 85's).
#![forbid(unsafe_code)]

use technical_skills::throw_back;
use weapon_pickup::{CHORD, USE};

use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::technical_return_to_sender::{self as native, NameKey, Played};
use crate::vostok::world::Player;
use crate::vostok::{ActionMask, Context, Flow, GameCell, InputHandler, NoExit, Thread};

/// set_input past its early-outs, in every game: a throw-back command is played.
pub fn on_input(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((place, time, played)) = native::input(ctx) {
        let what = match played {
            Played::NoSkill => "refused: no Return to Sender in its kit",
            Played::NoGrenade => "found no live enemy grenade in reach",
            Played::Thrown { .. } => "threw back the grenade of place ",
        };
        let owner = match played {
            Played::Thrown { owner } => Dec(u32::from(owner)),
            Played::NoSkill | Played::NoGrenade => Text(""),
        };
        log::line(&[Text("return_to_sender: place "), Dec(u32::from(place)), Text(" at "), Dec(time), Text(" ms "),
                    Text(what), owner]);
    }
    Flow::Continue
}

/// What the prompt shows: "Pick up <grenade> ({USE})" (`st_weapon_pick_up`, patch 85's line), its
/// name key empty when the stack's config has none.
#[derive(Clone, Copy)]
pub struct Prompt(pub NameKey);

/// The local player's offer, set by the prompt every frame and read at the input builder's end.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Offer {
    Nothing,
    /// A grenade to take; `kit` the current kit's modifier 47, what the server's check reads.
    Grenade { kit: f32 },
}

static OFFER: GameCell<Offer> = GameCell::new(Offer::Nothing);

/// The prompt (composed from artefact_carry's on_hud, first: a live grenade goes before a
/// teammate's artefact or a dropped gun): `viewer` the local player, none when there is none.
pub fn aim(ctx: &Context<'_>, viewer: Option<&Player<'_>>) -> Option<Prompt> {
    let found = viewer.and_then(native::offer);
    OFFER.set(ctx.thread(), found.map_or(Offer::Nothing, |(kit, _)| Offer::Grenade { kit }));
    found.map(|(_, target)| Prompt(native::name(target)))
}

/// Use at the input builder, as patch 85 keeps it: up, held (the game's), or a press sent as the
/// command, held still (the game sees no Use until it is let go).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UseKey {
    Up,
    Held,
    Sent,
}

static USE_KEY: GameCell<UseKey> = GameCell::new(UseKey::Up);

/// One input's Use: on its down edge while a grenade is offered (and the input carries no other
/// chord command), the actions become the command if the server's check passes it; then the press
/// stays the command's until Use is let go.
fn press(key: UseKey, offer: Offer, actions: u32) -> (UseKey, u32) {
    if actions & USE == 0 {
        return (UseKey::Up, actions);
    }
    match (key, offer) {
        (UseKey::Sent, _) => (UseKey::Sent, actions & !USE),
        (UseKey::Up, Offer::Grenade { kit }) if actions & CHORD == 0 => {
            let sent = throw_back::encode(actions);
            match throw_back::sane(sent, [kit]) {
                true => (UseKey::Sent, sent),
                false => (UseKey::Held, actions),
            }
        }
        (UseKey::Up | UseKey::Held, _) => (UseKey::Held, actions),
    }
}

/// The input builder's end (composed from artefact_carry's on_input_end, after patch 78's chord
/// step, before patch 85's): Use's press on an offered grenade is sent as the throw-back. Patch 85
/// then sees no Use (consumed) and sends no pickup.
pub fn on_input_end(handler: &mut InputHandler<'_>, thread: &Thread) {
    let actions = handler.actions().bits();
    let (key, sent) = press(USE_KEY.get(thread), OFFER.get(thread), actions);
    USE_KEY.set(thread, key);
    if sent != actions {
        handler.set_actions(ActionMask::from_bits(sent));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKILL: f32 = technical_skills::weight(8) as f32;

    #[test]
    fn a_press_on_a_grenade_is_sent_once_and_held() {
        let offer = Offer::Grenade { kit: SKILL };
        let (key, sent) = press(UseKey::Up, offer, USE | 0x1);
        assert_eq!(key, UseKey::Sent);
        assert!(throw_back::command(sent) && sent & USE == 0 && sent & 0x1 == 0x1);
        // Held: Use stays consumed, no second command.
        let (key, held) = press(key, offer, USE | 0x1);
        assert_eq!((key, held), (UseKey::Sent, 0x1));
        assert_eq!(press(key, offer, 0x1), (UseKey::Up, 0x1), "let go");
    }

    #[test]
    fn no_command_without_an_offer_or_over_another_chord() {
        assert_eq!(press(UseKey::Up, Offer::Nothing, USE), (UseKey::Held, USE));
        let offer = Offer::Grenade { kit: SKILL };
        assert_eq!(press(UseKey::Held, offer, USE), (UseKey::Held, USE), "pressed before the offer");
        let carry = USE | CHORD | 1 << 14;
        assert_eq!(press(UseKey::Up, offer, carry), (UseKey::Held, carry), "patch 78's command");
        // The client's twin of the server's check: a kit without the skill sends nothing.
        assert_eq!(press(UseKey::Up, Offer::Grenade { kit: 0.0 }, USE), (UseKey::Held, USE));
    }
}
