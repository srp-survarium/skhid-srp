//! A magazine reload and its `reload_commit` event (patch 205,
//! docs/notes/reload-commit.md).
//!
//! The game's reload loads the magazine once, at its clip's end:
//! weapon_core_reload_state_base::on_animation_end_impl (0x310E40) tops the
//! magazine up from the ammunition stack (load_magazine 0x2E17D0), resets the
//! recoil and clears m_need_to_auto_reload. A reload left before that end (a
//! melee, a sprint, a weapon switch, a grenade) loads nothing, however far its
//! clip went: the new magazine seen in the gun is not counted. 0.69d0 loads it
//! at an event of the item clip instead (its reload's `shoot` channel, domain
//! 1: weapon_core_reload_state_base::on_user_may_shoot_event 0x20038EBB0).
//!
//! Patch 205 does the same with a channel of its own, `reload_commit`, which the
//! content puts at each magazine reload clip's seat (content_edit.py
//! reload_commit): a magazine reload (a state whose slot 9 is 0x310E40) is
//! subscribed to it where the game subscribes it to its clip's end (its
//! initialize, and a world's deserialize), and unsubscribed where the game
//! unsubscribes that (its finalize). At the event the game's own
//! on_animation_end_impl runs, without the clip's end: the reload's state
//! goes on as before, and its end loads again. Loading twice is the same as
//! once ([`ChamberOnReload`]), so the commit needs no state of its own and a
//! world needs nothing new.

use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;

/// The channel's name, as the content writes it.
const CHANNEL: &[u8] = b"reload_commit\0";

/// The channel's one domain value (content_edit.py RELOAD_COMMIT_SEATED).
const SEATED: u8 = 1;

/// An event of the `reload_commit` channel, as patch 205 reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadEvent {
    /// The new ammunition is in the gun: the magazine counts.
    Seated,
}

impl ReloadEvent {
    /// The event of a domain value; None for a value the content never writes.
    pub fn parse(domain: u8) -> Option<ReloadEvent> {
        (domain == SEATED).then_some(ReloadEvent::Seated)
    }
}

/// A reloading weapon's chamber, as its reload's load sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamberOnReload {
    /// The weapon does not chamber a round on reload (chamber_a_round_on_reload
    /// false, +5F2h): the load is a top-up, the same twice as once.
    No,
    /// It chambers one on reload and its chamber is empty: the reload's
    /// initialize put the round back into the magazine (0x30FDD2), as the load
    /// wants it.
    Empty,
    /// It chambers one and one is chambered: an earlier load of this reload
    /// chambered it. Back into the magazine before the load, which tops the
    /// magazine up to the same rounds and chambers it again; without that the
    /// load would take one more round from the stack and chamber it over the
    /// one there (0x2E18C9), a round lost.
    Chambered,
}

impl ChamberOnReload {
    fn parse(chamber_on_reload: u8, chamber: u8) -> ChamberOnReload {
        match (chamber_on_reload != 0, chamber != 0) {
            (false, _) => ChamberOnReload::No,
            (true, false) => ChamberOnReload::Empty,
            (true, true) => ChamberOnReload::Chambered,
        }
    }
}

/// A weapon state at one of the three sites, when it is a magazine reload.
pub struct MagazineReload<'t> {
    state: raw::AnimationEndAwareState,
    channels: raw::EventChannels,
    _thread: &'t Thread,
}

/// A state the sites give whose class's slot 9 is the reload's end, and its
/// user's animation player: the channels its events come from.
///
/// # Safety
/// `state` is the live state of the site, on the game's thread.
unsafe fn magazine_reload(state: Option<raw::AnimationEndAwareState>) -> Option<(raw::AnimationEndAwareState, raw::EventChannels)> {
    // SAFETY: as the caller promises; a state's vtable is the image's, its
    // weapon and the weapon's user the game's (each site's code reads them
    // the same way, 0x30DAAB, 0x30DC0B, 0x30E453).
    unsafe {
        let state = state?;
        let methods = state.methods()?;
        if read(methods.on_animation_end()) != raw::image::RELOAD_ON_ANIMATION_END {
            return None;
        }
        let user = state.weapon()?.user()?;
        Some((state, user.event_channels()))
    }
}

impl<'t> MagazineReload<'t> {
    /// At `reload.listen` (an animation-end-aware state's initialize).
    pub fn at_initialize(ctx: &'t Context<'_>) -> Option<MagazineReload<'t>> {
        // SAFETY: the context is this site's, EBP the state being initialized.
        let (state, channels) = unsafe { magazine_reload(raw::site::reload_listen::state(ctx))? };
        Some(MagazineReload { state, channels, _thread: ctx.thread() })
    }

    /// At `reload.listen_again` (a world's state deserialized).
    pub fn at_deserialize(ctx: &'t Context<'_>) -> Option<MagazineReload<'t>> {
        // SAFETY: the context is this site's, EBX the state being read.
        let (state, channels) = unsafe { magazine_reload(raw::site::reload_listen_again::state(ctx))? };
        Some(MagazineReload { state, channels, _thread: ctx.thread() })
    }

    /// At `reload.stop_listening` (a reload's finalize).
    pub fn at_finalize(ctx: &'t Context<'_>) -> Option<MagazineReload<'t>> {
        // SAFETY: the context is this site's, ESI the state being finalized.
        let (state, channels) = unsafe { magazine_reload(raw::site::reload_stop_listening::state(ctx))? };
        Some(MagazineReload { state, channels, _thread: ctx.thread() })
    }

    /// Subscribes the state to its user's `reload_commit` events, its handler
    /// `entry` (patch 205's `on_event`), the state the subscription's key as
    /// the game's own clip-end subscription has it (0x30DABB).
    pub fn listen(&self, entry: super::hook::CallbackEntry<super::OrigReloadEvent>) {
        let mut functor = raw::EventFunctorStorage::zeroed();
        let mut resource = raw::EventResourceStorage::zeroed();
        // SAFETY: the functor and the null resource slot are ours, alive for
        // the call (the game copies the functor, 0x390DC0, and reads the
        // slot's pointer); the channels are the state's user's animation
        // player, alive while its weapon is; the name a static string.
        unsafe {
            let made = functor.handle();
            // A bound member functor as 0x30DCE0 makes one: the manager
            // tagged 1 (copied as bytes), the function, no adjustment, the state.
            *made.manager() = raw::image::EVENT_MEMBER_FUNCTOR | 1;
            *made.buffer().function() = entry.entry() as usize as u32;
            *made.buffer().adjust() = 0;
            *made.buffer().object() = self.state.address();
            raw::engine_subscribe_event_channel(Some(self.channels), CHANNEL.as_ptr(), Some(made),
                                                self.state.address() as usize as *const core::ffi::c_void,
                                                resource.handle().resource(), 0);
        }
    }

    /// Unsubscribes what [`MagazineReload::listen`] subscribed (nothing when it
    /// did not: the game's search finds none, 0x39221F).
    pub fn stop_listening(&self) {
        // SAFETY: the channels alive as above; the name a static string; the
        // state only a key.
        unsafe {
            raw::engine_unsubscribe_event_channel(CHANNEL.as_ptr(), Some(self.channels),
                                                  self.state.address() as usize as *const core::ffi::c_void);
        }
    }
}

/// The state a `reload_commit` event is delivered to: a magazine reload, the
/// only object patch 205 subscribes.
pub struct ReloadEventState<'t> {
    state: Option<raw::AnimationEndAwareState>,
    _thread: &'t Thread,
}

impl<'t> ReloadEventState<'t> {
    /// # Safety
    /// The subscription's object: a magazine reload subscribed by
    /// [`MagazineReload::listen`] (alive: finalize unsubscribes it), on the
    /// game's thread.
    pub(in crate::vostok) unsafe fn from_raw(thread: &'t Thread, state: Option<raw::AnimationEndAwareState>) -> ReloadEventState<'t> {
        ReloadEventState { state, _thread: thread }
    }

    /// The reload's own event, when `event` is one: of this state's weapon or
    /// its user (as the clip-end callback takes one, 0x30DB74..0x30DB7B).
    pub fn event(&self, event: &AnimationEvent<'_>) -> Option<ReloadEvent> {
        // SAFETY: the state alive as from_raw says; its weapon and user the
        // game's; the event's parameters alive for the callback.
        unsafe {
            let weapon = self.state?.weapon()?;
            let object = read(event.params?.object());
            let ours = object == weapon.address() || weapon.user().is_some_and(|user| user.address() == object);
            if !ours {
                return None;
            }
            ReloadEvent::parse(read(event.params?.domain()))
        }
    }

    /// Loads the magazine as the reload's end does (on_animation_end_impl
    /// 0x310E40, its load made idempotent by `reload.load_at_end`), without
    /// ending the reload: its done byte is ours.
    pub fn commit(&self) {
        let Some(state) = self.state else { return };
        let (mut unused, mut done) = (0u8, 0u8);
        // SAFETY: the state is a live magazine reload (from_raw); the bytes ours.
        unsafe { raw::engine_reload_commit(Some(state), &mut unused, &mut done) };
    }
}

/// An animation event's parameters, as the game hands them to a subscriber.
pub struct AnimationEvent<'t> {
    params: Option<raw::AnimationCallbackParams>,
    _thread: &'t Thread,
}

impl<'t> AnimationEvent<'t> {
    /// # Safety
    /// The parameters the game passes the callback, alive for it.
    pub(in crate::vostok) unsafe fn from_raw(thread: &'t Thread, params: Option<raw::AnimationCallbackParams>) -> AnimationEvent<'t> {
        AnimationEvent { params, _thread: thread }
    }
}

/// The weapon of a reload's end, at its load (`reload.load_at_end`).
pub struct ReloadLoad<'t> {
    weapon: raw::Weapon,
    chamber: ChamberOnReload,
    _thread: &'t Thread,
}

impl<'t> ReloadLoad<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ReloadLoad<'t>> {
        // SAFETY: the context is this site's, ECX the live reload state whose
        // weapon on_animation_end_impl reads next (0x310E41) and loads.
        unsafe {
            let weapon = raw::site::reload_load_at_end::state(ctx)?.weapon()?;
            let chamber = ChamberOnReload::parse(read(weapon.chamber_on_reload()), read(weapon.chamber()));
            Some(ReloadLoad { weapon, chamber, _thread: ctx.thread() })
        }
    }

    pub fn chamber(&self) -> ChamberOnReload {
        self.chamber
    }

    /// The chambered round back into the magazine, as the reload's
    /// initialize does (0x30FDD2..0x30FDD8).
    pub fn unchamber(&self) {
        if self.chamber != ChamberOnReload::Chambered {
            return;
        }
        // SAFETY: the weapon alive (at's); its two bytes, as the game writes them.
        unsafe {
            let loaded = self.weapon.loaded();
            *loaded = read(loaded).wrapping_add(1);
            *self.weapon.chamber() = 0;
        }
    }
}

/// One of the two clip sets a two-clip reload chooses between, as its picks
/// (get_weapon_lexeme and the hands') index them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadClip {
    /// State 314's tactical clips; state 311's one shell's.
    First,
    /// State 314's empty clips; state 311's two shells'.
    Second,
}

impl ReloadClip {
    fn of(second: bool) -> ReloadClip {
        if second { ReloadClip::Second } else { ReloadClip::First }
    }

    fn bit(self) -> u32 {
        match self {
            ReloadClip::First => 0,
            ReloadClip::Second => 1,
        }
    }
}

/// The reload classes' clip table: the state's own +144h, four pairs of
/// handles, pick (0 or 1) + 2 * posture (every constructor: 0x30E2FA,
/// 0x30F6AC, 0x30FF8C).
const PICKS_OFFSET: u32 = 0x144;
const PICK_BYTES: u32 = 8;
const PICKS: u32 = 4;
/// m_index_of_animation_to_wait before a reload's first pick (initialize
/// 0x30DA60, finalize 0x30E476).
const NOT_PICKED: u8 = 0xFF;

/// The clip set a reload state's m_index_of_animation_to_wait (+134h) names:
/// what its get_weapon_lexeme wrote, `(pick * 8 + state + 144h - [+130h]) >> 2`
/// (0x30FEE4, 0x30F5FA), the world's byte after a deserialize (0x30DC7C).
/// None before the reload's first pick (0FFh), or for an index that is not a
/// pick of the table.
fn saved_clip(index: u8, handles: u32, state: u32) -> Option<ReloadClip> {
    if index == NOT_PICKED {
        return None;
    }
    let offset = (u32::from(index) * 4).wrapping_add(handles).wrapping_sub(state.wrapping_add(PICKS_OFFSET));
    if offset % PICK_BYTES != 0 || offset / PICK_BYTES >= PICKS {
        return None;
    }
    Some(ReloadClip::of((offset / PICK_BYTES) & 1 == 1))
}

/// Where a two-clip reload picks a clip set, and the register its pick is in
/// there (sites_026g.tsv's reload.pick_*).
#[derive(Clone, Copy)]
enum PickSite {
    /// 314's weapon clips (reload.pick_empty): EBX, before +134h is written.
    Magazine,
    /// 314's hands (reload.pick_empty_hands): EAX, after.
    MagazineHands,
    /// 311's weapon clips (reload.pick_two_shells): ECX, before.
    Shells,
    /// 311's hands (reload.pick_two_shells_hands): ESI, after.
    ShellHands,
}

/// A two-clip reload's pick at one of its sites: the game's own (from the
/// weapon's rounds now) in a register, and the clip set the reload picked
/// before, which the game keeps in +134h (and in a world).
pub struct ClipPick<'c, 't> {
    ctx: &'c mut Context<'t>,
    register: super::Reg,
    saved: Option<ReloadClip>,
    loaded: u8,
}

impl<'c, 't> ClipPick<'c, 't> {
    fn at(ctx: &'c mut Context<'t>, site: PickSite) -> Option<ClipPick<'c, 't>> {
        use super::Reg;
        // SAFETY: the context is this site's, EDI the live reload state the
        // pick reads (every site's code reads +120h, +130h, +134h of it the
        // same way); its weapon the game's.
        let (state, register) = unsafe {
            match site {
                PickSite::Magazine => (raw::site::reload_pick_empty::state(ctx)?, Reg::Ebx),
                PickSite::MagazineHands => (raw::site::reload_pick_empty_hands::state(ctx)?, Reg::Eax),
                PickSite::Shells => (raw::site::reload_pick_two_shells::state(ctx)?, Reg::Ecx),
                PickSite::ShellHands => (raw::site::reload_pick_two_shells_hands::state(ctx)?, Reg::Esi),
            }
        };
        // SAFETY: as above.
        let (index, handles, loaded) = unsafe {
            (read(state.index_of_animation_to_wait()), read(state.animation_handles()), read(state.weapon()?.loaded()))
        };
        let saved = saved_clip(index, handles, state.address());
        Some(ClipPick { ctx, register, saved, loaded })
    }

    /// At `reload.pick_empty`: 314's weapon clips, before this pick is saved.
    pub fn magazine(ctx: &'c mut Context<'t>) -> Option<ClipPick<'c, 't>> {
        ClipPick::at(ctx, PickSite::Magazine)
    }

    /// At `reload.pick_empty_hands`: 314's hands, after the weapon's pick is saved.
    pub fn magazine_hands(ctx: &'c mut Context<'t>) -> Option<ClipPick<'c, 't>> {
        ClipPick::at(ctx, PickSite::MagazineHands)
    }

    /// At `reload.pick_two_shells`: 311's weapon clips, before this pick is saved.
    pub fn shells(ctx: &'c mut Context<'t>) -> Option<ClipPick<'c, 't>> {
        ClipPick::at(ctx, PickSite::Shells)
    }

    /// At `reload.pick_two_shells_hands`: 311's hands, after the weapon's pick is saved.
    pub fn shell_hands(ctx: &'c mut Context<'t>) -> Option<ClipPick<'c, 't>> {
        ClipPick::at(ctx, PickSite::ShellHands)
    }

    /// The clip set +134h names: at the weapon sites the reload's last pick
    /// (None at its first), at the hands sites this rebuild's weapon pick.
    pub fn saved(&self) -> Option<ReloadClip> {
        self.saved
    }

    /// The weapon's rounds in the magazine now (+5E5h).
    pub fn loaded(&self) -> u8 {
        self.loaded
    }

    /// The pick the game's code goes on with.
    pub fn choose(self, clip: ReloadClip) {
        self.ctx.set_reg(self.register, clip.bit());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_saved_clip_of_each_index() {
        let state = 0x3000_0000;
        let handles = state + PICKS_OFFSET;
        // pick + 2 * posture, as 0x30FEE4 writes it: (pick * 8) >> 2
        assert_eq!(saved_clip(0, handles, state), Some(ReloadClip::First));
        assert_eq!(saved_clip(2, handles, state), Some(ReloadClip::Second));
        assert_eq!(saved_clip(4, handles, state), Some(ReloadClip::First));
        assert_eq!(saved_clip(6, handles, state), Some(ReloadClip::Second));
        assert_eq!(saved_clip(NOT_PICKED, handles, state), None);
        for index in [1, 3, 5, 7, 8, 0x20] {
            assert_eq!(saved_clip(index, handles, state), None, "{index}");
        }
        // a table elsewhere: the index is relative to it
        assert_eq!(saved_clip(4, handles - 8, state), Some(ReloadClip::Second));
    }

    #[test]
    fn only_the_seated_domain_is_an_event() {
        assert_eq!(ReloadEvent::parse(1), Some(ReloadEvent::Seated));
        for domain in [0, 2, 255] {
            assert_eq!(ReloadEvent::parse(domain), None);
        }
    }

    #[test]
    fn the_chamber_as_the_load_sees_it() {
        assert_eq!(ChamberOnReload::parse(0, 0), ChamberOnReload::No);
        assert_eq!(ChamberOnReload::parse(0, 1), ChamberOnReload::No);
        assert_eq!(ChamberOnReload::parse(1, 0), ChamberOnReload::Empty);
        assert_eq!(ChamberOnReload::parse(0xFF, 1), ChamberOnReload::Chambered);
    }
}
