//! One owner for receipt sending (refusals are logged by `log::refusal`).
//!
//! One receipt per event, never one dropped for time: a world's load or
//! refusal is told once per generation (the server offers each generation
//! once and answers a refusal with the next one), a refused handshake
//! message (0xE3/0xE4) once per message (the server sends one again only in
//! answer to a refusal). A refusal the server never hears leaves its wait
//! for it without an end, so none is dropped:
//! - 0xE0 is plain and resent until acked (transport.md section 3.3), so
//!   the server gets copies of one refusal. A handshake refusal carries
//!   its number on this connection (1, 2, ...) in the generation byte; the
//!   server answers each number once. A world's carries its generation.
//! - A receipt the packet pool has no room for waits, the newest only, and
//!   goes on the next frame (`game.tick`) that has room. It goes with its
//!   connection: a closed one tells the server by its silence.
use super::{Context, GameCell, Thread};
use super::world::read;
use crate::raw;
use srp_validate::recovery::{Outcome, Receipt, SIZE};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key { client: usize, seed: u32 }
/// What this connection has told and has still to tell.
#[derive(Clone, Copy)]
struct Budget {
    key: Key,
    /// The last world outcome told.
    told: Option<Receipt>,
    /// Handshake messages refused so far.
    refused_handshakes: u8,
    /// The newest receipt the pool had no room for.
    waiting: Option<Receipt>,
}
static BUDGET: GameCell<Option<Budget>> = GameCell::new(None);

pub struct Connection<'a> {
    client: raw::NetworkClient,
    holder: raw::MatchOptionsHolder,
    key: Key,
    thread: &'a Thread,
}
impl<'a> Connection<'a> {
    pub(super) fn of(client: raw::NetworkClient, thread: &'a Thread) -> Option<Self> {
        // SAFETY: the synchronous dispatch/load boundary owns this client.
        unsafe {
            if read(client.match_connection_state()) > 2 { return None; }
            let holder = client.options_holder()?;
            let key = Key { client: client.address() as usize, seed: read(holder.match_seed()) };
            Some(Self { client, holder, key, thread })
        }
    }
    pub(crate) fn at_loaded(ctx: &'a Context<'_>) -> Option<Self> {
        // SAFETY: generated site owns EDI until this hook returns.
        let client = unsafe { raw::site::world_load_loaded::client(ctx)? };
        Self::of(client, ctx.thread())
    }
    /// The game's connection, at `game.tick` (EBX the game).
    pub(crate) fn at_game_tick(ctx: &'a Context<'_>) -> Option<Self> {
        // SAFETY: the site's EBX is the game, alive and ticking on this
        // thread; its network client (+53ACh) is the game's own.
        let client = unsafe { raw::site::game_tick::game(ctx)?.network_client()? };
        Self::of(client, ctx.thread())
    }
    pub(crate) fn closing(ctx: &Context<'_>) { BUDGET.set(ctx.thread(), None); }
    fn budget(&self) -> Budget {
        BUDGET.get(self.thread).filter(|budget| budget.key == self.key)
            .unwrap_or(Budget { key: self.key, told: None, refused_handshakes: 0, waiting: None })
    }
    pub(super) fn options_rejected(&self) {
        let mut budget = self.budget();
        budget.refused_handshakes = budget.refused_handshakes.wrapping_add(1);
        BUDGET.set(self.thread, Some(budget));
        let number = budget.refused_handshakes;
        self.send(Receipt { match_id: self.key.seed, generation: number, outcome: Outcome::OptionsRejected });
    }
    pub(super) fn rejected(&self, generation: Option<u8>) {
        // Envelope checks run before the native generation write. Semantic
        // checks run after it. The server accepts only its offered generation.
        let generation = generation.unwrap_or_else(|| unsafe { read(self.client.world_generation()) });
        self.send(Receipt { match_id: self.key.seed, generation, outcome: Outcome::Rejected });
    }
    pub(crate) fn loaded(&self) {
        // SAFETY: native deserialize returned; its envelope generation is current.
        let generation = unsafe { read(self.client.world_generation()) };
        self.send(Receipt { match_id: self.key.seed, generation, outcome: Outcome::Loaded });
    }
    /// The receipt that waited for room in the pool, if there is room now.
    pub(crate) fn send_waiting(&self) {
        let mut budget = self.budget();
        if let Some(receipt) = budget.waiting.take() {
            BUDGET.set(self.thread, Some(budget));
            self.send(receipt);
        }
    }
    fn send(&self, receipt: Receipt) {
        let mut budget = self.budget();
        // The same world's outcome again (a flood of one corrupt world): told
        // already. Each refused handshake message has its own number.
        if receipt.outcome != Outcome::OptionsRejected && budget.told == Some(receipt) { return; }
        // SAFETY: the connection and pool remain live during this synchronous
        // hook. Check headroom BEFORE begin, which asserts on an exhausted pool.
        let sent = unsafe {
            let room = self.holder.packet_pool()
                .is_some_and(|pool| read(pool.used()).checked_add(17).is_some_and(|used| used <= read(pool.capacity())));
            match room.then(|| raw::engine_match_packet_begin(Some(self.holder), 0xe0)).flatten() {
                Some(packet) => {
                    let flags = read(packet.flags()) & !0x40;
                    packet.flags().write_volatile(flags);
                    let bytes = receipt.encode();
                    raw::engine_append_packet_bytes(Some(packet.writer()), bytes.as_ptr(), SIZE as u32);
                    raw::engine_match_packet_send(Some(self.holder), Some(packet));
                    true
                }
                None => false,
            }
        };
        if sent {
            // A newer receipt makes an older one that waited moot: the
            // server waits for the answer to its newest offer only.
            budget.waiting = None;
            if receipt.outcome != Outcome::OptionsRejected { budget.told = Some(receipt); }
        } else {
            budget.waiting = Some(receipt);
        }
        BUDGET.set(self.thread, Some(budget));
    }
}
