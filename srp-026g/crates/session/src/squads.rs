//! Squads, as the 0.21d client forms them (`../srp-021d/docs/notes/lobby_spec.md`
//! §2.7, §5.3):
//! - A player invites another through the chat (important message type 2).
//! - The invited player accepts, and its client tells the inviter
//!   (type 3, "membership accepted").
//! - The inviter's client then asks the lobby to add that player (`2A 02 name`).
//!
//! The first member is the commander. A squad holds at most
//! [`Squads::size`] players: three, the client's own limit, or two when the
//! server says so (`SRP_SQUAD_SIZE`).
//! Each member readies with Play. The commander's Play is unlocked once all
//! the others are ready, and the squad is in the queue once everyone is.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::LockExt;

/// The client keeps three members (`read_squad_info`, spec R7): no server
/// setting goes above it.
pub const MAX_SQUAD_MEMBERS: usize = 3;

/// The smallest squad: a squad of one is none.
pub const MIN_SQUAD_SIZE: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Squad {
    pub id: u32,
    /// Account ids; the commander first.
    pub members: Vec<u32>,
    /// The members that pressed Play and are still waiting.
    pub ready: Vec<u32>,
    /// Bumped on every change, so a lobby connection knows when to resend.
    pub revision: u32,
}

impl Squad {
    pub fn commander(&self) -> u32 {
        self.members[0]
    }

    pub fn is_ready(&self, account_id: u32) -> bool {
        self.ready.contains(&account_id)
    }

    /// Everyone pressed Play: the squad waits for a match together.
    pub fn all_ready(&self) -> bool {
        self.members
            .iter()
            .all(|member| self.ready.contains(member))
    }
}

/// Why a squad change was refused. The client has no answer for 0x2A, so
/// these only go to the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SquadError {
    /// The player did not accept an invite from this commander.
    NotInvited,
    Full,
    /// The player is already in a squad.
    InASquad,
    NotCommander,
    NotInASquad,
}

pub struct Squads {
    inner: Mutex<Inner>,
    /// The most members a squad may have, [`MIN_SQUAD_SIZE`] to
    /// [`MAX_SQUAD_MEMBERS`] (`SRP_SQUAD_SIZE`).
    size: AtomicUsize,
}

impl Default for Squads {
    fn default() -> Self {
        Self {
            inner: Mutex::default(),
            size: AtomicUsize::new(MAX_SQUAD_MEMBERS),
        }
    }
}

#[derive(Default)]
struct Inner {
    next_id: u32,
    squads: HashMap<u32, Squad>,
    /// Which squad each member is in.
    squad_of: HashMap<u32, u32>,
    /// Accepted invites not used yet, as (inviter, invited).
    accepted: HashSet<(u32, u32)>,
}

impl Inner {
    fn squad_mut(&mut self, account_id: u32) -> Option<&mut Squad> {
        let id = *self.squad_of.get(&account_id)?;
        self.squads.get_mut(&id)
    }

    /// A membership change: everyone has to ready again.
    fn changed(squad: &mut Squad) {
        squad.ready.clear();
        squad.revision += 1;
    }

    fn remove_member(&mut self, account_id: u32) {
        let Some(id) = self.squad_of.remove(&account_id) else {
            return;
        };
        let Some(squad) = self.squads.get_mut(&id) else {
            return;
        };
        squad.members.retain(|member| *member != account_id);
        Self::changed(squad);
        // A squad of one is no squad.
        if squad.members.len() < 2 {
            for member in std::mem::take(&mut squad.members) {
                self.squad_of.remove(&member);
            }
            self.squads.remove(&id);
        }
    }
}

impl Squads {
    /// The most members a squad may have.
    pub fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Set [`Self::size`], kept from [`MIN_SQUAD_SIZE`] to
    /// [`MAX_SQUAD_MEMBERS`]. A squad that already has more keeps them; it
    /// takes nobody new until it is under the size.
    pub fn set_size(&self, size: usize) {
        let size = size.clamp(MIN_SQUAD_SIZE, MAX_SQUAD_MEMBERS);
        self.size.store(size, Ordering::Relaxed);
    }

    /// Whether `account_id`'s squad has as many members as it may, so that
    /// nobody more can be added. A player in no squad has room: it would
    /// start a squad of two.
    pub fn is_full(&self, account_id: u32) -> bool {
        let size = self.size();
        let mut inner = self.inner.locked();
        inner
            .squad_mut(account_id)
            .is_some_and(|squad| squad.members.len() >= size)
    }

    /// `invited` accepted `inviter`'s invite: `inviter` may now add it.
    pub fn invite_accepted(&self, inviter: u32, invited: u32) {
        self.inner.locked().accepted.insert((inviter, invited));
    }

    /// Whether `invited` accepted an invite from `inviter` that is not used yet.
    pub fn has_accepted(&self, inviter: u32, invited: u32) -> bool {
        self.inner.locked().accepted.contains(&(inviter, invited))
    }

    /// `2A 02`: `commander` adds `member`, who accepted its invite. A player
    /// in no squad starts one and commands it.
    pub fn add(&self, commander: u32, member: u32) -> Result<Squad, SquadError> {
        let size = self.size();
        let mut inner = self.inner.locked();
        if !inner.accepted.contains(&(commander, member)) {
            return Err(SquadError::NotInvited);
        }
        if inner.squad_of.contains_key(&member) {
            return Err(SquadError::InASquad);
        }
        if let Some(squad) = inner.squad_mut(commander) {
            if squad.commander() != commander {
                return Err(SquadError::NotCommander);
            }
            // Checked under the lock: of two accepted invites added at
            // once, only the first gets the last place. The other invite
            // is used up: it was for a place that is gone.
            if squad.members.len() >= size {
                inner.accepted.remove(&(commander, member));
                return Err(SquadError::Full);
            }
        }
        inner.accepted.remove(&(commander, member));
        let id = match inner.squad_of.get(&commander) {
            Some(id) => *id,
            None => {
                inner.next_id += 1;
                let id = inner.next_id;
                inner.squads.insert(
                    id,
                    Squad {
                        id,
                        members: vec![commander],
                        ready: Vec::new(),
                        revision: 0,
                    },
                );
                inner.squad_of.insert(commander, id);
                id
            }
        };
        inner.squad_of.insert(member, id);
        let squad = inner.squads.get_mut(&id).expect("the squad was just found");
        squad.members.push(member);
        Inner::changed(squad);
        Ok(squad.clone())
    }

    /// `2A 04`: the player leaves. A commander leaving hands the squad to the
    /// next member.
    pub fn leave(&self, account_id: u32) -> Result<(), SquadError> {
        let mut inner = self.inner.locked();
        if !inner.squad_of.contains_key(&account_id) {
            return Err(SquadError::NotInASquad);
        }
        inner.remove_member(account_id);
        Ok(())
    }

    /// `2A 03`: the commander drops a member.
    pub fn drop_member(&self, commander: u32, member: u32) -> Result<(), SquadError> {
        let mut inner = self.inner.locked();
        let squad = inner.squad_mut(commander).ok_or(SquadError::NotInASquad)?;
        if squad.commander() != commander {
            return Err(SquadError::NotCommander);
        }
        if member == commander || !squad.members.contains(&member) {
            return Err(SquadError::NotInASquad);
        }
        inner.remove_member(member);
        Ok(())
    }

    /// `2A 01`: the commander disbands the squad.
    pub fn disband(&self, commander: u32) -> Result<(), SquadError> {
        let mut inner = self.inner.locked();
        let squad = inner.squad_mut(commander).ok_or(SquadError::NotInASquad)?;
        if squad.commander() != commander {
            return Err(SquadError::NotCommander);
        }
        let id = squad.id;
        if let Some(squad) = inner.squads.remove(&id) {
            for member in squad.members {
                inner.squad_of.remove(&member);
            }
        }
        Ok(())
    }

    /// The player's squad as it is now.
    pub fn of(&self, account_id: u32) -> Option<Squad> {
        let mut inner = self.inner.locked();
        inner.squad_mut(account_id).map(|squad| squad.clone())
    }

    /// Play (ready) or leaving the queue (not ready). Returns the squad, or
    /// `None` for a player in no squad.
    pub fn set_ready(&self, account_id: u32, ready: bool) -> Option<Squad> {
        let mut inner = self.inner.locked();
        let squad = inner.squad_mut(account_id)?;
        if squad.is_ready(account_id) != ready {
            if ready {
                squad.ready.push(account_id);
            } else {
                squad.ready.retain(|member| *member != account_id);
            }
            squad.revision += 1;
        }
        Some(squad.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 10;
    const B: u32 = 20;
    const C: u32 = 30;
    const D: u32 = 40;

    fn squad_of(squads: &Squads, members: &[u32]) -> Squad {
        let mut squad = None;
        for member in &members[1..] {
            squads.invite_accepted(members[0], *member);
            squad = Some(squads.add(members[0], *member).unwrap());
        }
        squad.unwrap()
    }

    #[test]
    fn only_an_accepted_invite_adds_a_player() {
        let squads = Squads::default();
        assert_eq!(squads.add(A, B), Err(SquadError::NotInvited));
        squads.invite_accepted(A, B);
        let squad = squads.add(A, B).unwrap();
        assert_eq!(squad.members, vec![A, B]);
        assert_eq!(squads.of(B).unwrap().commander(), A);
        // The invite is used up.
        assert!(!squads.has_accepted(A, B));
    }

    #[test]
    fn a_squad_holds_three() {
        let squads = Squads::default();
        squad_of(&squads, &[A, B, C]);
        squads.invite_accepted(A, D);
        assert_eq!(squads.add(A, D), Err(SquadError::Full));
    }

    #[test]
    fn a_squad_of_two_takes_nobody_more() {
        let squads = Squads::default();
        squads.set_size(2);
        assert!(!squads.is_full(A));
        // Two invites accepted at once: whichever is added first gets the
        // place, the other is refused.
        squads.invite_accepted(A, B);
        squads.invite_accepted(A, C);
        assert_eq!(squads.add(A, B).unwrap().members, vec![A, B]);
        assert!(squads.is_full(A) && squads.is_full(B));
        assert_eq!(squads.add(A, C), Err(SquadError::Full));
        assert_eq!(squads.of(C), None);
        assert!(!squads.has_accepted(A, C));
        // A place freed later does not take C on that old invite.
        squads.leave(B).unwrap();
        assert_eq!(squads.add(A, C), Err(SquadError::NotInvited));
    }

    #[test]
    fn two_invites_added_from_two_threads_make_one_squad_of_two() {
        let squads = std::sync::Arc::new(Squads::default());
        squads.set_size(2);
        squads.invite_accepted(A, B);
        squads.invite_accepted(A, C);
        let adds: Vec<_> = [B, C]
            .into_iter()
            .map(|member| {
                let squads = squads.clone();
                std::thread::spawn(move || squads.add(A, member))
            })
            .collect();
        let results: Vec<_> = adds.into_iter().map(|add| add.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(results.contains(&Err(SquadError::Full)));
        assert_eq!(squads.of(A).unwrap().members.len(), 2);
    }

    #[test]
    fn a_member_leaving_frees_its_place() {
        let squads = Squads::default();
        squads.set_size(2);
        squads.invite_accepted(A, B);
        squads.add(A, B).unwrap();
        squads.leave(B).unwrap();
        // A squad of one is none: A starts a new one with C.
        assert!(!squads.is_full(A));
        squads.invite_accepted(A, C);
        assert_eq!(squads.add(A, C).unwrap().members, vec![A, C]);
    }

    #[test]
    fn the_size_is_two_or_three() {
        let squads = Squads::default();
        assert_eq!(squads.size(), MAX_SQUAD_MEMBERS);
        squads.set_size(1);
        assert_eq!(squads.size(), MIN_SQUAD_SIZE);
        squads.set_size(10);
        assert_eq!(squads.size(), MAX_SQUAD_MEMBERS);
        // Three, as before.
        squad_of(&squads, &[A, B]);
        assert!(!squads.is_full(A));
        squad_of(&squads, &[A, C]);
        assert!(squads.is_full(A));
    }

    #[test]
    fn only_the_commander_adds_and_drops() {
        let squads = Squads::default();
        squad_of(&squads, &[A, B]);
        squads.invite_accepted(B, C);
        assert_eq!(squads.add(B, C), Err(SquadError::NotCommander));
        assert_eq!(squads.drop_member(B, A), Err(SquadError::NotCommander));
        squads.invite_accepted(D, B);
        assert_eq!(squads.add(D, B), Err(SquadError::InASquad));
    }

    #[test]
    fn the_last_member_leaving_ends_the_squad() {
        let squads = Squads::default();
        squad_of(&squads, &[A, B, C]);
        squads.leave(A).unwrap();
        assert_eq!(squads.of(B).unwrap().members, vec![B, C]);
        squads.drop_member(B, C).unwrap();
        assert_eq!(squads.of(B), None);
        assert_eq!(squads.of(C), None);
    }

    #[test]
    fn disbanding_frees_everyone() {
        let squads = Squads::default();
        squad_of(&squads, &[A, B, C]);
        assert_eq!(squads.disband(B), Err(SquadError::NotCommander));
        squads.disband(A).unwrap();
        assert!([A, B, C].iter().all(|member| squads.of(*member).is_none()));
    }

    #[test]
    fn everyone_readies_and_a_change_unreadies() {
        let squads = Squads::default();
        squad_of(&squads, &[A, B, C]);
        squads.set_ready(B, true);
        squads.set_ready(C, true);
        assert!(!squads.of(A).unwrap().all_ready());
        let squad = squads.set_ready(A, true).unwrap();
        assert!(squad.all_ready());
        let revision = squad.revision;
        squads.leave(C).unwrap();
        let squad = squads.of(A).unwrap();
        assert!(squad.ready.is_empty());
        assert!(squad.revision > revision);
        assert_eq!(squads.set_ready(D, true), None);
    }
}
