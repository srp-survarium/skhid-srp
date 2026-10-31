//! player::mt_draw 0x2474A0 at its two lohook sites (sites_026g.tsv): the
//! cull (`player_draw.cull`) and the bones made (`player_draw.bones`).
//! What each site's registers hold is said here, once.

use super::context::{Context, Flag, Flow, NoExit, Reg};
use super::site;
use super::world::{Player, Skeleton, floats, read};
use crate::raw;

/// The player at `player_draw.held_item`, when it holds nothing: mt_draw
/// then skips every bone update (the `je 0x24781E` after the span).
pub fn unheld_player<'t>(ctx: &'t Context<'_, site::PlayerDrawHeldItem>) -> Option<Player<'t>> {
    Player::of(ctx, raw::site::player_draw_held_item::player(ctx))
        .filter(|p| p.held() == super::world::Held::Nothing)
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
    // (read through the same fields), alive for the draw; our matrices,
    // `count` of them, which update_skeleton copies into its command.
    unsafe {
        raw::engine_update_skeleton(Some(scene), Some(instance), bones.as_ptr().cast(), bones.as_ptr().cast(),
                                    count as u32)
    };
    true
}

/// A command owning copies of all 69 animation matrices and the separate
/// render bone. update_skeleton copies both arrays before returning.
pub fn post_headwear(player: &Player<'_>, bones: &[[f32; 16]], extra: Option<[f32; 16]>) -> bool {
    if bones.len() != super::headwear::BONE || super::headwear::of_player(player).is_none() {
        return false;
    }
    let mut extended = [[0.0; 16]; super::headwear::COUNT];
    extended[..bones.len()].copy_from_slice(bones);
    extended[super::headwear::BONE] = extra.unwrap_or(bones[super::headwear::HEAD]);
    post_bones(player, &extended)
}

/// The most matrices update_skeleton takes (its abi row: at most 80).
pub const MAX_BONES: usize = 80;

/// What the skip after `player_draw.held_item` leaves: ESI the held item,
/// 0 (the span it skips is `mov esi,[ebx+128h]`).
pub fn no_held_item(ctx: &mut Context<'_, site::PlayerDrawHeldItem>) {
    ctx.set_reg(Reg::Esi, 0);
}

/// The player mt_draw culls (at `player_draw.cull`).
pub fn culled_player<'t>(ctx: &'t Context<'_, site::PlayerDrawCull>) -> Option<Player<'t>> {
    Player::of(ctx, raw::site::player_draw_cull::player(ctx))
}

/// The cull's way for a player drawn whatever its model was last seen as:
/// ZF set (the `je` after the span draws), the compare skipped.
pub fn draw_anyway(ctx: &mut Context<'_, site::PlayerDrawCull>) -> Flow<NoExit> {
    ctx.set_flag(Flag::Zf, true);
    Flow::Skip
}

/// mt_draw's bones at `player_draw.bones` (the player and mt_draw's frame):
/// the skeleton's model-space matrices, the game's order (the root,
/// object_movement, has none: matrix `i` is bone `i + 1`'s). Only the draw
/// reads them (its stack array); writing one changes what is drawn alone.
#[derive(Clone, Copy)]
enum BonePass { Render, Damage }
impl BonePass { fn layer(self) -> u8 { match self { Self::Render => 1, Self::Damage => 2 } } }
pub struct DrawnBones<'t> {
    pass: BonePass,
    skeleton: Skeleton<'t>,
    player: Player<'t>,
    bones: *mut [f32; 16],
    count: u32,
}

impl<'t> DrawnBones<'t> {
    pub fn at(ctx: &'t Context<'_, site::PlayerDrawBones>) -> Option<DrawnBones<'t>> {
        use raw::site::player_draw_bones as at;
        Self::of(ctx, at::player(ctx), at::frame(ctx))
    }

    /// The same bones at `player_draw.post_bones` (mt_draw's frame and
    /// player, before update_skeleton's arguments are pushed).
    pub fn at_post_bones(ctx: &'t Context<'_, site::PlayerDrawPostBones>) -> Option<DrawnBones<'t>> {
        use raw::site::player_draw_post_bones as at;
        Self::of(ctx, at::player(ctx), at::frame(ctx))
    }

    /// From a site's player and mt_draw's frame (the player mt_draw draws
    /// and its frame, alive while it draws, on its thread).
    fn of<S>(ctx: &'t Context<'_, S>, player: Option<raw::Player>, frame: Option<raw::MtDrawFrame>) -> Option<DrawnBones<'t>> {
        // SAFETY: mt_draw's frame, whose locals are the bones' array and its
        // count, alive while it draws.
        let (bones, count) = unsafe {
            let frame = frame?;
            (frame.bones(), read(frame.bone_count()))
        };
        let player = Player::of(ctx, player)?;
        (!bones.is_null()).then_some(DrawnBones { pass: BonePass::Render, skeleton: player.skeleton()?, player, bones: bones.cast(), count })
    }

    pub fn at_damage(ctx: &'t Context<'_, site::CarryDamageBones>) -> Option<Self> {
        use raw::site::carry_damage_bones as at;
        let player = Player::of(ctx, at::player(ctx))?;
        let (bones, count) = (at::bones(ctx)?, at::count(ctx));

        // SAFETY: a bone matrix's 16 f32 start at its value.
        Some(Self { pass: BonePass::Damage, skeleton: player.core_skeleton()?,
            player, bones: unsafe { bones.value() }.cast(), count })
    }
    pub fn computed(&self, index: u32) -> bool { self.skeleton.bone_computed(index + 1, self.pass.layer()) }
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

    pub fn post_headwear(&self, extra: Option<[f32; 16]>) -> bool {
        if self.count as usize != super::headwear::BONE {
            return false;
        }
        let mut matrices = [[0.0; 16]; super::headwear::BONE];
        for (i, matrix) in matrices.iter_mut().enumerate() {
            let Some(value) = self.get(i as u32) else { return false };
            *matrix = value;
        }
        post_headwear(&self.player, &matrices, extra)
    }
}
