//! Carry an owned artefact above the right hand; Use takes a teammate's offer.
//! Native input, snapshots and inventory remain the simulation authority.
#![forbid(unsafe_code)]

use artefact_carry::{ArtefactId, Place, Refused};
use artefact_carry::carry::{self, Carry, CarryKey, Event, KeyPress, QuickSlot, Token};
use ragdoll::math::Vec3;
use crate::vostok::{Action, Context, Flow, InputHandler, Lock, GameCell, NoExit, ActionMask};
use crate::vostok::carry_runtime::{self as native, ArtefactClass, ArtefactDraw, HiddenWeapons, Key};
use crate::vostok::inventory::ContainerSpace;
use crate::vostok::draw::{self, DrawnBones};
use crate::vostok::world::Player;

mod pose;

#[derive(Clone, Copy)]
struct Attachment { item: u32, token: Token }
#[derive(Clone, Copy)]
struct Owner { key: Key, carry: Carry, attachment: Option<Attachment> }
impl Owner {
    const fn new(key: Key) -> Self { Self { key, carry: Carry::NEW, attachment: None } }
}
struct Owners([Option<Owner>; 60]);
impl Owners {
    fn get(&self, key: Key) -> Option<&Owner> { self.0[key.index].as_ref().filter(|o| o.key == key) }
    fn get_mut(&mut self, key: Key) -> &mut Owner {
        if self.get(key).is_none() { self.0[key.index] = Some(Owner::new(key)); }
        self.0[key.index].as_mut().unwrap()
    }
}
static OWNERS: Lock<Owners> = Lock::new(Owners([None; 60]));
/// By weapon, not by owner: a respawn or a new kit replaces the owner, and
/// the models it took out must still go back before anything draws them.
static HIDDEN: Lock<HiddenWeapons> = Lock::new(HiddenWeapons::NEW);
static CHORD: GameCell<carry::Chord> = GameCell::new(carry::Chord::Idle);
static CARRY_KEY: GameCell<CarryKey> = GameCell::new(CarryKey::Up);
static HINT: GameCell<Option<(usize, bool)>> = GameCell::new(None);

fn machine(player: &Player<'_>, ctx: &Context<'_>) -> Carry {
    player.carry_key().and_then(|key| OWNERS.with(ctx.thread(), |o| o.get(key).map(|o| o.carry))).unwrap_or_default()
}

fn event(player: &Player<'_>, ctx: &Context<'_>, event: Event) {
    let Some(key) = player.carry_key() else { return };
    OWNERS.with(ctx.thread(), |owners| {
        let owner = owners.get_mut(key);
        let before = owner.carry;
        owner.carry.step(event);
        if owner.carry.artefact() != before.artefact() { owner.attachment = None; }
    });
}

/// Called on every set_input, including inputs for which native early-out
/// skips its copy. Command and Use edge detection therefore sees releases.
pub fn on_input(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(player) = native::player(ctx, native::PlayerSite::Input) else { return Flow::Continue };
    let Some((now, actions)) = native::incoming(ctx) else { return Flow::Continue };
    let before = machine(&player, ctx);
    let allowed = player.carry_allowed();
    if !allowed {
        event(&player, ctx, Event::Gone);
    }
    let artefact = QuickSlot::from_actions(actions).filter(|_| allowed)
        .and_then(|slot| player.inventory()?.quick_artefact(slot.get()));
    event(&player, ctx, Event::Input { now, actions, artefact });
    // Use is ours only when a take happens; otherwise the game has it
    // (a battery, a trap, a native pickup).
    if before.use_pressed(actions) {
        let taken = match target(&player, ctx) {
            // Resolve once more at commit: native world owns the current kit,
            // and the exact generation must still be offered.
            Some((donor, token)) => machine(&donor, ctx).offer() == Some(token) && reachable(&player, &donor)
                && match (donor.inventory(), player.inventory()) {
                    (Some(mut from), Some(mut to)) => from.transfer_artefact(&mut to, token.artefact),
                    _ => Err(Refused::OwnershipChanged),
                }.inspect(|()| event(&donor, ctx, Event::Taken { now, token })).is_ok(),
            None => ground_target(&player).zip(player.inventory())
                .is_some_and(|(item, mut inventory)| item.take(&mut inventory).is_ok()),
        };
        if taken { consume_use(&player, ctx); }
    }
    Flow::Continue
}

fn consume_use(player: &Player<'_>, ctx: &Context<'_>) {
    if let Some(key) = player.carry_key() {
        OWNERS.with(ctx.thread(), |owners| owners.get_mut(key).carry.consume_use());
    }
}

pub fn on_client_death(ctx: &mut Context<'_>) -> Flow<NoExit> { on_death(ctx) }
pub fn on_death(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, native::PlayerSite::Death) {
        if let Some(artefact) = machine(&player, ctx).artefact() { player.drop_carried(artefact); }
        event(&player, ctx, Event::Gone);
    }
    Flow::Continue
}
pub fn on_ground_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::carry_ground::save(ctx); Flow::Continue
}
pub fn on_ground_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::carry_ground::load(ctx); Flow::Continue
}

pub fn on_filter(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, native::PlayerSite::Filter)
        && player.carry_key().is_some() {
            player.carry_filter(machine(&player, ctx).filtered());
        }
    Flow::Continue
}

pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, native::PlayerSite::Tick) {
        if player.carry_allowed() {
            let carry = machine(&player, ctx);
            if carry.pending().is_some_and(|a| !player.carry_owned(a)) { event(&player, ctx, Event::PendingLost); }
            let owned = carry.artefact().is_none_or(|a| player.carry_owned(a));
            event(&player, ctx, Event::Tick { now: player.carry_time(), owned });
        } else { event(&player, ctx, Event::Gone); }
    }
    Flow::Continue
}

pub fn on_construct(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let object = native::constructed(ctx);
    OWNERS.with(ctx.thread(), |owners| {
        for slot in &mut owners.0 { if slot.is_some_and(|o| o.key.object == object) { *slot = None; } }
    });
    Flow::Continue
}

pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = native::player(ctx, native::PlayerSite::Save) { native::write_snapshot(ctx, machine(&player, ctx)); }
    Flow::Continue
}
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let saved = native::read_snapshot(ctx).unwrap_or_default();
    if let Some(player) = native::player(ctx, native::PlayerSite::Load)
        && let Some(key) = player.carry_key() {
            OWNERS.with(ctx.thread(), |owners| {
                let owner = owners.get_mut(key);
                if owner.carry.artefact() != saved.artefact() { owner.attachment = None; }
                owner.carry = saved;
            });
        }
    Flow::Continue
}

/// H's own events, down and up, as the key handler gets them: the input
/// builder's action query sees the down and not always the up, which left
/// the key held.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let press = native::key_event(ctx, Action::ARTEFACT).map(|down| if down { KeyPress::Pressed } else { KeyPress::Released });
    if press.is_some() {
        let key = CARRY_KEY.get(ctx.thread()).step(press);
        CARRY_KEY.set(ctx.thread(), key);
    }
    Flow::Continue
}

pub fn on_input_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut handler) = InputHandler::at_builder_end(ctx) {
        let actions = handler.actions().bits();
        let mut chord = CHORD.get(ctx.thread());
        let key = CARRY_KEY.get(ctx.thread());
        let encoded = chord.step(actions, key.is_down());
        CHORD.set(ctx.thread(), chord);
        handler.set_actions(ActionMask::from_bits(encoded));
    }
    Flow::Continue
}

pub fn on_hud(ctx: &Context<'_>) {
    // Shown only when Use would take: otherwise the native hint stays.
    let text = native::local_player(ctx).filter(|p| target(p, ctx).is_some() || ground_target(p).is_some())
        .map(|_| c"st_artefact_take_teammate");
    let visible = text.is_some();
    if let Some(movie) = crate::vostok::GameWorldUi::at_update_icons(ctx).and_then(|ui| ui.hud_movie()) {
        let previous = HINT.get(ctx.thread());
        let next = Some((movie.id(), visible));
        // Refresh visible prompts while chat/profile visibility changes in
        // the movie, even if the world target itself did not change.
        if !visible && previous.is_none_or(|(id, shown)| id != movie.id() || !shown) { return; }
        if movie.invoke_text(c"root.show_artefact_offer", text.unwrap_or(c"")) { HINT.set(ctx.thread(), next); }
    }
}

fn v([x, y, z]: [f32; 3]) -> Vec3 { Vec3::new(x, y, z) }
fn reachable(recipient: &Player<'_>, donor: &Player<'_>) -> bool {
    if recipient.id() == donor.id() || !recipient.alive() || !recipient.inserted() || !donor.alive() || !donor.inserted()
        || recipient.carry_team().is_none() || recipient.carry_team() != donor.carry_team() { return false; }
    let (eye, direction) = recipient.carry_eye();
    let (donor_eye, _) = donor.carry_eye();
    let destination = v(donor_eye) - Vec3::Y * 0.25;
    let delta = destination - v(eye);
    let distance = delta.length();
    if !(0.15..=2.2).contains(&distance) || v(direction).dot(delta) < 0.9 * distance { return false; }
    let Some(physics) = recipient.physics_world() else { return false };
    let ray = delta * (1.0 / distance);
    // The native level collision filter, also used by the foot IK. Dynamic
    // player hit bodies must not make their own offered hand an obstruction.
    physics.ray_test(eye, [ray.x, ray.y, ray.z], distance, 0x24, 2).is_none()
}

/// The offer Use would take: none without a free container.
fn target<'a>(recipient: &'a Player<'_>, ctx: &Context<'_>) -> Option<(Player<'a>, Token)> {
    if recipient.container_space() != ContainerSpace::Available { return None; }
    let mut best = None;
    let (eye, _) = recipient.carry_eye();
    let mut nearest = 2.2;
    for n in 0..20 {
        let Some(donor) = recipient.carry_peer(Place::parse(n).unwrap()) else { continue };
        let Some(token) = machine(&donor, ctx).offer() else { continue };
        if !reachable(recipient, &donor) { continue; }
        let (point, _) = donor.carry_eye();
        let distance = (v(point) - Vec3::Y * 0.25 - v(eye)).length();
        if distance < nearest { nearest = distance; best = Some((donor, token)); }
    }
    best
}

/// The dropped artefact Use would pick up: none without a free container.
fn ground_target<'t>(recipient: &Player<'t>) -> Option<crate::vostok::carry_ground::GroundItem<'t>> {
    if !recipient.alive() || !recipient.inserted() || recipient.container_space() != ContainerSpace::Available { return None; }
    let physics = recipient.physics_world()?;
    let (eye, forward) = recipient.carry_eye();
    let mut nearest = 2.2;
    let mut target = None;
    for id in 0..255 {
        let Some(item) = recipient.ground_item(ArtefactId::parse(id).unwrap()) else { continue };
        let Some(position) = item.position(recipient.carry_time()) else { continue };
        let point = v(position) + Vec3::Y * 0.10;
        let delta = point - v(eye);
        let distance = delta.length();
        if distance < 0.15 || distance >= nearest || v(forward).dot(delta) < distance * 0.9 { continue; }
        let ray = delta * (1. / distance);
        if physics.ray_test(eye, [ray.x, ray.y, ray.z], distance, 0x24, 2).is_some() { continue; }
        nearest = distance; target = Some(item);
    }
    target
}

pub fn on_damage_bones(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(bones) = DrawnBones::at_damage(ctx) {
        let player = bones.player();
        if player.alive() {
            pose::apply(&bones, machine(&player, ctx).presentation(player.carry_time()));
        }
    }
    Flow::Continue
}

pub fn on_bones(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(bones) = DrawnBones::at(ctx) else { return Flow::Continue };
    let player = bones.player();
    let Some(key) = player.carry_key() else { return Flow::Continue };
    if !player.alive() { return Flow::Continue; }
    let carry = machine(&player, ctx);
    let presentation = carry.presentation(player.carry_time());
    if let Some(matrix) = pose::apply(&bones, presentation) {
        let attachment = presentation.artefact.and_then(|a| {
            let draw = player.inventory()?.presentation(a)?;
            let item = draw.id();
            let token = carry.presentation_token()?;
            draw.update(ctx, native::DrawCommand::Present(matrix));
            Some(Attachment { item, token })
        });
        OWNERS.with(ctx.thread(), |owners| owners.get_mut(key).attachment = attachment);
    }
    Flow::Continue
}

pub fn on_weapon(ctx: &mut Context<'_>) -> Flow<crate::vostok::ArtefactCarryOnWeaponExit> {
    let Some(player) = native::player(ctx, native::PlayerSite::Weapon) else { return Flow::Continue };
    // A dead body's weapon is drawn natively (its optics update their
    // models): the models go back here before it, never left out.
    let hide = player.alive() && machine(&player, ctx).presentation(player.carry_time()).hide_weapon;
    let hidden = HIDDEN.with(ctx.thread(), |weapons| native::weapon_visibility(ctx, hide, weapons));
    if hidden { Flow::Exit(crate::vostok::ArtefactCarryOnWeaponExit::Skeleton) } else { Flow::Continue }
}

/// Hand the temporarily removed models back before native on_hide removes
/// them. Its renderer asserts if asked to remove a model absent from scene.
pub fn on_weapon_hide(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(weapon) = native::WeaponHiding::at(ctx) {
        HIDDEN.with(ctx.thread(), |weapons| weapon.restore(weapons));
    }
    Flow::Continue
}

pub fn on_cull(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(player) = draw::culled_player(ctx)
        && player.alive() && machine(&player, ctx).active() { return draw::draw_anyway(ctx); }
    Flow::Continue
}

fn artefact_draw(ctx: &Context<'_>, class: ArtefactClass) -> bool {
    let Some(draw) = ArtefactDraw::at(ctx, class) else { return false };
    if draw.ground(ctx) { return true; }
    let held = OWNERS.with(ctx.thread(), |owners| owners.0.iter().flatten().find_map(|owner| {
        let a = owner.attachment?;
        (a.item == draw.id() && owner.carry.artefact() == Some(a.token.artefact)).then_some(())
    }));
    draw.update(ctx, if held.is_some() { native::DrawCommand::Keep } else { native::DrawCommand::Native })
}

macro_rules! callback {
    ($fn:ident, $class:ident, $exit:ident) => {
        pub fn $fn(ctx: &mut Context<'_>) -> Flow<crate::vostok::$exit> {
            if artefact_draw(ctx, ArtefactClass::$class) { Flow::Exit(crate::vostok::$exit::Done) } else { Flow::Continue }
        }
    };
}
callback!(on_spring, Spring, ArtefactCarryOnSpringExit);
callback!(on_lifebone, Lifebone, ArtefactCarryOnLifeboneExit);
callback!(on_onyx, Onyx, ArtefactCarryOnOnyxExit);
callback!(on_rattle, Rattle, ArtefactCarryOnRattleExit);

pub fn on_flower_query(ctx: &mut Context<'_>) -> Flow<crate::vostok::ArtefactCarryOnFlowerQueryExit> {
    if crate::vostok::carry_flower::query(ctx) { Flow::Exit(crate::vostok::ArtefactCarryOnFlowerQueryExit::Done) } else { Flow::Continue }
}
pub fn on_flower_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::carry_flower::ready(ctx);
    Flow::Continue
}
pub fn on_flower_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::carry_flower::destroy(ctx);
    Flow::Continue
}
