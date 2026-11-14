use crate::{
    Decoder, FRAME, Radio, SLOTS,
    radio::unit,
    sounds::{SQUELCH_IN, SQUELCH_OUT},
    wire::Message,
};
pub use voice_abi::{Ears, Heard};
const RING: usize = 32;
/// A frame of the mix: each sample's left and right, as WinMM's 16-bit
/// stereo takes them.
pub type Stereo = [[i16; 2]; FRAME];
/// What a speaker's frame is mixed by: its plain voice in each ear (the
/// game's sound engine's gains for a sound at its place) and its radio, in
/// both ears alike.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Gains {
    plain: [f32; 2],
    radio: f32,
}
impl Gains {
    const UNITY: Self = Self { plain: [1., 1.], radio: 0. };
    fn of(heard: Heard) -> Self {
        match heard {
            Heard::Plain(ears) => Self { plain: ears.get(), radio: 0. },
            Heard::Radio => Self { plain: [0., 0.], radio: 1. },
            Heard::Both(ears) => Self { plain: ears.get(), radio: 1. },
        }
    }
}
/// How every slot is heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hearing([Heard; SLOTS]);
impl Hearing {
    /// Every slot plain, as decoded in both ears.
    pub const UNITY: Self = Self([Heard::Plain(Ears::UNITY); SLOTS]);
    /// The slots of `radio` through the radio; the rest plain, as decoded.
    pub fn radio(radio: u32) -> Self {
        Self(std::array::from_fn(|slot| {
            if radio & (1 << slot) != 0 {
                Heard::Radio
            } else {
                Heard::Plain(Ears::UNITY)
            }
        }))
    }
    /// A slot past the 20 changes nothing.
    pub fn set(&mut self, slot: usize, heard: Heard) {
        if let Some(place) = self.0.get_mut(slot) {
            *place = heard;
        }
    }
}
/// The radio's static under its words, of retail's recording: at retail's
/// level, as revision 58 played it (the owner, 2026-11-14, after a test where
/// half of it, #1635's, could not be heard: "static like live"; of revision
/// 58's on 2026-11-13: "static is mostly fine"). The radio's words are raised
/// by their own makeup (`radio::VOICE`), as on main before.
const RADIO_BED: f32 = 1.0;
/// How much louder than decoded a plain voice plays at the full "Voice chat"
/// volume (the owner, 2026-11-14, in a game: "voice chat in general can be
/// louder by x1.5 for people who can always tune it down"). Not the radio
/// (the owner, the same day: "make radio effect like it is currently on
/// main"): its words and squelch play at the slider's volume as before.
pub(crate) const LOUDER: f32 = 1.5;
/// The mix's knee: below it a sample is as summed; above it the rest of the
/// way to full scale is approached smoothly, so loud or several talkers
/// never clip hard.
const KNEE: f32 = 0.5;
fn soft_limit(x: f32) -> f32 {
    let over = x.abs() - KNEE;
    if over <= 0. {
        x
    } else {
        x.signum() * (KNEE + (1. - KNEE) * (over / (1. - KNEE)).tanh())
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub decoded: u32,
    pub concealed: u32,
    /// Frames the decoder refused (concealed in their place).
    pub undecodable: u32,
    pub late: u32,
    pub underruns: u32,
    pub closed: u32,
    pub timeouts: u32,
    pub squelch_in: u32,
    pub squelch_out: u32,
    /// Frames over [`CEILING`], held to it.
    pub limited: u32,
}
/// The loudest a speaker is played, as the RMS of a 20 ms frame (-12 dBFS).
/// A game's microphone goes through automatic gain (`codec::Preprocessor`)
/// and never reaches it held: decoded, its frames measured -13.6 dBFS at
/// most at the highest sensitivity and a full-scale voice, -19 dBFS at the
/// default. Only a stream that skipped the gain is held to it (a full-scale
/// square wave decodes at -0.3 dBFS), and the gain's first fifth of a
/// second after a sudden shout at the highest sensitivity (up to -8 dBFS).
const CEILING: f32 = 0.25;
/// After a frame over [`CEILING`], the gain comes back 0.5 dB a frame.
const RELEASE: f32 = 1.059_3;
/// A speaker's level held at [`CEILING`]: a frame over it is brought down at
/// once (the whole frame: none of it plays louder), then the gain comes back
/// at [`RELEASE`], ramped within each frame. Under it, untouched. A new
/// utterance (or the speaker's reset) starts it again at unity.
struct Limiter {
    gain: f32,
}
impl Limiter {
    const UNITY: Self = Self { gain: 1. };
    /// Whether the frame was over [`CEILING`].
    fn apply(&mut self, pcm: &mut [f32; FRAME]) -> bool {
        let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / FRAME as f32).sqrt();
        let over = rms > CEILING;
        let most = if over { CEILING / rms } else { 1. };
        let from = self.gain.min(most);
        let to = (self.gain * RELEASE).min(most);
        self.gain = to;
        if from >= 1. && to >= 1. {
            return over;
        }
        let step = (to - from) / FRAME as f32;
        for (i, sample) in pcm.iter_mut().enumerate() {
            *sample *= from + step * i as f32;
        }
        over
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Buffering,
    LeadIn(u8),
    Playing,
    Rebuffering,
}
#[derive(Clone, Copy, PartialEq)]
enum Reception {
    Receiving,
    Ended,
}
struct Utterance {
    id: u16,
    next: u32,
    top: u32,
    phase: Phase,
    reception: Reception,
    ring: [Option<(u32, Vec<u8>)>; RING],
}
enum State {
    Closed(Option<u16>),
    Open(Box<Utterance>),
}
struct Effect {
    samples: &'static [i16],
    at: usize,
}
struct Speaker {
    state: State,
    decoder: Decoder,
    radio: Radio,
    heard: Option<u32>,
    effect: Option<Effect>,
    closing_effect: bool,
    limiter: Limiter,
    /// The gains of the last frame mixed: a frame ramps from them to its
    /// own, so a turn, a step, or the switch to the radio never clicks.
    gains: Gains,
}
impl Speaker {
    fn new() -> Result<Self, String> {
        Ok(Self {
            state: State::Closed(None),
            decoder: Decoder::new()?,
            radio: Radio::new(0),
            heard: None,
            effect: None,
            closing_effect: false,
            limiter: Limiter::UNITY,
            gains: Gains::UNITY,
        })
    }
    fn take(&mut self, id: u16, slot: u8, stats: &mut Stats) -> bool {
        match &self.state {
            State::Open(u) if u.id == id => return true,
            State::Open(u) if !newer(id, u.id) => return false,
            State::Closed(Some(last)) if !newer(id, *last) => return false,
            _ => {}
        }
        self.close(stats);
        self.decoder.reset();
        self.limiter = Limiter::UNITY;
        self.radio = Radio::new(
            0x9e3779b9u32
                .wrapping_mul(u32::from(slot) + 1)
                .wrapping_add(u32::from(id)),
        );
        self.state = State::Open(Box::new(Utterance {
            id,
            next: 0,
            top: 0,
            phase: Phase::Buffering,
            reception: Reception::Receiving,
            ring: std::array::from_fn(|_| None),
        }));
        true
    }
    fn close(&mut self, stats: &mut Stats) {
        if let State::Open(u) = &self.state {
            self.closing_effect = !matches!(u.phase, Phase::Buffering);
            let last = u.id;
            self.state = State::Closed(Some(last));
            stats.closed += 1;
        }
    }
    fn frame(&mut self, index: u32, bytes: &[u8], stats: &mut Stats) {
        let State::Open(u) = &mut self.state else {
            return;
        };
        if u.top == 0 {
            u.next = index;
            u.top = index;
        } else if matches!(u.phase, Phase::Buffering | Phase::LeadIn(_))
            && index < u.next
            && u.top - index < RING as u32
        {
            u.next = index;
        }
        if index < u.next {
            stats.late += 1;
            return;
        }
        if index >= u.next + RING as u32 {
            u.next = index - RING as u32 + 1;
        }
        u.ring[index as usize % RING] = Some((index, bytes.to_vec()));
        u.top = u.top.max(index + 1);
    }
    fn finish_effect(&mut self, far: bool, stats: &mut Stats) {
        if self.closing_effect {
            self.closing_effect = false;
            if far {
                self.effect = Some(Effect {
                    samples: SQUELCH_OUT,
                    at: 0,
                });
                stats.squelch_out += 1;
            }
        }
    }
    fn play(&mut self, now: u32, far: bool, stats: &mut Stats) -> Option<[f32; FRAME]> {
        let State::Open(u) = &mut self.state else {
            return None;
        };
        if matches!(u.phase, Phase::Buffering | Phase::Rebuffering) {
            if u.top - u.next >= 3 || (u.reception == Reception::Ended && u.top > u.next) {
                if matches!(u.phase, Phase::Buffering) && far {
                    self.effect = Some(Effect {
                        samples: SQUELCH_IN,
                        at: 0,
                    });
                    stats.squelch_in += 1;
                    u.phase = Phase::LeadIn(8);
                } else {
                    u.phase = Phase::Playing;
                }
            } else {
                if u.reception == Reception::Ended
                    || self.heard.is_some_and(|last| elapsed(now, last, 1000))
                {
                    if u.reception != Reception::Ended {
                        stats.timeouts += 1;
                    }
                    self.close(stats);
                }
                return None;
            }
        }
        if let Phase::LeadIn(left) = u.phase {
            u.phase = if left <= 1 {
                Phase::Playing
            } else {
                Phase::LeadIn(left - 1)
            };
            return None;
        }
        if u.next >= u.top {
            if u.reception == Reception::Ended
                || self.heard.is_some_and(|last| elapsed(now, last, 1000))
            {
                if u.reception != Reception::Ended {
                    stats.timeouts += 1;
                }
                self.close(stats);
            } else {
                stats.underruns += 1;
                u.phase = Phase::Rebuffering;
            }
            return None;
        }
        let cell = u.ring[u.next as usize % RING].take();
        let packet = cell
            .as_ref()
            .filter(|(index, _)| *index == u.next)
            .map(|(_, bytes)| bytes.as_slice());
        let pcm = match (packet, self.decoder.decode(packet)) {
            (Some(_), Ok(pcm)) => {
                stats.decoded = stats.decoded.saturating_add(1);
                Some(pcm)
            }
            // A frame the decoder refuses is lost: concealed like one.
            (Some(_), Err(_)) => {
                stats.undecodable = stats.undecodable.saturating_add(1);
                self.decoder.decode(None).ok()
            }
            (None, pcm) => {
                let pcm = pcm.ok();
                if pcm.is_some() {
                    stats.concealed = stats.concealed.saturating_add(1);
                }
                pcm
            }
        };
        u.next += 1;
        let ended = u.reception == Reception::Ended && u.next >= u.top;
        if ended {
            self.close(stats);
        }
        pcm.map(|mut pcm| {
            if self.limiter.apply(&mut pcm) {
                stats.limited = stats.limited.saturating_add(1);
            }
            pcm
        })
    }
}
fn newer(a: u16, b: u16) -> bool {
    (a.wrapping_sub(b) as i16) > 0
}
fn elapsed(now: u32, then: u32, ms: u32) -> bool {
    now.wrapping_sub(then) as i32 >= ms as i32
}
pub struct Core {
    speakers: Vec<Speaker>,
    stats: Stats,
    muted: u32,
}
impl Core {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            speakers: (0..SLOTS)
                .map(|_| Speaker::new())
                .collect::<Result<_, _>>()?,
            stats: Stats::default(),
            muted: 0,
        })
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    pub fn reset(&mut self) {
        for s in &mut self.speakers {
            s.state = State::Closed(None);
            s.decoder.reset();
            s.limiter = Limiter::UNITY;
            s.heard = None;
            s.effect = None;
            s.closing_effect = false;
        }
    }
    /// A roster identity changed: its new occupant has an independent
    /// utterance sequence and must never inherit the old occupant's audio.
    pub fn reset_slot(&mut self, slot: u8) {
        let Some(s) = self.speakers.get_mut(usize::from(slot)) else {
            return;
        };
        s.close(&mut self.stats);
        s.state = State::Closed(None);
        s.decoder.reset();
        s.limiter = Limiter::UNITY;
        s.heard = None;
        s.effect = None;
        s.closing_effect = false;
    }
    pub fn mute(&mut self, mask: u32) {
        self.muted = mask;
        for (slot, s) in self.speakers.iter_mut().enumerate() {
            if mask & (1 << slot) != 0 {
                s.close(&mut self.stats);
                s.heard = None;
                s.effect = None;
                s.closing_effect = false;
            }
        }
    }
    pub fn receive(&mut self, message: &Message, now: u32) {
        let (slot, id) = match message {
            Message::RelayStart { slot, utt, .. }
            | Message::RelayAudio { slot, utt, .. }
            | Message::RelayEnd { slot, utt, .. } => (*slot, *utt),
            _ => return,
        };
        if usize::from(slot) >= SLOTS || self.muted & (1 << slot) != 0 {
            return;
        }
        let s = &mut self.speakers[usize::from(slot)];
        match message {
            Message::RelayStart { .. } => {
                if s.take(id, slot, &mut self.stats) {
                    s.heard = Some(now);
                }
            }
            Message::RelayAudio { seq, frames, .. } => {
                if !s.take(id, slot, &mut self.stats) {
                    self.stats.late = self.stats.late.saturating_add(frames.len() as u32);
                    return;
                }
                s.heard = Some(now);
                for (k, frame) in frames.iter().take(2).enumerate() {
                    s.frame(u32::from(*seq) * 2 + k as u32, frame, &mut self.stats);
                }
            }
            Message::RelayEnd { .. } => {
                if let State::Open(u) = &mut s.state
                    && u.id == id
                {
                    u.reception = Reception::Ended;
                    if u.next >= u.top {
                        s.close(&mut self.stats);
                    }
                }
            }
            _ => {}
        }
    }
    pub fn mix(&mut self, volume: f32, hearing: &Hearing, now: u32) -> Stereo {
        let mut acc = [[0f32; 2]; FRAME];
        let volume = unit(volume);
        let plain = volume * LOUDER;
        for (s, heard) in self.speakers.iter_mut().zip(hearing.0) {
            let to = Gains::of(heard);
            let from = std::mem::replace(&mut s.gains, to);
            let far = to.radio > 0.;
            s.finish_effect(far, &mut self.stats);
            if let Some(pcm) = s.play(now, far, &mut self.stats) {
                let ramp = |from: f32, to: f32, i: usize| from + (to - from) * (i + 1) as f32 / FRAME as f32;
                if from.plain != [0., 0.] || to.plain != [0., 0.] {
                    for (i, (a, p)) in acc.iter_mut().zip(pcm).enumerate() {
                        for ((ear, from), to) in a.iter_mut().zip(from.plain).zip(to.plain) {
                            *ear += p * plain * ramp(from, to, i);
                        }
                    }
                }
                // The radio's filter runs only while it is heard (as it
                // did): the frame it comes in on ramps it up from nothing.
                if from.radio > 0. || to.radio > 0. {
                    let mut radio = pcm;
                    s.radio.process(&mut radio, RADIO_BED);
                    for (i, (a, p)) in acc.iter_mut().zip(radio).enumerate() {
                        let p = p * volume * ramp(from.radio, to.radio, i);
                        a[0] += p;
                        a[1] += p;
                    }
                }
            }
            s.finish_effect(far, &mut self.stats);
            // The squelch is the radio's: centred.
            if let Some(fx) = &mut s.effect {
                for sample in &mut acc {
                    if fx.at >= fx.samples.len() {
                        break;
                    }
                    let fx_sample = f32::from(fx.samples[fx.at]) / 32768. * volume;
                    sample[0] += fx_sample;
                    sample[1] += fx_sample;
                    fx.at += 1;
                }
                if fx.at >= fx.samples.len() {
                    s.effect = None;
                }
            }
        }
        acc.map(|ears| ears.map(|x| (soft_limit(x) * 32767.).round().clamp(-32768., 32767.) as i16))
    }
    pub fn talking(&self, now: u32) -> u32 {
        self.speakers.iter().enumerate().fold(0, |mask, (slot, s)| {
            if matches!(s.state, State::Open(_))
                || s.heard.is_some_and(|last| !elapsed(now, last, 250))
            {
                mask | 1 << slot
            } else {
                mask
            }
        })
    }
}
