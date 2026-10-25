//! The game-objects section (world apply 0x2D2BB0..0x2D2BE6,
//! world_validation.md section 9).
//!
//! The section is a bare concatenation of the map's game objects, in the
//! order `server_game_project::set_game_world_core` 0x318800 links them
//! into the world's list -- the damage zones, then the usable objects (the
//! resupply points and the two containers, which derive from
//! `usable_object`), then the anomalies -- with **no count, no id and no
//! length per object**: each object deserializes its own bytes (world apply
//! 0x2D2BB0 walks the list and calls each object's reader with the section
//! reader, `[item+0xBDF3C]`). So the section can be read only when the
//! map's object list is known ([`Context::objects`]); a game derives it
//! from its loaded project, the server from the project's `client_objects`
//! (carried in the server game data). Without the list (a guessed match)
//! the section is not checked, like the respawn points of a guessed match.
//!
//! What each object reads from this section (PROVEN, re-kit rows; the
//! effect-ptr wrappers and the anomalies' zone groups read from the second
//! stream, the effects section `[item+0xBFF5C]`, held to its own template,
//! not from here):
//!
//! - `damage_zone_core` 0x319470: a `u32` and a `u8`, 5 bytes, no index.
//! - `usable_object` 0x2F84C0: a `u8 N` and `N` user ids, each a roster
//!   place (into a buffer_vector, no out-of-bounds write).
//! - `artefact_container_core` 0x31C5B0: the usable base, a `u8` artefact
//!   id (0xFF none, else an **unbounded** index into the game's artefacts,
//!   a crash path) and a `u32`.
//! - `victory_items_container_core` 0x31CEE0: the usable base, a `u32`
//!   count and `count` battery indices, each an **unbounded** index into
//!   the map's batteries (crash paths).
//! - `generic_anomaly_core` 0x31B0A0: the scalar head, with a `u8` state
//!   index (0xFF none, else an **unbounded** index into the states at
//!   `[+0x68]`, a crash path); the state's own `u32` is in this section
//!   only when the index is not 0xFF (self-describing).
//!
//! The check reads the list to the section's exact end and holds each index
//! within the bound the game needs; anything else refuses the world
//! (strict, world_validation.md section 1).

use super::{Context, Section};
use crate::reader::{Reader, Short};
use crate::{MAX_PLAYERS, Part, Reason, Refusal};

/// One game object of a map, in the order `set_game_world_core` links them
/// (the damage zones, the usable objects, then the anomalies). The server
/// fills the list from the project's `client_objects`, a game from its
/// loaded objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    /// `damage_zone_core`: a `u32` and a `u8`, 5 bytes.
    DamageZone,
    /// `usable_object` (a resupply point): `u8 N` then `N` user ids.
    Usable,
    /// `artefact_container_core`: the usable base, a `u8` artefact id and a
    /// `u32`. `artefacts` is how many artefacts the game has (the id's
    /// bound; the id may also be 0xFF, none).
    ArtefactContainer { artefacts: usize },
    /// `victory_items_container_core`: the usable base, a `u32` count and
    /// that many battery indices. The count and the indices are bounded by
    /// the map's batteries ([`Context::batteries`]).
    VictoryContainer,
    /// `generic_anomaly_core`: the scalar head with a state index bounded by
    /// `states` (the anomaly's state count). `gated`: the object's `[+0x0C]`
    /// is set, so the head opens with a `u8` and, if set, a `u32`.
    Anomaly { gated: bool, states: usize },
}

/// The game objects read to the section's exact end, and every index they
/// name is within the bound the game needs. The list unknown (a guessed
/// match): not checked.
pub fn objects(section: &Section, context: &Context) -> Result<(), Refusal> {
    let Some(list) = context.objects else {
        return Ok(());
    };
    let refuse = |reason| Refusal::new(Part::Objects, section.at, reason);
    let unread = || {
        refuse(Reason::ObjectsUnread {
            len: section.bytes.len(),
            objects: list.len(),
        })
    };
    let mut reader = Reader::new(section.bytes, section.at);
    for (index, object) in list.iter().enumerate() {
        match read_object(&mut reader, object, context, index) {
            Ok(()) => {}
            Err(Fault::Short) => return Err(unread()),
            Err(Fault::Reason(reason)) => return Err(refuse(reason)),
        }
    }
    if reader.is_done() {
        Ok(())
    } else {
        Err(unread())
    }
}

/// A world's object did not read (`Short`: it ran off the section's end;
/// the exact-fit test turns this into [`Reason::ObjectsUnread`]), or it
/// named an index past its bound.
enum Fault {
    Short,
    Reason(Reason),
}

impl From<Short> for Fault {
    fn from(_: Short) -> Self {
        Fault::Short
    }
}

/// Reads one object's bytes from the section, bounding every index.
fn read_object(
    reader: &mut Reader,
    object: &Object,
    context: &Context,
    index: usize,
) -> Result<(), Fault> {
    match *object {
        Object::DamageZone => {
            // u32 (+0x9C) and a u8 (+0xA1); neither is an index.
            reader.skip(5)?;
        }
        Object::Usable => {
            users(reader, context, index)?;
        }
        Object::ArtefactContainer { artefacts } => {
            users(reader, context, index)?;
            let artefact = reader.u8()?;
            if artefact != 0xFF && usize::from(artefact) >= artefacts {
                return Err(Fault::Reason(Reason::ObjectsArtefactPast {
                    object: index,
                    artefact,
                    artefacts,
                }));
            }
            reader.skip(4)?;
        }
        Object::VictoryContainer => {
            users(reader, context, index)?;
            let count = reader.u32()?;
            let batteries = context.batteries;
            if count as usize > batteries {
                return Err(Fault::Reason(Reason::ObjectsBatteriesPast {
                    object: index,
                    count,
                    batteries,
                }));
            }
            for _ in 0..count {
                let battery = reader.u8()?;
                if usize::from(battery) >= batteries {
                    return Err(Fault::Reason(Reason::ObjectsBatteryPast {
                        object: index,
                        battery,
                        batteries,
                    }));
                }
            }
        }
        Object::Anomaly { gated, states } => {
            if gated && reader.u8()? != 0 {
                reader.skip(4)?;
            }
            // f32 (+0x38).
            reader.skip(4)?;
            let state = reader.u8()?;
            if state != 0xFF {
                if usize::from(state) >= states {
                    return Err(Fault::Reason(Reason::ObjectsStatePast {
                        object: index,
                        state,
                        states,
                    }));
                }
                // anomaly_state 0x31B230 reads one u32 of this section when
                // the state is active (the byte is not 0xFF).
                reader.skip(4)?;
            }
            // u8 flags (bits 0, 1) and a u32 (+0xA0).
            reader.skip(1 + 4)?;
        }
    }
    Ok(())
}

/// The usable base (`usable_object::deserialize` 0x2F84C0): `u8 N`, then
/// `N` user ids, each a roster place.
fn users(reader: &mut Reader, context: &Context, object: usize) -> Result<(), Fault> {
    let places = context.places.min(MAX_PLAYERS);
    let count = reader.u8()?;
    for _ in 0..count {
        let user = reader.u8()?;
        if usize::from(user) >= places {
            return Err(Fault::Reason(Reason::ObjectsUserPast {
                object,
                user,
                places,
            }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(objects: Option<&'a [Object]>) -> Context<'a> {
        Context {
            time: 0,
            places: 4,
            game_mode: crate::mode::BATTERIES,
            batteries: 3,
            respawn_points: None,
            profiles: None,
            objects,
            artefacts: None,
            inventory: None,
        }
    }

    fn section(bytes: &[u8]) -> Section<'_> {
        Section { at: 0x100, bytes }
    }

    /// A map with every object kind, in link order.
    const MAP: [Object; 5] = [
        Object::DamageZone,
        Object::Usable,
        Object::ArtefactContainer { artefacts: 8 },
        Object::VictoryContainer,
        Object::Anomaly {
            gated: true,
            states: 4,
        },
    ];

    /// A section that reads to its end for [`MAP`]: a damage zone (u32,
    /// u8), a usable of two users, an artefact container (one user,
    /// artefact 5, u32), a victory container (no user, batteries 0 and 2),
    /// and an anomaly (gated bool set so a u32, f32, state 1 active so a
    /// u32, flags, u32).
    const GOOD: [u8; 41] = [
        0xFF, 0xFF, 0xFF, 0xFF, 0x00, // DamageZone
        2, 0, 3, // Usable
        1, 1, 5, 0, 0, 0, 0, // ArtefactContainer
        0, 2, 0, 0, 0, 0, 2, // VictoryContainer
        0xFF, 1, 2, 3, 4, // Anomaly: gate bool + u32
        0, 0, 0, 0, // f32
        1, // state index (< 4), active
        9, 9, 9, 9, // anomaly_state u32
        3, // flags
        0, 0, 0, 0, // u32
    ];

    #[test]
    fn an_unknown_list_is_not_checked() {
        // Any bytes pass when the list is unknown.
        assert_eq!(objects(&section(&[1, 2, 3]), &context(None)), Ok(()));
    }

    #[test]
    fn a_section_that_reads_to_its_end_passes() {
        assert_eq!(objects(&section(&GOOD), &context(Some(&MAP))), Ok(()));
    }

    #[test]
    fn an_anomaly_in_the_idle_state_has_no_state_word() {
        // State byte 0xFF: no following u32.
        let map = [Object::Anomaly {
            gated: false,
            states: 4,
        }];
        let bytes = [0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0];
        assert_eq!(objects(&section(&bytes), &context(Some(&map))), Ok(()));
    }

    #[test]
    fn a_short_or_long_section_is_refused() {
        assert!(matches!(
            objects(&section(&GOOD[..40]), &context(Some(&MAP)))
                .unwrap_err()
                .reason,
            Reason::ObjectsUnread { .. }
        ));
        let mut long = [0u8; 42];
        long[..41].copy_from_slice(&GOOD);
        assert!(matches!(
            objects(&section(&long), &context(Some(&MAP)))
                .unwrap_err()
                .reason,
            Reason::ObjectsUnread { .. }
        ));
    }

    #[test]
    fn an_index_past_its_bound_is_refused() {
        // A user past the roster (place 4, roster 0..3).
        let map = [Object::Usable];
        assert!(matches!(
            objects(&section(&[1, 4]), &context(Some(&map)))
                .unwrap_err()
                .reason,
            Reason::ObjectsUserPast { user: 4, .. }
        ));
        // An artefact past the game's.
        let map = [Object::ArtefactContainer { artefacts: 3 }];
        assert!(matches!(
            objects(&section(&[0, 5, 0, 0, 0, 0]), &context(Some(&map)))
                .unwrap_err()
                .reason,
            Reason::ObjectsArtefactPast { artefact: 5, .. }
        ));
        // A battery past the map's.
        let map = [Object::VictoryContainer];
        assert!(matches!(
            objects(&section(&[0, 1, 0, 0, 0, 7]), &context(Some(&map)))
                .unwrap_err()
                .reason,
            Reason::ObjectsBatteryPast { battery: 7, .. }
        ));
        // A victory container holding more than the map has.
        assert!(matches!(
            objects(&section(&[0, 9, 0, 0, 0]), &context(Some(&map)))
                .unwrap_err()
                .reason,
            Reason::ObjectsBatteriesPast { count: 9, .. }
        ));
        // An anomaly state past its own.
        let map = [Object::Anomaly {
            gated: false,
            states: 2,
        }];
        assert!(matches!(
            objects(
                &section(&[0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                &context(Some(&map))
            )
            .unwrap_err()
            .reason,
            Reason::ObjectsStatePast { state: 5, .. }
        ));
    }
}
