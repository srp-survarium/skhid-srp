//! One owner for receipt sending (refusals are logged by `log::refusal`). A pool shortage
//! skips allocation; the server's missing-receipt timeout requests a fresh world.
use super::{Context, GameCell, Thread};
use super::world::read;
use crate::raw;
use srp_validate::recovery::{Outcome, Receipt, SIZE};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key { client: usize, seed: u32 }
#[derive(Clone, Copy)]
struct Millis(u32);
impl Millis {
    fn elapsed(self, earlier: Self) -> u32 { self.0.wrapping_sub(earlier.0) }
}
#[derive(Clone, Copy)]
struct Budget { key: Key, sent: Option<(Receipt, Millis)> }
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
    pub(crate) fn closing(ctx: &Context<'_>) { BUDGET.set(ctx.thread(), None); }
    fn now(&self) -> Millis {
        // SAFETY: GetTickCount takes no arguments and has no engine lifecycle.
        Millis(unsafe { raw::engine_tick_count() })
    }
    fn budget(&self) -> Budget {
        BUDGET.get(self.thread).filter(|budget| budget.key == self.key)
            .unwrap_or(Budget { key: self.key, sent: None })
    }
    pub(super) fn options_rejected(&self) {
        self.send(Receipt { match_id: self.key.seed, generation: 0, outcome: Outcome::OptionsRejected });
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
    fn send(&self, receipt: Receipt) {
        let now = self.now(); let mut budget = self.budget();
        if budget.sent.is_some_and(|(sent, at)| sent == receipt && now.elapsed(at) < 1000) { return; }
        // A different corrupt generation must not evade the rejection rate cap.
        if receipt.outcome != Outcome::Loaded && budget.sent.is_some_and(|(_, at)| now.elapsed(at) < 1000) { return; }
        // SAFETY: the connection and pool remain live during this synchronous
        // hook. Check headroom BEFORE begin, which asserts on an exhausted pool.
        unsafe {
            let Some(pool) = self.holder.packet_pool() else { return; };
            if read(pool.used()).checked_add(17).is_none_or(|used| used > read(pool.capacity())) { return; }
            let Some(packet) = raw::engine_match_packet_begin(Some(self.holder), 0xe0) else { return; };
            let flags = read(packet.flags()) & !0x40;
            packet.flags().write_volatile(flags);
            let bytes = receipt.encode();
            raw::engine_append_packet_bytes(Some(packet.writer()), bytes.as_ptr(), SIZE as u32);
            raw::engine_match_packet_send(Some(self.holder), Some(packet));
        }
        budget.sent = Some((receipt, now)); BUDGET.set(self.thread, Some(budget));
    }
}
