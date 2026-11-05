//! A hit damage_model::hit_body_part (0x2E5080) is dealing, at `medical.hit`
//! (0x2E50C7): its damage is the hit's (+34h) times the victim's
//! invulnerability, and goes next to the zone's armour and its pools
//! (body_part_parameters::hit_by_type 0x329FF0). Every game runs it for
//! every hit of every player in its simulation, so what is decided there is
//! decided alike everywhere (patch 108, docs/notes/force-protein-hand-kill.md).
//! Every field is fields_026g.tsv's.

use core::ffi::CStr;

use super::context::Context;
use super::drug::HitType;
use super::world::read;
use crate::raw;

/// A player modifier's index in a kit's 68 (the exe's names at 0x928448).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModifierIndex(u32);

impl ModifierIndex {
    /// `stamina_boost_interval` (29): no 0.26g code reads it (no
    /// apply_modifier or apply_modifiers call names it) and nothing sets it
    /// but skill 205's 0; additive (0x927E08's list). srp's: a booster that sets it
    /// makes its carrier's gun-stock hits on a hand lethal.
    pub const MELEE_HAND_KILL: ModifierIndex = ModifierIndex(29);
}

/// The hit at `medical.hit`, before armour.
pub struct DealingHit<'c> {
    kind: Option<HitType>,
    zone: Option<&'c CStr>,
    victim: Option<raw::Player>,
    attacker: Option<u8>,
    frame: raw::MedicalDamageFrame,
}

/// The longest zone name (hit_params' names: `right_forearm`, 13 bytes).
const ZONE_LIMIT: usize = 32;

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
            let victim = site::model(ctx).and_then(|model| model.owner());
            let zone = super::ui::c_string(hit.name(), ZONE_LIMIT);
            let attacker = Some(read(hit.attacker())).filter(|&place| place != 0xFF);
            Some(DealingHit { kind: HitType::parse(read(hit.kind())), zone, victim, attacker, frame })
        }
    }

    pub fn kind(&self) -> Option<HitType> {
        self.kind
    }

    /// The hit zone's name (`right_forearm`), as hit_params names it.
    pub fn zone(&self) -> Option<&CStr> {
        self.zone
    }

    /// The sum of `index` in the attacker's current kit (the skills, the
    /// armour's and the carried boosters' entries: apply_modifier
    /// 0x2D5AE0, additive, of 0); None when the hit has no attacker or it
    /// is the victim's own.
    pub fn attacker_modifier(&self, index: ModifierIndex) -> Option<f32> {
        let victim = self.victim?;
        let place = usize::from(self.attacker?);
        // SAFETY: the victim's world core and its places' vector, the place
        // bounded by the vector's end before its entry is read; a place
        // keeps its kits' players for the match (core_active_player reads
        // them so), and a player's profile its kits' modifiers.
        unsafe {
            let core = victim.world_core()?;
            let (begin, end) = (core.places()?, core.places_end()?);
            if end.index_from(begin)? <= place {
                return None;
            }
            let entry = begin.at(place);
            let kit = read(entry.kit());
            let attacker = match kit {
                0 => entry.kit0(),
                1 => entry.kit1(),
                2 => entry.kit2(),
                _ => None,
            }?;
            if attacker == victim {
                return None;
            }
            let kit = usize::from(read(attacker.kit()));
            if kit >= 3 {
                return None;
            }
            let modifiers = attacker.profile()?.kits().at(kit).modifiers();
            Some(raw::engine_apply_modifier(Some(modifiers), index.0, 0.0, 1.0))
        }
    }

    /// The damage the zone's armour and pools are dealt from here.
    pub fn set_damage(&mut self, damage: f32) {
        // SAFETY: the frame's local, hit_body_part's own (0x2E50B7), read
        // next by hit_by_type at 0x2E50D4.
        unsafe { *self.frame.amount() = damage }
    }
}
