//! The markers over thrown grenades (patch 194, docs/notes/grenade-icons.md). player::st_draw's
//! sensor (0x246F50) gives each of the followed player's team's grenades within 7 m one of six
//! markers, icons 2Fh..34h, made by initialize_match with type 0 (the frag's glyph); the game never
//! tells a marker the grenade's kind. Here a sensed grenade is read once into its [`Kind`], and a
//! marker whose kind differs from the one it was last told gets set_hud_icon_type (0x23BEA0), the
//! movie's MarkerType. Presentation only: what the match computes is untouched.

use super::context::Context;
use super::hook::Thread;
use super::GameCell;
use crate::raw;

/// The markers the sensor fills (show_grenade_icons 0x23C3D0: six, from icon 2Fh).
pub const MARKERS: usize = 6;
const FIRST_ICON: u32 = 0x2F;

/// A thrown grenade's kind, as its marker shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A frag grenade (RGD-5): the stock glyph, MarkerType.GRENADE.
    Frag,
    /// A smoke grenade (RDG-M: its stack's splash_max_damage is 0), MarkerType.SMOKE.
    Smoke,
    /// A smoke grenade whose thrower's kit has Toxic Smoke (441), MarkerType.TOXIC_SMOKE.
    ToxicSmoke,
}

impl Kind {
    /// The movie's MarkerType (player_icons' survarium.constants.MarkerType).
    pub fn marker_type(self) -> u32 {
        match self {
            Kind::Frag => 0,
            Kind::Smoke => 3,
            Kind::ToxicSmoke => 4,
        }
    }
}

/// One of the six grenade markers (0..5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker(u8);

impl Marker {
    /// The marker of the sensor's count byte (BL), none past the six.
    pub fn of(count: u32) -> Option<Marker> {
        let index = (count & 0xFF) as u8;
        (usize::from(index) < MARKERS).then_some(Marker(index))
    }

    fn icon(self) -> u32 {
        FIRST_ICON + u32::from(self.0)
    }
}

/// What each marker was last told: all [`Kind::Frag`] when initialize_match has just made them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Told([Kind; MARKERS]);

impl Told {
    pub const MADE: Told = Told([Kind::Frag; MARKERS]);

    /// `marker` now shows a grenade of `kind`: the told kinds after, and the type to tell it, if
    /// it changed.
    pub fn sensed(self, marker: Marker, kind: Kind) -> (Told, Option<u32>) {
        let mut next = self;
        let slot = &mut next.0[usize::from(marker.0)];
        let change = (*slot != kind).then(|| kind.marker_type());
        *slot = kind;
        (next, change)
    }
}

static TOLD: GameCell<Told> = GameCell::new(Told::MADE);

/// A grenade the sensor gave a marker (`grenade_icons.sensed`).
pub struct Sensed {
    ui: raw::GameWorldUi,
    marker: Marker,
    kind: Kind,
}

impl Sensed {
    /// The site's grenade, its marker and its kind; none when a read is not there.
    pub fn at(ctx: &Context<'_>) -> Option<Sensed> {
        // SAFETY: the sensor's live body, whose user answered as a grenade (the site follows the
        // native as_grenade test), a live grenade of a live stack; the followed player's hud.
        unsafe {
            let marker = Marker::of(raw::site::grenade_icons_sensed::slot(ctx))?;
            let ui = raw::site::grenade_icons_sensed::player(ctx)?.ui()?;
            let grenade = raw::site::grenade_icons_sensed::body(ctx)?.user()?.grenade();
            let kind = if !super::technical_smoke::smoke_grenade(grenade) {
                Kind::Frag
            } else if super::technical_smoke::thrower(grenade)
                .is_some_and(|player| super::technical::skills(player).toxic_smoke().get() > 0)
            {
                Kind::ToxicSmoke
            } else {
                Kind::Smoke
            };
            Some(Sensed { ui, marker, kind })
        }
    }

    /// Tells the marker its kind when it showed another.
    pub fn tell(self, thread: &Thread) {
        let (told, change) = TOLD.get(thread).sensed(self.marker, self.kind);
        TOLD.set(thread, told);
        if let Some(marker_type) = change {
            // SAFETY: the hud of the match being drawn; its player icons movie is loaded with the
            // game world (initialize_match sends it the same calls).
            unsafe { raw::engine_set_hud_icon_type(Some(self.ui), self.marker.icon(), marker_type) }
        }
    }
}

/// initialize_match is making the match's markers anew (`player_icons.enemy_ids`, before them):
/// every grenade marker is a frag's again.
pub fn markers_made(thread: &Thread) {
    TOLD.set(thread, Told::MADE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_is_told_only_a_change() {
        let first = Marker::of(0).unwrap();
        let (told, change) = Told::MADE.sensed(first, Kind::Frag);
        assert_eq!((told, change), (Told::MADE, None));
        let (told, change) = told.sensed(first, Kind::Smoke);
        assert_eq!(change, Some(3));
        let (told, change) = told.sensed(first, Kind::Smoke);
        assert_eq!(change, None);
        // the slot taken by a frag next: back to the frag's glyph
        let (told, change) = told.sensed(first, Kind::Frag);
        assert_eq!(change, Some(0));
        let (_, change) = told.sensed(Marker::of(5).unwrap(), Kind::ToxicSmoke);
        assert_eq!(change, Some(4));
    }

    #[test]
    fn only_six_markers() {
        assert_eq!(Marker::of(5).map(Marker::icon), Some(0x34));
        assert_eq!(Marker::of(6), None);
        // BL is the count byte: EBX's upper bytes are the host's
        assert_eq!(Marker::of(0xABCD_0102).map(Marker::icon), Some(0x31));
    }
}
