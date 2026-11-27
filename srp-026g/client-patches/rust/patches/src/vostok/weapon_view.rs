//! Patch 161's first-person projection, the game's side
//! (`docs/notes/attachments-scopes.md`, "Why the EOTech looks small on the
//! A-545"): what update_camera holds (`weapon_view.camera` with a gun in
//! hand, `weapon_view.no_gun` with anything else or nothing), the lens the
//! scene's draw command carries to the render thread (`weapon_view.submit`,
//! read back at `weapon_view.begin`) and the foreground pass's projection
//! (`weapon_view.foreground`); the guns' profiles, the dictionary's
//! `srp_weapon_views` ([`load_profiles`]); patch 219's eye move at the hands'
//! aim alignment (`hands_ik.aim_alignment`, `hands_ik.aim_aligned`).
use core::{mem, ptr::NonNull, slice};
use weapon_modules::DictId;
use super::{Context, Lock, attachments::{self, Named, WeaponId}, config::{ConfigError, ConfigValue}, gun_cook::{allocator, free}, log::{Piece, line}, world::{floats, read, write_floats}};
use crate::{attachments::view::{Focal, GunView, Profile}, raw};

/// A scene view by its address: compared, never read through.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewId(u32);

/// The optic a held gun draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Optic {
    /// None: its iron sights.
    None,
    /// The modules table's, fitted by `data/srp_scope`: one of 0.69d0's.
    Fitted,
    /// Its own (`addons/rifle_scope_dict_id`): one of 0.26g's.
    Own,
}

/// A gun in first-person hands, as the camera samples it.
#[derive(Clone, Copy)]
pub(crate) struct HeldGun {
    pub(crate) weapon: WeaponId,
    /// Its item's dict id (the log's).
    pub(crate) gun: Option<DictId>,
    /// The gun and its fitted modules, when patch 161 keeps it: its eyes
    /// are the table's.
    pub(crate) named: Option<Named>,
    /// Its item's row; None for a gun the dictionary gives none.
    pub(crate) view: Option<GunView>,
    /// The aim's progress, 0 at the hip to 1 fully aimed.
    pub(crate) progress: f32,
    pub(crate) optic: Optic,
    /// Its `parameters/aim_offset` as the cook made it (+6A0h's
    /// translation): how far its own eye goes along its line.
    pub(crate) aim_offset: f32,
}

/// The rows copied from the dictionary while its config is retained, sorted
/// by gun: memory of the game's allocator, freed with them.
struct Profiles {
    first: NonNull<(DictId, GunView)>,
    len: usize,
}

// SAFETY: Profiles alone owns its block, whose rows are plain numbers; the
// game's and the render thread reach them only inside PROFILES's lock.
unsafe impl Send for Profiles {}

/// Why the dictionary gives no profiles.
#[derive(Clone, Copy)]
enum NoViews {
    /// Not the dictionary's query, or a dictionary without `srp_weapon_views`
    /// (content without them): nothing to say.
    NotThere,
    /// `srp_weapon_views` is no table of rows.
    Malformed,
    OutOfMemory,
}

/// Why a row of `srp_weapon_views` is left out.
#[derive(Clone, Copy)]
enum BadRow {
    /// No table, or `gun` no dict id, or `fov` no float2.
    Shape,
    /// A factor outside (0, 1].
    Factor,
    /// `aim_offset_026g` no number of a metre or less.
    AimOffset,
    /// A gun an earlier row gives.
    Twice,
}

fn row(value: Result<ConfigValue<'_>, ConfigError>) -> Result<(DictId, GunView), BadRow> {
    let row = value.and_then(ConfigValue::table).map_err(|_| BadRow::Shape)?;
    let gun = row.required(c"gun").and_then(ConfigValue::integer).map_err(|_| BadRow::Shape)?;
    let gun = u16::try_from(gun).ok().and_then(DictId::new).ok_or(BadRow::Shape)?;
    let Ok(ConfigValue::Pair(fov)) = row.required(c"fov") else { return Err(BadRow::Shape) };
    let profile = Profile::parse(fov).ok_or(BadRow::Factor)?;
    let aim_offset_026g = match row.get(c"aim_offset_026g").map_err(|_| BadRow::AimOffset)? {
        None => None,
        Some(value) => Some(value.number().map_err(|_| BadRow::AimOffset)?),
    };
    Ok((gun, GunView::parse(profile, aim_offset_026g).ok_or(BadRow::AimOffset)?))
}

impl Profiles {
    /// The rows of the dictionary at `attachments.dictionary_configs`; a row
    /// that is not one is left out with a line.
    fn load(ctx: &Context<'_>) -> Result<Self, NoViews> {
        // SAFETY: the site's ESP, on_configs_loaded's entry.
        let frame = unsafe { raw::site::attachments_dictionary_configs::frame(ctx) }.ok_or(NoViews::NotThere)?;
        // SAFETY: [esp+4], its queries_result, alive while the callback runs
        // on this thread.
        let results = unsafe { frame.results() }.ok_or(NoViews::NotThere)?;
        // SAFETY: the results' count, which the callback reads too (0x2C8AF5).
        if unsafe { read(results.count()) } == 0 {
            return Err(NoViews::NotThere);
        }
        // SAFETY: the results' first, there by the count.
        let first = unsafe { results.results() };
        // SAFETY: the first request's, the static dictionary: a binary config
        // (class 1Fh, read so at 0x2C8AF5).
        let config = unsafe { first.binary_config() }.ok_or(NoViews::NotThere)?;
        // SAFETY: that config's root, relocated by its loader.
        let root = unsafe { config.root() }.ok_or(NoViews::NotThere)?;
        // SAFETY: the callback holds the config for its run, which these views do not outlive.
        let root = unsafe { ConfigValue::read(root) }.and_then(ConfigValue::table).map_err(|_| NoViews::NotThere)?;
        let views = root.get(c"srp_weapon_views").map_err(|_| NoViews::Malformed)?.ok_or(NoViews::NotThere)?;
        let entries = views.table().map_err(|_| NoViews::Malformed)?.entries();
        let size = u32::try_from(entries.len().max(1) * mem::size_of::<(DictId, GunView)>()).map_err(|_| NoViews::Malformed)?;
        let allocator = allocator().ok_or(NoViews::OutOfMemory)?;
        // SAFETY: the game's allocator, alive from before any cook; room for a row an entry.
        let memory = unsafe { raw::engine_malloc(allocator, size, c"srp weapon views".as_ptr().cast()) };
        let mut profiles = Self { first: NonNull::new(memory.cast()).ok_or(NoViews::OutOfMemory)?, len: 0 };
        for (index, value) in entries.iter().enumerate() {
            let made = row(value).and_then(|(gun, profile)| match profiles.get(gun) {
                Some(_) => Err(BadRow::Twice),
                None => Ok((gun, profile)),
            });
            match made {
                Ok(made) => {
                    // SAFETY: row `len` of the block's `entries.len()` (fewer are written than entries).
                    let at = unsafe { profiles.first.add(profiles.len) };
                    // SAFETY: that row, unwritten, written once.
                    unsafe { at.write(made) };
                    profiles.len += 1;
                    profiles.rows().sort_unstable_by_key(|(gun, _)| *gun);
                }
                Err(why) => refused(index, why),
            }
        }
        Ok(profiles)
    }

    fn rows(&mut self) -> &mut [(DictId, GunView)] {
        // SAFETY: `len` rows written, ours alone.
        unsafe { slice::from_raw_parts_mut(self.first.as_ptr(), self.len) }
    }

    fn get(&self, gun: DictId) -> Option<GunView> {
        // SAFETY: `len` rows written; the borrow ends before any is written again.
        let rows = unsafe { slice::from_raw_parts(self.first.as_ptr(), self.len) };
        rows.binary_search_by_key(&gun, |(id, _)| *id).ok().map(|at| rows[at].1)
    }
}

impl Drop for Profiles {
    fn drop(&mut self) {
        free(self.first.as_ptr() as usize as u32);
    }
}

fn refused(index: usize, why: BadRow) {
    let text = match why {
        BadRow::Shape => "weapon views: a row is not {gun, fov}, row ",
        BadRow::Factor => "weapon views: a row's factor is outside (0, 1], row ",
        BadRow::Twice => "weapon views: a row's gun has a row before it, row ",
        BadRow::AimOffset => "weapon views: a row's aim_offset_026g is no number of a metre or less, row ",
    };
    line(&[Piece::Text(text), Piece::Dec(index as u32)]);
}

static PROFILES: Lock<Option<Profiles>> = Lock::new(None);

/// `attachments.dictionary_configs`: the profiles of the dictionary being
/// made, in place of the last one's (a game without them: none).
pub(crate) fn load_profiles(ctx: &Context<'_>) {
    let profiles = match Profiles::load(ctx) {
        Ok(profiles) => Some(profiles),
        Err(why) => {
            match why {
                NoViews::NotThere => {}
                NoViews::Malformed => line(&[Piece::Text("weapon views: srp_weapon_views is not a table of rows")]),
                NoViews::OutOfMemory => line(&[Piece::Text("weapon views: no memory for the profiles")]),
            }
            None
        }
    };
    // the old block freed outside the lock
    drop(PROFILES.with(ctx.thread(), |old| mem::replace(old, profiles)));
}

/// The scene view a player's camera draws into.
fn scene_view(player: raw::Player) -> Option<ViewId> {
    // SAFETY: update_camera's player (EBX at both sites), alive for the frame.
    let world = unsafe { player.game_world() }?;
    // SAFETY: its game world's referenced scene view, handed to
    // set_view_matrix at 0x23DE84, alive with the world.
    let view = unsafe { world.camera_scene_view() }?;
    Some(ViewId(view.address()))
}

/// `weapon_view.camera`: the gun update_camera aims in first person.
pub(crate) fn gun_camera(ctx: &Context<'_>) -> Option<(ViewId, HeldGun)> {
    // SAFETY: the site's EBX, update_camera's player (0x1F348A).
    let view = scene_view(unsafe { raw::site::weapon_view_camera::player(ctx) }?)?;
    // SAFETY: the site's EAX, the player's held item (+128h, 0x1F34D2), which
    // the game's own cast (vtable+70h, 0x1F34C5) took for a client weapon and
    // reads as one around the site (+1808h at 0x1F34D8, +5FCh at 0x1F350F).
    let weapon = unsafe { raw::site::weapon_view_camera::weapon(ctx) }?;
    // SAFETY: that weapon's aim progress, which the game reads next (0x1F350F).
    let progress = unsafe { read(weapon.aim_progress()) };
    let id = WeaponId::of(weapon);
    let named = attachments::kept(ctx.thread(), id).map(|kept| kept.named);
    // SAFETY: its optic (+1808h), which the game reads at 0x1F34D8.
    let optic = match unsafe { weapon.optics() } {
        None => Optic::None,
        Some(_) if named.is_some_and(|named| named.wanted.scope.is_some()) => Optic::Fitted,
        Some(_) => Optic::Own,
    };
    // SAFETY: the aim offset matrix's translation z (+6D8h), which the cook
    // wrote once and calculate_aim_alignment reads.
    let aim_offset = unsafe { read(weapon.aim_offset_z()) };
    // SAFETY: the weapon's inventory item (+10h), its own.
    let item = unsafe { weapon.item() };
    // SAFETY: that item's dictionary entry, the dictionary's, made before any weapon.
    let entry = unsafe { item.entry() };
    // SAFETY: that entry's dict id.
    let gun = entry.and_then(|entry| DictId::new(unsafe { read(entry.dict_id()) }));
    let row = gun.and_then(|gun| PROFILES.with(ctx.thread(), |profiles| profiles.as_ref()?.get(gun)));
    Some((view, HeldGun { weapon: id, gun, named, view: row, progress, optic, aim_offset }))
}

/// `weapon_view.no_gun`: update_camera in first person with no gun in hand.
pub(crate) fn bare_camera(ctx: &Context<'_>) -> Option<ViewId> {
    // SAFETY: the site's EBX, update_camera's player (0x1F348A).
    scene_view(unsafe { raw::site::weapon_view_no_gun::player(ctx) }?)
}

/// `weapon_view.submit`: the lens `sample` gives the command's own view, in
/// the command; 0 for none.
pub(crate) fn submit(ctx: &Context<'_>, sample: Option<(ViewId, Focal)>) {
    // SAFETY: the site's EAX, the command draw_scene just made (0x1F8EE), or
    // 0 when its allocation failed.
    let Some(command) = (unsafe { raw::site::weapon_view_submit::command(ctx) }) else { return };
    // SAFETY: the command is the producer's until the push right after the
    // site (0x1F8FF); its params' second reference at +9Ch (0x20500).
    let view = unsafe { command.view() }.map(|view| ViewId(view.address()));
    let focal = sample.filter(|(id, _)| Some(*id) == view).map_or(0.0, |(_, focal)| focal.value());
    // SAFETY: +14h, which the constructor leaves (0x20310 writes +0, +4,
    // +10h, +11h, +54h and from +58h on): every draw_scene command is made at
    // 0x1F8EE, its one caller, so this write is before its publication, and
    // the render channel runs the command itself, never a copy (0x1F2C0 links
    // it; on_defer_execution, 0x28620, is handed the command).
    unsafe { command.weapon_focal_y().write(focal) };
}

/// `weapon_view.begin`: the lens of the command being drawn.
pub(crate) fn queued(ctx: &Context<'_>) -> Option<Focal> {
    // SAFETY: the site's ESI, the params draw_scene_impl was bound with: the
    // inline payload at +98h of the command running it (0x28460 passes it;
    // draw_scene_impl's one binding, 0x1F8B8).
    let params = unsafe { raw::site::weapon_view_begin::params(ctx) }?;
    // SAFETY: the command around those params.
    let command = unsafe { params.command() };
    // SAFETY: the field submit wrote before the command was published.
    Focal::decode(unsafe { read(command.weapon_focal_y()) })
}

/// `weapon_view.foreground`: the foreground pass's projection takes `focal`.
pub(crate) fn project(ctx: &Context<'_>, focal: Focal) {
    // SAFETY: the site's ESP, foreground_begin's own stack copy of the
    // projection (0x13E0C0), used after the site.
    let Some(frame) = (unsafe { raw::site::weapon_view_foreground::frame(ctx) }) else { return };
    // SAFETY: its first diagonal scale term, the frame's.
    let world_x = unsafe { read(frame.projection_x()) };
    // SAFETY: its second, likewise.
    let world_y = unsafe { read(frame.projection_y()) };
    let [x, y] = focal.diagonal(world_x, world_y);
    // SAFETY: the first, the frame's to change before foreground_begin uses it.
    unsafe { frame.projection_x().write(x) };
    // SAFETY: the second, likewise.
    unsafe { frame.projection_y().write(y) };
}

/// `hands_ik.aim_alignment`: the weapon whose hands are about to be aligned
/// to its eye and the camera's line there (the Item bone's third row).
pub(crate) fn aligning(ctx: &Context<'_>) -> Option<(WeaponId, [f32; 3])> {
    // SAFETY: process_hands_ik's frame: the held weapon (kept at 0x295A09,
    // tested at 0x295A0F) and the call's pushed argument, the Item bone's
    // matrix in the user's bones (0x295AA4), both alive while mt_draw's bone
    // pass runs on this thread.
    let frame = unsafe { raw::site::hands_ik_aim_alignment::frame(ctx) }?;
    // SAFETY: as above.
    let weapon = unsafe { frame.weapon() }?;
    // SAFETY: as above.
    let item = unsafe { frame.item() }?;
    // SAFETY: that matrix's sixteen floats, read before the call reads them.
    let item: [f32; 16] = unsafe { floats(item.value()) };
    Some((WeaponId::of(weapon), [item[8], item[9], item[10]]))
}

/// `hands_ik.aim_aligned`: the alignment calculate_aim_alignment just made
/// moved by `by` (the translation's row).
pub(crate) fn aligned(ctx: &Context<'_>, by: [f32; 3]) {
    // SAFETY: the site's ESI, the frame's matrix the call wrote (esp+24h),
    // read next by the blend or copied to the Weapon bone, on this thread.
    let Some(result) = (unsafe { raw::site::hands_ik_aim_aligned::result(ctx) }) else { return };
    // SAFETY: as above, its sixteen floats.
    let mut matrix: [f32; 16] = unsafe { floats(result.value()) };
    for (place, by) in matrix[12..15].iter_mut().zip(by) {
        *place += by;
    }
    // SAFETY: as above, written whole before its one reader runs.
    unsafe { write_floats(result.value(), matrix) };
}
