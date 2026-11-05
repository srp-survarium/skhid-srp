//! A hit damage_model::hit_body_part (0x2E5080) is dealing, at `medical.hit`
//! (0x2E50C7): its damage is the hit's (+34h) times the victim's
//! invulnerability, and goes next to the zone's armour and its pools
//! (body_part_parameters::hit_by_type 0x329FF0). Every game runs it for
//! every hit of every player in its simulation, so what is decided there is
//! decided alike everywhere (patch 108, docs/notes/hand-kill.md). Every field
//! is fields_026g.tsv's.

use core::ffi::CStr;

use super::context::Context;
use super::drug::HitType;
use super::world::read;
use crate::raw;

/// The hit at `medical.hit`, before armour.
pub struct DealingHit<'c> {
    kind: Option<HitType>,
    zone: Option<&'c CStr>,
    by_another: bool,
    frame: raw::MedicalDamageFrame,
}

/// The longest zone name (hit_params' names: `right_forearm`, 13 bytes).
const ZONE_LIMIT: usize = 32;

/// hit_info's attacker for none (hit_info::hit_info's stack+8).
const NO_PLACE: u8 = 0xFF;

impl<'c> DealingHit<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<DealingHit<'c>> {
        use raw::site::medical_hit as site;
        // SAFETY: the context is this site's: hit_body_part's damage model
        // (its owner the victim, alive: hit_impl returned at 0x2D9BA3
        // otherwise), its hit_info and the frame holding the damage, on the
        // thread running the hit. The zone is a hit_params name the hit
        // model's bone table keeps for the match.
        unsafe {
            let (hit, frame) = (site::hit(ctx)?, site::frame(ctx)?);
            let victim = site::model(ctx)?.owner()?;
            let zone = super::ui::c_string(hit.name(), ZONE_LIMIT);
            let attacker = read(hit.attacker());
            let by_another = attacker != NO_PLACE && attacker != read(victim.place());
            Some(DealingHit { kind: HitType::parse(read(hit.kind())), zone, by_another, frame })
        }
    }

    pub fn kind(&self) -> Option<HitType> {
        self.kind
    }

    /// The hit zone's name (`right_forearm`), as hit_params names it.
    pub fn zone(&self) -> Option<&CStr> {
        self.zone
    }

    /// The hit has an attacker, another player than the victim (a fall has
    /// none).
    pub fn by_another(&self) -> bool {
        self.by_another
    }

    /// The damage the zone's armour and pools are dealt from here.
    pub fn set_damage(&mut self, damage: f32) {
        // SAFETY: the frame's local, hit_body_part's own (0x2E50B7), read
        // next by hit_by_type at 0x2E50D4.
        unsafe { *self.frame.amount() = damage }
    }
}
