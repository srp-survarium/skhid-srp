//! Fake rooms, **for local tests only** (`SRP_FAKE_ROOMS`): rooms made at
//! start whose members are no clients, so one game window can test a full
//! room and its queue (the owner's of 2026-10-20).
//!
//! - A fake member (`rooms::is_fake`: an id no account has) is shown as any
//!   member, by its name and a gear level, in the list and the room; it
//!   fills a place (a full room offers Queue), but it is no player: it never
//!   counts for a vote or a match (`Rooms::tick`), the lobby never seats it
//!   (`room_ready`) and never sends it anything (it has no link).
//! - Nothing of it is saved: the rooms live in memory only, and a fake has
//!   no account, so no rating, result or stat.
//! - The local admin (`tab=fake_room` of the web admin, `room.sh` beside the
//!   local launcher) adds a fake to a room or takes one out, so a queued
//!   player moves in; `SRP_FAKE_ROOMS_LEAVE_SECONDS` does the same on a
//!   timer for a full fake room with a queue.
//!
//! Unset or empty, nothing of this exists and the admin's action is refused.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use session::LockExt;
use session::v026g::matchmaking;
use session::v026g::rooms::{FAKE_MEMBERS, Spec, Stage, is_fake};

use super::core::Core;

/// A room `SRP_FAKE_ROOMS` asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeRoom {
    /// The room's name: its maker's (the first fake's) name.
    pub name: String,
    pub members: u8,
    /// Its places, both sides: an even number, 2 to 20.
    pub places: u8,
    pub pin: Option<u16>,
}

/// `SRP_FAKE_ROOMS` and `SRP_FAKE_ROOMS_LEAVE_SECONDS`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub rooms: Vec<FakeRoom>,
    /// One fake leaves a full fake room with a queue this often.
    pub leave_every: Option<Duration>,
}

impl Config {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let Some(value) = var("SRP_FAKE_ROOMS") else {
            return Self::default();
        };
        let rooms = parse_rooms(&value).unwrap_or_else(|error| {
            log::error!(
                "SRP_FAKE_ROOMS is `{value}`: {error}. It is `Name:members/places[:PIN],…` \
                 (e.g. `Full 8:8/8,Half:3/8`). No fake rooms"
            );
            Vec::new()
        });
        let leave_every = var("SRP_FAKE_ROOMS_LEAVE_SECONDS").and_then(|value| {
            match value.trim().parse::<u16>() {
                Ok(seconds) if seconds > 0 => Some(Duration::from_secs(seconds.into())),
                _ => {
                    log::error!(
                        "SRP_FAKE_ROOMS_LEAVE_SECONDS is `{value}`; it must be a number of \
                         seconds over 0. No timer"
                    );
                    None
                }
            }
        });
        Self { rooms, leave_every }
    }
}

/// `Name:members/places[:PIN]`, comma-separated.
pub(super) fn parse_rooms(value: &str) -> Result<Vec<FakeRoom>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let mut parts = entry.split(':').map(str::trim);
            let name = parts.next().unwrap_or_default();
            let counts = parts
                .next()
                .ok_or(format!("`{entry}` has no members/places"))?;
            let pin = match parts.next() {
                None => None,
                Some(pin) => Some(
                    pin.parse::<u16>()
                        .ok()
                        .filter(|pin| (1000..=9999).contains(pin))
                        .ok_or(format!("`{entry}`: a PIN is 4 digits"))?,
                ),
            };
            if parts.next().is_some() {
                return Err(format!("`{entry}` has too many parts"));
            }
            let (members, places) = counts
                .split_once('/')
                .and_then(|(members, places)| {
                    Some((
                        members.trim().parse::<u8>().ok()?,
                        places.trim().parse::<u8>().ok()?,
                    ))
                })
                .ok_or(format!("`{entry}`: members/places are two numbers"))?;
            if name.is_empty() || name.len() > 31 {
                return Err(format!("`{entry}`: a name of 1 to 31 bytes"));
            }
            if places % 2 != 0 || !(2..=20).contains(&places) {
                return Err(format!("`{entry}`: places are both sides, even, 2 to 20"));
            }
            if members == 0 || members > places {
                return Err(format!("`{entry}`: 1 to {places} members"));
            }
            Ok(FakeRoom {
                name: name.to_owned(),
                members,
                places,
                pin,
            })
        })
        .collect()
}

/// A fake member.
#[derive(Clone, Debug)]
pub struct Fake {
    pub name: String,
    /// The equipment level it is shown with.
    pub gear: u8,
}

/// The fakes of a local test.
#[derive(Debug, Default)]
pub struct Fakes {
    pub members: HashMap<u32, Fake>,
    /// The fake rooms made at start, in `SRP_FAKE_ROOMS`' order.
    pub rooms: Vec<u32>,
    count: u32,
    next_leave: Option<Instant>,
}

impl Fakes {
    /// A new fake member: "Fake N", gear 4 to 8.
    fn make(&mut self, name: Option<&str>) -> u32 {
        self.count += 1;
        let id = FAKE_MEMBERS + self.count;
        let name = name.map_or_else(|| format!("Fake {}", self.count), str::to_owned);
        let gear = 4 + (self.count % 5) as u8;
        self.members.insert(id, Fake { name, gear });
        id
    }
}

impl Core<'_> {
    /// The fake rooms `SRP_FAKE_ROOMS` asks for, made once at start.
    pub(super) fn seed_fake_rooms(&mut self, now: Instant) {
        let wanted = self.settings.rooms.fakes.rooms.clone();
        if wanted.is_empty() {
            return;
        }
        log::warn!(
            "FAKE ROOMS for local tests (SRP_FAKE_ROOMS): {}; a fake leaves a full one with a \
             queue {}",
            wanted
                .iter()
                .map(|room| format!("{} {}/{}", room.name, room.members, room.places))
                .collect::<Vec<_>>()
                .join(", "),
            self.settings
                .rooms
                .fakes
                .leave_every
                .map_or("never".to_owned(), |every| format!(
                    "every {} s",
                    every.as_secs()
                ))
        );
        for room in wanted {
            let members: Vec<u32> = (0..room.members)
                .map(|n| self.fakes.make((n == 0).then_some(room.name.as_str())))
                .collect();
            let spec = Spec {
                size: room.places / 2,
                max_level: 0,
                wait: super::rooms::QUICK_WAIT,
                pin: room.pin,
                min_players: 2,
            };
            let pool = self.settings.maps.clone();
            let rng = &mut self.rng;
            match self.rooms.seed(&members, spec, pool, &mut |bound| {
                rng.below(bound as u32) as usize
            }) {
                Some(key) => {
                    log::warn!(
                        "FAKE ROOM {key} \"{}\": {}/{}",
                        room.name,
                        room.members,
                        room.places
                    );
                    self.fakes.rooms.push(key);
                }
                None => log::error!("FAKE ROOM \"{}\" could not be made", room.name),
            }
        }
        self.fakes.next_leave = self
            .settings
            .rooms
            .fakes
            .leave_every
            .map(|every| now + every);
    }

    /// The fake member's name and gear, if it is one.
    pub(super) fn fake(&self, account_id: u32) -> Option<&Fake> {
        is_fake(account_id)
            .then(|| self.fakes.members.get(&account_id))
            .flatten()
    }

    /// `SRP_FAKE_ROOMS_LEAVE_SECONDS`: when due, one fake leaves the first
    /// full fake room with a queue (its first then moves in).
    pub(super) fn fake_rooms_tick(&mut self, now: Instant) {
        let (Some(at), Some(every)) =
            (self.fakes.next_leave, self.settings.rooms.fakes.leave_every)
        else {
            return;
        };
        if now < at {
            return;
        }
        self.fakes.next_leave = Some(now + every);
        let due = self.fakes.rooms.iter().copied().find(|&key| {
            self.rooms.queue_len(key) > 0
                && self.rooms.get(key).is_some_and(|room| {
                    room.is_full() && room.members.iter().any(|&member| is_fake(member))
                })
        });
        if let Some(key) = due {
            self.remove_fake(key);
        }
    }

    /// The last fake of room `key` leaves it, unless it is the room's last
    /// member (the room would be gone): its name.
    fn remove_fake(&mut self, key: u32) -> Option<String> {
        let room = self.rooms.get(key)?;
        if room.members.len() < 2 {
            return None;
        }
        let fake = *room.members.iter().rev().find(|&&member| is_fake(member))?;
        self.rooms.leave(fake);
        let name = self.name_of(fake).unwrap_or_default();
        log::warn!("FAKE ROOMS: {name} left room {key}");
        Some(name)
    }

    /// The local admin's action on the fake members (`tab=fake_room`):
    /// `add` one to a room, `remove` one from it (a queued player then
    /// moves in), or `show`; `room` is a room's name (its maker's) or key,
    /// empty for the first fake room. What the rooms are after it, as text.
    pub fn fake_rooms_action(&mut self, action: &str, room: &str, now: Instant) -> String {
        if self.settings.rooms.fakes.rooms.is_empty() {
            return "refused: fake rooms are off (SRP_FAKE_ROOMS is not set)".to_owned();
        }
        let action = action.trim();
        if !matches!(action, "show" | "" | "add" | "remove") {
            return format!("refused: unknown action `{action}` (add, remove, show)");
        }
        let room = room.trim();
        let key = if room.is_empty() {
            self.fakes
                .rooms
                .iter()
                .copied()
                .find(|&key| self.rooms.get(key).is_some())
        } else {
            self.rooms
                .all()
                .find(|each| {
                    each.key.to_string() == room
                        || self
                            .name_of(each.maker)
                            .is_some_and(|name| name.eq_ignore_ascii_case(room))
                })
                .map(|each| each.key)
        };
        let done = match (action, key) {
            ("show" | "", _) => String::new(),
            (_, None) => format!("no room \"{room}\"\n"),
            ("add", Some(key)) => {
                let fake = self.fakes.make(None);
                if self.rooms.admit(fake, key) {
                    log::warn!(
                        "FAKE ROOMS: {} joined room {key}",
                        self.name_of(fake).unwrap_or_default()
                    );
                    format!("added {}\n", self.name_of(fake).unwrap_or_default())
                } else {
                    self.fakes.members.remove(&fake);
                    format!("refused: room {key} is full\n")
                }
            }
            ("remove", Some(key)) => match self.remove_fake(key) {
                Some(name) => {
                    // Its queue's first moves in now, not at the next tick.
                    self.room_admit_queues(now);
                    format!("removed {name}\n")
                }
                None => format!("refused: room {key} has no fake member but its last\n"),
            },
            _ => String::new(),
        };
        // `show` alone: every room.
        let shown: Vec<u32> = match key {
            _ if matches!(action, "show" | "") && room.is_empty() => {
                self.rooms.all().map(|each| each.key).collect()
            }
            key => key.into_iter().collect(),
        };
        let mut text = done;
        for key in shown {
            text.push_str(&self.room_line(key, now));
            text.push('\n');
        }
        text.trim_end().to_owned()
    }

    /// One room as the admin's text shows it.
    fn room_line(&self, key: u32, now: Instant) -> String {
        let Some(room) = self.rooms.get(key) else {
            return format!("room {key}: gone");
        };
        let stage = match room.stage {
            Stage::Waiting => "waiting".to_owned(),
            Stage::Vote { until } => {
                format!(
                    "vote, {} s left",
                    until.saturating_duration_since(now).as_secs()
                )
            }
            Stage::InMatch { .. } => "in its match".to_owned(),
        };
        let unix = session::v026g::quests::now();
        let members: Vec<String> = room
            .members
            .iter()
            .map(|&member| {
                let gear = match self.fake(member) {
                    Some(fake) => fake.gear,
                    None => self.accounts.get(member).map_or(0, |account| {
                        matchmaking::gear_level(&account.locked(), unix)
                    }),
                };
                let name = self.name_of(member).unwrap_or_default();
                if is_fake(member) {
                    format!("{name} (gear {gear})")
                } else {
                    format!("{name} (gear {gear}, real)")
                }
            })
            .collect();
        let queue = self.rooms.queue_len(key);
        format!(
            "room {key} \"{}\": {}/{}{}, {stage}, queue {queue}: {}",
            self.name_of(room.maker).unwrap_or_default(),
            room.members.len(),
            room.capacity(),
            if room.is_full() { " full" } else { "" },
            members.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_rooms_read_as_documented() {
        assert_eq!(
            parse_rooms("Full 8:8/8, Half:3/8 ,Full 2:2/2:1234"),
            Ok(vec![
                FakeRoom {
                    name: "Full 8".into(),
                    members: 8,
                    places: 8,
                    pin: None
                },
                FakeRoom {
                    name: "Half".into(),
                    members: 3,
                    places: 8,
                    pin: None
                },
                FakeRoom {
                    name: "Full 2".into(),
                    members: 2,
                    places: 2,
                    pin: Some(1234)
                },
            ])
        );
        assert_eq!(parse_rooms(""), Ok(vec![]));
        for bad in [
            "A",
            "A:3",
            "A:9/8",
            "A:0/8",
            "A:3/7",
            "A:3/22",
            ":3/8",
            "A:3/8:12",
            "A:1/2:1234:x",
        ] {
            assert!(parse_rooms(bad).is_err(), "{bad}");
        }
    }
}
