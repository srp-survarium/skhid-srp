//! Account-owned allowances for mode introductions. Each mode gets its own three matches.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mode {
    Blackout,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Blackout => "blackout",
        }
    }
    pub fn parse(key: &str) -> Option<Self> {
        match key {
            "blackout" => Some(Self::Blackout),
            _ => None,
        }
    }
}

/// Storage keys are mode names, independent of the lobby's map/mode numbering.
/// Add a mode here when its rules and HUD are implemented (Scarcity is planned).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Allowances(BTreeMap<Mode, Introductions>);

impl Allowances {
    pub fn take(&mut self, mode: Mode) -> bool {
        self.0.entry(mode).or_default().take()
    }
    pub fn rows(&self) -> impl Iterator<Item = (Mode, u8)> + '_ {
        self.0.iter().map(|(&mode, &state)| (mode, state.shown()))
    }
    pub fn insert(&mut self, mode: Mode, shown: u8) -> Option<()> {
        self.0.insert(mode, Introductions::from_shown(shown)?);
        Some(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Introductions {
    #[default]
    ThreeLeft,
    TwoLeft,
    OneLeft,
    Done,
}

impl Introductions {
    pub fn shown(self) -> u8 {
        match self {
            Self::ThreeLeft => 0,
            Self::TwoLeft => 1,
            Self::OneLeft => 2,
            Self::Done => 3,
        }
    }

    pub fn from_shown(shown: u8) -> Option<Self> {
        match shown {
            0 => Some(Self::ThreeLeft),
            1 => Some(Self::TwoLeft),
            2 => Some(Self::OneLeft),
            3 => Some(Self::Done),
            _ => None,
        }
    }

    /// Called once per distinct match, only when its HUD requests the introduction.
    pub fn take(&mut self) -> bool {
        *self = match *self {
            Self::ThreeLeft => Self::TwoLeft,
            Self::TwoLeft => Self::OneLeft,
            Self::OneLeft => Self::Done,
            Self::Done => return false,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exactly_three_introductions() {
        let mut left = Introductions::default();
        for shown in 0..=3 {
            assert_eq!(left.shown(), shown);
            assert_eq!(Introductions::from_shown(shown), Some(left));
            assert_eq!(left.take(), shown < 3);
        }
        assert!(!left.take());
        assert_eq!(Introductions::from_shown(4), None);
    }
}
