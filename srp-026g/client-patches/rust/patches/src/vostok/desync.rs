//! The sync log's reads (patch 210, `sync_log`; SKHID_SYNC_LOG): what an input's apply, an
//! animation tick and a world load are given, read once at their sites into plain values. Nothing
//! here writes the game's memory.
use super::world::read;
use super::Context;
use crate::raw;

/// A return address in the exe: where a call came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Return(pub u32);

/// Who called apply_new_input: player::set_input (its only call in the exe), with where
/// set_input itself returns to; or a call from elsewhere (a patch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCaller {
    SetInput(Return),
    Other(Return),
}

/// Who called tick_animations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickCaller {
    /// base_player::tick: a step of the world.
    Step,
    /// apply_new_input: an input applied at its own time.
    Input,
    Other(Return),
}

impl TickCaller {
    pub fn parse(at: u32) -> TickCaller {
        match at {
            raw::image::STEP_TICKED => TickCaller::Step,
            raw::image::INPUT_TICKED => TickCaller::Input,
            other => TickCaller::Other(Return(other)),
        }
    }
}

/// Who loaded a world (game_world_core::deserialize's caller).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    /// game_world_core::move for an input of a past tick (0x2D2FF0).
    Rollback,
    /// game_world_core::move: the newest state again before a frame's step (0x2D357A).
    Frame,
    /// The server's world (0xE8, 0x2D0B70's call 0x2D0FB9): a join or a resync.
    Server,
    /// load_history_state (0x325270; 0x325291, 0x325300): on_hash_mismatch's debug compare and
    /// dump_game_state only.
    History,
    Other(Return),
}

impl LoadKind {
    pub fn parse(at: u32) -> LoadKind {
        match at {
            raw::image::ROLLBACK_LOADED => LoadKind::Rollback,
            raw::image::FRAME_LOADED => LoadKind::Frame,
            raw::image::SERVER_WORLD_LOADED => LoadKind::Server,
            raw::image::HISTORY_LOADED | raw::image::HISTORY_LOADED_AGAIN => LoadKind::History,
            other => LoadKind::Other(Return(other)),
        }
    }
}

/// The world core's newest simulation time played once (`maximum_time`): a time at or before it
/// is played again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Newest(pub u32);

/// apply_new_input's entry (`carry.input`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    pub place: u8,
    /// The input's time.
    pub time: u32,
    /// The player's last tick time (+324h).
    pub previous: u32,
    pub caller: InputCaller,
    pub newest: Option<Newest>,
}

impl Input {
    pub fn at(ctx: &Context<'_>) -> Option<Input> {
        use raw::site::carry_input as site;
        // SAFETY: apply_new_input's entry: ECX the live player, the stack its return address, its
        // two arguments and (when player::set_input called it) set_input's saved ESI and return;
        // the player's core lives as long as the player. Only reads.
        unsafe {
            let (player, frame) = (site::player(ctx)?, site::frame(ctx)?);
            let caller = match read(frame.return_address()) {
                raw::image::SET_INPUT_APPLIED => InputCaller::SetInput(Return(read(frame.set_input_return()))),
                other => InputCaller::Other(Return(other)),
            };
            Some(Input {
                place: read(player.place()),
                time: read(frame.time()),
                previous: read(player.time()),
                caller,
                newest: player.world_core().map(|core| Newest(read(core.maximum_time()))),
            })
        }
    }
}

/// base_player::tick_animations' entry (`sync.animations`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    pub place: u8,
    pub time: u32,
    pub previous: u32,
    pub caller: TickCaller,
    pub newest: Option<Newest>,
}

impl Tick {
    pub fn at(ctx: &Context<'_>) -> Option<Tick> {
        use raw::site::sync_animations as site;
        // SAFETY: tick_animations' entry: EAX the live player, the stack its return address and
        // two arguments; the player's core lives as long as the player. Only reads.
        unsafe {
            let (player, frame) = (site::player(ctx)?, site::frame(ctx)?);
            Some(Tick {
                place: read(player.place()),
                time: read(frame.now()),
                previous: read(frame.previous()),
                caller: TickCaller::parse(read(frame.return_address())),
                newest: player.world_core().map(|core| Newest(read(core.maximum_time()))),
            })
        }
    }
}

/// game_world_core::deserialize's entry (`sync.world_load`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Load {
    pub kind: LoadKind,
    /// The loaded state's time.
    pub time: u32,
}

impl Load {
    pub fn at(ctx: &Context<'_>) -> Option<Load> {
        use raw::site::sync_world_load as site;
        // SAFETY: deserialize's entry: the stack its return address and the history item it is
        // about to load, alive for the call. Only reads.
        unsafe {
            let frame = site::frame(ctx)?;
            let item = frame.item()?;
            Some(Load { kind: LoadKind::parse(read(frame.return_address())), time: read(item.time()) })
        }
    }
}
