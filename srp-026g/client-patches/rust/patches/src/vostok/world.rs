//! The match's world as the ragdoll reads it (patch 76): the game world, the
//! network client and its places, a place's player (its state, transform,
//! model's skeleton, inventory, hit model), the physics world's ray. Every
//! field is fields_026g.tsv's, every function engine_026g.tsv's; this is
//! only what their values mean.
//!
//! Each handle borrows the hook's [`Thread`] (a patch cannot keep one past
//! the hook); what a patch keeps between hooks is an id ([`PlayerId`],
//! [`SkeletonId`], [`BodyId`]): a game address as a value, never read
//! through.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::Thread;
use crate::raw;

/// Reads a field's place (raw's accessor gives it).
///
/// # Safety
/// `at` is the field's place in a live object.
pub(in crate::vostok) unsafe fn read<T: Copy>(at: *mut T) -> T {
    unsafe { at.read_volatile() }
}

/// `N` f32 from a field on.
///
/// # Safety
/// As `read`, for the whole run.
pub(in crate::vostok) unsafe fn floats<const N: usize>(at: *mut f32) -> [f32; N] {
    unsafe { at.cast::<[f32; N]>().read_unaligned() }
}

/// Writes `N` f32 from a field on.
///
/// # Safety
/// As `floats`, and the run is the caller's to write now.
pub(in crate::vostok) unsafe fn write_floats<const N: usize>(at: *mut f32, value: [f32; N]) {
    unsafe { at.cast::<[f32; N]>().write_unaligned(value) }
}

/// The game world, at `game_world.after_move`.
pub struct GameWorld<'t> {
    pub(in crate::vostok) this: raw::GameWorld,
    thread: &'t Thread,
}

impl<'t> GameWorld<'t> {
    /// A player's game world, on the hook's thread.
    pub(in crate::vostok) fn of(this: raw::GameWorld, thread: &'t Thread) -> GameWorld<'t> {
        GameWorld { this, thread }
    }

    /// The world the site's code is ticking.
    pub fn at_after_move(ctx: &'t Context<'_>) -> Option<GameWorld<'t>> {
        // SAFETY: the context is this site's: the game world it ticks, alive
        // on the game's thread.
        let this = unsafe { raw::site::game_world_after_move::world(ctx) }?;
        Some(GameWorld { this, thread: ctx.thread() })
    }

    /// Its game's network client.
    pub fn network_client(&self) -> Option<NetworkClient<'t>> {
        // SAFETY: the world, its game and the game's client, alive for the hook.
        let this = unsafe { self.this.game()?.network_client() }?;
        Some(NetworkClient { this, thread: self.thread })
    }
}

/// The network client: the match's places and the local player.
pub struct NetworkClient<'t> {
    this: raw::NetworkClient,
    thread: &'t Thread,
}

impl<'t> NetworkClient<'t> {
    /// The client a site's typed register holds, on its hook's thread.
    pub(in crate::vostok) fn of(this: raw::NetworkClient, thread: &'t Thread) -> NetworkClient<'t> {
        NetworkClient { this, thread }
    }

    /// The match's places' count (0 without match options).
    pub fn places(&self) -> u32 {
        // SAFETY: the client, alive for the hook; match_options is its own.
        unsafe { raw::engine_match_options(Some(self.this)).map_or(0, |options| u32::from(read(options.places()))) }
    }

    /// The match it plays ([`MatchId`]), when it has match options.
    pub fn match_id(&self) -> Option<MatchId> {
        // SAFETY: the client, alive for the hook; match_options is its own.
        unsafe { raw::engine_match_options(Some(self.this)).map(|options| MatchId(read(options.seed()))) }
    }

    /// The team word of each place of the match (0 or 1 a team; its
    /// options' record, the one a player's profile is), None past its count.
    pub(in crate::vostok) fn place_teams(&self) -> [Option<u32>; super::types::Place::COUNT] {
        let mut teams = [None; super::types::Place::COUNT];
        // SAFETY: the client, alive for the hook; match_options is its own
        // (a live or a replay client's, its vtable's), its places as long as
        // the client; a place below the count (at most the 20 of the array)
        // is one of them.
        unsafe {
            if let Some(options) = raw::engine_match_options(Some(self.this)) {
                let places = usize::from(read(options.places())).min(teams.len());
                for (place, team) in teams.iter_mut().enumerate().take(places) {
                    *team = Some(read(options.first_place().at(place).team()));
                }
            }
        }
        teams
    }

    /// The player the camera follows: the local player from the match's
    /// first world on, a spectator's choice in a spectator's game
    /// ([`super::spectate`]).
    pub fn local_player(&self) -> Option<Player<'t>> {
        // SAFETY: the client's local player, alive for the hook.
        let this = unsafe { self.this.local_player() }?;
        Some(Player { this, thread: self.thread })
    }

    /// The local player's place, if it has a player.
    pub fn local_place(&self) -> Option<u8> {
        // SAFETY: the client's local player, alive for the hook.
        unsafe { Some(read(self.this.local_player()?.place())) }
    }

    /// The place the static info marked the local player's (its profile's
    /// is_local, set at 0x27BE5F), if any: the options' own bytes, read
    /// without the local player's object.
    pub fn local_profile(&self) -> Option<u8> {
        // SAFETY: the client's options and their places live as long as the
        // client, on the hook's thread; a place below the count (at most the
        // 20 of the array) is one of them.
        unsafe {
            let options = self.this.options_holder()?.options();
            let places = read(options.places()).min(20);
            (0..places).find(|&p| read(options.first_place().at(usize::from(p)).is_local()) != 0)
        }
    }

    /// The place's current player. get_active_player adds a reference; it is
    /// given back at once (the place holds its own), as the game does when
    /// the last one goes (0x0E0210).
    pub fn player(&self, place: u32) -> Option<Player<'t>> {
        let mut out = None;
        // SAFETY: the client's own get_active_player (it writes the player, a
        // reference added, through `out`).
        unsafe { raw::engine_active_player(Some(self.this), &mut out, place) };
        let player = out?;
        // SAFETY: a player, its reference count a u32 the game changes with
        // locked instructions.
        unsafe {
            let count = player.references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(Some(player), count);
                return None;
            }
        }
        Some(Player { this: player, thread: self.thread })
    }

    /// What the game is in its match ([`Role`]), read once from its local
    /// place; and it builds and sends no input exactly while it is a
    /// spectator (the client's +0Ch, as a replay's). `None`: a replay's
    /// client, which has no local place (a smaller object, its vtable not
    /// network_client's).
    pub fn take_role(&self) -> Option<Role> {
        // SAFETY: the client's first dword, its vtable: alive for the hook.
        if unsafe { read(self.this.vtable()) } != raw::image::NETWORK_CLIENT_VTABLE {
            return None;
        }
        // SAFETY: a network client's local place byte and its +0Ch, the
        // game's thread's (game_world::tick reads +0Ch on it).
        unsafe {
            let role = Role::of(read(self.this.local_place()));
            self.this.observer().write_volatile(u8::from(role == Role::Spectator));
            Some(role)
        }
    }

    /// What the game is in its match, as [`Self::take_role`] reads it,
    /// changing nothing.
    pub fn role(&self) -> Option<Role> {
        // SAFETY: as take_role.
        if unsafe { read(self.this.vtable()) } != raw::image::NETWORK_CLIENT_VTABLE {
            return None;
        }
        // SAFETY: as take_role.
        Some(Role::of(unsafe { read(self.this.local_place()) }))
    }

    /// A spectator's camera to the player `step` names
    /// ([`spectate_order`]): set_followed_player (0x242B90) and the attached
    /// spectator's camera (mode 4, 0x1F8360), as spectate_next_player
    /// (0x242D50) does, over every team. The place followed, or `None`
    /// when no other player can be followed (the camera stays), or the game
    /// is no spectator (a player's camera is never moved: its client's
    /// input would go to the player followed).
    pub fn follow(&self, step: Step) -> Option<u8> {
        if self.take_role()? != Role::Spectator {
            return None;
        }
        // From the player followed, or after a free flight the last one
        // (+8, which detach_from_player keeps; its reference keeps it alive).
        // SAFETY: the client's +8, a player it holds a reference to.
        let last = unsafe { self.this.last_followed() }.map(|this| Player { this, thread: self.thread });
        let current = self.local_player().or(last).map(|player| player.place());
        let mut found = None;
        let mut any = None;
        for place in spectate_order(self.places(), current, step) {
            let Some(player) = self.player(place) else { continue };
            if !player.inserted() {
                continue;
            }
            if player.alive() {
                found = Some(place);
                break;
            }
            any = any.or(Some(place));
        }
        // A first camera takes a dead player rather than none (the warmup,
        // everyone between lives); the keys look for the living.
        let place = found.or(if step == Step::First { any } else { None })?;
        let mut out = None;
        // SAFETY: the client's own get_active_player: the player, a
        // reference added, through `out`; set_followed_player takes that
        // reference (it releases its argument, 0x242C8B); the game world is
        // the client's game's own, the one spectate_next_player gives it.
        unsafe {
            raw::engine_active_player(Some(self.this), &mut out, place);
            let player = out?;
            raw::engine_follow_player(Some(self.this), Some(player));
            let world = self.this.game()?.world();
            raw::engine_spectator_camera(Some(world));
        }
        u8::try_from(place).ok()
    }
}

impl NetworkClient<'_> {
    /// A spectator's game leaves its match, back to the lobby with no
    /// results screen: close_current_match(0), the world ended and nothing
    /// sent (the end a game takes after its server went silent, match.md
    /// section 34). A player's game is not touched: its own 0xEA is the
    /// game's (`31 <order> FF`, close_current_match(3), the results).
    pub fn stop_watching(&self) {
        if self.role() != Some(Role::Spectator) {
            return;
        }
        // SAFETY: the client's own close_current_match, on the game's
        // thread from inside the 0xEA's dispatch, where the game's own end of
        // a local place calls it (0x27C774).
        unsafe { raw::engine_close_current_match(Some(self.this), WORLD_ENDED) }
    }

    /// A spectator's camera lets go of the player it follows and flies
    /// freely: detach_from_player (0x242CC0, the replay's free view, F2):
    /// the player's handler and hud unbound, +4 0 (+8 kept), then the game
    /// world's own free-fly camera (switch_to_free_fly_camera 0x1F8270, the
    /// handler unbound, so its mode is not written). No player is followed,
    /// so game_world::tick builds no input (0x1F784D: +4 0). A player's game
    /// is not touched.
    pub fn fly_free(&self) {
        if self.take_role() != Some(Role::Spectator) {
            return;
        }
        // SAFETY: the client's own detach_from_player, on the game's thread
        // from the game world's keys, as the replay client's free view calls
        // it there (vtable+84h -> +60h, 0x26E000).
        unsafe { raw::engine_detach_player(Some(self.this)) }
    }
}

/// close_current_match's reason 0: the world ended, nothing sent, no results
/// (its jump table 0x27A958 -> 0x27A941).
const WORLD_ENDED: u32 = 0;

/// The places a spectator's camera tries, in order, for `step` from the
/// place it follows (`current`), of `places`: a first camera every place
/// from 0; the next or the previous one every other place from the one after
/// (before) it, round, the one it follows left out (from none: as a first).
pub fn spectate_order(places: u32, current: Option<u8>, step: Step) -> impl Iterator<Item = u32> {
    let current = current.map(u32::from).filter(|&place| place < places);
    let (from, count, back) = match (step, current) {
        (Step::Next, Some(place)) => (place, places.saturating_sub(1), false),
        (Step::Previous, Some(place)) => (place, places.saturating_sub(1), true),
        _ => (places.saturating_sub(1), places, false),
    };
    (1..=count).map(move |k| if back { (from + places - k) % places } else { (from + k) % places })
}

/// What a game is in its match: a player (its 0xE4 named a place its own)
/// or a spectator (it named none: the client's local place 0xFF; the
/// queue's, docs/notes/queue-spectate.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Player,
    Spectator,
}

impl Role {
    /// The client's local place byte: 0xFF none (its constructor 0x219AE2,
    /// unload 0x27B4F0; on_match_ready sets it for the 0xE4's local place).
    fn of(local_place: u8) -> Role {
        if local_place == 0xFF { Role::Spectator } else { Role::Player }
    }
}

/// Which player a spectator's camera goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The first player of the match (the first world, or the one followed gone).
    First,
    /// The next player by place (the binding next_player, F6).
    Next,
    /// The previous one (prev_player, F5).
    Previous,
}

/// What a player holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Nothing,
    /// A battery (a victory_item): no gun drops from a carrier's hands.
    Battery,
    /// A weapon or another item.
    Item,
}

/// A player as a value kept between hooks: its address, compared, never
/// read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerId(u32);

/// An inventory item (or an active object, a weapon) as a value kept between
/// hooks: its address, compared, never read through. Made only from its
/// handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemId(u32);

impl ItemId {
    pub(in crate::vostok) fn of(item: raw::Item) -> ItemId {
        ItemId(item.address())
    }

    /// Its address, for the log.
    pub fn address(self) -> u32 {
        self.0
    }
}

/// Which match: its options' seed (+577B0h, the 0xE4's match id: the
/// server's own a match, logged by the client as `Match id`). A world
/// core's address (`WorldKey`) can be the next match's too; this cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchId(u32);

impl MatchId {
    /// Its value, for the log.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl<'t> GameWorld<'t> {
    /// The match this world is of.
    pub fn match_id(&self) -> Option<MatchId> {
        self.network_client()?.match_id()
    }
}

impl Player<'_> {
    /// The match this player is of.
    pub fn match_id(&self) -> Option<MatchId> {
        // SAFETY: the live player's world, game and network client.
        let client = unsafe { self.this.game_world()?.game()?.network_client() }?;
        NetworkClient::of(client, self.thread).match_id()
    }
}

/// A match's world core as a value kept between hooks: which match a
/// record is of. Made only from its handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldKey(u32);

impl WorldKey {
    pub(in crate::vostok) fn of(core: raw::CoreWorld) -> WorldKey {
        WorldKey(core.address())
    }
}

/// A model's skeleton, kept between hooks (a cache's key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkeletonId(u32);

/// A body in the physics world (a hit model's), kept between hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyId(pub(in crate::vostok) u32);

/// A player modifier's index in a kit's 68 (the exe's names at 0x928448).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModifierIndex(u32);

impl ModifierIndex {
    /// `stamina_boost_interval` (29): no 0.26g code reads it (no
    /// apply_modifier or apply_modifiers call names it) and nothing sets it
    /// but skill 205's 0; additive (0x927E08's list). srp's: the Force
    /// Protein's built-in modification 1330 gives 15: its carrier's
    /// gun-stock kills push the ragdoll 1 + 15 times (shot::knocked_back).
    pub const MELEE_KNOCKBACK: ModifierIndex = ModifierIndex(29);
}

/// A player of the match.
#[derive(Clone, Copy)]
pub struct Player<'t> {
    pub(in crate::vostok) this: raw::Player,
    pub(in crate::vostok) thread: &'t Thread,
}

impl<'t> Player<'t> {
    pub fn eye_transform(&self) -> [f32; 16] {
        // SAFETY: the live player's complete computed eye matrix.
        unsafe { floats(self.this.eye()) }
    }

    pub fn inventory(&self) -> Option<super::inventory::Inventory<'t>> {
        super::inventory::Inventory::of_player(self.this, self.thread)
    }
    pub fn core_skeleton(&self) -> Option<Skeleton<'t>> {
        // SAFETY: the live core player retains its simulation skeleton.
        Some(Skeleton { this: unsafe { self.this.core_skeleton() }?, thread: PhantomData })
    }

    /// The player a site's typed register holds (raw's `site`), on its hook's thread.
    pub(in crate::vostok) fn of(ctx: &'t Context<'_>, this: Option<raw::Player>) -> Option<Player<'t>> {
        this.map(|this| Player { this, thread: ctx.thread() })
    }

    /// The hook's thread it borrows.
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    pub fn id(&self) -> PlayerId {
        PlayerId(self.this.address())
    }

    /// The sum of the player modifier `index` in its current kit (the
    /// skills, the armour's and the carried boosters' entries:
    /// apply_modifier 0x2D5AE0 of 0, weight 1, an additive index's sum);
    /// None without a profile or a kit past the three.
    pub fn kit_modifier(&self, index: ModifierIndex) -> Option<f32> {
        // SAFETY: the live player's profile and current kit (+115h, below 3
        // for a match player: checked), the kit's modifiers container.
        unsafe {
            let kit = usize::from(read(self.this.kit()));
            if kit >= 3 {
                return None;
            }
            let modifiers = self.this.profile()?.kits().at(kit).modifiers();
            Some(raw::engine_apply_modifier(Some(modifiers), index.0, 0.0, 1.0))
        }
    }

    /// The player's place in the match roster.
    pub fn place(&self) -> u8 {
        // SAFETY: the live base player's place byte.
        unsafe { read(self.this.place()) }
    }

    /// In the match's world (m_has_been_inserted).
    pub fn inserted(&self) -> bool {
        // SAFETY: the player is alive for the hook.
        unsafe { read(self.this.inserted()) != 0 }
    }

    /// Alive (m_is_alive): a rollback or a loaded world sets it again.
    pub fn alive(&self) -> bool {
        // SAFETY: as inserted.
        unsafe { read(self.this.alive()) != 0 }
    }

    /// The life's spawn time (m_inserted_time_in_ms): one death a life.
    pub fn spawned(&self) -> u32 {
        // SAFETY: as inserted.
        unsafe { read(self.this.spawned()) }
    }

    /// What it holds (+128h: mt_draw draws it at the "Weapon" bone).
    pub fn held(&self) -> Held {
        // SAFETY: as inserted; the held item is alive as long.
        unsafe {
            match self.this.held_item() {
                None => Held::Nothing,
                Some(item) if read(item.vtable()) == raw::image::VICTORY_ITEM_VTABLE => Held::Battery,
                Some(_) => Held::Item,
            }
        }
    }

    /// The scene renderer and the model's render instance, what
    /// update_skeleton takes (mt_draw 0x2477E2..0x24780A).
    pub(in crate::vostok) fn renderer(&self) -> Option<(raw::SceneRenderer, raw::RenderModelInstance)> {
        // SAFETY: the player's model and game world, the world's game, its
        // renderer, the renderer's scene renderer: alive for the hook.
        unsafe {
            let model = self.this.model()?;
            let scene = self.this.game_world()?.game()?.renderer()?.scene_renderer()?;
            Some((scene, model.render_instance()))
        }
    }

    /// The simulated transform, a row-vector 4x4 (rows i, j, k, place).
    pub fn transform(&self) -> [f32; 16] {
        // SAFETY: the player is alive for the hook; 16 f32 from its field on.
        unsafe { floats(self.this.transform()) }
    }

    /// The camera's transform's address (what update_camera gives the view
    /// in the first person): compared with what the site was given.
    pub(in crate::vostok) fn eye_address(&self) -> u32 {
        // SAFETY: an address only.
        unsafe { self.this.eye() as usize as u32 }
    }

    /// The physics world (the level's ray).
    pub fn physics_world(&self) -> Option<PhysicsWorld<'t>> {
        // SAFETY: as inserted.
        let this = unsafe { self.this.physics_world() }?;
        Some(PhysicsWorld { this, thread: self.thread })
    }

    /// Its model's skeleton.
    pub fn skeleton(&self) -> Option<Skeleton<'t>> {
        // SAFETY: the player's model, the model's skeleton: alive for the hook.
        let this = unsafe { self.this.model()?.skeleton() }?;
        Some(Skeleton { this, thread: PhantomData })
    }

    /// Its hit model's body in the physics world (what a bullet's ray names).
    pub fn hit_body(&self) -> Option<BodyId> {
        // SAFETY: the player's hit model, its body: alive for the hook.
        unsafe { self.this.hit_model()?.body() }.map(|body| BodyId(body.address()))
    }

    /// What each inventory slot's item weighs, kg (a piece's weight times a
    /// stack's count, as inventory::update_weight sums it; None for an empty
    /// slot), the slots in the game's order (profile slots 0..6 helmet, mask,
    /// torso, back, legs, gloves, boots, then the weapons and the rest), to
    /// `f`; nothing without an inventory.
    pub fn slot_weights(&self, mut f: impl FnMut(usize, Option<f32>)) {
        // SAFETY: the player's inventory, alive for the hook; its slots' run.
        let Some((first, slots)) = (unsafe {
            self.this.inventory().map(|inventory| (inventory.slots(), inventory.slots_end()))
        }) else {
            return;
        };
        for slot in 0..slots.index_from(first).unwrap_or(0) {
            // SAFETY: a slot of the run (below its end).
            f(slot, unsafe { item_weight(first.at(slot)) });
        }
    }
}

/// The weight of the item a slot holds.
///
/// # Safety
/// `slot` is a live inventory's slot.
unsafe fn item_weight(slot: raw::InventorySlot) -> Option<f32> {
    // SAFETY: the slot's item, its fields, its entry's.
    unsafe {
        let item = slot.item()?;
        let piece = read(item.weight());
        let stack = item.entry().is_some_and(|entry| read(entry.stack()) != 0);
        let count = if stack { f32::from(read(item.count())) } else { 1.0 };
        Some(piece * count)
    }
}

/// A model's skeleton: its bones, object_movement first, each its name and
/// its parent (add_bone 0x3ACDA0's records).
pub struct Skeleton<'t> {
    this: raw::Skeleton,
    thread: PhantomData<&'t Thread>,
}

impl Skeleton<'_> {
    pub fn bone_computed(&self, index: u32, layer: u8) -> bool {
        // SAFETY: bounded native skeleton record.
        self.bone(index).is_some_and(|bone| unsafe { read(bone.computed_layers()) & layer != 0 })
    }

    pub fn id(&self) -> SkeletonId {
        SkeletonId(self.this.address())
    }

    /// Its bones' count.
    pub fn bone_count(&self) -> u32 {
        // SAFETY: the skeleton is alive for the hook.
        unsafe { read(self.this.bone_count()) }
    }

    /// The `index`th bone's record (within the count).
    fn bone(&self, index: u32) -> Option<raw::SkeletonBone> {
        // SAFETY: a record of the run (below the count).
        (index < self.bone_count()).then(|| unsafe { self.this.bones().at(index as usize) })
    }

    /// Whether the `index`th bone is named `name` (false past the count or
    /// for a bone with no name).
    pub fn bone_named(&self, index: u32, name: &str) -> bool {
        let Some(bone) = self.bone(index) else { return false };
        // SAFETY: the record's name, a C string: compared up to its 0.
        unsafe {
            let text = bone.name();
            if text.is_null() {
                return false;
            }
            for (n, &b) in name.as_bytes().iter().enumerate() {
                if text.add(n).read_volatile() != b {
                    return false;
                }
            }
            text.add(name.len()).read_volatile() == 0
        }
    }

    /// Whether the `index`th bone has a name at all.
    pub fn bone_has_name(&self, index: u32) -> bool {
        let Some(bone) = self.bone(index) else { return false };
        // SAFETY: the record is the live skeleton's.
        unsafe { !bone.name().is_null() }
    }

    /// The `index`th bone's parent's index (the record it names; the first
    /// record is index 0).
    pub fn bone_parent(&self, index: u32) -> Option<u32> {
        let bone = self.bone(index)?;
        // SAFETY: the record's parent field, the skeleton's run.
        let (parent, first) = unsafe { (bone.parent()?, self.this.bones()) };
        parent.index_from(first).map(|i| i as u32)
    }
}

/// The physics world (the simulation's): read-only queries.
pub struct PhysicsWorld<'t> {
    this: raw::BulletPhysicsWorld,
    thread: &'t Thread,
}

/// A ray's first hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub point: [f32; 3],
    /// Toward the ray's start.
    pub normal: [f32; 3],
}

impl PhysicsWorld<'_> {
    /// bullet_physics_world::ray_test from `origin` along the unit
    /// `direction`, `length` long, with the filter `group`/`mask` (an object
    /// is tried when its mask has a bit of the group and its group a bit of
    /// the mask): the first hit. It changes nothing (Bullet's scratch only).
    pub fn ray_test(&self, origin: [f32; 3], direction: [f32; 3], length: f32, group: u32, mask: u32) -> Option<RayHit> {
        let _ = self.thread;
        let mut room = raw::RayResultStorage::zeroed();
        let result = room.handle();
        // SAFETY: the physics world, alive for the hook; the result is ours;
        // the origin and direction float3s are ours.
        unsafe {
            raw::engine_ray_test(Some(self.this), Some(result), origin.as_ptr(), direction.as_ptr(), length, group,
                                 mask, core::ptr::null(), 0);
            result.object()?;
            Some(RayHit { point: floats(result.point()), normal: floats(result.normal()) })
        }
    }
}

impl BodyId {
    /// The body as a value (a shot's key).
    pub fn key(self) -> u32 {
        self.0
    }
}
