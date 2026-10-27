//! What each feature of the bots keeps, one struct a feature in its own module, and the lists that gather them
//! ([`BotFeatures`] in a [`Bot`], [`MatchFeatures`] in the match's [`Bots`], [`TeamFeatures`] in a team's
//! [`TeamState`]).
//!
//! A new feature's state is **its module and one line in a list**: `name: module::State,`, in alphabetical order (a
//! test reads the lists). The lists make the struct, its `Default` and, for a bot's, the reset at a new life
//! ([`Feature::new_life`], a no-op unless the state has something to reset); `Bot::new`, `Bots::new` and
//! `Bot::new_life` do not change. Fields are read by name, `bot.features.crossing.goal`.
//!
//! Not every state is here: a state moves into a feature when a change next touches it. What is still a field of
//! `Bot`, `Bots` or `TeamState` is the core (identity, the plays, the motor, the knowledge) or waits its turn.
//!
//! A macro (`macro_rules`, no more) rather than a list of boxed states: a state is read in the hot path by
//! name, and the macro keeps the fields, the `Default` and the reset in the one list the compiler checks.

use super::*;

/// What a feature may need to know at the start of a bot's new life.
pub(super) struct Life {
    /// The kit the life starts in (its index).
    pub(super) kit: usize,
}

/// A bot's state of one feature: made by `Default`, reset at each new life.
pub(super) trait Feature {
    /// A new life begins. What is not reset is the state's of the match.
    fn new_life(&mut self, _life: &Life) {}
}

/// A struct of states, one field a feature, and its `Default`. `$reset` (`reset`) also makes `new_life`, which
/// gives the new life to each state in turn. Attributes and docs of a field are carried over.
macro_rules! features {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident reset $reset:ident {
            $( $(#[$fmeta:meta])* $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        features! {
            $(#[$meta])*
            $vis struct $name { $( $(#[$fmeta])* $field: $ty ),* }
        }

        impl $name {
            /// A new life: each state's own reset.
            pub(super) fn $reset(&mut self, life: &Life) {
                $( Feature::new_life(&mut self.$field, life); )*
            }
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$fmeta:meta])* $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Default)]
        $vis struct $name {
            $( $(#[$fmeta])* pub(super) $field: $ty, )*
        }
    };
}

features! {
    /// A bot's states, by feature.
    pub(super) struct BotFeatures reset new_life {
        /// Stepping out of the live grenades it knows of, and what it did (`grenade_threat`).
        avoid: grenade_threat::Avoid,
        /// Crossing zones (`crossing`).
        crossing: crossing::BotState,
        /// What its plays remember between runs (`plays::memory`).
        memory: plays::memory::BotState,
        /// The drugs it carries and its health pools (`stock`).
        stock: stock::BotState,
        /// The bear-trap habit and the notice of the traps of others (`traps`).
        traps: traps::BotState,
    }
}

features! {
    /// The match's states, by feature.
    pub(super) struct MatchFeatures {
        /// The artefact places of the map and how well the cores' artefacts are known (`artefact_know`;
        /// `tuning.hazard.artefact_know`, built with the tactics, none while it is off).
        artefacts: Option<artefact_know::ArtefactKnow>,
        /// Kit choices made since they were last taken (`Bots::take_kit_choices`): the place and the kit's index.
        kit_choices: Vec<(Place, u8)>,
        /// How the teams' own traps went off (`trap_know`).
        own_traps: trap_know::OwnTrapCounts,
        /// This poll's route searches (`search`).
        searching: search::Searching,
        /// The armed traps the last world showed and the snaps given to the ears (`trap_know`).
        snaps: trap_know::Snaps,
        /// Every place's weapon 1 kind, where known (play 11).
        weapons: Vec<Option<reflex::WeaponKind>>,
    }
}

features! {
    /// A team's states, by feature.
    pub(super) struct TeamFeatures {
        /// Where its bots got stuck, and the goals they proved no way to (`stuck`).
        stuck: stuck::TeamStuck,
        /// Where a shooter killed its bots from (`threat`).
        threat: threat::TeamThreat,
        /// The floors of the traps its bots put down, and the armed traps the last world showed (`trap_know`).
        traps: trap_know::TeamTraps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Counts {
        lives: u32,
    }

    impl Feature for Counts {
        fn new_life(&mut self, life: &Life) {
            self.lives += 1 + life.kit as u32;
        }
    }

    /// A state that resets nothing: the default `new_life`.
    #[derive(Default)]
    struct Keeps(u8);

    impl Feature for Keeps {}

    features! {
        /// A test's states.
        struct Toy reset start_life {
            /// Counts the lives, and the kit.
            counts: Counts,
            keeps: Keeps,
            /// A state that is just a list.
            #[allow(dead_code)]
            names: Vec<String>,
        }
    }

    features! {
        struct ToyMatch {
            counts: Counts,
        }
    }

    impl Feature for Vec<String> {}

    /// The list makes the struct with each state's `Default`, and the reset gives each its new life.
    #[test]
    fn a_list_makes_the_states_and_their_reset() {
        let mut toy = Toy::default();
        assert_eq!((toy.counts.lives, toy.keeps.0, toy.names.len()), (0, 0, 0));
        toy.keeps.0 = 7;
        toy.names.push("kept".to_owned());
        toy.start_life(&Life { kit: 2 });
        toy.start_life(&Life { kit: 0 });
        assert_eq!(toy.counts.lives, (1 + 2) + 1);
        assert_eq!((toy.keeps.0, toy.names.len()), (7, 1), "a state with no reset keeps what it had");
        // A list without a reset is only the struct and its Default.
        assert_eq!(ToyMatch::default().counts.lives, 0);
    }

    /// The names of the fields of each list (`features!`) in this file, from its source, are in order.
    #[test]
    fn the_lists_are_in_alphabetical_order() {
        let source = include_str!("features.rs");
        // Only the real lists: from `features! {` to its closing brace at the margin, outside the test module.
        let real = source.split("#[cfg(test)]").next().unwrap();
        let mut wrong = Vec::new();
        let mut in_list = false;
        let mut previous: Option<&str> = None;
        for (n, line) in real.lines().enumerate() {
            if line.starts_with("features! {") {
                in_list = true;
                previous = None;
            } else if in_list && line == "}" {
                in_list = false;
            } else if in_list
                && line.starts_with("        ")
                && !line.starts_with("         ")
                && !line.trim_start().starts_with("//")
                && !line.trim_start().starts_with('#')
                && let Some((name, _)) = line.trim().split_once(':')
            {
                if previous.is_some_and(|p| p > name) {
                    wrong.push((n + 1, name.to_owned()));
                }
                previous = Some(name);
            }
        }
        assert!(wrong.is_empty(), "out of order: {wrong:?}");
    }
}
