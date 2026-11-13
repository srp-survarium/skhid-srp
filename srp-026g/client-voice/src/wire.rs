//! Bounded version-one UDP protocol. Raw bytes become one typed message here,
//! or a [`Malformed`] saying why they do not.
use crate::SLOTS;
pub const MAX_DATAGRAM: usize = 400;
/// The most frames an AUDIO carries: [`crate::Sender`] sends two, one at an
/// utterance's end on an odd frame, and [`crate::Core`] plays two. The
/// relay forwards no more.
pub const MAX_FRAMES: usize = 2;
/// The longest frame: [`crate::Encoder`] (16 kbit/s, 40 bytes a 20 ms frame
/// on average, 93 at most measured on full-scale square waves and noise) is
/// given no more room than this, 3 times its rate.
pub const MAX_FRAME_BYTES: usize = 120;
/// The TOC bytes (RFC 6716 3.1) of [`crate::Encoder`]'s frames: SILK, 20 ms,
/// mono, code 0, of narrow, medium or wide band. It encodes 16 kHz speech at
/// 16 kbit/s, where Opus stays in SILK; its own are all 0x48.
pub const FRAME_TOCS: [u8; 3] = [0x08, 0x28, 0x48];
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
/// Why a datagram is not a [`Message`] (the drop log's words).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Over [`MAX_DATAGRAM`].
    TooLarge,
    /// Shorter than the magic, version and type.
    Short,
    NotVoice,
    Version(u8),
    /// A type this version does not have.
    Kind(u8),
    /// A body shorter or longer than its type's.
    Length,
    /// A slot of no player ([`SLOTS`] or more).
    Slot(u8),
    /// AUDIO not of 1 to [`MAX_FRAMES`] frames that fill it exactly.
    Frames,
    /// A frame [`crate::Encoder`] does not make ([`our_frame`]).
    NotOpus,
    /// A ROSTER naming a slot twice.
    Roster,
    /// A client's type from the relay.
    FromClient,
}
impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "over {MAX_DATAGRAM} bytes"),
            Self::Short => write!(f, "truncated header"),
            Self::NotVoice => write!(f, "not voice"),
            Self::Version(version) => write!(f, "version {version}"),
            Self::Kind(kind) => write!(f, "type {kind:#04x}"),
            Self::Length => write!(f, "wrong length"),
            Self::Slot(slot) => write!(f, "slot {slot}"),
            Self::Frames => write!(f, "malformed frames"),
            Self::NotOpus => write!(f, "a frame our encoder does not make"),
            Self::Roster => write!(f, "a slot twice in the roster"),
            Self::FromClient => write!(f, "a client's type"),
        }
    }
}
/// Whether `frame` is one [`crate::Encoder`] makes: of one of [`FRAME_TOCS`]
/// and at most [`MAX_FRAME_BYTES`]. Nothing else reaches the decoder; the
/// relay checks the same (`match-server`'s `voice::our_frame`).
pub fn our_frame(frame: &[u8]) -> bool {
    frame.len() <= MAX_FRAME_BYTES && frame.first().is_some_and(|toc| FRAME_TOCS.contains(toc))
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Malformed> {
        let (out, rest) = self.0.split_first_chunk::<N>().ok_or(Malformed::Length)?;
        self.0 = rest;
        Ok(*out)
    }
    fn byte(&mut self) -> Result<u8, Malformed> {
        let [byte] = self.take::<1>()?;
        Ok(byte)
    }
    fn u16(&mut self) -> Result<u16, Malformed> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn slot(&mut self) -> Result<u8, Malformed> {
        let slot = self.byte()?;
        if usize::from(slot) < SLOTS {
            Ok(slot)
        } else {
            Err(Malformed::Slot(slot))
        }
    }
    fn frames(&mut self) -> Result<Vec<Vec<u8>>, Malformed> {
        let count = usize::from(self.byte()?);
        if !(1..=MAX_FRAMES).contains(&count) {
            return Err(Malformed::Frames);
        }
        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            let len = usize::from(self.byte().map_err(|_| Malformed::Frames)?);
            let (frame, rest) = self.0.split_at_checked(len).ok_or(Malformed::Frames)?;
            if !our_frame(frame) {
                return Err(Malformed::NotOpus);
            }
            frames.push(frame.to_vec());
            self.0 = rest;
        }
        if self.0.is_empty() {
            Ok(frames)
        } else {
            Err(Malformed::Frames)
        }
    }
}
pub fn parse(bytes: &[u8]) -> Result<Message, Malformed> {
    if bytes.len() > MAX_DATAGRAM {
        return Err(Malformed::TooLarge);
    }
    let Some((&[magic, version, kind], body)) = bytes.split_first_chunk::<3>() else {
        return Err(Malformed::Short);
    };
    if magic != 0x56 {
        return Err(Malformed::NotVoice);
    }
    if version != 1 {
        return Err(Malformed::Version(version));
    }
    let mut r = Reader(body);
    let message = match kind {
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
                return Err(Malformed::Length);
            }
            let mut entries = Vec::with_capacity(count);
            let mut seen = 0u32;
            for _ in 0..count {
                let slot = r.slot()?;
                if seen & (1 << slot) != 0 {
                    return Err(Malformed::Roster);
                }
                seen |= 1 << slot;
                entries.push((slot, r.u32()?));
            }
            Message::Roster { entries }
        }
        _ => return Err(Malformed::Kind(kind)),
    };
    // Nothing may follow a message.
    if r.0.is_empty() {
        Ok(message)
    } else {
        Err(Malformed::Length)
    }
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
    if frames.is_empty() || frames.len() > MAX_FRAMES {
        return None;
    }
    b.extend(utt.to_le_bytes());
    b.extend(seq.to_le_bytes());
    b.push(frames.len() as u8);
    for f in frames {
        if !our_frame(f) {
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
            frames: vec![vec![0x08, 2, 3], vec![0x48]],
        };
        let b = m.encode().unwrap();
        assert_eq!(parse(&b), Ok(m));
        for n in 0..b.len() {
            assert!(parse(&b[..n]).is_err());
        }
        let mut extra = b.clone();
        extra.push(0);
        assert_eq!(parse(&extra), Err(Malformed::Frames));
        assert_eq!(parse(&[0x56, 1, 0x81, 20]), Err(Malformed::Slot(20)));
    }
    #[test]
    fn oversized_audio_rejected() {
        assert!(
            Message::Audio {
                session: 1,
                utt: 1,
                seq: 0,
                frames: vec![vec![0x48; MAX_FRAME_BYTES + 1]; 2]
            }
            .encode()
            .is_none()
        );
    }
}
