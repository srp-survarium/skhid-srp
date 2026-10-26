//! A WAV through the real codec/jitter/radio path, without devices or a server.
use skhid_voice::{Core, Encoder, FRAME, RATE, wire::Message};
use std::{error::Error, fs};
fn read_wav(path: &str) -> Result<Vec<i16>, Box<dyn Error>> {
    let b = fs::read(path)?;
    if b.get(..4) != Some(b"RIFF") || b.get(8..12) != Some(b"WAVE") {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut at = 12usize;
    let mut format = None;
    let mut data = None;
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into()?) as usize;
        let end = at
            .checked_add(8)
            .and_then(|x| x.checked_add(size))
            .ok_or("WAV length overflow")?;
        let chunk = b.get(at + 8..end).ok_or("truncated WAV chunk")?;
        match &b[at..at + 4] {
            b"fmt " => format = Some(chunk),
            b"data" => data = Some(chunk),
            _ => {}
        }
        at = end.checked_add(size & 1).ok_or("WAV length overflow")?;
    }
    let f = format.ok_or("WAV has no format")?;
    if f.len() < 16
        || f[..4] != [1, 0, 1, 0]
        || u32::from_le_bytes(f[4..8].try_into()?) != RATE as u32
        || f[14..16] != [16, 0]
    {
        return Err("expected PCM 16-bit mono 16 kHz".into());
    }
    let bytes = data.ok_or("WAV has no audio")?;
    if bytes.len() % 2 != 0 {
        return Err("partial PCM sample".into());
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|p| i16::from_le_bytes([p[0], p[1]]))
        .collect())
}
fn write_wav(path: &str, pcm: &[i16]) -> Result<(), Box<dyn Error>> {
    let size = u32::try_from(pcm.len().checked_mul(2).ok_or("WAV too long")?)?;
    let mut b = b"RIFF".to_vec();
    b.extend(size.checked_add(36).ok_or("WAV too long")?.to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend([1, 0, 1, 0]);
    b.extend((RATE as u32).to_le_bytes());
    b.extend((RATE as u32 * 2).to_le_bytes());
    b.extend([2, 0, 16, 0]);
    b.extend(b"data");
    b.extend(size.to_le_bytes());
    for x in pcm {
        b.extend(x.to_le_bytes());
    }
    fs::write(path, b)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || !matches!(args[3].as_str(), "near" | "far") {
        return Err("usage: radio_demo input.wav output.wav near|far".into());
    }
    let input = read_wav(&args[1])?;
    let frames = input.len().div_ceil(FRAME);
    let far = u32::from(args[3] == "far");
    let mut encoder = Encoder::new()?;
    let mut core = Core::new()?;
    let mut output = Vec::with_capacity(frames * FRAME + RATE * 2);
    core.receive(
        &Message::RelayStart {
            slot: 0,
            utt: 1,
            ms: 0,
        },
        0,
    );
    for tick in 0..frames + RATE * 2 / FRAME {
        let now = tick as u32 * 20;
        if tick % 2 == 1 && tick - 1 < frames {
            let mut pair = Vec::new();
            for k in 0..2 {
                let begin = (tick - 1 + k) * FRAME;
                if begin >= input.len() {
                    break;
                }
                let mut pcm = [0; FRAME];
                let end = (begin + FRAME).min(input.len());
                pcm[..end - begin].copy_from_slice(&input[begin..end]);
                pair.push(encoder.encode(&pcm)?);
            }
            core.receive(
                &Message::RelayAudio {
                    slot: 0,
                    utt: 1,
                    seq: (tick / 2) as u16,
                    frames: pair,
                },
                now,
            );
        }
        if tick == frames + 2 {
            core.receive(
                &Message::RelayEnd {
                    slot: 0,
                    utt: 1,
                    reason: 0,
                },
                now,
            );
        }
        output.extend(core.mix(1., far, now));
    }
    write_wav(&args[2], &output)?;
    eprintln!("{:?}", core.stats());
    Ok(())
}
