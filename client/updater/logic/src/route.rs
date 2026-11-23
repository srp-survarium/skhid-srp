//! The player's route to the game servers, in a download with a relay
//! (`updater_skhid.relay`, `srp-026g/deploy/relay/README.md`): the launcher's
//! "Relay" row (launcher patch 16: "Relay: on", "off", in auto what the
//! check does or found, and a menu of the three) shows it and changes it,
//! the updater keeps it (`<Documents>\survarium\launcher_skhid.route`, beside
//! user.cfg: the Windows user's, as the language) and PLAY and the updater's
//! HTTPS follow it.
//!
//! The check of the routes ([`Check`]) runs when the launcher opens, beside
//! the update, and takes the relay only when the direct route really fails:
//! several short rounds of its probes, more once a part of it has answered
//! (loss, not a block), one more when direct was found last. What it found
//! is kept ([`FOUND_FILE`], [`Found`]), and PLAY in Automatic starts by that
//! at once: PLAY never waits on a check of the launcher's.

use core::net::Ipv4Addr;

use crate::diagnosis::Via;

/// Beside user.cfg: the player's [`Choice`], one word.
pub const CHOICE_FILE: &str = "launcher_skhid.route";

/// Beside [`CHOICE_FILE`]: the route PLAY starts the next game by in
/// Automatic ([`Found`]), one word.
pub const FOUND_FILE: &str = "launcher_skhid.route_found";

/// Rounds of the direct route's probes that a check runs while nothing of
/// it answers.
pub const SILENT_ROUNDS: u8 = 3;
/// More rounds once a probe of the direct route has answered (the login
/// TCP, say, while a UDP probe was lost): packets are lost on the way, or a
/// part is blocked, and only more rounds tell which.
pub const HEARD_ROUNDS: u8 = 2;
/// One more round when direct was found last.
pub const PLAYED_ROUNDS: u8 = 1;

fn first_word(text: &[u8]) -> Option<&[u8]> {
    text.split(|&byte| byte <= b' ').find(|word| !word.is_empty())
}

/// The route PLAY starts the next game by in Automatic: what the last check
/// found, or what the last game showed of the direct route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// The direct route answered whole.
    Direct,
    /// The direct route failed its rounds and the relay answered whole.
    Relay,
    /// A game played direct lost its connection on the way: the next game
    /// goes through the relay, whatever a check finds meanwhile (the direct
    /// route answered that game's check too).
    DirectLost,
}

/// What tells the next game's route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Learned {
    /// A check found this route whole (the relay: the direct route failed).
    Checked(Via),
    /// The last game played direct and lost its connection ([`last_game`]).
    Lost,
    /// PLAY started a game by [`Found::via`].
    Played,
}

impl Found {
    /// [`FOUND_FILE`]'s text: its first word.
    pub fn parse(text: &[u8]) -> Option<Self> {
        match first_word(text)? {
            b"direct" => Some(Self::Direct),
            b"relay" => Some(Self::Relay),
            b"direct-lost" => Some(Self::DirectLost),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Relay => "relay",
            Self::DirectLost => "direct-lost",
        }
    }

    /// The route PLAY starts a game by.
    pub fn via(self) -> Via {
        match self {
            Self::Direct => Via::Direct,
            Self::Relay | Self::DirectLost => Via::Relay,
        }
    }

    /// The one transition. A check's route is kept, except that a direct
    /// route lost in a game is not taken back by a check until a game has
    /// gone through the relay.
    pub fn learn(found: Option<Self>, learned: Learned) -> Option<Self> {
        match (found, learned) {
            (_, Learned::Lost) => Some(Self::DirectLost),
            (Some(Self::DirectLost), Learned::Checked(Via::Direct)) => Some(Self::DirectLost),
            (_, Learned::Checked(Via::Direct)) => Some(Self::Direct),
            (_, Learned::Checked(Via::Relay)) => Some(Self::Relay),
            (Some(Self::DirectLost), Learned::Played) => Some(Self::Relay),
            (found, Learned::Played) => found,
        }
    }
}

/// What a game's engine log (`game\logs\Survarium_<user>.<stamp>.log`, game
/// patch 147) shows of its route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastGame {
    /// It connected to no server (a replay, a game closed at once).
    Unconnected,
    /// Its servers were the relay's address.
    Relayed,
    /// It connected to another address than the relay's; `lost`: a link of
    /// it timed out on the way (the lobby's, then reconnecting, or the
    /// match's).
    Direct { lost: bool },
}

/// The engine's lines: each connection the game opens (login's lobby and
/// match addresses, by the route the game came by), and a link that ended
/// unasked.
const CONNECTING: &[u8] = b"connecting from ";
const LOST: [&[u8]; 3] = [
    b"disconnected by timeout",
    b"close_current_match [disconnected_by_connection_lost]",
    b"close_current_match [disconnected_on_expired_multipacket_ordered_message]",
];

/// The last game's route by its engine log, `relay` the relay's address.
pub fn last_game(log: &[u8], relay: Ipv4Addr) -> LastGame {
    let (mut direct, mut relayed, mut lost) = (false, false, false);
    for line in log.split(|&byte| byte == b'\n') {
        if let Some(at) = find(line, CONNECTING) {
            // "connecting from 0.0.0.0:54264 to 64.176.67.220:1235"
            let target = line[at..].split(|&byte| byte == b' ').nth(4).unwrap_or_default();
            let host = target.split(|&byte| byte == b':').next().unwrap_or_default();
            match core::str::from_utf8(host).ok().and_then(|host| host.parse::<Ipv4Addr>().ok()) {
                Some(address) if address == relay => relayed = true,
                Some(_) => direct = true,
                None => {}
            }
        } else if LOST.iter().any(|text| find(line, text).is_some()) {
            lost = true;
        }
    }
    match (direct, relayed) {
        (true, _) => LastGame::Direct { lost },
        (false, true) => LastGame::Relayed,
        (false, false) => LastGame::Unconnected,
    }
}

fn find(line: &[u8], text: &[u8]) -> Option<usize> {
    line.windows(text.len()).position(|window| window == text)
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
    /// the route found last ([`Found::via`]).
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
    /// answered, [`PLAYED_ROUNDS`] more when direct was found last.
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
/// the route the check found (`None` while it runs): "Relay: on", "Relay:
/// off", "Auto: checking...", "Auto: direct", "Auto: via relay", "Auto: no
/// answer".
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
    /// The row at the start, for the saved choice. Direct is checked too,
    /// for a block (the row shows "off" meanwhile): found, it turns to
    /// Automatic before PLAY is pressed.
    pub fn start(choice: Choice) -> Step {
        Step {
            probe: choice != Choice::Relay,
            ..Self::step(choice, None)
        }
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

    /// The one transition. A probe's end is the one the row last asked for
    /// (a pick drops one still running: `Step::probe` false). A block moves
    /// the choice to Automatic, through the relay: saved and shown as a
    /// pick, the player's to change. The check of Direct at the start finds
    /// one when it ends at the relay; a block the update finds is checked.
    pub fn on(self, event: Event) -> Step {
        match (self, event) {
            (_, Event::Chosen(choice)) => Self::step(choice, Some(choice)),
            (_, Event::Blocked) => Self::step(Choice::Automatic, Some(Choice::Automatic)),
            (Self::Automatic(None), Event::Probed(probed)) => Step {
                shown: Self::Automatic(Some(probed)),
                save: None,
                probe: false,
            },
            (Self::Direct, Event::Probed(Probed::Relay)) => Step {
                shown: Self::Automatic(Some(Probed::Relay)),
                save: Some(Choice::Automatic),
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
    fn direct_found_last_gives_the_direct_route_one_more_round() {
        use Round::*;
        let direct = Some(Via::Direct);
        assert_eq!(check(direct, &[Silent, Silent, Silent, Answered]), (Probed::Direct, 4, 0));
        assert_eq!(check(direct, &[Silent, Silent, Silent, Silent, Answered]), (Probed::Relay, 4, 1));
        assert_eq!(check(direct, &[Partly, Partly, Partly, Partly, Partly, Answered]), (Probed::Direct, 6, 0));
        assert_eq!(check(direct, &[Partly, Partly, Partly, Partly, Partly, Partly, Answered]), (Probed::Relay, 6, 1));
        // Not after the relay was found, nor before anything was.
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
    fn the_found_route_is_one_word() {
        for found in [Found::Direct, Found::Relay, Found::DirectLost] {
            assert_eq!(Found::parse(found.word().as_bytes()), Some(found));
            assert_eq!(Found::parse(alloc::format!("{}\r\n", found.word()).as_bytes()), Some(found));
        }
        for bad in ["", "\r\n", "Direct", "automatic", "relays", "direct-lost-"] {
            assert_eq!(Found::parse(bad.as_bytes()), None, "{bad}");
        }
    }

    #[test]
    fn a_direct_route_lost_in_a_game_gives_one_game_through_the_relay() {
        use Learned::*;
        let learn = |found, steps: &[Learned]| steps.iter().fold(found, |found, &step| Found::learn(found, step));
        // A check's route is kept, and PLAY starts by it.
        assert_eq!(learn(None, &[Checked(Via::Direct)]), Some(Found::Direct));
        assert_eq!(learn(Some(Found::Direct), &[Checked(Via::Relay), Played]), Some(Found::Relay));
        assert_eq!(learn(Some(Found::Relay), &[Checked(Via::Direct), Played]), Some(Found::Direct));
        assert_eq!(learn(None, &[Played]), None);
        // Lost on the way: a check that finds direct whole again does not take
        // it back; the game after it goes through the relay; then checks rule.
        let lost = learn(Some(Found::Direct), &[Lost, Checked(Via::Direct)]);
        assert_eq!((lost, lost.map(Found::via)), (Some(Found::DirectLost), Some(Via::Relay)));
        assert_eq!(learn(lost, &[Played]), Some(Found::Relay));
        assert_eq!(learn(lost, &[Played, Checked(Via::Direct)]), Some(Found::Direct));
        assert_eq!(learn(lost, &[Checked(Via::Relay)]), Some(Found::Relay));
    }

    #[test]
    fn the_last_game_s_route_and_a_lost_link_are_read_from_its_log() {
        let relay = Ipv4Addr::new(203, 0, 113, 8);
        let lobby = |to: &str| alloc::format!("network_core <info>\t[11:33:41.943]\tconnecting from 0.0.0.0:36213 to {to}\r\n");
        let start = alloc::format!("game <info>\t[11:33:41.940]\tLOBBY: try reconnect\r\n{}", lobby("64.176.67.220:1235"));
        let timeout = "network_core <Warning>\t[14:29:00.042]\tclient [client] disconnected by timeout (19.069s)(24.164s)\r\n";
        let lost_match = "game <ERROR>\t[11:06:41.000]\tnetwork_client::close_current_match [disconnected_by_connection_lost]\r\n";
        let left = "game <ERROR>\t[11:06:41.000]\tnetwork_client::close_current_match [disconnected_by_initiator]\r\n";
        let game = |lines: &[&str]| last_game(lines.concat().as_bytes(), relay);
        assert_eq!(game(&[]), LastGame::Unconnected);
        assert_eq!(game(&[timeout]), LastGame::Unconnected);
        assert_eq!(game(&[&start, left]), LastGame::Direct { lost: false });
        assert_eq!(game(&[&start, timeout, &start]), LastGame::Direct { lost: true });
        assert_eq!(game(&[&start, lost_match]), LastGame::Direct { lost: true });
        let relayed = lobby("203.0.113.8:1235");
        assert_eq!(game(&[&relayed, timeout, &relayed]), LastGame::Relayed);
        // A name or a cut line is no address.
        assert_eq!(game(&[&lobby("skhid.link:1235"), &lobby("")]), LastGame::Unconnected);
    }

    #[test]
    fn a_pick_is_shown_and_saved_and_automatic_probes() {
        let start = Shown::start(Choice::Automatic);
        assert_eq!(start, Step { shown: Shown::Automatic(None), save: None, probe: true });
        // Direct is checked at the start too (for a block); the relay is not.
        assert_eq!(Shown::start(Choice::Direct), Step { shown: Shown::Direct, save: None, probe: true });
        assert_eq!(Shown::start(Choice::Relay), Step { shown: Shown::Relay, save: None, probe: false });
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
            for shown in [Shown::Relay, Shown::Automatic(Some(Probed::Relay))] {
                assert_eq!(shown.on(Event::Probed(probed)), Step { shown, save: None, probe: false });
            }
        }
    }

    #[test]
    fn direct_checked_at_the_start_turns_to_automatic_on_a_block_only() {
        for probed in [Probed::Direct, Probed::Neither] {
            let shown = Shown::Direct;
            assert_eq!(shown.on(Event::Probed(probed)), Step { shown, save: None, probe: false });
        }
        assert_eq!(
            Shown::Direct.on(Event::Probed(Probed::Relay)),
            Step { shown: Shown::Automatic(Some(Probed::Relay)), save: Some(Choice::Automatic), probe: false }
        );
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
