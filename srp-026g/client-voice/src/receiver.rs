use crate::{
    Decoder, FRAME, Radio, SLOTS,
    radio::unit,
    sounds::{SQUELCH_IN, SQUELCH_OUT},
    wire::Message,
};
const RING: usize = 32;
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub decoded: u32,
    pub concealed: u32,
    pub late: u32,
    pub underruns: u32,
    pub closed: u32,
    pub timeouts: u32,
    pub squelch_in: u32,
    pub squelch_out: u32,
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
        let pcm = self.decoder.decode(packet).ok();
        if pcm.is_some() {
            if packet.is_some() {
                stats.decoded += 1;
            } else {
                stats.concealed += 1;
            }
        }
        u.next += 1;
        let ended = u.reception == Reception::Ended && u.next >= u.top;
        if ended {
            self.close(stats);
        }
        pcm
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
    last: Option<u8>,
}
impl Core {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            speakers: (0..SLOTS)
                .map(|_| Speaker::new())
                .collect::<Result<_, _>>()?,
            stats: Stats::default(),
            muted: 0,
            last: None,
        })
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    pub fn reset(&mut self) {
        for s in &mut self.speakers {
            s.state = State::Closed(None);
            s.decoder.reset();
            s.heard = None;
            s.effect = None;
            s.closing_effect = false;
        }
        self.last = None;
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
        s.heard = None;
        s.effect = None;
        s.closing_effect = false;
        if self.last == Some(slot) {
            self.last = None;
        }
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
    pub fn last_speaker(&self) -> Option<u8> {
        self.last
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
                    self.last = Some(slot);
                }
            }
            Message::RelayAudio { seq, frames, .. } => {
                if !s.take(id, slot, &mut self.stats) {
                    self.stats.late += frames.len() as u32;
                    return;
                }
                s.heard = Some(now);
                self.last = Some(slot);
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
    pub fn mix(&mut self, volume: f32, far_mask: u32, now: u32) -> [i16; FRAME] {
        let mut acc = [0f32; FRAME];
        let volume = unit(volume);
        for (slot, s) in self.speakers.iter_mut().enumerate() {
            let far = far_mask & (1 << slot) != 0;
            s.finish_effect(far, &mut self.stats);
            if let Some(mut pcm) = s.play(now, far, &mut self.stats) {
                if far {
                    s.radio.process(&mut pcm, 1.);
                }
                for (a, p) in acc.iter_mut().zip(pcm) {
                    *a += p * volume;
                }
            }
            s.finish_effect(far, &mut self.stats);
            if let Some(fx) = &mut s.effect {
                for sample in &mut acc {
                    if fx.at >= fx.samples.len() {
                        break;
                    }
                    *sample += f32::from(fx.samples[fx.at]) / 32768. * volume;
                    fx.at += 1;
                }
                if fx.at >= fx.samples.len() {
                    s.effect = None;
                }
            }
        }
        acc.map(|x| (x * 32767.).round().clamp(-32768., 32767.) as i16)
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
