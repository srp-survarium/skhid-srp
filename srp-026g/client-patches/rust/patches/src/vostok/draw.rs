//! player::mt_draw 0x2474A0 at its two lohook sites (sites_026g.tsv): the
//! cull (`player_draw.cull`) and the bones made (`player_draw.bones`).
//! What each site's registers hold is said here, once.

use core::ffi::c_void;

use super::context::{Context, Flag, Flow, NoExit, Reg};
use super::world::{Player, floats};
use crate::raw;

/// The player mt_draw culls (EBX at `player_draw.cull`).
pub fn culled_player<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    Player::at(ctx, Reg::Ebx)
}

/// The cull's way for a player drawn whatever its model was last seen as:
/// ZF set (the `je` after the span draws), the compare skipped.
pub fn draw_anyway(ctx: &mut Context<'_>) -> Flow<NoExit> {
    ctx.set_flag(Flag::Zf, true);
    Flow::Skip
}

/// mt_draw's bones at `player_draw.bones` (EBX the player, EBP its frame):
/// the skeleton's model-space matrices, the game's order (the root,
/// object_movement, has none: matrix `i` is bone `i + 1`'s). Only the draw
/// reads them (its stack array); writing one changes what is drawn alone.
pub struct DrawnBones<'t> {
    player: Player<'t>,
    bones: *mut [f32; 16],
    count: u32,
}

impl<'t> DrawnBones<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<DrawnBones<'t>> {
        let player = Player::at(ctx, Reg::Ebx)?;
        let frame = ctx.reg(Reg::Ebp) as usize as *mut c_void;
        // SAFETY: mt_draw's frame at the site (EBP): its locals, the bones'
        // array and its count.
        let (bones, count) =
            unsafe { (super::world::read(raw::mt_draw_frame::bones(frame)), super::world::read(raw::mt_draw_frame::bone_count(frame))) };
        (bones != 0).then_some(DrawnBones { player, bones: bones as usize as *mut [f32; 16], count })
    }

    pub fn player(&self) -> Player<'t> {
        self.player
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// The `index`th matrix (row-vector 4x4), below the count.
    pub fn get(&self, index: u32) -> Option<[f32; 16]> {
        // SAFETY: a matrix of the array (below its count), for the draw.
        (index < self.count).then(|| unsafe { floats(self.bones.add(index as usize).cast()) })
    }

    /// Writes the `index`th matrix, below the count.
    pub fn set(&self, index: u32, matrix: [f32; 16]) {
        if index < self.count {
            // SAFETY: as get; the draw's own array.
            unsafe { self.bones.add(index as usize).write_volatile(matrix) }
        }
    }
}
