//! Patch 63's view of the game (team_damage.rs is its logic): a hit as
//! hit_impl starts it and as its pain is taken, its dealer, the turned hit,
//! and the effects buffer of a world being saved or loaded.
//!
//! A hit's dealer is the hit's place (+44h) when it is another place than
//! the victim's, the hit has a source (+20h: a fall has none and is the
//! player's own place) and the place has a player in the victim's world
//! (core_active_player). 0.28a2 counted the same hits (0x5AA41B, 0x5AA62F).

use super::medical::{OwnedPlayer, Player};
use super::world::read;
use super::{Context, Thread};
use crate::raw;
use crate::vostok::types::Place;

/// Another player's hit on a victim, as the victim's world names its dealer.
pub struct Dealer<'t> {
    place: Place,
    ally: bool,
    player: OwnedPlayer<'t>,
}

impl Dealer<'_> {
    pub fn place(&self) -> Place {
        self.place
    }

    /// The dealer is on the victim's team (0 or 1, the match profile's): never
    /// in a free-for-all, where each player is his own side.
    pub fn ally(&self) -> bool {
        self.ally
    }
}

/// The dealer of `hit` on `victim`, if the hit counts.
///
/// # Safety
/// `victim` and `hit` are a hook's live hit_impl arguments on `thread`.
unsafe fn dealer_of<'t>(thread: &'t Thread, victim: raw::Player, hit: raw::HitInfo) -> Option<Dealer<'t>> {
    // SAFETY: the caller's live victim and hit.
    let (own, dealer, source) = unsafe { (read(victim.place()), read(hit.dealer()), read(hit.source())) };
    let place = Place::new(u32::from(dealer)).filter(|_| dealer != own && source != 0)?;
    let victim = Player::new(thread, victim);
    let player = victim.at_place(thread, place.get())?;
    Some(Dealer { place, ally: victim.ally(player.get()), player })
}

/// A hit at hit_impl's start (`player.hit_start`): the victim neither dead
/// nor dying (hit_impl returned before otherwise).
pub struct HitStart<'t> {
    victim: raw::Player,
    hit: raw::HitInfo,
    thread: &'t Thread,
}

impl<'t> HitStart<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<HitStart<'t>> {
        use raw::site::player_hit_start as site;
        // SAFETY: the context is this site's: hit_impl's victim and its frame,
        // whose second argument is the hit, on the thread running the hit.
        unsafe {
            let victim = site::victim(ctx)?;
            let hit = site::frame(ctx)?.hit()?;
            Some(HitStart { victim, hit, thread: ctx.thread() })
        }
    }

    pub fn dealer(&self) -> Option<Dealer<'t>> {
        // SAFETY: the site's live victim and hit.
        unsafe { dealer_of(self.thread, self.victim, self.hit) }
    }

    /// The hit dealt to its dealer instead, its damage times `factor`,
    /// through hit_impl itself (his own armour and protectors; the hit's
    /// place is then his own, so it is neither counted nor turned again,
    /// and a death by it is a suicide). The victim takes nothing: the hook
    /// leaves hit_impl by its way out of a hit it does not deal.
    pub fn turn(self, dealer: Dealer<'_>, factor: f32) {
        let mut copy = raw::HitInfoStorage::zeroed();
        let turned = copy.handle();
        // SAFETY: the site's live hit, 48h bytes, copied into our room,
        // which lives past the call; hit_impl (stdcall) with the dealer's
        // player, whose reference `dealer` holds until it drops.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.hit.address() as usize as *const u8,
                turned.address() as usize as *mut u8,
                raw::HitInfo::SIZE,
            );
            *turned.damage() *= factor;
            raw::engine_hit_impl(Some(dealer.player.get().get()), Some(turned));
        }
    }
}

/// The pain a hit took (`player.pain_lost`), and its dealer.
pub struct PainLost<'t> {
    pub dealer: Dealer<'t>,
    /// The victim's pain before the hit less after it: more than 0, a number.
    pub lost: f32,
}

impl<'t> PainLost<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<PainLost<'t>> {
        use raw::site::player_pain_lost as site;
        // SAFETY: the context is this site's: hit_impl's victim, its hit and
        // its pain pool, whose health is the pain after the hit; XMM0 the
        // pain before it.
        unsafe {
            let (victim, hit, pool) = (site::victim(ctx)?, site::hit(ctx)?, site::pool(ctx)?);
            let lost = ctx.xmm_f32(0) - read(pool.health());
            if !(lost.is_finite() && lost > 0.0) {
                return None;
            }
            Some(PainLost { dealer: dealer_of(ctx.thread(), victim, hit)?, lost })
        }
    }
}

/// The effects writer of a world being saved (`world_save.effects_written`).
pub struct EffectsWriter<'t> {
    writer: raw::BufferWriter,
    _context: &'t Context<'t>,
}

impl<'t> EffectsWriter<'t> {
    pub fn at(ctx: &'t Context<'t>) -> Option<EffectsWriter<'t>> {
        // SAFETY: game_world_core::serialize's live effects writer (EDI).
        let writer = unsafe { raw::site::world_save_effects_written::writer(ctx)? };
        Some(EffectsWriter { writer, _context: ctx })
    }

    /// `bytes` after the effects (the game's writer grows its buffer).
    pub fn append(&mut self, bytes: &[u8]) {
        super::carry_runtime::write_bytes(self.writer, bytes);
    }
}

/// The effects reader of a world being loaded (`world_load.effects_read`).
pub struct EffectsReader<'t> {
    reader: raw::BufferReader,
    _context: &'t Context<'t>,
}

impl<'t> EffectsReader<'t> {
    pub fn at(ctx: &'t Context<'t>) -> Option<EffectsReader<'t>> {
        // SAFETY: game_world_core::deserialize's frame: its effects reader,
        // inside the frame, alive for the hook.
        let reader = unsafe { raw::site::world_load_effects_read::frame(ctx)?.reader() };
        Some(EffectsReader { reader, _context: ctx })
    }

    /// What `parse` makes of the bytes after the effects; the reader moves
    /// past what it took only when it made something.
    pub fn decode<T>(&mut self, parse: impl FnOnce(&mut super::parse::Reader<'_>) -> Option<T>) -> Option<T> {
        // SAFETY: the game's reader {begin, cursor, size} over the history
        // item's effects buffer; its range is checked before a slice is made
        // or the cursor moves.
        unsafe {
            let begin = self.reader.begin() as usize;
            let cursor = self.reader.cursor();
            let end = begin.checked_add(read(self.reader.size()) as usize)?;
            let at = cursor as usize;
            if cursor.is_null() || at < begin || at > end {
                return None;
            }
            let mut source = super::parse::Reader::of_memory(cursor, end - at);
            let value = parse(&mut source)?;
            self.reader.set_cursor(cursor.add(end - at - source.remaining().len()));
            Some(value)
        }
    }
}
