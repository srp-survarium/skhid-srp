//! The camera and render queue boundaries for the first-person weapon lens.
use core::{mem, ptr::NonNull, slice};
use weapon_modules::DictId;
use super::{Context, Lock, config::ConfigValue, gun_cook::{allocator, free}};
use crate::{attachments::view::{Focal, Profile}, raw};

/// Compared only while submitting this frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewId(u32);

/// Immutable rows copied from the dictionary while its config is retained.
/// Replacing the dictionary drops its old allocation after the lookup lock is released.
struct Profiles {
    first: NonNull<(DictId, Profile)>,
    len: usize,
}

// SAFETY: Profiles exclusively owns this game-allocator block. Its rows are
// plain numbers and readers on either thread access them under PROFILES.
unsafe impl Send for Profiles {}

impl Profiles {
    fn load(ctx: &Context<'_>) -> Option<Self> {
        // SAFETY: the dictionary callback owns the completed requests. Request
        // zero is the static dictionary (binary config, class 1Fh).
        let root = unsafe {
            let result = raw::site::attachments_dictionary_configs::frame(ctx)?.results()?;
            if result.count().read() == 0 { return None; }
            ConfigValue::read(result.results().binary_config()?.root()?).ok()?
        };
        let entries = root.table().ok()?.required(c"srp_weapon_views").ok()?.table().ok()?.entries();
        if entries.is_empty() { return None; }
        let size = u32::try_from(entries.len().checked_mul(mem::size_of::<(DictId, Profile)>())?).ok()?;
        // SAFETY: size is positive and holds one aligned row for each entry.
        let first = NonNull::new(unsafe { raw::engine_malloc(allocator()?, size, c"weapon view profiles".as_ptr().cast()) }.cast())?;
        let mut profiles = Self { first, len: 0 };
        for entry in entries.iter() {
            let row = entry.ok()?.table().ok()?;
            let id = DictId::new(u16::try_from(row.required(c"gun").ok()?.integer().ok()?).ok()?)?;
            let ConfigValue::Pair(fov) = row.required(c"fov").ok()? else { return None };
            let profile = Profile::parse(fov)?;
            // SAFETY: one row per entry, written once within the allocation.
            unsafe { profiles.first.as_ptr().add(profiles.len).write((id, profile)) };
            profiles.len += 1;
        }
        // SAFETY: these len rows are initialized and exclusively owned.
        let rows = unsafe { slice::from_raw_parts_mut(profiles.first.as_ptr(), profiles.len) };
        rows.sort_unstable_by_key(|row| row.0);
        if rows.windows(2).any(|w| w[0].0 == w[1].0) { return None; }
        Some(profiles)
    }

    fn get(&self, id: DictId) -> Option<Profile> {
        // SAFETY: the owner lives across this borrow and all len rows were initialized.
        let rows = unsafe { slice::from_raw_parts(self.first.as_ptr(), self.len) };
        rows.binary_search_by_key(&id, |row| row.0).ok().map(|i| rows[i].1)
    }
}

impl Drop for Profiles {
    fn drop(&mut self) { free(self.first.as_ptr() as usize as u32); }
}

static PROFILES: Lock<Option<Profiles>> = Lock::new(None);

pub(crate) fn load_profiles(ctx: &Context<'_>) {
    let profiles = Profiles::load(ctx);
    drop(PROFILES.with(ctx.thread(), |old| mem::replace(old, profiles)));
}

pub(crate) fn camera(ctx: &Context<'_>) -> Option<(ViewId, Profile, f32)> {
    // SAFETY: update_camera has checked the held weapon and owns its player,
    // game world and inventory entry for this draw.
    let (view, gun, progress) = unsafe {
        let player = raw::site::weapon_view_camera::player(ctx)?;
        let weapon = raw::site::weapon_view_camera::weapon(ctx)?;
        let view = player.game_world()?.camera_scene_view()?;
        (ViewId(view.address()), DictId::new(weapon.item().entry()?.dict_id().read())?, weapon.aim_progress().read())
    };
    let factors = PROFILES.with(ctx.thread(), |profiles| profiles.as_ref()?.get(gun))?;
    Some((view, factors, progress))
}

pub(crate) fn submit(ctx: &Context<'_>, sample: Option<(ViewId, Focal)>) {
    // SAFETY: the producer owns the newly constructed command until the
    // enqueue immediately following this site, including the padding at +14h.
    unsafe {
        let Some(command) = raw::site::weapon_view_submit::command(ctx) else { return };
        let view = command.view().map(|view| ViewId(view.address()));
        let focal = sample.filter(|(id, _)| Some(*id) == view);
        command.weapon_focal_y().write(focal.map_or(0.0, |(_, focal)| focal.value()));
    }
}

pub(crate) fn queued(ctx: &Context<'_>) -> Option<Focal> {
    // SAFETY: the bound draw_scene_impl receives its executing command's
    // inline payload at +98h. submit initialized this field before publication.
    unsafe { Focal::decode(raw::site::weapon_view_begin::params(ctx)?.command().weapon_focal_y().read()) }
}

pub(crate) fn project(ctx: &Context<'_>, focal: Focal) {
    // SAFETY: foreground_begin just copied the projection to its own stack.
    // Only the two diagonal scale elements are replaced before it uses them.
    unsafe {
        let Some(frame) = raw::site::weapon_view_foreground::frame(ctx) else { return };
        if let Some([x, y]) = focal.diagonal(frame.projection_x().read(), frame.projection_y().read()) {
            frame.projection_x().write(x);
            frame.projection_y().write(y);
        }
    }
}
