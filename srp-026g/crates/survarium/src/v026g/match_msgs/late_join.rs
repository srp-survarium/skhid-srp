//! Growing-roster protocol: E4 prepares a profile; E0 confirms installation.
//! Every client must support this dialect. Only matching resource ACKs release
//! admission. E0's impossible snapshot prefix also permits multi-datagram framing.

use super::super::dictionary::Dictionary;
use super::MAX_PLAYERS;
use super::static_info::{self, MatchProfile, StaticInfoError};

const PREPARE_MAGIC: [u8; 4] = [0xff, b'L', b'J', 1];
const PREPARED_MAGIC: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0];
const PREPARE_HEADER: usize = 14;
pub const PREPARED_SIZE: usize = 20;
/// D8's debug byte is exactly 0x80 (normal) or 0x81 (debug) for dialect 1.
/// The stock writer sends 0 or 0xff: testing bit 7 alone is insufficient.
/// Released snapshot-upload patches do not support this dialect.
pub const CONNECT_CAPABILITY: u8 = 0x80;
/// The login revision of an exe built by hand from these sources before
/// the dialect had a released revision.
pub const PATCH_REVISION: u8 = 0x80;
/// The first released patch revision whose exe speaks this dialect
/// (`client-patches/patch_026g.py`'s `REVISION`): every later one does.
pub const FIRST_REVISION: u8 = 5;

/// Whether a game that signed in with this patch revision speaks the
/// dialect. Such a game and one that does not never play one match.
pub fn spoken_by(revision: u8) -> bool {
    revision >= FIRST_REVISION
}

/// D8's bit for the per-tick protocol (`sync_core::wire`), on top of this
/// dialect: 0x82, or 0x83 with `-debug_hash_mismatch`. A server that does
/// not speak it takes such a game as one of this dialect (its 0xE4 then
/// has no per-tick trailer, and the game plays the stock protocol).
pub const TICKS_CAPABILITY: u8 = 0x02;
/// D8's bit for `-debug_hash_mismatch` in this dialect.
pub const DEBUG_CAPABILITY: u8 = 0x01;

pub fn has_connect_capability(debug: u8) -> bool {
    debug & !(DEBUG_CAPABILITY | TICKS_CAPABILITY) == CONNECT_CAPABILITY
}

/// Whether a D8 of this dialect announces the per-tick protocol.
pub fn speaks_ticks(debug: u8) -> bool {
    has_connect_capability(debug) && debug & TICKS_CAPABILITY != 0
}

/// Whether a D8 of this dialect is of a game run with
/// `-debug_hash_mismatch`.
pub fn runs_debug(debug: u8) -> bool {
    has_connect_capability(debug) && debug & DEBUG_CAPABILITY != 0
}

/// Binds an acknowledgement to one match and one immutable profile proposal.
/// The server must never reuse a serial for a different profile in that match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proposal {
    /// Static-info seed, not the lobby's match ID.
    pub match_id: u32,
    pub serial: u32,
    /// Append-only slot; the first player cannot be a late joiner.
    pub place: u8,
}

impl Proposal {
    fn check(self) -> Result<(), Error> {
        if self.serial == 0 || self.place == 0 || usize::from(self.place) >= MAX_PLAYERS {
            return Err(Error::Proposal);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prepare {
    pub proposal: Proposal,
    pub profile: MatchProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    /// Profile and all usable kits are installed, still inactive until E9.
    Installed = 0,
    /// No admission: a resource could not be loaded.
    Failed = 1,
    /// Retry the same proposal; another query is still outstanding.
    Busy = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub proposal: Proposal,
    pub status: Status,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Header,
    Proposal,
    Profile(StaticInfoError),
}

impl Prepare {
    pub fn encode(&self, dictionary: &Dictionary) -> Result<Vec<u8>, Error> {
        self.proposal.check()?;
        let profile =
            static_info::profile_bytes(usize::from(self.proposal.place), &self.profile, dictionary)
                .map_err(Error::Profile)?;
        let mut bytes = Vec::with_capacity(PREPARE_HEADER + profile.len());
        bytes.extend(PREPARE_MAGIC);
        bytes.extend(self.proposal.match_id.to_le_bytes());
        bytes.extend(self.proposal.serial.to_le_bytes());
        bytes.push(self.proposal.place);
        bytes.push(self.profile.squad);
        bytes.extend(profile);
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], dictionary: &Dictionary) -> Result<Self, Error> {
        if bytes.len() < PREPARE_HEADER || bytes[..4] != PREPARE_MAGIC {
            return Err(Error::Header);
        }
        let proposal = Proposal {
            match_id: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            serial: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            place: bytes[12],
        };
        proposal.check()?;
        let mut profile = static_info::parse_profile(
            &bytes[PREPARE_HEADER..],
            usize::from(proposal.place),
            dictionary,
        )
        .map_err(Error::Profile)?;
        profile.squad = bytes[13];
        Ok(Self { proposal, profile })
    }
}

impl Prepared {
    pub fn encode(self) -> Result<[u8; PREPARED_SIZE], Error> {
        self.proposal.check()?;
        let mut bytes = [0; PREPARED_SIZE];
        bytes[..8].copy_from_slice(&PREPARED_MAGIC);
        bytes[8..12].copy_from_slice(&self.proposal.match_id.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.proposal.serial.to_le_bytes());
        bytes[16] = self.proposal.place;
        bytes[17] = self.status as u8;
        bytes[18] = 1; // protocol version; byte 19 reserved, must be zero
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != PREPARED_SIZE || bytes[..8] != PREPARED_MAGIC || bytes[18..] != [1, 0] {
            return Err(Error::Header);
        }
        let proposal = Proposal {
            match_id: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            serial: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            place: bytes[16],
        };
        proposal.check()?;
        let status = match bytes[17] {
            0 => Status::Installed,
            1 => Status::Failed,
            2 => Status::Busy,
            _ => return Err(Error::Header),
        };
        Ok(Self { proposal, status })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ClientMessage, id, parse_client};
    use super::*;
    use static_info::{MatchKit, NO_PROTECTION};

    fn prepare() -> Prepare {
        Prepare {
            proposal: Proposal {
                match_id: 0x80001234,
                serial: 1,
                place: 2,
            },
            profile: MatchProfile {
                team: 1,
                name: "New player".into(),
                clan_tag: String::new(),
                protection: NO_PROTECTION,
                kits: vec![MatchKit::empty(1, "kit")],
                items: vec![],
                squad: 7,
            },
        }
    }

    #[test]
    fn a_new_profile_round_trips_and_every_truncation_is_rejected() {
        let dictionary = Dictionary::shared();
        let prepare = prepare();
        let bytes = prepare.encode(dictionary).unwrap();
        assert_eq!(Prepare::decode(&bytes, dictionary).unwrap(), prepare);
        for end in 0..bytes.len() {
            assert!(Prepare::decode(&bytes[..end], dictionary).is_err(), "{end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(Prepare::decode(&trailing, dictionary).is_err());
        let mut wrong_version = bytes;
        wrong_version[3] = 2;
        assert_eq!(
            Prepare::decode(&wrong_version, dictionary),
            Err(Error::Header)
        );
    }

    #[test]
    fn populated_kits_use_the_same_item_codec_as_static_info() {
        use super::super::super::loadouts::{Loadout, STARTER};
        use super::super::super::skills::MODIFIERS;

        let dictionary = Dictionary::shared();
        let loadout = Loadout::from_names(dictionary, STARTER).unwrap();
        let mut request = prepare();
        request.profile = MatchProfile::from_loadout(
            "New player",
            1,
            &loadout,
            [0.0; MODIFIERS],
            dictionary,
            100,
        );
        request.profile.kits.push(request.profile.kits[0].clone());
        request.profile.kits[1].name = "Second kit".into();
        request.profile.kits[1].id = 2;
        request.profile.kits[1].modifiers[67] = 1.25;
        request.profile.squad = 3;
        let bytes = request.encode(dictionary).unwrap();
        assert_eq!(Prepare::decode(&bytes, dictionary).unwrap(), request);
        for end in 0..bytes.len() {
            assert!(Prepare::decode(&bytes[..end], dictionary).is_err(), "{end}");
        }
    }

    #[test]
    fn resource_ack_cannot_be_confused_with_a_leave_or_snapshot() {
        for status in [Status::Installed, Status::Failed, Status::Busy] {
            let ack = Prepared {
                proposal: prepare().proposal,
                status,
            };
            let bytes = ack.encode().unwrap();
            assert_eq!(Prepared::decode(&bytes), Ok(ack));
            assert!(parse_client(id::LEAVE, &bytes).is_err());
            for end in 0..bytes.len() {
                assert!(Prepared::decode(&bytes[..end]).is_err());
            }
        }
        assert!(matches!(
            parse_client(id::LEAVE, &[]),
            Ok(ClientMessage::Leave)
        ));
        assert!(Prepared::decode(&[]).is_err());
    }

    #[test]
    fn invalid_slots_and_profiles_are_refused_before_sending() {
        let dictionary = Dictionary::shared();
        for place in [0, 20, 255] {
            let mut p = prepare();
            p.proposal.place = place;
            assert_eq!(p.encode(dictionary), Err(Error::Proposal));
        }
        let mut p = prepare();
        p.profile.name = "x".repeat(64);
        assert!(matches!(p.encode(dictionary), Err(Error::Profile(_))));
        let mut p = prepare();
        p.profile.kits.clear();
        assert!(matches!(p.encode(dictionary), Err(Error::Profile(_))));
    }
}
