//! The player's route to the game servers, in a download with a relay
//! (`updater_skhid.relay`, `srp-026g/deploy/relay/README.md`): the launcher's
//! "Relay" row (launcher patch 16: "Relay: auto", "on", "off", and a menu of
//! the three) shows it and changes it, the updater keeps it (`<Documents>\survarium\launcher_skhid.route`, beside user.cfg:
//! the Windows user's, as the language) and PLAY and the updater's HTTPS
//! follow it.
//!
//! PLAY's check of the routes ([`Check`]) takes the relay only when the
//! direct route really fails: several rounds of its probes over a few
//! seconds, more once a part of it has answered (loss, not a block), one more
//! when the last game played direct ([`PLAYED_FILE`]).

use crate::diagnosis::Via;

/// Beside user.cfg: the player's [`Choice`], one word.
pub const CHOICE_FILE: &str = "launcher_skhid.route";

/// Beside [`CHOICE_FILE`]: the route PLAY last started a game by after its
/// check ([`played`]), one word. Evidence for the next check, never a
/// shortcut past it.
pub const PLAYED_FILE: &str = "launcher_skhid.route_played";

/// Rounds of the direct route's probes that a check runs while nothing of
/// it answers.
pub const SILENT_ROUNDS: u8 = 3;
/// More rounds once a probe of the direct route has answered (the login
/// TCP, say, while a UDP probe was lost): packets are lost on the way, or a
/// part is blocked, and only more rounds tell which.
pub const HEARD_ROUNDS: u8 = 2;
/// One more round when the last game played direct.
pub const PLAYED_ROUNDS: u8 = 1;

fn first_word(text: &[u8]) -> Option<&[u8]> {
    text.split(|&byte| byte <= b' ').find(|word| !word.is_empty())
}

/// [`PLAYED_FILE`]'s text: its first word.
pub fn played(text: &[u8]) -> Option<Via> {
    match first_word(text)? {
        b"direct" => Some(Via::Direct),
        b"relay" => Some(Via::Relay),
        _ => None,
    }
}

pub fn played_word(via: Via) -> &'static str {
    match via {
        Via::Direct => "direct",
        Via::Relay => "relay",
    }
}

/// Windows' GeoID of Russia (`GetUserGeoID(GEOCLASS_NATION)`), where
/// providers block the server: its players start in [`Choice::Automatic`].
pub const RUSSIA: i32 = 203;

/// How PLAY and the updater reach the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Directly when the server answers so, else through the relay: checked
    /// before every game ([`Check`]).
    Automatic,
    /// Through the relay always, unprobed.
    Relay,
    /// Directly always: never through the relay.
    Direct,
}

impl Choice {
    /// Before the player has chosen: decided once, by the Windows region
    /// (not the language: Russian speakers elsewhere are not blocked).
    pub fn first(nation: i32) -> Self {
        if nation == RUSSIA {
            Self::Automatic
        } else {
            Self::Direct
        }
    }

    /// [`CHOICE_FILE`]'s text: its first word.
    pub fn parse(text: &[u8]) -> Option<Self> {
        match first_word(text)? {
            b"automatic" => Some(Self::Automatic),
            b"relay" => Some(Self::Relay),
            b"direct" => Some(Self::Direct),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Relay => "relay",
            Self::Direct => "direct",
        }
    }

    /// The launcher's message of the menu's row picked (`22`, `23`, `24`:
    /// auto, on, off; launcher patch 16).
    pub fn of_message(id: u8) -> Option<Self> {
        match id {
            0x22 => Some(Self::Automatic),
            0x23 => Some(Self::Relay),
            0x24 => Some(Self::Direct),
            _ => None,
        }
    }
}

/// What the check found (`relay_client::Selection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probed {
    Direct,
    Relay,
    Neither,
}

/// What one round of a route's three probes (login TCP, lobby UDP, match
/// UDP, together) found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    /// All three answered.
    Answered,
    /// Some answered and some did not.
    Partly,
    /// None answered (or the server's name was not found).
    Silent,
}

/// What the direct route's rounds have found while none answered whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// No probe of any round answered.
    Nothing,
    /// A probe answered in a round where another was lost.
    Part,
}

/// The check of the routes as it goes: the direct route's rounds, then the
/// relay's one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// `rounds` of the direct route have run, none answered whole. `last`:
    /// the route the last game played by.
    Direct { rounds: u8, heard: Heard, last: Option<Via> },
    /// The direct route failed its rounds.
    Relay,
}

/// What a round leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Another round, of [`Check::via`]'s route.
    Probe(Check),
    Found(Probed),
}

impl Check {
    pub fn start(last: Option<Via>) -> Self {
        Self::Direct { rounds: 0, heard: Heard::Nothing, last }
    }

    /// The route whose round is to run.
    pub fn via(self) -> Via {
        match self {
            Self::Direct { .. } => Via::Direct,
            Self::Relay => Via::Relay,
        }
    }

    /// The one transition. Any round of the direct route that answers
    /// whole is direct, at once. While none does, its rounds go on:
    /// [`SILENT_ROUNDS`], [`HEARD_ROUNDS`] more once a part of it has
    /// answered, [`PLAYED_ROUNDS`] more when the last game played direct.
    /// Then the relay's one round decides.
    pub fn on(self, round: Round) -> Next {
        match (self, round) {
            (Self::Direct { .. }, Round::Answered) => Next::Found(Probed::Direct),
            (Self::Direct { rounds, heard, last }, round) => {
                let rounds = rounds + 1;
                let heard = if round == Round::Partly { Heard::Part } else { heard };
                let given = SILENT_ROUNDS
                    + if heard == Heard::Part { HEARD_ROUNDS } else { 0 }
                    + if last == Some(Via::Direct) { PLAYED_ROUNDS } else { 0 };
                Next::Probe(if rounds < given { Self::Direct { rounds, heard, last } } else { Self::Relay })
            }
            (Self::Relay, Round::Answered) => Next::Found(Probed::Relay),
            (Self::Relay, Round::Partly | Round::Silent) => Next::Found(Probed::Neither),
        }
    }
}

/// What the launcher's row shows: the choice and, in [`Choice::Automatic`],
/// the route the probe picked (`None` while it runs): "Relay: on", "off",
/// "auto", "auto (on)", "auto (off)".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Automatic(Option<Probed>),
    Relay,
    Direct,
}

/// What happens to the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The player picked a row of the menu.
    Chosen(Choice),
    /// A probe ended.
    Probed(Probed),
    /// The direct route was found blocked while the relay answers
    /// (`crate::diagnosis::Diagnosis::Blocked`): the row turns to auto by
    /// itself.
    Blocked,
}

/// A transition's result: what the row shows now, the choice to save (a
/// pick's, a block's), and whether a probe is to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub shown: Shown,
    pub save: Option<Choice>,
    pub probe: bool,
}

impl Shown {
    /// The row at the start, for the saved choice.
    pub fn start(choice: Choice) -> Step {
        Self::step(choice, None)
    }

    fn step(choice: Choice, save: Option<Choice>) -> Step {
        let shown = match choice {
            Choice::Automatic => Self::Automatic(None),
            Choice::Relay => Self::Relay,
            Choice::Direct => Self::Direct,
        };
        Step {
            shown,
            save,
            probe: choice == Choice::Automatic,
        }
    }

    pub fn choice(self) -> Choice {
        match self {
            Self::Automatic(_) => Choice::Automatic,
            Self::Relay => Choice::Relay,
            Self::Direct => Choice::Direct,
        }
    }

    /// The one transition. A probe that ends after the player picked
    /// another row than Automatic is dropped; one that ends after they came
    /// back is taken (it is a few seconds old at most). A block moves the
    /// choice to Automatic, whose probe then picks the relay: saved and
    /// shown as a pick, the player's to change.
    pub fn on(self, event: Event) -> Step {
        match (self, event) {
            (_, Event::Chosen(choice)) => Self::step(choice, Some(choice)),
            (_, Event::Blocked) => Self::step(Choice::Automatic, Some(Choice::Automatic)),
            (Self::Automatic(None), Event::Probed(probed)) => Step {
                shown: Self::Automatic(Some(probed)),
                save: None,
                probe: false,
            },
            (shown, Event::Probed(_)) => Step {
                shown,
                save: None,
                probe: false,
            },
        }
    }

    /// The byte of the launcher's message `0A` (launcher patch 16): 1
    /// probing, 2 direct, 3 the relay, 4 neither answered (Automatic), 5 the
    /// relay, 6 direct (chosen).
    pub fn byte(self) -> u8 {
        match self {
            Self::Automatic(None) => 1,
            Self::Automatic(Some(Probed::Direct)) => 2,
            Self::Automatic(Some(Probed::Relay)) => 3,
            Self::Automatic(Some(Probed::Neither)) => 4,
            Self::Relay => 5,
            Self::Direct => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_choice_is_the_regions_and_the_file_is_one_word() {
        assert_eq!(Choice::first(RUSSIA), Choice::Automatic);
        assert_eq!(Choice::first(244), Choice::Direct); // United States
        assert_eq!(Choice::first(241), Choice::Direct); // Ukraine
        assert_eq!(Choice::first(0), Choice::Direct); // no region
        for choice in [Choice::Automatic, Choice::Relay, Choice::Direct] {
            assert_eq!(Choice::parse(choice.word().as_bytes()), Some(choice));
            let line = alloc::format!("{}\r\n", choice.word());
            assert_eq!(Choice::parse(line.as_bytes()), Some(choice));
        }
        for bad in ["", "\r\n", "Relay", "relays", "auto"] {
            assert_eq!(Choice::parse(bad.as_bytes()), None, "{bad}");
        }
    }

    /// The check's end for `rounds` (the last repeated for as long as it
    /// asks), and the rounds of the direct route and of the relay it ran.
    fn check(last: Option<Via>, rounds: &[Round]) -> (Probed, usize, usize) {
        let mut check = Check::start(last);
        let (mut direct, mut relay) = (0, 0);
        let mut rounds = rounds.iter().copied().chain(core::iter::repeat(*rounds.last().unwrap()));
        loop {
            match check.via() {
                Via::Direct => direct += 1,
                Via::Relay => relay += 1,
            }
            match check.on(rounds.next().unwrap()) {
                Next::Probe(next) => check = next,
                Next::Found(probed) => return (probed, direct, relay),
            }
        }
    }

    #[test]
    fn direct_that_answers_is_direct_at_once_and_the_relay_is_not_asked() {
        use Round::*;
        for last in [None, Some(Via::Direct), Some(Via::Relay)] {
            assert_eq!(check(last, &[Answered]), (Probed::Direct, 1, 0));
            // One lost UDP probe, or one silent round, then an answer.
            assert_eq!(check(last, &[Partly, Answered]), (Probed::Direct, 2, 0));
            assert_eq!(check(last, &[Silent, Answered]), (Probed::Direct, 2, 0));
            // In the last round given.
            assert_eq!(check(last, &[Silent, Silent, Answered]), (Probed::Direct, 3, 0));
            assert_eq!(check(last, &[Partly, Partly, Partly, Partly, Answered]), (Probed::Direct, 5, 0));
        }
    }

    #[test]
    fn the_relay_only_after_the_direct_rounds_and_only_when_it_answers() {
        use Round::*;
        // Nothing answers directly: three rounds, then the relay's one.
        assert_eq!(check(None, &[Silent, Silent, Silent, Answered]), (Probed::Relay, 3, 1));
        assert_eq!(check(Some(Via::Relay), &[Silent, Silent, Silent, Answered]), (Probed::Relay, 3, 1));
        // The login TCP answers and UDP never does (a block of UDP alone):
        // the relay still, after two rounds more.
        assert_eq!(check(None, &[Partly, Partly, Partly, Partly, Partly, Answered]), (Probed::Relay, 5, 1));
        // A part heard in the third round gives the two more as well.
        assert_eq!(check(None, &[Silent, Silent, Partly, Silent, Silent, Answered]), (Probed::Relay, 5, 1));
        // Nothing and no relay: neither, the relay asked once, whole or not at all.
        assert_eq!(check(None, &[Silent]), (Probed::Neither, 3, 1));
        assert_eq!(check(None, &[Partly]), (Probed::Neither, 5, 1));
    }

    #[test]
    fn a_game_played_direct_gives_the_direct_route_one_more_round() {
        use Round::*;
        let direct = Some(Via::Direct);
        assert_eq!(check(direct, &[Silent, Silent, Silent, Answered]), (Probed::Direct, 4, 0));
        assert_eq!(check(direct, &[Silent, Silent, Silent, Silent, Answered]), (Probed::Relay, 4, 1));
        assert_eq!(check(direct, &[Partly, Partly, Partly, Partly, Partly, Answered]), (Probed::Direct, 6, 0));
        assert_eq!(check(direct, &[Partly, Partly, Partly, Partly, Partly, Partly, Answered]), (Probed::Relay, 6, 1));
        // Not after one that played through the relay, nor before any.
        for last in [None, Some(Via::Relay)] {
            assert_eq!(check(last, &[Silent, Silent, Silent, Answered]), (Probed::Relay, 3, 1));
        }
    }

    #[test]
    fn a_check_is_bounded_whatever_the_rounds_find() {
        // Every run of rounds: six of the direct route at most, one of the relay.
        const MOST: u8 = SILENT_ROUNDS + HEARD_ROUNDS + PLAYED_ROUNDS;
        let all = [Round::Answered, Round::Partly, Round::Silent];
        for last in [None, Some(Via::Direct), Some(Via::Relay)] {
            let mut open = alloc::vec![Check::start(last)];
            for _ in 0..=MOST {
                open = open
                    .iter()
                    .flat_map(|check| all.map(|round| check.on(round)))
                    .filter_map(|step| match step {
                        Next::Probe(next) => Some(next),
                        Next::Found(_) => None,
                    })
                    .collect();
                open.dedup();
            }
            assert_eq!(open, [], "{last:?}");
        }
    }

    #[test]
    fn the_played_route_is_one_word() {
        for via in [Via::Direct, Via::Relay] {
            assert_eq!(played(played_word(via).as_bytes()), Some(via));
            assert_eq!(played(alloc::format!("{}\r\n", played_word(via)).as_bytes()), Some(via));
        }
        for bad in ["", "\r\n", "Direct", "automatic", "relays"] {
            assert_eq!(played(bad.as_bytes()), None, "{bad}");
        }
    }

    #[test]
    fn a_pick_is_shown_and_saved_and_automatic_probes() {
        let start = Shown::start(Choice::Automatic);
        assert_eq!(start, Step { shown: Shown::Automatic(None), save: None, probe: true });
        assert_eq!(Shown::start(Choice::Direct), Step { shown: Shown::Direct, save: None, probe: false });
        for from in [Shown::Automatic(Some(Probed::Direct)), Shown::Relay, Shown::Direct] {
            for (choice, shown, probe) in [
                (Choice::Relay, Shown::Relay, false),
                (Choice::Direct, Shown::Direct, false),
                (Choice::Automatic, Shown::Automatic(None), true),
            ] {
                assert_eq!(from.on(Event::Chosen(choice)), Step { shown, save: Some(choice), probe });
            }
        }
        // The menu's three messages, and no other.
        assert_eq!(
            [0x21, 0x22, 0x23, 0x24, 0x25].map(Choice::of_message),
            [None, Some(Choice::Automatic), Some(Choice::Relay), Some(Choice::Direct), None]
        );
    }

    #[test]
    fn a_probe_shows_only_in_automatic_while_it_runs() {
        for probed in [Probed::Direct, Probed::Relay, Probed::Neither] {
            let step = Shown::Automatic(None).on(Event::Probed(probed));
            assert_eq!(step, Step { shown: Shown::Automatic(Some(probed)), save: None, probe: false });
            for shown in [Shown::Relay, Shown::Direct, Shown::Automatic(Some(Probed::Relay))] {
                assert_eq!(shown.on(Event::Probed(probed)), Step { shown, save: None, probe: false });
            }
        }
    }

    #[test]
    fn a_block_turns_the_row_to_automatic_whatever_it_showed() {
        for shown in [Shown::Direct, Shown::Relay, Shown::Automatic(Some(Probed::Neither))] {
            assert_eq!(
                shown.on(Event::Blocked),
                Step { shown: Shown::Automatic(None), save: Some(Choice::Automatic), probe: true }
            );
        }
    }

    #[test]
    fn each_view_has_its_own_byte() {
        let all = [
            Shown::Automatic(None),
            Shown::Automatic(Some(Probed::Direct)),
            Shown::Automatic(Some(Probed::Relay)),
            Shown::Automatic(Some(Probed::Neither)),
            Shown::Relay,
            Shown::Direct,
        ];
        for (at, shown) in all.iter().enumerate() {
            assert_eq!(shown.byte() as usize, at + 1);
        }
    }
}
