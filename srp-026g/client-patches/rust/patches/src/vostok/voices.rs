//! The sound thread giving a scene's sounds the world's voices (patch 96,
//! docs/notes/sound_voices.md).
//!
//! The world has 64 sound voices for every scene (sound_world +68h). Each
//! tick, calculate_hdr_audio_frame (0x5C7A0) asks create_sound_voice
//! (0x59E70) for one for every audible propagator above the gain floor that
//! has none; with all 64 taken the answer is 0 and the sound waits, in
//! silence, for one to come free. [`VoiceRequest`] is that moment
//! (`sound_scene.voice_made`): the propagator that asked, the answer, the
//! scene's propagators and their loudness, and the one way to hand a voice
//! over ([`VoiceRequest::take_from`]).

use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::world::read;
use crate::raw;

/// What the HDR cull writes into a propagator whose voice it takes
/// (0x5CAD9): 2000 ms.
const CULL_HOLD_MS: u32 = 2000;
/// The longest master chain followed (amplitude 0x5C1C0 has no limit; a
/// longer chain is not the engine's).
const CHAIN: usize = 16;

/// How loud a sound is at the listener this tick: its gain by distance
/// times its sample's amplitude at its playing offset, both the frame's.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Loudness(pub f32);

/// One of the scene's propagators: a sound instance playing this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playing<'t> {
    propagator: raw::NewSoundPropagator,
    _tick: PhantomData<&'t ()>,
}

impl Playing<'_> {
    /// Whether it holds one of the world's voices.
    pub fn voiced(&self) -> bool {
        // SAFETY: a propagator of the scene's vector, alive for the tick on
        // the sound thread (only that thread frees one).
        unsafe { read(self.propagator.voice()) != 0 }
    }

    /// Whether the engine may take its voice: its sound's options'
    /// can_be_displaced (+11h), asked of its emitter's slot 2 as the HDR cull
    /// asks it (0x5CAC3..0x5CACD). raw_options can set it false: a sound that
    /// must never lose its voice (single_sound_cook's default is true).
    pub fn displaceable(&self) -> bool {
        // SAFETY: as voiced; the emitter is its proxy's sound's, alive while
        // the proxy plays, and its slot 2 only returns its options (0x38990).
        unsafe {
            let Some(emitter) = self.propagator.emitter() else { return false };
            raw::engine_sound_options(Some(emitter)).is_some_and(|options| read(options.can_be_displaced()) != 0)
        }
    }

    /// Its loudness this tick: the gain its chain's last propagator was
    /// given (as amplitude 0x5C1C0 reads it) times its own amplitude.
    pub fn loudness(&self) -> Loudness {
        // SAFETY: as voiced; the master chain is the scene's propagators.
        unsafe {
            let mut last = self.propagator;
            for _ in 0..CHAIN {
                match last.master() {
                    Some(next) => last = next,
                    None => break,
                }
            }
            Loudness(read(last.gain()) * read(self.propagator.level()))
        }
    }

    /// It, to hand its voice over ([`VoiceRequest::take_from`]).
    pub fn claim(self) -> Victim {
        Victim(self.propagator)
    }
}

/// A playing sound picked to give up its voice: only a [`Playing`] of this
/// request makes one, and only [`VoiceRequest::take_from`] takes it.
pub struct Victim(raw::NewSoundPropagator);

/// calculate_hdr_audio_frame just after create_sound_voice
/// (`sound_scene.voice_made`): a propagator asked the world for a voice.
pub struct VoiceRequest<'c, 'a> {
    ctx: &'c mut Context<'a>,
    asking: raw::NewSoundPropagator,
    list: raw::PropagatorList,
}

impl<'c, 'a> VoiceRequest<'c, 'a> {
    pub fn at(ctx: &'c mut Context<'a>) -> Option<VoiceRequest<'c, 'a>> {
        use raw::site::sound_scene_voice_made as site;
        // SAFETY: the context is this site's: ESI the propagator that asked
        // and EBP calculate_hdr_audio_frame's frame, whose second argument
        // is the scene's propagators, alive for the tick on the sound thread.
        let (asking, list) = unsafe { (site::propagator(ctx)?, site::frame(ctx)?.propagators()?) };
        Some(VoiceRequest { ctx, asking, list })
    }

    /// Whether the world gave the voice (EAX, the call's result).
    pub fn granted(&self) -> bool {
        self.ctx.reg(Reg::Eax) != 0
    }

    /// The propagator that asked.
    pub fn asking(&self) -> Playing<'_> {
        Playing { propagator: self.asking, _tick: PhantomData }
    }

    /// The scene's propagators this tick (the one that asked among them).
    pub fn playing(&self) -> impl Iterator<Item = Playing<'_>> + '_ {
        // SAFETY: the frame's vector: begin..end are its propagator
        // pointers, none 0 (notify_listener2 pushes each of a proxy's).
        let (begin, count) = unsafe {
            match (self.list.begin(), self.list.end()) {
                (Some(begin), Some(end)) => (Some(begin), end.index_from(begin).unwrap_or(0)),
                _ => (None, 0),
            }
        };
        (0..count).filter_map(move |i| {
            // SAFETY: i is below the vector's count.
            let propagator = unsafe { begin?.at(i).propagator()? };
            Some(Playing { propagator, _tick: PhantomData })
        })
    }

    /// Hands `victim`'s voice to the propagator that asked: the victim's
    /// voice is let go as the HDR cull lets one go (detach_voice, then its
    /// 2000 ms), and the world asked again, its answer the call's. Whether
    /// the asking propagator has a voice now.
    pub fn take_from(&mut self, victim: Victim) -> bool {
        let victim = Playing { propagator: victim.0, _tick: PhantomData };
        if self.granted() || !victim.voiced() || !victim.displaceable() || victim.propagator == self.asking {
            return self.granted();
        }
        // SAFETY: the sound thread's own calls on its own objects, as
        // calculate_hdr_audio_frame makes them (0x5CA96..0x5CAAB, 0x5CAD3):
        // the victim's proxy's scene and world are the asking one's (one
        // vector is one scene's); the voice detach_voice frees goes back to
        // the pool create_sound_voice takes from at once.
        unsafe {
            let Some(proxy) = self.asking.proxy() else { return false };
            let (Some(scene), Some(world)) = (proxy.scene(), proxy.user().and_then(|user| user.world())) else {
                return false;
            };
            raw::engine_detach_voice(Some(victim.propagator));
            *victim.propagator.hold() = CULL_HOLD_MS;
            let emitter = self.asking.emitter().map_or(core::ptr::null(), |e| e.address() as usize as *const c_void);
            let voice = raw::engine_create_sound_voice(Some(world), Some(scene), Some(self.asking), emitter);
            self.ctx.set_reg(Reg::Eax, voice as usize as u32);
        }
        self.granted()
    }
}
