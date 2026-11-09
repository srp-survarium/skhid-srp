//! Patch 11: the late join (client_patch.md section 3.11), and patch 14's
//! place of a late place's grenades and traps in the saved objects (3.14).
//!
//! The server proposes a newcomer to every game of a match in play (0xE4,
//! the late join's magic): its place, squad and profile. Each game checks
//! the profile ([`profile`]), keeps it ([`ProfileBox`]: the players are
//! made from it and keep pointing into it), asks the resource engine for
//! the newcomer's players in the background, installs them into the
//! roster's next place without making them enter, and answers (0xE0:
//! installed, failed or busy); the server lets the newcomer in when every
//! game has. The face srp gives the newcomer (its block after the profile)
//! goes to patch 123 before the players are asked for
//! (`faces::late_joiner`). The proposals' lifecycle is [`runtime`] (one
//! state machine: the activation they belong to, each proposal's phase), its
//! steps between the game's calls `flow`; this module is the hooks and the
//! game's side of those steps (`Native`).
//!
//! The hooks (late_join.toml): `on_ready` (on_match_ready: the match the late
//! join is for), `on_prepare` (0xE4), `on_query_done` (the players' query's
//! callback), `on_disconnect` and `on_client_destroyed` (the match left),
//! `on_player_destroyed` (a player's proposal lets go of it, any thread),
//! `on_bind_place` (every place's bind at a world's start: its ids of its
//! own, its objects in front of the list, patch 14) and `on_animations`
//! (a player's animation ids that a later place does not move).

#![forbid(unsafe_code)]

mod animations;
mod flow;
pub mod profile;
pub mod runtime;

use flow::{Game, Shared, Storage};
use profile::CheckedProfile;
use runtime::{Ack, ClientKey, DictionaryKey, KitMask, KitPlayers, MatchKey, Place, PlayerKey, ProfileKey, Runtime, Squad};

use crate::faces::{self, Face, Name};
use crate::vostok::late_join::{
    AnimationRegistry, Client, Connection, Core, Destroyed, ItemsDictionary, LoadedPlayer, PlaceSlot, ProfileArrival,
    ProfileBox,
};
use crate::vostok::resources::QueryResults;
use crate::vostok::{
    late_join_on_query_done_entry, Context, Flow, LateJoinOnAnimationsExit, LateJoinOnBindPlaceExit, Lock, NoExit,
    Thread,
};

/// The late join's proposals: reached from the game's thread and from a
/// player's destruction on a resource thread.
static RUNTIME: Lock<Runtime<ProfileBox>> = Lock::new(Runtime::new());

struct Locked<'t>(&'t Thread);

impl Shared<ProfileBox> for Locked<'_> {
    fn with<T>(&self, f: impl FnOnce(&mut Runtime<ProfileBox>) -> T) -> T {
        RUNTIME.with(self.0, f)
    }
}

impl Storage for ProfileBox {
    fn profile(&self) -> ProfileKey {
        self.key()
    }

    fn same_body(&self, body: &[u8]) -> bool {
        ProfileBox::same_body(self, body)
    }
}

/// The game, for one hook: the network client it was given (or the game's
/// own), and the players a query loaded (their lifetime `'r`).
struct Native<'t, 'r> {
    thread: &'t Thread,
    client: Option<Client<'t>>,
    players: core::marker::PhantomData<&'r ()>,
}

impl<'t> Native<'t, '_> {
    fn of(thread: &'t Thread, client: Option<Client<'t>>) -> Self {
        Native { thread, client, players: core::marker::PhantomData }
    }

    /// The live dictionary the activation was made with.
    fn dictionary(&self, key: DictionaryKey) -> Option<ItemsDictionary<'t>> {
        self.client.as_ref()?.matched()?.dictionary().filter(|d| d.key() == key)
    }
}

impl<'r> Game for Native<'_, 'r> {
    type Storage = ProfileBox;
    type Player = LoadedPlayer<'r>;

    fn player(&self, player: LoadedPlayer<'r>) -> PlayerKey {
        player.key()
    }

    fn check<'a>(&self, dictionary: DictionaryKey, body: &'a [u8]) -> Option<CheckedProfile<'a>> {
        CheckedProfile::parse(body, &self.dictionary(dictionary)?)
    }

    fn learn_face(&mut self, place: Place, name: Name, face: Face) {
        faces::late_joiner(self.thread, place, name, face);
    }

    fn store(&mut self, profile: &CheckedProfile<'_>) -> Option<ProfileBox> {
        let matched = self.client.as_ref()?.matched()?;
        ProfileBox::store(self.thread, profile, &matched.dictionary()?)
    }

    fn query(&mut self, storage: ProfileBox, _: KitMask, matched: MatchKey, place: Place) {
        match self.client.as_ref().and_then(|c| c.matched()).filter(|m| m.key() == matched) {
            Some(live) => storage.query(&live, place, late_join_on_query_done_entry()),
            // never (its match is the hook's): no query, so its answer now
            None => drop(flow::complete(&Locked(self.thread), self, &storage, [None; runtime::KITS])),
        }
    }

    fn install(&mut self, storage: &ProfileBox, players: KitPlayers<LoadedPlayer<'r>>, place: Place, squad: Squad,
               matched: MatchKey, dictionary: DictionaryKey) -> bool {
        install(self.client.as_ref(), storage, players, place, squad, matched, dictionary)
    }

    fn ack(&mut self, client: ClientKey, ack: Ack) {
        if let Some(live) = self.client.as_ref().filter(|c| c.key() == client) {
            live.ack(ack);
        }
    }
}

/// Puts the newcomer's players into the next place of the client's match
/// (before the Rust port `late_join_install_026g.c`): every check that
/// can refuse it comes before the place is counted in.
fn install(client: Option<&Client<'_>>, storage: &ProfileBox, players: KitPlayers<LoadedPlayer<'_>>, place: Place,
           squad: Squad, matched: MatchKey, dictionary: DictionaryKey) -> bool {
    let Some(client) = client.filter(|c| c.key() == matched.client && c.connection() == Connection::Playing) else {
        return false;
    };
    let Some(live) = client.matched().filter(|m| m.key() == matched) else { return false };
    let Some(dictionary) = live.dictionary().filter(|d| d.key() == dictionary) else { return false };
    let Some(registry) = live.animations() else { return false };
    let Some(next) = live.next_place(place, matched.seed) else { return false };
    next.read_profile(storage, &dictionary);
    let open = next.open();
    for (kit, player) in players.each() {
        open.hold(kit, player);
        animations::register(&registry, player.animated());
    }
    let core = open.publish(squad);
    let listed = core.first_object();
    bind_place(&core, &open.slot());
    // The place's grenades and traps are in front of the list now: every
    // state kept to roll back to gets their bytes in front too, so a state
    // loaded later reads in step (an untouched one saves itself as a zero).
    if let Some(added) = core.nodes_before(listed).filter(|&added| added > 0) {
        core.shift_saved_objects(added);
    }
    live.insert_physics(&open);
    open.player_ready();
    // patch 13: the lists of players made at the match's start, again
    live.rebuild_lists();
    true
}

/// A place bound (patch 11's ids, patch 14's order): its callback ids from
/// the place's own range ((place + 1) << 16), its weapons' from its six of
/// the twenty places' 120, and its grenades' and traps' nodes in front of
/// everything the game-object list held, in the order the game binds them,
/// so the list is the same in every game whenever the place was bound.
fn bind_place(core: &Core<'_>, slot: &PlaceSlot<'_>) {
    let aside = core.put_aside();
    core.bind(slot, (u32::from(slot.place()) + 1) << 16);
    core.take_back(aside);
}

/// `late_join.ready`: the client's match is ready (its world loaded): the
/// match the late join takes newcomers into.
pub fn on_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(matched) = Client::at_ready(ctx).and_then(|client| client.matched()) {
        let locked = Locked(thread);
        // Without the game's dictionary no proposal can be checked: none taken.
        drop(match matched.dictionary() {
            Some(dictionary) => flow::ready(&locked, matched.key(), dictionary.key(), matched.roster()),
            None => flow::deactivate(&locked),
        });
    }
    Flow::Continue
}

/// `late_join.prepare`: a 0xE4. In a match's first state (its options) the
/// late join retires; in play, the late join's proposal of the client's
/// match. Then the case's own compare (Continue).
pub fn on_prepare(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some((client, bytes)) = Client::at_prepare(ctx) {
        match client.connection() {
            Connection::Starting => drop(flow::deactivate(&Locked(thread))),
            Connection::Playing => {
                let key = client.key();
                flow::prepare(&Locked(thread), &mut Native::of(thread, Some(client)), key, bytes);
            }
            Connection::Other => {}
        }
    }
    Flow::Continue
}

/// `late_join.query_done`: a proposal's players came (or did not). Then the
/// query's storage and, last, its world are let go (the world's release may
/// destroy players at once).
pub fn on_query_done(results: QueryResults<'_>, arrival: ProfileArrival) {
    let thread = results.thread();
    let players = arrival.players(&results);
    let (storage, world) = arrival.into_parts();
    let mut native = Native::of(thread, Client::current(thread));
    drop(flow::complete(&Locked(thread), &mut native, &storage, players));
    drop(storage);
    if let Some(world) = world {
        world.release(thread);
    }
}

/// `late_join.disconnect`: the client leaves its match.
pub fn on_disconnect(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(client) = Client::at_disconnect(ctx) {
        drop(flow::retire(&Locked(ctx.thread()), client.key()));
    }
    Flow::Continue
}

/// `late_join.client_destroyed`: before the client's match client and pvp
/// match are destroyed.
pub fn on_client_destroyed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(client) = Client::at_destroyed(ctx) {
        drop(flow::retire(&Locked(ctx.thread()), client.key()));
    }
    Flow::Continue
}

/// `late_join.player_destroyed`: the player destroyed (the game's ~player,
/// which the site called), then its proposal lets go of it (its profile
/// read before). Any thread.
pub fn on_player_destroyed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(player) = Destroyed::at(ctx) else { return Flow::Continue };
    let (key, profile) = (player.key(), player.profile());
    player.destroy();
    if let Some(profile) = profile {
        drop(flow::destroyed(&Locked(thread), key, profile));
    }
    Flow::Skip
}

/// `late_join.bind_place`: a place of a world being made, bound as a late
/// place is (patches 11 and 14), instead of the game's call.
pub fn on_bind_place(ctx: &mut Context<'_>) -> Flow<LateJoinOnBindPlaceExit> {
    let Some((core, slot)) = Core::at_bind_place(ctx) else { return Flow::Continue };
    bind_place(&core, &slot);
    Flow::Exit(LateJoinOnBindPlaceExit::Bound)
}

/// `late_join.animations`: a player's animations registered, their ids
/// given as a later place's are (instead of the game's call).
pub fn on_animations(ctx: &mut Context<'_>) -> Flow<LateJoinOnAnimationsExit> {
    let Some((player, registry)) = AnimationRegistry::at_register(ctx) else { return Flow::Continue };
    animations::register(&registry, player);
    Flow::Exit(LateJoinOnAnimationsExit::Registered)
}
