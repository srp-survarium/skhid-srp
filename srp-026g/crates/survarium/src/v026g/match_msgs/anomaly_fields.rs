//! The anomaly fields a world holds: the places that hurt for a while and are not in the map file
//! (the dynamic anomalies of the redesign: a core made and destroyed at run time, `live-zones-design.md`).
//!
//! A field is read from the world's "game objects" section ([`super::snapshot::Sections::objects`]) once, here, into
//! the typed values below; the bots take only those ([`WorldField`]) and never look at bytes. The bench makes the
//! same values without any bytes.
//!
//! **The layout is not known yet.** The pre-built cores of #225 (the baked timer zones) are found in the section
//! by the value of their random generator, which the bots' simulation predicts from the map's cores
//! (`bots/anomalies.rs`, `sync_from_world`): that is not a read of the section alone, and those zones are in the
//! map file already. A field made at run time has no entry in the map and no value to search for. Until the
//! developer's layout is known ([`find`]), the function returns no field, so nothing in the game feeds the bots'
//! board and the switch (`Tuning::live_zones`) is an identity by construction. A parser replaces [`find`] and
//! nothing else changes.

/// A field's id, stable for the field's life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub u32);

/// What the field is made of (the pack's artefact kinds that lie in a lethal anomaly).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Lifebone,
    Spring,
    Onyx,
}

/// Where a field hurts, round its centre, in world metres, already scaled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldShape {
    /// Round about the vertical axis.
    Cylinder { radius: f32, half_height: f32 },
    /// Half sizes along the axes.
    Box { half: [f32; 3] },
}

/// Where a field is in its life, in match milliseconds. A field that rests or idles is not a field: [`find`] does not
/// return it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldPhase {
    /// Harmless until `damage_from` (the release: 20 s after it was made).
    Charging { damage_from: u32 },
    /// Hurting; `until` its end when it is known (the release: 4 s of damage, then [`Self::Holding`]).
    Damaging { until: Option<u32> },
    /// The artefact waits and the zone stays on, with no known end (until the artefact is taken).
    Holding { since: u32 },
}

/// One field of a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldField {
    pub id: FieldId,
    pub kind: FieldKind,
    /// The middle of the field, world metres.
    pub centre: [f32; 3],
    pub shape: FieldShape,
    pub phase: FieldPhase,
}

/// The fields of a world's objects section at the match time `world_ms`: those that can hurt or are about to. None
/// until the layout is known (this module's doc).
pub fn find(_objects: &[u8], _world_ms: u32) -> Vec<WorldField> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn until_the_layout_is_known_a_world_holds_no_field() {
        assert!(find(&[0u8; 4096], 60_000).is_empty());
        assert!(find(&[], 0).is_empty());
    }
}
