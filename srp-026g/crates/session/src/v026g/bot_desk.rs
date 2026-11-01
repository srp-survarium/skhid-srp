//! The admin page's "Bots" section and the server bots' tuning between them (A246; the owner, 2026-11-01: "an ability to
//! adjust brackets on the live server including starting level ... along with other configurable things", and "there
//! should be button to reset to defaults").
//!
//! The match server keeps the tuning its matches run on and the knobs the page may set
//! (`match_server::v026g::bots::tuning::LiveTuning` implements [`BotDesk`]); the page shows them and sends what the
//! admin changed. srp joins the two; neither crate knows the other. A change applies from the next match: a match keeps
//! the tuning it started with.

/// A text in the page's two languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text {
    pub en: &'static str,
    pub uk: &'static str,
}

/// What the page can do with the bots' tuning.
pub trait BotDesk: Send + Sync {
    /// The knobs as the next match will run them, the latest changes, and where the tuning comes from.
    fn view(&self) -> DeskView;
    /// Sets each knob named to the value its text says, all of them or none: a name the desk does not have, a value
    /// out of its limits, or a tuning the values together break refuses the whole, and the error says the first
    /// reason. `from` is who asked (the page's client address), for the audit. Gives the changes made (none when every
    /// value was already so).
    fn set(&self, changes: &[(String, String)], from: &str) -> Result<Vec<Change>, String>;
    /// Puts back the defaults of one group's knobs ([`Group::key`]), or of every group's (`None`): what the server's
    /// tuning (its file, else the shipped defaults) says without the page's changes. Audited like a change.
    fn reset(&self, group: Option<&str>, from: &str) -> Result<Vec<Change>, String>;
}

/// The Bots section as it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct DeskView {
    /// Where the tuning comes from: the shipped defaults, or the file and its digest.
    pub source: String,
    /// Where the page's changes are kept.
    pub kept: Kept,
    /// Why the matches do not run on what the files say now (a file refused: the last good tuning is kept).
    pub refused: Option<Refused>,
    pub groups: Vec<Group>,
    /// The latest changes, the newest first.
    pub audit: Vec<Change>,
}

/// A file the matches do not run on, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// Unix time, seconds: when it was first refused.
    pub at: u64,
    pub why: String,
}

/// Where the page's changes are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kept {
    /// Written to this file at each change, and read when the server starts.
    File(String),
    /// In memory only: a restart forgets them.
    Memory,
}

/// A group of knobs, with a reset of its own when one of them is settable.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Its name in a form ([`BotDesk::reset`]).
    pub key: &'static str,
    pub title: Text,
    /// What the group's shown-only knobs, or knobs to come, wait for.
    pub note: Option<Text>,
    pub knobs: Vec<Knob>,
}

/// Whether the page may set a knob yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnobState {
    Settable,
    /// Shown only (the group's note says why).
    Shown,
}

/// A knob: its value for the next match, what a reset gives, and its limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Knob {
    /// Its name in a form ([`BotDesk::set`]): the tuning's key, `skill.step`.
    pub key: String,
    pub label: Text,
    pub state: KnobState,
    pub value: String,
    pub default: String,
    /// The values it takes, as the page says them (`0–1`).
    pub limits: String,
}

/// One knob changed, for the audit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// Unix time, seconds.
    pub at: u64,
    /// Who asked: the page's client address.
    pub from: String,
    pub key: String,
    pub old: String,
    pub new: String,
}
