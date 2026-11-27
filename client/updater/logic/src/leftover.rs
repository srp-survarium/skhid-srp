//! What the launcher's PLAY does with a `survarium_skhid.exe` an earlier
//! start of this install left running (hung, on its crash box, or never
//! ended): it is closed before the new game starts. Such a game keeps the
//! mouse it took with DirectInput (exclusive, in a match), and the new one
//! then gets no mouse input in matches (2026-11-23: a player on revision 72
//! could not aim until he restarted the computer).
//!
//! The choice of processes ([`GameExe`]) and each one's way from asked to
//! closed ([`Leftover`]) are here; the Windows calls are the exe's
//! `leftover.rs`.

use alloc::format;
use alloc::string::String;

/// How long the earlier games have, all together, to close when asked
/// (WM_CLOSE to their windows: the game quits, its crash box closes).
pub const ASK_MS: u32 = 3000;
/// How long a terminated game has to end.
pub const END_MS: u32 = 5000;

/// The game exe of this install, as a running process's image path is
/// compared with it: the whole path, whatever the letters' case, `/` or `\`,
/// and a `\\?\` prefix. A game of another folder (another install, a test
/// client) is never this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameExe {
    path: String,
    /// Its file name, as the process list gives it, to pass over other
    /// programs without opening them.
    name_at: usize,
}

impl GameExe {
    pub fn new(path: &str) -> Self {
        let path = canonical(path);
        let name_at = path.rfind('\\').map_or(0, |at| at + 1);
        Self { path, name_at }
    }

    /// Whether a process of this file name (the process list's) may be this
    /// game: worth opening to read its whole path.
    pub fn may_be(&self, file_name: &str) -> bool {
        file_name.to_lowercase() == self.path[self.name_at..]
    }

    /// Whether a process whose image is at `image` (its whole path) is this
    /// game.
    pub fn is(&self, image: &str) -> bool {
        canonical(image) == self.path
    }
}

fn canonical(path: &str) -> String {
    let path = path.replace('/', "\\");
    let path = match path.strip_prefix("\\\\?\\") {
        Some(unc) if unc.len() >= 4 && unc[..4].eq_ignore_ascii_case("UNC\\") => format!("\\\\{}", &unc[4..]),
        Some(local) => local.into(),
        None => path,
    };
    path.to_lowercase()
}

/// One earlier game being closed. Its windows were asked first; one that
/// did not close by [`ASK_MS`] after is terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leftover {
    /// Its windows were asked to close.
    Asked,
    /// It did not close when asked: being terminated.
    Terminating,
    Closed(Closed),
    /// It may still run: the new game is not started.
    Failed(Failure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    WhenAsked,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Windows refused to open, wait for or terminate it (its error code).
    Refused(u32),
    /// Terminated, and still running after [`END_MS`].
    Stuck,
}

/// What Windows said of the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// It has ended.
    Ended,
    /// The wait ran out while it runs.
    Running,
    /// TerminateProcess took it.
    Terminated,
    /// A call failed (Windows' error code).
    Refused(u32),
}

/// What is done next with the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Wait for it until [`ASK_MS`] after the ask.
    AwaitClose,
    Terminate,
    /// Wait for it [`END_MS`].
    AwaitEnd,
    Done,
}

impl Leftover {
    /// Its windows have been asked to close.
    pub fn asked() -> (Self, Step) {
        (Self::Asked, Step::AwaitClose)
    }

    pub fn on(self, event: Event) -> (Self, Step) {
        match (self, event) {
            (Self::Asked, Event::Ended) => (Self::Closed(Closed::WhenAsked), Step::Done),
            // A failed wait: termination may still take it.
            (Self::Asked, Event::Running | Event::Refused(_)) => (Self::Terminating, Step::Terminate),
            (Self::Asked | Self::Terminating, Event::Terminated) => (Self::Terminating, Step::AwaitEnd),
            (Self::Terminating, Event::Ended) => (Self::Closed(Closed::Terminated), Step::Done),
            (Self::Terminating, Event::Running) => (Self::Failed(Failure::Stuck), Step::Done),
            (Self::Terminating, Event::Refused(code)) => (Self::Failed(Failure::Refused(code)), Step::Done),
            (Self::Closed(_) | Self::Failed(_), _) => (self, Step::Done),
        }
    }
}

/// The player's box when an earlier game could not be closed, in the
/// launcher's language (`crate::replay::LANGUAGES` order; English for an
/// unknown one). Its buttons are Windows' Retry and Cancel.
pub fn cannot_close(language: Option<u32>) -> &'static str {
    match language.filter(|&n| n < 5).unwrap_or(0) {
        1 => "Survarium, запущенный ранее, всё ещё работает и не закрывается. Закройте survarium_skhid.exe в диспетчере задач или перезагрузите компьютер, затем попробуйте снова.",
        2 => "Survarium, запущений раніше, досі працює і не закривається. Закрийте survarium_skhid.exe у диспетчері завдань або перезавантажте комп'ютер, потім спробуйте знову.",
        3 => "Ein zuvor gestartetes Survarium läuft noch und lässt sich nicht beenden. Beende survarium_skhid.exe im Task-Manager oder starte den Computer neu und versuche es dann erneut.",
        4 => "Wcześniej uruchomiony Survarium nadal działa i nie daje się zamknąć. Zamknij survarium_skhid.exe w Menedżerze zadań lub uruchom ponownie komputer, a następnie spróbuj ponownie.",
        _ => "An earlier Survarium is still running and cannot be closed. Close survarium_skhid.exe in Task Manager or restart the computer, then try again.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAME: &str = "C:\\Games\\Skhid\\game\\binaries\\x86\\survarium_skhid.exe";

    #[test]
    fn the_same_exe_whatever_the_case_slashes_or_prefix() {
        let game = GameExe::new(GAME);
        for image in [
            GAME,
            "c:\\games\\skhid\\GAME\\binaries\\X86\\Survarium_Skhid.EXE",
            "C:/Games/Skhid/game/binaries/x86/survarium_skhid.exe",
            "\\\\?\\C:\\Games\\Skhid\\game\\binaries\\x86\\survarium_skhid.exe",
        ] {
            assert!(game.is(image), "{image}");
        }
        let share = GameExe::new("\\\\pc\\games\\x86\\survarium_skhid.exe");
        assert!(share.is("\\\\?\\UNC\\pc\\games\\x86\\survarium_skhid.exe"));
        let cyrillic = GameExe::new("D:\\Игры\\Скхід\\game\\binaries\\x86\\survarium_skhid.exe");
        assert!(cyrillic.is("d:\\игры\\СКХІД\\game\\binaries\\x86\\survarium_skhid.exe"));
    }

    #[test]
    fn another_install_or_another_program_is_never_the_game() {
        let game = GameExe::new(GAME);
        for image in [
            "D:\\Games\\Skhid\\game\\binaries\\x86\\survarium_skhid.exe",
            "C:\\Games\\Skhid2\\game\\binaries\\x86\\survarium_skhid.exe",
            "C:\\Games\\Skhid\\game\\binaries\\x86\\survarium.exe",
            "C:\\Games\\Skhid\\game\\binaries\\x86\\survarium_skhid.exe.bak",
            "C:\\Games\\Skhid\\game\\binaries\\x86\\updater_skhid.exe",
            "survarium_skhid.exe",
            "",
        ] {
            assert!(!game.is(image), "{image}");
        }
    }

    #[test]
    fn only_the_games_file_name_is_opened() {
        let game = GameExe::new(GAME);
        assert!(game.may_be("survarium_skhid.exe"));
        assert!(game.may_be("SURVARIUM_SKHID.EXE"));
        assert!(!game.may_be("survarium.exe"));
        assert!(!game.may_be("explorer.exe"));
        assert!(GameExe::new("survarium_skhid.exe").may_be("survarium_skhid.exe"));
    }

    fn run(events: &[Event]) -> Leftover {
        let (mut state, mut step) = Leftover::asked();
        for &event in events {
            assert_ne!(step, Step::Done, "{events:?}");
            (state, step) = state.on(event);
        }
        assert_eq!(step, Step::Done, "{events:?}");
        state
    }

    #[test]
    fn a_game_that_closes_when_asked_is_not_terminated() {
        assert_eq!(Leftover::asked(), (Leftover::Asked, Step::AwaitClose));
        assert_eq!(Leftover::Asked.on(Event::Ended), (Leftover::Closed(Closed::WhenAsked), Step::Done));
    }

    #[test]
    fn a_game_that_does_not_close_is_terminated_and_waited_for() {
        assert_eq!(Leftover::Asked.on(Event::Running), (Leftover::Terminating, Step::Terminate));
        assert_eq!(Leftover::Terminating.on(Event::Terminated), (Leftover::Terminating, Step::AwaitEnd));
        assert_eq!(run(&[Event::Running, Event::Terminated, Event::Ended]), Leftover::Closed(Closed::Terminated));
        // A wait that failed: terminated all the same.
        assert_eq!(run(&[Event::Refused(6), Event::Terminated, Event::Ended]), Leftover::Closed(Closed::Terminated));
    }

    #[test]
    fn a_game_that_cannot_be_terminated_or_does_not_end_fails() {
        assert_eq!(run(&[Event::Running, Event::Refused(5)]), Leftover::Failed(Failure::Refused(5)));
        assert_eq!(run(&[Event::Running, Event::Terminated, Event::Running]), Leftover::Failed(Failure::Stuck));
        assert_eq!(run(&[Event::Running, Event::Terminated, Event::Refused(6)]), Leftover::Failed(Failure::Refused(6)));
    }

    #[test]
    fn an_ended_game_stays_ended() {
        for state in [Leftover::Closed(Closed::WhenAsked), Leftover::Failed(Failure::Stuck)] {
            for event in [Event::Ended, Event::Running, Event::Terminated, Event::Refused(5)] {
                assert_eq!(state.on(event), (state, Step::Done));
            }
        }
    }

    #[test]
    fn every_language_names_the_exe_on_one_line() {
        let english = cannot_close(None);
        assert_eq!(cannot_close(Some(9)), english);
        for language in 0..5 {
            let text = cannot_close(Some(language));
            assert!(text.contains("survarium_skhid.exe"), "{text}");
            assert_eq!(text.lines().count(), 1, "{text}");
            assert!(language == 0 || text != english, "{language}");
        }
    }
}
