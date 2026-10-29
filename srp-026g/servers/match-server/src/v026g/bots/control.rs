//! Who has a bot's body. Ordinarily its play does (and the reflex fights for it); the bench's stand-in camper keeps its
//! legs on its spot; a client test's script has all of it, bar what the step lets the reflex and the reactions do. One
//! enum on the bot ([`super::Bot::control`]), set from the runner once a poll, so the drive reads the one owner of the
//! body and never a flag a think wrote.

use super::reflex::Override;

#[cfg(any(test, feature = "client-test"))]
use super::client_tests::Grip;
#[cfg(any(test, feature = "client-test"))]
use crate::v026g::drive::key;

/// Who has the body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Driver {
    /// Its play and its reflex, as ever.
    #[default]
    Own,
    /// The bench's stand-in camper (`SRP_BENCH_CAMPER`): the reflex never takes its legs off its spot.
    #[cfg(test)]
    BenchCamper,
    /// A client test's script (`client_tests`): the legs are the script's; what the reflex and the reactions may do is the grip's.
    #[cfg(any(test, feature = "client-test"))]
    Scripted(Grip),
}

impl Driver {
    /// The reflex's wish as this control lets it stand: a script that lets the reflex fight keeps its aim and trigger and
    /// loses its legs (and the side step that goes with them); one that does not has only the reload.
    pub fn filter(self, over: Override) -> Override {
        match self {
            Self::Own => over,
            #[cfg(test)]
            Self::BenchCamper => over,
            #[cfg(any(test, feature = "client-test"))]
            Self::Scripted(grip) => grip.filter(over),
        }
    }

    /// Whether the legs stay where the control put them (the reflex never moves them).
    pub fn holds_legs(self) -> bool {
        match self {
            Self::Own => false,
            #[cfg(test)]
            Self::BenchCamper => true,
            #[cfg(any(test, feature = "client-test"))]
            Self::Scripted(grip) => grip.holds_legs(),
        }
    }

    /// Whether the reactions that move a body apply to it: the trap habit and its reaction to a trap, the dodge of a grenade,
    /// the step in off a lip at rest.
    pub fn reacts(self) -> bool {
        match self {
            Self::Own => true,
            #[cfg(test)]
            Self::BenchCamper => true,
            #[cfg(any(test, feature = "client-test"))]
            Self::Scripted(grip) => grip.reacts(),
        }
    }
}

#[cfg(any(test, feature = "client-test"))]
/// What a script that lets the reflex do nothing leaves of its wish: the reload, which fires nothing and moves nothing.
pub fn only_reload(over: Override) -> Override {
    Override {
        keys: over.keys & key::RELOAD,
        ..Override::default()
    }
}
