use super::*;
use wire::Message;
fn tone(frame: usize, freq: f32, amplitude: f32) -> [i16; FRAME] {
    std::array::from_fn(|i| {
        ((2. * std::f32::consts::PI * freq * (frame * FRAME + i) as f32 / RATE as f32).sin()
            * amplitude) as i16
    })
}
fn encoded(n: usize) -> Vec<Vec<u8>> {
    let mut encoder = Encoder::new().unwrap();
    (0..n)
        .map(|i| encoder.encode(&tone(i, 500., 8000.)).unwrap())
        .collect()
}
fn packet(seq: u16, frames: &[Vec<u8>]) -> Message {
    Message::RelayAudio {
        slot: 3,
        utt: 1,
        seq,
        frames: frames.to_vec(),
    }
}
fn feed(core: &mut Core, frames: &[Vec<u8>]) {
    for (seq, pair) in frames.chunks(2).enumerate() {
        core.receive(&packet(seq as u16, pair), 0);
    }
}
#[test]
fn opus_round_trip_preserves_tone() {
    let frames = encoded(12);
    let mut decoder = Decoder::new().unwrap();
    let mut samples = Vec::new();
    for frame in frames {
        samples.extend(decoder.decode(Some(&frame)).unwrap());
    }
    let samples = &samples[FRAME * 3..];
    let energy = samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32;
    assert!((0.01..0.08).contains(&energy));
    let mut re = 0.;
    let mut im = 0.;
    for (i, x) in samples.iter().enumerate() {
        let phase = 2. * std::f32::consts::PI * 500. * i as f32 / RATE as f32;
        re += x * phase.cos();
        im += x * phase.sin();
    }
    let spectral = 2. * (re * re + im * im) / (samples.len() * samples.len()) as f32;
    assert!(spectral / energy > 0.8);
}
#[test]
fn sender_flushes_odd_frame_and_wraps_utterance() {
    let mut sender = Sender::new(123, u16::MAX);
    assert!(matches!(
        wire::parse(&sender.start(9)),
        Some(Message::Start { utt: u16::MAX, .. })
    ));
    assert!(sender.frame(&[1, 2]).is_none());
    assert!(
        matches!(wire::parse(&sender.flush().unwrap()),Some(Message::Audio{seq:0,frames,..})if frames==vec![vec![1,2]])
    );
    assert!(matches!(
        wire::parse(&sender.end().unwrap()),
        Some(Message::End { frames: 1, .. })
    ));
    assert!(sender.end().is_none());
    assert!(sender.frame(&[1]).is_none());
    assert!(matches!(
        wire::parse(&sender.start(10)),
        Some(Message::Start { utt: 0, .. })
    ));
}
#[test]
fn reorder_before_playout_and_packet_loss() {
    let frames = encoded(10);
    let mut core = Core::new().unwrap();
    core.receive(&packet(1, &frames[2..4]), 0);
    core.receive(&packet(0, &frames[..2]), 0);
    core.receive(&packet(3, &frames[6..8]), 0);
    core.receive(&packet(4, &frames[8..]), 0);
    core.receive(
        &Message::RelayEnd {
            slot: 3,
            utt: 1,
            reason: 0,
        },
        0,
    );
    for t in 0..10 {
        core.mix(1., 0, t * 20);
    }
    assert_eq!(core.stats().decoded, 8);
    assert_eq!(core.stats().concealed, 2);
    assert_eq!(core.stats().closed, 1);
    core.receive(&packet(1, &frames[2..4]), 300);
    assert_eq!(core.stats().late, 2);
    assert_eq!(core.talking(300), 0);
}
#[test]
fn late_frame_does_not_overwrite_unplayed_ring_cell() {
    let frames = encoded(8);
    let mut core = Core::new().unwrap();
    feed(&mut core, &frames);
    for t in 0..4 {
        core.mix(1., 0, t * 20);
    }
    core.receive(&packet(0, &frames[..2]), 80);
    for t in 4..8 {
        core.mix(1., 0, t * 20);
    }
    assert_eq!(core.stats().decoded, 8);
    assert_eq!(core.stats().late, 2);
}
#[test]
fn silence_closes_and_muting_cuts_audio_and_effects() {
    let frames = encoded(12);
    let mut core = Core::new().unwrap();
    feed(&mut core, &frames);
    for t in 0..55 {
        core.mix(1., 1 << 3, t * 20);
    }
    assert_eq!(core.stats().timeouts, 1);
    assert_eq!(core.stats().squelch_in, 1);
    assert_eq!(core.stats().squelch_out, 1);
    core.reset();
    feed(&mut core, &frames);
    core.mix(1., 1 << 3, 0);
    core.mute(1 << 3);
    assert_eq!(core.mix(1., 1 << 3, 20), [0; FRAME]);
    assert_eq!(core.talking(20), 0);
    core.receive(&packet(5, &frames[10..]), 40);
    assert_eq!(core.talking(40), 0);
}
#[test]
fn far_squelch_delays_words_near_has_none() {
    let frames = encoded(12);
    for (far, delay) in [(0, 0), (1 << 3, 8)] {
        let mut core = Core::new().unwrap();
        feed(&mut core, &frames);
        core.receive(
            &Message::RelayEnd {
                slot: 3,
                utt: 1,
                reason: 0,
            },
            0,
        );
        for tick in 0..delay {
            let pcm = core.mix(1., far, tick * 20);
            assert_eq!(core.stats().decoded, 0);
            let expected = &sounds::SQUELCH_IN[tick as usize * FRAME..(tick as usize + 1) * FRAME];
            for (got, want) in pcm.iter().zip(expected) {
                assert!((i32::from(*got) - i32::from(*want)).abs() <= 1);
            }
        }
        core.mix(1., far, delay * 20);
        assert_eq!(core.stats().decoded, 1);
        for tick in 1..13 {
            core.mix(1., far, (delay + tick) * 20);
        }
        assert_eq!(core.stats().squelch_in, u32::from(far != 0));
        assert_eq!(core.stats().squelch_out, u32::from(far != 0));
    }
}
#[test]
fn mid_utterance_radio_switch_does_not_add_opening_effect() {
    let frames = encoded(12);
    let mut core = Core::new().unwrap();
    feed(&mut core, &frames);
    core.mix(1., 0, 0);
    core.mix(1., 1 << 3, 20);
    assert_eq!(core.stats().squelch_in, 0);
    assert_eq!(core.stats().decoded, 2);
}
#[test]
fn utterance_order_wrap_and_reset() {
    let frames = encoded(4);
    let mut core = Core::new().unwrap();
    let mut message = packet(0, &frames[..2]);
    if let Message::RelayAudio { utt, .. } = &mut message {
        *utt = u16::MAX;
    }
    core.receive(&message, 0);
    core.receive(
        &Message::RelayStart {
            slot: 3,
            utt: 0,
            ms: 10,
        },
        10,
    );
    assert_eq!(core.stats().closed, 1);
    core.receive(&message, 20);
    assert_eq!(core.stats().late, 2);
    core.reset();
    core.receive(&message, 30);
    assert_eq!(core.last_speaker(), Some(3));
}
fn sine_gain(freq: f32) -> f32 {
    let mut radio = Radio::new(0);
    let mut signal: Vec<f32> = (0..RATE)
        .map(|i| 0.0001 * (2. * std::f32::consts::PI * freq * i as f32 / RATE as f32).sin())
        .collect();
    radio.process(&mut signal, 0.);
    let energy = signal[RATE / 2..].iter().map(|v| v * v).sum::<f32>() / (RATE / 2) as f32;
    10. * (energy / (0.0001 * 0.0001 / 2.)).log10()
}
#[test]
fn radio_matches_retail_response_and_static_level() {
    let centres = [
        400., 500., 630., 800., 1000., 1250., 1600., 2000., 2500., 3150., 4000., 5000., 6300.,
    ];
    let retail = [
        -1.7, 7.2, 6., 1.1, -6.5, -2.1, 0., 3., -1.8, -6.6, 7.7, 9.7, 3.7,
    ];
    let gains = centres.map(sine_gain);
    let middle = gains[1..10].iter().sum::<f32>() / 9.;
    for i in 0..centres.len() {
        assert!(
            (gains[i] - middle - retail[i]).abs() < 4.5,
            "{} Hz: {} vs {}",
            centres[i],
            gains[i] - middle,
            retail[i]
        );
    }
    assert!(sine_gain(125.) - middle < -40.);
    assert!(sine_gain(250.) - middle < -25.);
    let mut radio = Radio::new(1);
    let mut silence = vec![0.; RATE];
    radio.process(&mut silence, 1.);
    let db = 10. * (silence.iter().map(|x| x * x).sum::<f32>() / RATE as f32).log10();
    assert!((-46.0..-42.0).contains(&db));
    assert_eq!(mic_gain(f32::NAN), 0.25);
    assert_eq!(mic_gain(0.5), 1.);
    assert_eq!(mic_gain(1.), 4.);
}
#[test]
fn roster_replacement_resets_only_departed_speaker() {
    let frames = encoded(4);
    let mut core = Core::new().unwrap();
    core.receive(
        &Message::RelayAudio {
            slot: 3,
            utt: 100,
            seq: 0,
            frames: frames.clone(),
        },
        0,
    );
    core.receive(
        &Message::RelayStart {
            slot: 4,
            utt: 80,
            ms: 0,
        },
        0,
    );
    core.reset_slot(3);
    assert_eq!(core.talking(20), 1 << 4);
    core.receive(&packet(0, &frames[..2]), 20);
    assert_eq!(core.talking(20), (1 << 3) | (1 << 4));
    core.mute(1 << 3);
    core.reset_slot(3);
    core.receive(&packet(1, &frames[2..]), 40);
    assert_eq!(core.talking(40), 1 << 4);
}
