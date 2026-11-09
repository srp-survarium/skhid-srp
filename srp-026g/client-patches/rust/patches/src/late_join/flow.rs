//! The late join's steps between the game's events and the [`Runtime`]: each
//! takes the runtime's lock for its own steps and makes the game's calls
//! between them, never inside one (the game may call back into another step:
//! a query can complete at once, a world's release can destroy players).
//! The same code runs over the game (late_join.rs's [`Game`]) and over the
//! tests' model of it.

use super::profile::CheckedProfile;
use super::runtime::{
    Ack, Admission, Answer, ClientKey, Completion, Dead, DictionaryKey, KitMask, KitPlayers, MatchKey, Outcome,
    Place, PlayerKey, Prepare, ProfileKey, Roster, Runtime, Squad, KITS,
};
use crate::faces::{Face, Name};

/// The runtime behind its lock (the game's `vostok::Lock`, the tests' mutex).
pub trait Shared<S> {
    fn with<T>(&self, f: impl FnOnce(&mut Runtime<S>) -> T) -> T;
}

/// A proposal's storage: the profile its players are loaded from and its
/// checked bytes, shared with its query; the last reference frees it.
pub trait Storage: Clone {
    fn profile(&self) -> ProfileKey;
    fn same_body(&self, body: &[u8]) -> bool;
}


/// The game's side of the late join.
pub trait Game {
    type Storage: Storage;
    /// A player a query loaded, live for its callback.
    type Player: Copy;

    fn player(&self, player: Self::Player) -> PlayerKey;
    /// Stores a checked profile: allocated, its profile body made from it.
    fn store(&mut self, profile: &CheckedProfile<'_>) -> Option<Self::Storage>;
    /// The match's dictionary checks the body.
    fn check<'a>(&self, dictionary: DictionaryKey, body: &'a [u8]) -> Option<CheckedProfile<'a>>;
    /// Patch 123 learns srp's face of the newcomer of `place`, named `name`.
    fn learn_face(&mut self, place: Place, name: Name, face: Face);
    /// Asks for the players of the kits `usable` (it holds the storage and
    /// the match's world until its callback); its callback may come at once.
    fn query(&mut self, storage: Self::Storage, usable: KitMask, matched: MatchKey, place: Place);
    /// Puts the players into the match's next place, its profile the
    /// storage's; whether they are in.
    fn install(&mut self, storage: &Self::Storage, players: KitPlayers<Self::Player>, place: Place, squad: Squad,
               matched: MatchKey, dictionary: DictionaryKey) -> bool;
    fn ack(&mut self, client: ClientKey, ack: Ack);
}

/// A match is ready to play: it is the late join's.
pub fn ready<S>(shared: &impl Shared<S>, matched: MatchKey, dictionary: DictionaryKey, roster: Roster) -> Dead<S> {
    shared.with(|rt| rt.ready(matched, dictionary, roster))
}

/// No match takes newcomers.
pub fn deactivate<S>(shared: &impl Shared<S>) -> Dead<S> {
    shared.with(|rt| rt.deactivate())
}

/// `client` left its match.
pub fn retire<S>(shared: &impl Shared<S>, client: ClientKey) -> Dead<S> {
    shared.with(|rt| rt.retire(client))
}

/// The server's proposal for a newcomer (an E4 of the late join's, from
/// `client` while it plays): answered, or its profile checked, its face
/// learnt, the profile stored and its players asked for (the face known
/// before the players' cook draws them).
pub fn prepare<G: Game>(shared: &impl Shared<G::Storage>, game: &mut G, client: ClientKey, bytes: &[u8]) {
    let Some(prepare) = Prepare::parse(bytes) else { return };
    let admitted = match shared.with(|rt| rt.propose(client, &prepare, |storage, body| storage.same_body(body))) {
        Admission::Ignore => return,
        Admission::Answer(ack) => return game.ack(client, ack),
        Admission::Load(admitted) => admitted,
    };
    let Some(checked) = game.check(admitted.dictionary, prepare.body) else {
        return game.ack(client, admitted.refused(Answer::Failed));
    };
    if let Some(name) = checked.name() {
        game.learn_face(admitted.place, name, checked.face());
    }
    let Some(storage) = game.store(&checked) else {
        return game.ack(client, admitted.refused(Answer::Busy));
    };
    let usable = checked.usable();
    let kept = shared.with(|rt| rt.admit(admitted, usable, storage.profile(), storage.clone()));
    match kept {
        Ok(()) => game.query(storage, usable, admitted.matched, admitted.place),
        Err(_) => game.ack(client, admitted.refused(Answer::Busy)),
    }
}

/// A proposal's query came back with its players (by kit): installed or
/// refused, and told, when its match is still active. The caller then lets
/// the query's storage and world go (the world last: its release may destroy
/// players at once).
pub fn complete<G: Game>(shared: &impl Shared<G::Storage>, game: &mut G, storage: &G::Storage,
                         players: [Option<G::Player>; KITS]) -> Dead<G::Storage> {
    let profile = storage.profile();
    let keys = players.map(|p| p.map(|p| game.player(p)));
    let outcome = match shared.with(|rt| rt.loaded(profile, keys)) {
        Completion::Unknown => return Dead::default(),
        Completion::Stale => None,
        Completion::Refuse => Some(Outcome::Failed),
        Completion::Install { players: matched_keys, place, squad, matched, dictionary } => {
            // the live players of the kits the runtime matched
            let installed = matched_keys.mask().matched(players)
                .is_some_and(|live| game.install(storage, live, place, squad, matched, dictionary));
            Some(if installed { Outcome::Installed } else { Outcome::Failed })
        }
    };
    let answered = shared.with(|rt| rt.answered(profile, outcome));
    if let Some((client, ack)) = answered.ack {
        game.ack(client, ack);
    }
    answered.dead
}

/// A player was destroyed (any thread): its proposal lets go of it.
pub fn destroyed<S>(shared: &impl Shared<S>, player: PlayerKey, profile: ProfileKey) -> Dead<S> {
    shared.with(|rt| rt.destroyed(player, profile))
}

#[cfg(test)]
mod tests;
