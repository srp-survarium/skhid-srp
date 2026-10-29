//! player::mt_draw 0x2474A0 at its two lohook sites (sites_026g.tsv): the
//! cull (`player_draw.cull`) and the bones made (`player_draw.bones`).
//! What each site's registers hold is said here, once.

use core::ffi::c_void;

use super::context::{Context, Flag, Flow, NoExit, Reg};
use super::world::{Player, Skeleton, floats};
use crate::raw;

/// The player at `player_draw.held_item` (EBX), when it holds nothing:
/// mt_draw then skips every bone update (the `je 0x24781E` after the span).
pub fn unheld_player<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    Player::at(ctx, Reg::Ebx).filter(|p| p.held() == super::world::Held::Nothing)
}

/// The bones of a player mt_draw made none for, given to the renderer as
/// mt_draw gives its own (update_skeleton 0x22490: the model's render
/// instance, the matrices twice, their count): `bones` model-space
/// row-vector 4x4s, the game's order, at most [`MAX_BONES`]. False if the
/// player has no renderer.
pub fn post_bones(player: &Player<'_>, bones: &[[f32; 16]]) -> bool {
    let Some((scene, instance)) = player.renderer() else { return false };
    let count = bones.len().min(MAX_BONES);
    // SAFETY: the scene renderer and the render instance mt_draw passes
    // (read through the same fields); our matrices, `count` of them, which
    // update_skeleton copies into its command.
    unsafe {
        raw::engine_update_skeleton(scene, instance, bones.as_ptr().cast(), bones.as_ptr().cast(), count as u32)
    };
    true
}

/// The most matrices update_skeleton takes (its abi row: at most 80).
pub const MAX_BONES: usize = 80;

/// What the skip after `player_draw.held_item` leaves: ESI the held item,
/// 0 (the span it skips is `mov esi,[ebx+128h]`).
pub fn no_held_item(ctx: &mut Context<'_>) {
    ctx.set_reg(Reg::Esi, 0);
}

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
/// reads those (its stack array); at_damage wraps the independent simulation
/// array immediately before it updates collision shapes.
#[derive(Clone, Copy)]
enum BonePass { Render, Damage }
impl BonePass {
    fn layer(self) -> u8 { match self { Self::Render => 1, Self::Damage => 2 } }
}

pub struct DrawnBones<'t> {
    pass: BonePass,
    player: Player<'t>,
    skeleton: Skeleton<'t>,
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
        (bones != 0).then_some(DrawnBones { pass: BonePass::Render, player, skeleton: player.skeleton()?, bones: bones as usize as *mut [f32; 16], count })
    }

    /// Damage recomputation at 0x2DA769, immediately before hit_model::update:
    /// EDI player, ESI model-space matrices, EBX non-root count. Uses the
    /// core skeleton, also present in headless simulation and replay.
    pub fn at_damage(ctx: &'t Context<'_>) -> Option<Self> {
        let player = Player::at(ctx, Reg::Edi)?;
        let bones = ctx.reg(Reg::Esi);
        (bones != 0).then_some(Self { pass: BonePass::Damage, player, skeleton: player.core_skeleton()?,
            bones: bones as usize as *mut [f32; 16], count: ctx.reg(Reg::Ebx) })
    }

    pub fn computed(&self, index: u32) -> bool {
        self.skeleton.bone_computed(index + 1, self.pass.layer())
    }

    pub fn skeleton(&self) -> &Skeleton<'t> { &self.skeleton }

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
