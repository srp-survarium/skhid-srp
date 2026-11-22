//! `game_material_manager::material_pair` (0x306D50) at its start
//! (`material_pair.entry`, patch 193): it indexes its two u16 ids into the
//! manager's 128 material slots (+108h) and the 128 x 128 pair slots (+308h)
//! with no bound. A step's foot material read before its table (patch 101)
//! and a ground of a shape without a material table (get_triangle_material
//! 0x3570A0's 0FFFFh, which on_step and a grenade's bounce pass as it is)
//! read past the manager and hand on whatever is there.

use super::bullet::MaterialId;
use super::context::{Context, Reg};
use super::world::read;
use crate::raw;

/// One of material_pair's ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairId {
    /// A slot of the table.
    Slot(MaterialId),
    /// 3FFFh: a face the cook gave no material (get_triangle_material's
    /// mask). Its read is the pair slot [127][127], inside the manager and
    /// empty while there are fewer than 128 materials: the default, as the
    /// game has it.
    Unnamed,
    /// Past the table: 0FFFFh, or a value no material has.
    Outside(u16),
}

impl PairId {
    fn parse(register: u32) -> PairId {
        let id = register as u16;
        match MaterialId::parse(u32::from(id)) {
            Some(slot) => PairId::Slot(slot),
            None if id == 0x3FFF => PairId::Unnamed,
            None => PairId::Outside(id),
        }
    }

    fn value(self) -> u32 {
        match self {
            PairId::Slot(slot) => slot.value(),
            PairId::Unnamed => 0x3FFF,
            PairId::Outside(id) => u32::from(id),
        }
    }
}

/// material_pair's call, at its start.
pub struct PairCall {
    manager: raw::GameMaterialManager,
    first: PairId,
    second: PairId,
}

impl PairCall {
    /// The call at `material_pair.entry`; None without a manager (the game's
    /// first read faults then, as before).
    pub fn at_entry(ctx: &Context<'_>) -> Option<PairCall> {
        use raw::site::material_pair_entry as site;
        // SAFETY: the context is this site's: ECX the manager, DX and DI the
        // ids (their high halves are not read, 0x306D50 / 0x306D84).
        unsafe {
            Some(PairCall {
                manager: site::manager(ctx)?,
                first: PairId::parse(site::first(ctx)),
                second: PairId::parse(site::second(ctx)),
            })
        }
    }

    /// The first id past the table, of either.
    pub fn outside(&self) -> Option<u16> {
        [self.first, self.second].into_iter().find_map(|id| match id {
            PairId::Outside(id) => Some(id),
            PairId::Slot(_) | PairId::Unnamed => None,
        })
    }

    /// Answers the call itself, an id past the table taken as the default
    /// material's (what material_pair gives an id whose slot is empty): the
    /// game's material_pair of the bounded ids, its result in EAX for the
    /// exit at its `ret`. A default past the table too: no pair (0, which
    /// every caller tests).
    pub fn answer_bounded(&self, ctx: &mut Context<'_>) {
        // SAFETY: the manager the game called material_pair with, alive for
        // the call; its default id read only; the nested call is the game's
        // own with ids inside its tables (this hook lets it through).
        let pair = unsafe {
            let default = PairId::parse(u32::from(read(self.manager.default_id())));
            let bounded = |id| match id {
                PairId::Outside(_) => default,
                PairId::Slot(_) | PairId::Unnamed => id,
            };
            match (bounded(self.first), bounded(self.second)) {
                (PairId::Outside(_), _) | (_, PairId::Outside(_)) => None,
                (first, second) => raw::engine_material_pair(Some(self.manager), first.value(), second.value()),
            }
        };
        ctx.set_reg(Reg::Eax, pair.map_or(0, |pair| pair.address()));
    }
}
