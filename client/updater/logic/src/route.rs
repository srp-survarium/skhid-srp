//! The player's route to the game servers, in a download with a relay
//! (`updater_skhid.relay`, `srp-026g/deploy/relay/README.md`): the launcher's
//! "Relay" row (launcher patch 16: "Relay: auto", "on", "off", and a menu of
//! the three) shows it and changes it, the updater keeps it (`<Documents>\survarium\launcher_skhid.route`, beside user.cfg:
//! the Windows user's, as the language) and PLAY and the updater's HTTPS
//! follow it.

/// Beside user.cfg: the player's [`Choice`], one word.
pub const CHOICE_FILE: &str = "launcher_skhid.route";

/// Windows' GeoID of Russia (`GetUserGeoID(GEOCLASS_NATION)`), where
/// providers block the server: its players start in [`Choice::Automatic`].
pub const RUSSIA: i32 = 203;

/// How PLAY and the updater reach the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Directly when the server answers so, else through the relay: probed
    /// before every game.
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
        match text.split(|&byte| byte <= b' ').find(|word| !word.is_empty())? {
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

/// What the probe found (`relay_client::Selection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probed {
    Direct,
    Relay,
    Neither,
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
