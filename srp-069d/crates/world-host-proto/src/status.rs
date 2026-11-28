//! The world host's room for new matches: `ListStatus` 0x06 (no payload, on a connection of its own like the project
//! list) is answered with one `Status` 0x8C ([`HostStatus`]) and the connection is closed. The lobby asks for it about
//! every 0.5 s and starts a queue match on a map the host can take now instead of one the host would refuse.
//!
//! `Status` payload: `u8 + [u32 max_zygotes]` (0 = no limit), `u32 max_matches`, `u8 + [u32 free_mb]` (MemAvailable,
//! 0 = not read), `u8 new-map kind + [u8 n, n x (u8 load kind, u16 evicts)]` ([`NewMaps`]), `u16 n`, n x (`str project,
//! u16 running, u16 starting, u8 room, u8 + [u16 evict rank], u32 loaded_secs`) ([`LoadedMap`]), `u8 + [str drain]`
//! ([`HostStatus::drain`]).

use super::{check_text, ty, Error, Frame, R, W, MAX_PROJECTS};

/// Most maps [`NewMaps::Room`] lists (a queue starts far fewer matches between two status reads).
pub const MAX_NEW_MAPS: usize = 32;

/// Where the next new map's zygote fits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    /// within max_zygotes (or no limit)
    Within,
    /// one over max_zygotes: every loaded map runs matches and the memory allows one more
    Over,
}

/// One map that is not loaded and can load now, in the front's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewMap {
    pub kind: LoadKind,
    /// idle maps the front evicts first for it: the next ones by [`LoadedMap::evict`] rank
    pub evicts: u16,
}

/// Whether maps that are not loaded can load now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewMaps {
    /// these new maps can load one after another (each counted busy once loaded; at most [`MAX_NEW_MAPS`]), in order
    Room(Vec<NewMap>),
    /// no new map can load now: the busy maps fill max_zygotes and the memory does not allow one over, or an evicted
    /// map is still exiting
    NoRoom,
    /// the base process is not ready (starting, or started again): no map loads until it is
    BaseNotReady,
}

impl NewMaps {
    /// A new map can load now.
    pub fn loads(&self) -> bool {
        matches!(self, NewMaps::Room(v) if !v.is_empty())
    }

    /// How many new maps can load now.
    pub fn count(&self) -> usize {
        match self {
            NewMaps::Room(v) => v.len(),
            _ => 0,
        }
    }
}

/// Whether a loaded map takes one more match (the zygote's own rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchRoom {
    /// under max_matches
    Within,
    /// over max_matches (at most twice it): the memory allows one more
    Over,
    /// no more: twice max_matches, or not enough memory for one over it
    Full,
}

impl MatchRoom {
    pub fn takes_one(self) -> bool {
        self != MatchRoom::Full
    }
}

/// One loaded map (a live zygote).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedMap {
    /// its level project
    pub project: String,
    /// matches running on it
    pub running: u16,
    /// matches on their way to it: handed over and not taken yet, or arrived at the front and not handed over yet
    pub starting: u16,
    pub room: MatchRoom,
    /// idle (no match running or on its way): the order in which the front evicts it for a new map (0 first: the least
    /// recently used, `prestart` maps last); None = busy, never evicted
    pub evict: Option<u16>,
    /// seconds since it was loaded
    pub loaded_secs: u32,
}

/// The world host's answer to `ListStatus`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostStatus {
    /// max_zygotes (None = no limit)
    pub max_zygotes: Option<u32>,
    pub max_matches: u32,
    /// MemAvailable in MB when the host read it
    pub free_mb: Option<u32>,
    pub new_maps: NewMaps,
    pub loaded: Vec<LoadedMap>,
    /// the loaded map the queues should drain (no new match while another fits), chosen by the front while no new map
    /// can load; None otherwise
    pub drain: Option<String>,
}

impl HostStatus {
    /// The loaded map of `project`.
    pub fn map(&self, project: &str) -> Option<&LoadedMap> {
        self.loaded.iter().find(|m| m.project == project)
    }

    /// `project` is loaded and takes one more match.
    pub fn takes_one(&self, project: &str) -> bool {
        self.map(project).is_some_and(|m| m.room.takes_one())
    }

    /// "L loaded (R with room, I idle, M matches), ..." for the log lines.
    pub fn summary(&self) -> String {
        let open = self.loaded.iter().filter(|m| m.room.takes_one()).count();
        let idle = self.loaded.iter().filter(|m| m.evict.is_some()).count();
        let matches: u32 = self.loaded.iter().map(|m| u32::from(m.running) + u32::from(m.starting)).sum();
        let cap = self.max_zygotes.map_or("no limit".to_string(), |c| c.to_string());
        let new = match &self.new_maps {
            NewMaps::Room(v) => {
                let evicts: u32 = v.iter().map(|m| u32::from(m.evicts)).sum();
                let over = v.iter().filter(|m| m.kind == LoadKind::Over).count();
                format!("{} can load ({evicts} idle map(s) evicted for them, {over} over the limit)", v.len())
            }
            NewMaps::NoRoom => "none can load".into(),
            NewMaps::BaseNotReady => "none can load (base process not ready)".into(),
        };
        let mem = self.free_mb.map_or("memory not read".to_string(), |mb| format!("{mb} MB available"));
        let drain = self.drain.as_ref().map_or(String::new(), |d| format!(", draining {d}"));
        format!("{} loaded ({open} with room, {idle} idle, {matches} matches), max_zygotes {cap}, max_matches {}, new maps: {new}, {mem}{drain}", self.loaded.len(), self.max_matches)
    }

    /// The `Status` payload (0x8C).
    pub fn payload(&self) -> Result<Vec<u8>, Error> {
        if self.loaded.len() > MAX_PROJECTS {
            return Err(Error::Limit("Status count"));
        }
        let mut w = W::default();
        opt_u32(&mut w, self.max_zygotes);
        w.u32(self.max_matches);
        opt_u32(&mut w, self.free_mb);
        match &self.new_maps {
            NewMaps::Room(v) => {
                if v.len() > MAX_NEW_MAPS {
                    return Err(Error::Limit("Status new maps"));
                }
                w.u8(0).u8(v.len() as u8);
                for m in v {
                    w.u8(match m.kind {
                        LoadKind::Within => 0,
                        LoadKind::Over => 1,
                    })
                    .u16(m.evicts);
                }
            }
            NewMaps::NoRoom => {
                w.u8(1);
            }
            NewMaps::BaseNotReady => {
                w.u8(2);
            }
        }
        w.u16(self.loaded.len() as u16);
        for m in &self.loaded {
            check_text(&m.project, "Status project")?;
            w.str(&m.project).u16(m.running).u16(m.starting).u8(match m.room {
                MatchRoom::Within => 0,
                MatchRoom::Over => 1,
                MatchRoom::Full => 2,
            });
            match m.evict {
                Some(r) => {
                    w.u8(1).u16(r);
                }
                None => {
                    w.u8(0);
                }
            }
            w.u32(m.loaded_secs);
        }
        match &self.drain {
            Some(d) => {
                check_text(d, "Status drain")?;
                w.u8(1).str(d);
            }
            None => {
                w.u8(0);
            }
        }
        Ok(w.0)
    }

    /// A `Status` frame (0x8C); any other type is [`Error::Type`].
    pub fn decode(f: &Frame) -> Result<HostStatus, Error> {
        if f.ty != ty::STATUS {
            return Err(Error::Type(f.ty));
        }
        let mut r = R::new(&f.payload);
        let max_zygotes = get_opt_u32(&mut r)?;
        let max_matches = r.u32()?;
        let free_mb = get_opt_u32(&mut r)?;
        let new_maps = match r.u8()? {
            0 => {
                let n = usize::from(r.u8()?);
                if n > MAX_NEW_MAPS {
                    return Err(Error::Limit("Status new maps"));
                }
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    let kind = match r.u8()? {
                        0 => LoadKind::Within,
                        1 => LoadKind::Over,
                        _ => return Err(Error::Limit("Status load kind")),
                    };
                    v.push(NewMap { kind, evicts: r.u16()? });
                }
                NewMaps::Room(v)
            }
            1 => NewMaps::NoRoom,
            2 => NewMaps::BaseNotReady,
            _ => return Err(Error::Limit("Status new-map kind")),
        };
        let n = usize::from(r.u16()?);
        if n > MAX_PROJECTS {
            return Err(Error::Limit("Status count"));
        }
        let mut loaded = Vec::with_capacity(n);
        for _ in 0..n {
            let project = r.str("Status project")?;
            let (running, starting) = (r.u16()?, r.u16()?);
            let room = match r.u8()? {
                0 => MatchRoom::Within,
                1 => MatchRoom::Over,
                2 => MatchRoom::Full,
                _ => return Err(Error::Limit("Status room")),
            };
            let evict = match r.u8()? {
                0 => None,
                1 => Some(r.u16()?),
                _ => return Err(Error::Limit("Status option")),
            };
            loaded.push(LoadedMap { project, running, starting, room, evict, loaded_secs: r.u32()? });
        }
        let drain = match r.u8()? {
            0 => None,
            1 => Some(r.str("Status drain")?),
            _ => return Err(Error::Limit("Status option")),
        };
        r.end()?;
        Ok(HostStatus { max_zygotes, max_matches, free_mb, new_maps, loaded, drain })
    }
}

fn opt_u32(w: &mut W, v: Option<u32>) {
    match v {
        Some(x) => {
            w.u8(1).u32(x);
        }
        None => {
            w.u8(0);
        }
    }
}

fn get_opt_u32(r: &mut R) -> Result<Option<u32>, Error> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(r.u32()?)),
        _ => Err(Error::Limit("Status option")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> HostStatus {
        HostStatus {
            max_zygotes: Some(3),
            max_matches: 4,
            free_mb: Some(2100),
            new_maps: NewMaps::Room(vec![NewMap { kind: LoadKind::Within, evicts: 1 }, NewMap { kind: LoadKind::Over, evicts: 0 }]),
            loaded: vec![
                LoadedMap { project: "level_01_day_tdm".into(), running: 4, starting: 1, room: MatchRoom::Over, evict: None, loaded_secs: 3600 },
                LoadedMap { project: "pve_01_v1_maze2".into(), running: 8, starting: 0, room: MatchRoom::Full, evict: None, loaded_secs: 7200 },
                LoadedMap { project: "level_05_v1_day_br".into(), running: 0, starting: 0, room: MatchRoom::Within, evict: Some(0), loaded_secs: 12 },
            ],
            drain: Some("level_01_day_tdm".into()),
        }
    }

    #[test]
    fn the_status_round_trips() {
        let mut s = sample();
        let f = Frame { ty: ty::STATUS, payload: s.payload().unwrap() };
        assert_eq!(HostStatus::decode(&f).unwrap(), s);
        let most = NewMaps::Room(vec![NewMap { kind: LoadKind::Within, evicts: 0 }; MAX_NEW_MAPS]);
        for nm in [NewMaps::NoRoom, NewMaps::BaseNotReady, NewMaps::Room(vec![]), most] {
            s.new_maps = nm;
            s.max_zygotes = None;
            s.free_mb = None;
            s.loaded.truncate(1);
            s.drain = None;
            let f = Frame { ty: ty::STATUS, payload: s.payload().unwrap() };
            assert_eq!(HostStatus::decode(&f).unwrap(), s);
        }
    }

    #[test]
    fn a_bad_status_is_refused() {
        let good = sample().payload().unwrap();
        assert_eq!(HostStatus::decode(&Frame { ty: ty::PROJECTS, payload: good.clone() }), Err(Error::Type(ty::PROJECTS)));
        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(HostStatus::decode(&Frame { ty: ty::STATUS, payload: trailing }), Err(Error::Trailing(1)));
        assert_eq!(HostStatus::decode(&Frame { ty: ty::STATUS, payload: good[..good.len() - 1].to_vec() }), Err(Error::Short));
        // the third map's room byte: after its name, running and starting; its evict and loaded_secs follow (1 + 2 + 4),
        // then the drain (1 + 2 + 16 bytes of "level_01_day_tdm")
        let mut room = good.clone();
        let at = room.len() - 19 - 7 - 1;
        room[at] = 3;
        assert_eq!(HostStatus::decode(&Frame { ty: ty::STATUS, payload: room }), Err(Error::Limit("Status room")));
        // more new maps than a status lists
        let s = HostStatus { new_maps: NewMaps::Room(vec![NewMap { kind: LoadKind::Within, evicts: 0 }; MAX_NEW_MAPS + 1]), ..sample() };
        assert_eq!(s.payload(), Err(Error::Limit("Status new maps")));
        let mut p = HostStatus { new_maps: NewMaps::Room(vec![]), ..sample() }.payload().unwrap();
        p[1 + 4 + 4 + 1 + 4 + 1] = (MAX_NEW_MAPS + 1) as u8;
        assert_eq!(HostStatus::decode(&Frame { ty: ty::STATUS, payload: p }), Err(Error::Limit("Status new maps")));
    }

    #[test]
    fn the_room_reads_take_the_loaded_maps_and_the_new_ones() {
        let s = sample();
        assert!(s.new_maps.loads() && s.new_maps.count() == 2);
        assert!(!NewMaps::Room(vec![]).loads(), "an empty list is no room");
        assert!(!NewMaps::NoRoom.loads() && NewMaps::BaseNotReady.count() == 0);
        assert!(s.takes_one("level_01_day_tdm") && s.takes_one("level_05_v1_day_br"));
        assert!(!s.takes_one("pve_01_v1_maze2"), "full");
        assert!(!s.takes_one("level_02_day_tdm"), "not loaded");
        assert!(s.summary().contains("3 loaded (2 with room, 1 idle, 13 matches)"), "{}", s.summary());
        assert!(s.summary().ends_with(", draining level_01_day_tdm"), "{}", s.summary());
        let mut bad = s.payload().unwrap();
        let n = bad.len();
        bad[n - 19] = 2;
        assert_eq!(HostStatus::decode(&Frame { ty: ty::STATUS, payload: bad }), Err(Error::Limit("Status option")), "the drain's option byte");
    }
}
