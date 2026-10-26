//! Bounded version-one UDP protocol. Raw bytes become one typed message here.
use crate::SLOTS;
pub const MAX_DATAGRAM: usize = 400;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Hello {
        session: u32,
        hears: bool,
        muted: u32,
    },
    Start {
        session: u32,
        utt: u16,
        ms: u32,
    },
    Audio {
        session: u32,
        utt: u16,
        seq: u16,
        frames: Vec<Vec<u8>>,
    },
    End {
        session: u32,
        utt: u16,
        frames: u16,
    },
    Welcome {
        slot: u8,
    },
    Roster {
        entries: Vec<(u8, u32)>,
    },
    RelayStart {
        slot: u8,
        utt: u16,
        ms: u32,
    },
    RelayAudio {
        slot: u8,
        utt: u16,
        seq: u16,
        frames: Vec<Vec<u8>>,
    },
    RelayEnd {
        slot: u8,
        utt: u16,
        reason: u8,
    },
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let out = self.0.get(..N)?.try_into().ok()?;
        self.0 = &self.0[N..];
        Some(out)
    }
    fn byte(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn slot(&mut self) -> Option<u8> {
        let slot = self.byte()?;
        (usize::from(slot) < SLOTS).then_some(slot)
    }
    fn frames(&mut self) -> Option<Vec<Vec<u8>>> {
        let count = self.byte()?;
        if !(1..=8).contains(&count) {
            return None;
        }
        let mut frames = Vec::with_capacity(count.into());
        for _ in 0..count {
            let len = usize::from(self.byte()?);
            if len == 0 {
                return None;
            }
            frames.push(self.0.get(..len)?.to_vec());
            self.0 = &self.0[len..];
        }
        self.0.is_empty().then_some(frames)
    }
}
pub fn parse(bytes: &[u8]) -> Option<Message> {
    if bytes.len() > MAX_DATAGRAM || bytes.get(..2) != Some(&[0x56, 1]) {
        return None;
    }
    let kind = *bytes.get(2)?;
    let mut r = Reader(&bytes[3..]);
    Some(match kind {
        1 => {
            let session = r.u32()?;
            let hears = r.byte()? & 1 != 0;
            let muted = if r.0.is_empty() { 0 } else { r.u32()? };
            Message::Hello {
                session,
                hears,
                muted,
            }
        }
        2 => Message::Start {
            session: r.u32()?,
            utt: r.u16()?,
            ms: r.u32()?,
        },
        3 => Message::Audio {
            session: r.u32()?,
            utt: r.u16()?,
            seq: r.u16()?,
            frames: r.frames()?,
        },
        4 => Message::End {
            session: r.u32()?,
            utt: r.u16()?,
            frames: r.u16()?,
        },
        0x81 => Message::Welcome { slot: r.slot()? },
        0x82 => Message::RelayStart {
            slot: r.slot()?,
            utt: r.u16()?,
            ms: r.u32()?,
        },
        0x83 => Message::RelayAudio {
            slot: r.slot()?,
            utt: r.u16()?,
            seq: r.u16()?,
            frames: r.frames()?,
        },
        0x84 => Message::RelayEnd {
            slot: r.slot()?,
            utt: r.u16()?,
            reason: r.byte()?,
        },
        0x90 => {
            let count = usize::from(r.byte()?);
            if count > SLOTS {
                return None;
            }
            let mut entries = Vec::with_capacity(count);
            let mut seen = 0u32;
            for _ in 0..count {
                let slot = r.slot()?;
                if seen & (1 << slot) != 0 {
                    return None;
                }
                seen |= 1 << slot;
                entries.push((slot, r.u32()?));
            }
            Message::Roster { entries }
        }
        _ => return None,
    })
}
impl Message {
    pub fn encode(&self) -> Option<Vec<u8>> {
        let mut b = vec![0x56, 1];
        match self {
            Self::Hello {
                session,
                hears,
                muted,
            } => {
                b.push(1);
                b.extend(session.to_le_bytes());
                b.push(u8::from(*hears));
                b.extend(muted.to_le_bytes());
            }
            Self::Start { session, utt, ms } => {
                b.push(2);
                b.extend(session.to_le_bytes());
                b.extend(utt.to_le_bytes());
                b.extend(ms.to_le_bytes());
            }
            Self::End {
                session,
                utt,
                frames,
            } => {
                b.push(4);
                b.extend(session.to_le_bytes());
                b.extend(utt.to_le_bytes());
                b.extend(frames.to_le_bytes());
            }
            Self::Welcome { slot } => {
                if usize::from(*slot) >= SLOTS {
                    return None;
                }
                b.extend([0x81, *slot]);
            }
            Self::Roster { entries } => {
                if entries.len() > SLOTS {
                    return None;
                }
                b.extend([0x90, entries.len() as u8]);
                for (slot, id) in entries {
                    if usize::from(*slot) >= SLOTS {
                        return None;
                    }
                    b.push(*slot);
                    b.extend(id.to_le_bytes());
                }
            }
            Self::RelayStart { slot, utt, ms } => {
                if usize::from(*slot) >= SLOTS {
                    return None;
                }
                b.extend([0x82, *slot]);
                b.extend(utt.to_le_bytes());
                b.extend(ms.to_le_bytes());
            }
            Self::RelayEnd { slot, utt, reason } => {
                if usize::from(*slot) >= SLOTS {
                    return None;
                }
                b.extend([0x84, *slot]);
                b.extend(utt.to_le_bytes());
                b.push(*reason);
            }
            Self::Audio {
                session,
                utt,
                seq,
                frames,
            } => {
                b.push(3);
                b.extend(session.to_le_bytes());
                audio(&mut b, *utt, *seq, frames)?;
            }
            Self::RelayAudio {
                slot,
                utt,
                seq,
                frames,
            } => {
                if usize::from(*slot) >= SLOTS {
                    return None;
                }
                b.extend([0x83, *slot]);
                audio(&mut b, *utt, *seq, frames)?;
            }
        }
        (b.len() <= MAX_DATAGRAM).then_some(b)
    }
}
fn audio(b: &mut Vec<u8>, utt: u16, seq: u16, frames: &[Vec<u8>]) -> Option<()> {
    if frames.is_empty() || frames.len() > 8 {
        return None;
    }
    b.extend(utt.to_le_bytes());
    b.extend(seq.to_le_bytes());
    b.push(frames.len() as u8);
    for f in frames {
        if f.is_empty() || f.len() > 255 {
            return None;
        }
        b.push(f.len() as u8);
        b.extend(f);
    }
    Some(())
}
pub fn hello(session: u32, hears: bool, muted: u32) -> Vec<u8> {
    Message::Hello {
        session,
        hears,
        muted,
    }
    .encode()
    .expect("fixed HELLO fits")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_bounds() {
        let m = Message::RelayAudio {
            slot: 3,
            utt: 65535,
            seq: 2,
            frames: vec![vec![1, 2, 3], vec![4]],
        };
        let b = m.encode().unwrap();
        assert_eq!(parse(&b), Some(m));
        for n in 0..b.len() {
            assert!(parse(&b[..n]).is_none());
        }
        let mut extra = b.clone();
        extra.push(0);
        assert!(parse(&extra).is_none());
        assert!(parse(&[0x56, 1, 0x81, 20]).is_none());
    }
    #[test]
    fn oversized_audio_rejected() {
        assert!(
            Message::Audio {
                session: 1,
                utt: 1,
                seq: 0,
                frames: vec![vec![1; 255]; 2]
            }
            .encode()
            .is_none()
        );
    }
}
