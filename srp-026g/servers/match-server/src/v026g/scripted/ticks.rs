//! The scripted client's side of the per-tick protocol: what the patched
//! client's handler does (`netcode_port_069.md` 5.1, 5.2), on the model's
//! lists instead of the game's.
//!
//! - its own inputs go out in one update a tick (0xE2), from the tick
//!   after its world on, empty or not; each update with inputs waits in
//!   the book (`sync_core::confirm::OwnInputs`) for its outcome;
//! - an update of the server (0xF7) is taken once a sync generation: its
//!   outcomes make the client's own list what the others have, its inputs
//!   go into the list of their place, its roster changes to the
//!   simulation.
//!
//! The lists are what the tests compare: every game must have the same
//! inputs at the same times for every place. What the real game would do
//! otherwise than this model is counted: an input for a time its history
//! has passed (the game re-times it on its own), an own move or discard
//! past its history (the game refuses it).

use std::collections::{BTreeMap, BTreeSet};

use survarium::v026g::match_msgs::{ClientUpdate, GameTime, Grid, Update, id};
use sync_core::confirm::{Action, OwnInputs, Resolved, Sent};
use sync_core::wire::{CLIENT_UPDATE_MAX, Input, Outcome};

/// The game's history: what a list of its can still take.
const HISTORY_MS: u32 = 500;

/// How far back a client sends its updates after its clock jumped: as far
/// as its history (0.69: "at most max delay back").
const SEND_BACK_TICKS: u32 = 10;

/// The per-tick state of a scripted client.
pub struct PerTick {
    pub grid: Grid,
    /// The last tick it sent its update of.
    last_sent: Option<GameTime>,
    /// Its own inputs made and not in an update yet: when.
    made: Vec<(GameTime, Input)>,
    made_profiles: Vec<(GameTime, u8)>,
    book: OwnInputs,
    /// The ticks of the updates taken, in the generation it holds.
    seen: (u8, BTreeSet<u32>),
    /// Its own inputs as its lists have them: by time.
    pub own: BTreeMap<u32, Input>,
    pub own_profiles: BTreeMap<u32, u8>,
    /// The others' inputs as its lists have them: by place and time.
    pub remote: BTreeMap<(u8, u32), Input>,
    pub remote_profiles: BTreeMap<(u8, u32), u8>,
    /// The outcomes taken.
    pub confirmed: u32,
    pub moved: u32,
    pub discarded: u32,
    /// Outcomes for no update it waits for.
    pub unknown_outcomes: u32,
    /// Updates given up waiting for, the book full.
    pub evicted: u32,
    /// Updates sent, and those of them with inputs.
    pub sent: u32,
    pub sent_with_inputs: u32,
    /// Updates of the server taken (copies not counted).
    pub updates: u32,
    /// How early its updates came, as the server said (ms).
    pub arrivals: Vec<i16>,
    /// The others' inputs that came for a time its history had passed: the
    /// game takes them at a time of its own choosing.
    pub inputs_past_history: u32,
    /// Its own moves and discards for a time its history had passed: the
    /// game refuses them.
    pub answers_past_history: u32,
    /// Each update of the server taken: its tick, and the client's clock
    /// then (for the depth of the rollbacks it makes).
    pub taken_at: Vec<(u32, u32)>,
}

impl PerTick {
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            last_sent: None,
            made: Vec::new(),
            made_profiles: Vec::new(),
            book: OwnInputs::new(),
            seen: (0, BTreeSet::new()),
            own: BTreeMap::new(),
            own_profiles: BTreeMap::new(),
            remote: BTreeMap::new(),
            remote_profiles: BTreeMap::new(),
            confirmed: 0,
            moved: 0,
            discarded: 0,
            unknown_outcomes: 0,
            evicted: 0,
            sent: 0,
            sent_with_inputs: 0,
            updates: 0,
            arrivals: Vec::new(),
            inputs_past_history: 0,
            answers_past_history: 0,
            taken_at: Vec::new(),
        }
    }

    /// An input of its own at `time`: in its lists from now (snapped to the
    /// grid unless `precise`, as the game does), and in its next update.
    pub fn make_input(&mut self, time: u32, precise: bool, actions: u32) -> GameTime {
        self.make(time, precise, [0x00, 0x3C, 0x00, 0xC0], actions)
    }

    /// As [`PerTick::make_input`], with its rotation.
    pub fn make(&mut self, time: u32, precise: bool, rotation: [u8; 4], actions: u32) -> GameTime {
        let time = if precise {
            GameTime(time)
        } else {
            self.grid.ceil(GameTime(time))
        };
        // Its update of that tick went: the game makes it in its frame
        // before its send pass, so it is of the next tick.
        let time = match self.last_sent {
            Some(last) if time <= last => GameTime(last.0 + self.grid.tick_ms()),
            _ => time,
        };
        let input = Input {
            back: 0,
            rotation,
            actions,
        };
        // A later input at the same time replaces it, as the game merges.
        self.made.retain(|(at, _)| *at != time);
        self.made.push((time, input));
        self.own.insert(time.0, input);
        time
    }

    /// A kit choice of its own at the tick of `time`.
    pub fn make_profile(&mut self, time: u32, value: u8) -> GameTime {
        let tick = self.grid.ceil(GameTime(time));
        self.made_profiles.retain(|(at, _)| *at != tick);
        self.made_profiles.push((tick, value));
        self.own_profiles.insert(tick.0, value);
        tick
    }

    /// The updates due by the clock `clock` (its world loaded): one a tick
    /// since the last, the payloads to send.
    pub fn due_updates(&mut self, clock: u32) -> Vec<Vec<u8>> {
        let tick_ms = self.grid.tick_ms();
        let now = self.grid.floor(GameTime(clock));
        let first = match self.last_sent {
            None => {
                // From the tick its world goes on from.
                self.last_sent = Some(now);
                return Vec::new();
            }
            Some(last) if last >= now => return Vec::new(),
            Some(last) => GameTime(last.0 + tick_ms),
        };
        // An empty one further back than its history says nothing of use.
        let oldest_empty = GameTime(now.0.saturating_sub(SEND_BACK_TICKS * tick_ms));
        let mut payloads = Vec::new();
        let mut tick = first;
        while tick <= now {
            let low = tick.0 - tick_ms;
            let empty = !self.made.iter().any(|(at, _)| at.0 > low && at.0 <= tick.0)
                && !self.made_profiles.iter().any(|(at, _)| *at == tick);
            if empty && tick < oldest_empty {
                tick = GameTime(tick.0 + tick_ms);
                continue;
            }
            let mut update = ClientUpdate::empty(tick);
            for (at, input) in &self.made {
                if at.0 > low && at.0 <= tick.0 {
                    let _ = update.push_input(Input {
                        back: (tick.0 - at.0) as u8,
                        ..*input
                    });
                }
            }
            for (at, value) in &self.made_profiles {
                if *at == tick {
                    let _ = update.push_profile(*value);
                }
            }
            self.made.retain(|(at, _)| at.0 > tick.0);
            self.made_profiles.retain(|(at, _)| at.0 > tick.0);
            if let Sent::Evicted { .. } = self.book.sent(&update) {
                self.evicted += 1;
            }
            self.sent += 1;
            if update.has_inputs() {
                self.sent_with_inputs += 1;
            }
            let mut bytes = vec![0; CLIENT_UPDATE_MAX];
            let len = update.write(&mut bytes).expect("an update fits");
            bytes.truncate(len);
            payloads.push(bytes);
            tick = GameTime(tick.0 + tick_ms);
        }
        self.last_sent = Some(now);
        payloads
    }

    /// The server's update: whether it is new to this generation (its
    /// game takes it), and then its own outcomes and the others' inputs
    /// into the lists. `resynced`: its last world's time; `clock`: its own.
    pub fn take(&mut self, update: &Update, generation: u8, resynced: u32, clock: u32) -> bool {
        if self.seen.0 != generation {
            self.seen = (generation, BTreeSet::new());
        }
        if !self.seen.1.insert(update.tick.0) {
            return false;
        }
        self.updates += 1;
        self.taken_at.push((update.tick.0, clock));
        if let Some(early) = update.arrival {
            self.arrivals.push(early);
        }
        let past = |time: GameTime| time.0 + HISTORY_MS <= clock;
        for own in &update.outcomes {
            let mut actions = Vec::new();
            let resolved = self
                .book
                .resolve(*own, GameTime(resynced), &mut |action| actions.push(action));
            if resolved == Resolved::Unknown {
                self.unknown_outcomes += 1;
                continue;
            }
            match own.outcome {
                Outcome::Confirmed => self.confirmed += 1,
                Outcome::Moved { .. } => self.moved += 1,
                Outcome::Discarded => self.discarded += 1,
            }
            for action in actions {
                match action {
                    Action::DiscardInput { time } => {
                        if past(time) {
                            self.answers_past_history += 1;
                        }
                        self.own.remove(&time.0);
                    }
                    Action::DiscardProfile { time } => {
                        self.own_profiles.remove(&time.0);
                    }
                    Action::MoveInput { from, to } => {
                        if past(from) {
                            self.answers_past_history += 1;
                        }
                        if let Some(input) = self.own.remove(&from.0) {
                            self.own.insert(to.0, input);
                        }
                    }
                    Action::MoveProfile { from, to } => {
                        if let Some(value) = self.own_profiles.remove(&from.0) {
                            self.own_profiles.insert(to.0, value);
                        }
                    }
                    Action::InsertInput { time, input } => {
                        self.own.insert(
                            time.0,
                            Input {
                                back: 0,
                                ..input
                            },
                        );
                    }
                    Action::InsertProfile { time, value } => {
                        self.own_profiles.insert(time.0, value);
                    }
                }
            }
        }
        for each in &update.inputs {
            let time = each.input.time(update.tick);
            // At or before its world: "older than last resync".
            if time.0 <= resynced {
                continue;
            }
            if past(time) {
                self.inputs_past_history += 1;
            }
            self.remote.insert(
                (each.place.get(), time.0),
                Input {
                    back: 0,
                    ..each.input
                },
            );
        }
        for each in &update.profiles {
            if update.tick.0 > resynced {
                self.remote_profiles
                    .insert((each.place.get(), update.tick.0), each.value);
            }
        }
        true
    }

    /// The inputs of `place` its lists have, after `from` and up to `to`:
    /// its own for its own place.
    pub fn timeline(&self, own_place: u8, place: u8, from: u32, to: u32) -> Vec<(u32, u32)> {
        if place == own_place {
            self.own
                .range(from + 1..=to)
                .map(|(at, input)| (*at, input.actions))
                .collect()
        } else {
            self.remote
                .range((place, from + 1)..=(place, to))
                .map(|((_, at), input)| (*at, input.actions))
                .collect()
        }
    }

    /// How many of its own updates wait for their outcome.
    pub fn pending(&self) -> usize {
        self.book.len()
    }
}

/// The id of the client's update.
pub const UPDATE_ID: u8 = id::CLIENT_UPDATE;
