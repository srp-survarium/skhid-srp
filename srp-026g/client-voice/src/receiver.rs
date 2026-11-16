use crate::{
    Decoder, FRAME, Radio, SLOTS,
    radio::unit,
    sounds::{SQUELCH_IN, SQUELCH_OUT},
    wire::Message,
};
pub use voice_abi::{Channel, Ears, Heard};
const RING: usize = 32;
/// A frame of the mix: each sample's left and right, as WinMM's 16-bit
/// stereo takes them.
pub type Stereo = [[i16; 2]; FRAME];
/// What a speaker's frame is mixed by: its 10 m voice in each ear (the
/// game's sound engine's gains for a sound at its place) or its radio, in
/// both ears alike, by the channel of its utterance.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Gains {
    plain: [f32; 2],
    radio: f32,
}
impl Gains {
    const SILENT: Self = Self { plain: [0., 0.], radio: 0. };
    /// An utterance on `channel` of a slot heard so.
    fn of(heard: Heard, channel: Channel) -> Self {
        match channel {
            Channel::Near => Self { plain: heard.near.get(), radio: 0. },
            Channel::Radio => Self { plain: [0., 0.], radio: f32::from(u8::from(heard.radio)) },
        }
    }
}
/// How every slot is heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hearing([Heard; SLOTS]);
impl Hearing {
    /// Every slot heard on both channels: its 10 m voice as decoded in both
    /// ears, its radio on.
    pub const UNITY: Self = Self([Heard { near: Ears::UNITY, radio: true }; SLOTS]);
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
/// The whole mix this much louder (+6 dB) before its limiter: every voice,
/// plain and through the radio, with the radio's static and squelch and the
/// microphone test's playback, so their balance stays as it is (the owner,
/// 2026-11-14, of revision 59 with "Voice chat" at 100: "it is quiet",
/// "especially compared to gunshots"). The mix's limiter ([`Master`]) holds
/// what would pass its ceiling.
const MIX: f32 = 2.0;
/// The mix's ceiling: no sample of it is played past this (-0.9 dB of full
/// scale).
const MIX_CEILING: f32 = 0.9;
/// How long the mix's gain stays where a peak brought it before it starts
/// back: a frame, 20 ms, longer than a voice's pitch period (12 ms at 80
/// Hz), so the next pulse of the same voice finds it down and is not cut.
const MIX_HOLD: usize = FRAME;
/// How fast the gain then comes back: 0.6 dB a frame, 6 dB in 200 ms.
const MIX_RELEASE: f32 = 1.071_5;
/// The mix's limiter: where a sample would pass [`MIX_CEILING`] the whole
/// mix, both ears alike, is turned down at once so that it does not; the
/// gain stays for [`MIX_HOLD`] and comes back at [`MIX_RELEASE`]. A loud
/// voice, or several at once, plays quieter for a moment and stays itself.
///
/// Until 2026-11-16 a curve bent every sample over half scale (tanh). Speech
/// through the automatic gain played 3 times as decoded (a plain voice:
/// [`LOUDER`] and [`MIX`]) was bent on 2 % of its samples at the default
/// "Microphone sensitivity" and on 8 % at the highest, where what the curve
/// added was 13 dB under the voice: such a talker was crushed for everyone
/// (the owner: "some people sound good, some are barely recognizable").
#[derive(Clone, Copy, Debug)]
pub struct Master {
    gain: f32,
    /// The samples left of [`MIX_HOLD`].
    hold: usize,
}
impl Master {
    pub const fn new() -> Self {
        Self { gain: 1., hold: 0 }
    }
    /// A frame of the mix from its sum: [`MIX`] louder, held under the
    /// ceiling, as 16-bit samples.
    pub fn finish(&mut self, acc: [[f32; 2]; FRAME]) -> Stereo {
        let release = MIX_RELEASE.powf(1. / FRAME as f32);
        acc.map(|ears| {
            let ears = ears.map(|x| x * MIX);
            let peak = ears[0].abs().max(ears[1].abs());
            if peak * self.gain * release > MIX_CEILING {
                // It would pass the ceiling were the gain let back: down to
                // it (or kept there), and held from here.
                self.gain = self.gain.min(MIX_CEILING / peak);
                self.hold = MIX_HOLD;
            } else if self.hold > 0 {
                self.hold -= 1;
            } else {
                self.gain = (self.gain * release).min(1.);
            }
            ears.map(|x| (x * self.gain * 32767.).round().clamp(-32768., 32767.) as i16)
        })
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub decoded: u32,
    pub concealed: u32,
    /// Frames that did not come, played from the next frame's copy of them.
    pub recovered: u32,
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
/// The frames of a speaker's voice the leveller goes by: its last 3 s of them.
const LEVEL_FRAMES: usize = 150;
/// A frame quieter than this (dB of full scale, its RMS) is no voice: it
/// is not counted.
const LEVEL_GATE_DB: f32 = -55.;
/// The frames counted before a speaker is levelled at all (100 ms): its
/// first frames are a word's onset, quieter than the word.
const LEVEL_LEARNT: usize = 5;
/// The level every speaker is brought to (dB of full scale: the mean level
/// of the loudest third of its frames): where the automatic gain puts a
/// microphone at the default "Microphone sensitivity", so such a talker
/// plays as decoded.
pub(crate) const LEVEL_TARGET_DB: f32 = -20.;
/// The most a speaker is raised and lowered to reach it, dB.
const LEVEL_MOST_DB: f32 = 20.;
const LEVEL_LEAST_DB: f32 = -12.;
/// Every speaker at one loudness for the listener (the owner, 2026-11-16:
/// "we should either way have a way to bring people with lower sensitivity
/// to sound the same for receivers", "equalize everyone essentially"). The
/// talker's own automatic gain does it only as far as its microphone and its
/// "Microphone sensitivity" let it; here the listener's game measures each
/// speaker's voice, the loudest third of its last [`LEVEL_FRAMES`] frames
/// above [`LEVEL_GATE_DB`] (its pauses and its room are the quiet ones), and
/// plays it [`LEVEL_TARGET_DB`] loud, before the 10 m voice's gains and the
/// radio alike. What it has learnt of a speaker stays from one utterance to
/// the next.
#[derive(Clone, Copy)]
struct Leveller {
    /// The counted frames' levels, dB: a ring of the last ones.
    frames: [f32; LEVEL_FRAMES],
    count: usize,
    next: usize,
    /// The gain the last frame ended at: the next ramps from it.
    gain: f32,
}
impl Leveller {
    const NEW: Self = Self { frames: [0.; LEVEL_FRAMES], count: 0, next: 0, gain: 1. };
    /// The speaker's level, once enough of its voice was heard.
    fn level(&self) -> Option<f32> {
        if self.count < LEVEL_LEARNT {
            return None;
        }
        let mut heard = self.frames;
        let heard = &mut heard[..self.count];
        heard.sort_unstable_by(|a, b| b.total_cmp(a));
        let loudest = &heard[..self.count.div_ceil(3)];
        Some(loudest.iter().sum::<f32>() / loudest.len() as f32)
    }
    fn apply(&mut self, pcm: &mut [f32; FRAME]) {
        let power = pcm.iter().map(|x| x * x).sum::<f32>() / FRAME as f32;
        let db = 10. * power.log10();
        // (A frame of no number, or of silence, is no voice either.)
        if db > LEVEL_GATE_DB {
            self.frames[self.next] = db;
            self.next = (self.next + 1) % LEVEL_FRAMES;
            self.count = (self.count + 1).min(LEVEL_FRAMES);
        }
        let to = self.level().map_or(1., |level| {
            10f32.powf((LEVEL_TARGET_DB - level).clamp(LEVEL_LEAST_DB, LEVEL_MOST_DB) / 20.)
        });
        let from = std::mem::replace(&mut self.gain, to);
        if from == 1. && to == 1. {
            return;
        }
        let step = (to - from) / FRAME as f32;
        for (i, sample) in pcm.iter_mut().enumerate() {
            *sample *= from + step * (i + 1) as f32;
        }
    }
}
/// A voice heard directly, as [`Speaker::play`] decodes and holds one. The
/// microphone test plays its recording through it (`mic_test`), so that what
/// the player hears of itself is what a teammate beside it would.
pub struct Direct {
    decoder: Decoder,
    leveller: Leveller,
    limiter: Limiter,
}
impl Direct {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            decoder: Decoder::new()?,
            leveller: Leveller::NEW,
            limiter: Limiter::UNITY,
        })
    }
    /// The frame's 20 ms; one the decoder refuses is concealed.
    pub fn frame(&mut self, frame: &[u8]) -> Option<[f32; FRAME]> {
        let mut pcm = self
            .decoder
            .decode(Some(frame))
            .or_else(|_| self.decoder.decode(None))
            .ok()?;
        self.leveller.apply(&mut pcm);
        self.limiter.apply(&mut pcm);
        Some(pcm)
    }
}
/// `mono` in both ears as a plain voice beside the listener is: as decoded
/// ([`Ears::UNITY`]).
fn ahead(mono: &[f32; FRAME]) -> [[f32; 2]; FRAME] {
    let [left, right] = Ears::UNITY.get();
    mono.map(|sample| [sample * left, sample * right])
}
impl Master {
    /// `mono` alone as a frame of the mix, centred (the microphone test's
    /// playback when no match's voices play: [`Core::mix_over`] without them).
    #[cfg(any(windows, test))]
    pub fn centred(&mut self, mono: &[f32; FRAME]) -> Stereo {
        self.finish(ahead(mono))
    }
}
/// [`Master::centred`] of a frame alone, the limiter at rest.
#[cfg(test)]
pub fn centred(mono: &[f32; FRAME]) -> Stereo {
    Master::new().centred(mono)
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
    /// A radio utterance that played has closed, its squelch not yet begun.
    closing_effect: bool,
    /// What its voice's level is, learnt over its utterances.
    leveller: Leveller,
    limiter: Limiter,
    /// The channel of its utterance (of its last one, when none is open).
    channel: Channel,
    /// The gains of the last frame mixed: a frame ramps from them to its
    /// own, so a turn or a step never clicks.
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
            leveller: Leveller::NEW,
            limiter: Limiter::UNITY,
            channel: Channel::Near,
            gains: Gains::SILENT,
        })
    }
    /// The utterance `id` on `channel` is this speaker's open one (now, or
    /// already: then on the channel it began on).
    fn take(&mut self, id: u16, slot: u8, channel: Channel, stats: &mut Stats) -> bool {
        match &self.state {
            State::Open(u) if u.id == id => return true,
            State::Open(u) if !newer(id, u.id) => return false,
            State::Closed(Some(last)) if !newer(id, *last) => return false,
            _ => {}
        }
        self.close(stats);
        self.decoder.reset();
        self.limiter = Limiter::UNITY;
        // Nothing of it plays by the last utterance's channel: it comes in
        // from silence.
        self.channel = channel;
        self.gains = Gains::SILENT;
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
            // By the channel of the utterance that closes: a new one's takes
            // its place only after (`take`).
            self.closing_effect = self.channel == Channel::Radio && !matches!(u.phase, Phase::Buffering);
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
    /// The squelch at the end of a radio utterance that closed, when the
    /// slot's radio is heard.
    fn finish_effect(&mut self, radio_heard: bool, stats: &mut Stats) {
        if self.closing_effect {
            self.closing_effect = false;
            if radio_heard {
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
        // The frame after it, when it has come: it carries a copy of this one.
        let after = u.ring[(u.next as usize + 1) % RING]
            .as_ref()
            .filter(|(index, _)| *index == u.next + 1)
            .map(|(_, bytes)| bytes.as_slice());
        let pcm = match (packet, after) {
            (Some(packet), _) => match self.decoder.decode(Some(packet)) {
                Ok(pcm) => {
                    stats.decoded = stats.decoded.saturating_add(1);
                    Some(pcm)
                }
                // A frame the decoder refuses is lost: concealed like one.
                Err(_) => {
                    stats.undecodable = stats.undecodable.saturating_add(1);
                    self.decoder.decode(None).ok()
                }
            },
            // A frame that did not come: from the next one's copy of it.
            (None, Some(after)) => {
                let pcm = self.decoder.recover(after).or_else(|_| self.decoder.decode(None)).ok();
                if pcm.is_some() {
                    stats.recovered = stats.recovered.saturating_add(1);
                }
                pcm
            }
            (None, None) => {
                let pcm = self.decoder.decode(None).ok();
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
            self.leveller.apply(&mut pcm);
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
    master: Master,
}
impl Core {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            speakers: (0..SLOTS)
                .map(|_| Speaker::new())
                .collect::<Result<_, _>>()?,
            stats: Stats::default(),
            muted: 0,
            master: Master::new(),
        })
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    pub fn reset(&mut self) {
        for s in &mut self.speakers {
            s.state = State::Closed(None);
            s.decoder.reset();
            s.leveller = Leveller::NEW;
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
        s.leveller = Leveller::NEW;
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
            Message::RelayStart { channel, .. } => {
                if s.take(id, slot, *channel, &mut self.stats) {
                    s.heard = Some(now);
                }
            }
            Message::RelayAudio { channel, seq, frames, .. } => {
                if !s.take(id, slot, *channel, &mut self.stats) {
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
        self.mix_over(None, volume, hearing, now)
    }
    /// The mix over `under`, 20 ms that play beside the speakers, centred and
    /// at a volume of their own (the microphone test's playback).
    pub fn mix_over(
        &mut self,
        under: Option<&[f32; FRAME]>,
        volume: f32,
        hearing: &Hearing,
        now: u32,
    ) -> Stereo {
        let mut acc = under.map_or([[0f32; 2]; FRAME], ahead);
        let volume = unit(volume);
        let plain = volume * LOUDER;
        for (s, heard) in self.speakers.iter_mut().zip(hearing.0) {
            let to = Gains::of(heard, s.channel);
            let from = std::mem::replace(&mut s.gains, to);
            let far = to.radio > 0.;
            s.finish_effect(heard.radio, &mut self.stats);
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
            s.finish_effect(heard.radio, &mut self.stats);
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
        self.master.finish(acc)
    }
    /// The slots heard talking now: each with an utterance (or one just
    /// over) on a channel `hearing` hears of it.
    pub fn talking(&self, hearing: &Hearing, now: u32) -> u32 {
        self.speakers.iter().zip(hearing.0).enumerate().fold(0, |mask, (slot, (s, heard))| {
            if heard.hears(s.channel)
                && (matches!(s.state, State::Open(_))
                    || s.heard.is_some_and(|last| !elapsed(now, last, 250)))
            {
                mask | 1 << slot
            } else {
                mask
            }
        })
    }
}
