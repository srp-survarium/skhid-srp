use crate::wire::Message;
struct Utterance {
    id: u16,
    seq: u16,
    frames: u16,
    pending: Option<Vec<u8>>,
}
pub struct Sender {
    session: u32,
    next_utt: u16,
    active: Option<Utterance>,
}
impl Sender {
    pub fn new(session: u32, first_utt: u16) -> Self {
        Self {
            session,
            next_utt: first_utt,
            active: None,
        }
    }
    pub fn start(&mut self, now: u32) -> Vec<u8> {
        let id = self.next_utt;
        self.next_utt = self.next_utt.wrapping_add(1);
        self.active = Some(Utterance {
            id,
            seq: 0,
            frames: 0,
            pending: None,
        });
        Message::Start {
            session: self.session,
            utt: id,
            ms: now,
        }
        .encode()
        .expect("fixed START fits")
    }
    pub fn frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        // A pair fits the relay's cap. Invalid input does not alter the sequence.
        if frame.is_empty() || frame.len() > 192 {
            return None;
        }
        let active = self.active.as_mut()?;
        match active.pending.take() {
            None => {
                active.pending = Some(frame.to_vec());
                None
            }
            Some(first) => self.audio(vec![first, frame.to_vec()]),
        }
    }
    fn audio(&mut self, frames: Vec<Vec<u8>>) -> Option<Vec<u8>> {
        let active = self.active.as_mut()?;
        let n = frames.len() as u16;
        let out = Message::Audio {
            session: self.session,
            utt: active.id,
            seq: active.seq,
            frames,
        }
        .encode()?;
        active.seq = active.seq.wrapping_add(1);
        active.frames = active.frames.wrapping_add(n);
        Some(out)
    }
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        let pending = self.active.as_mut()?.pending.take()?;
        self.audio(vec![pending])
    }
    pub fn end(&mut self) -> Option<Vec<u8>> {
        let active = self.active.take()?;
        Message::End {
            session: self.session,
            utt: active.id,
            frames: active.frames,
        }
        .encode()
    }
}
