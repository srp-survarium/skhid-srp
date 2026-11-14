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
/// A frame mixed with the slots of `far` through the radio and the rest
/// plain as decoded: both ears alike, the mono mix of before.
fn mix(core: &mut Core, far: u32, now: u32) -> [i16; FRAME] {
    let stereo = core.mix(1., &Hearing::radio(far), now);
    assert!(stereo.iter().all(|[left, right]| left == right));
    stereo.map(|[left, _]| left)
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
        Ok(Message::Start { utt: u16::MAX, .. })
    ));
    assert!(sender.frame(&[0x48, 2]).is_none());
    assert!(
        matches!(wire::parse(&sender.flush().unwrap()),Ok(Message::Audio{seq:0,frames,..})if frames==vec![vec![0x48,2]])
    );
    assert!(matches!(
        wire::parse(&sender.end().unwrap()),
        Ok(Message::End { frames: 1, .. })
    ));
    assert!(sender.end().is_none());
    assert!(sender.frame(&[0x48]).is_none());
    assert!(matches!(
        wire::parse(&sender.start(10)),
        Ok(Message::Start { utt: 0, .. })
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
        mix(&mut core, 0, t * 20);
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
        mix(&mut core, 0, t * 20);
    }
    core.receive(&packet(0, &frames[..2]), 80);
    for t in 4..8 {
        mix(&mut core, 0, t * 20);
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
        mix(&mut core, 1 << 3, t * 20);
    }
    assert_eq!(core.stats().timeouts, 1);
    assert_eq!(core.stats().squelch_in, 1);
    assert_eq!(core.stats().squelch_out, 1);
    core.reset();
    feed(&mut core, &frames);
    mix(&mut core, 1 << 3, 0);
    core.mute(1 << 3);
    assert_eq!(mix(&mut core, 1 << 3, 20), [0; FRAME]);
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
            let pcm = mix(&mut core, far, tick * 20);
            assert_eq!(core.stats().decoded, 0);
            let expected = &sounds::SQUELCH_IN[tick as usize * FRAME..(tick as usize + 1) * FRAME];
            for (got, want) in pcm.iter().zip(expected) {
                assert!((i32::from(*got) - i32::from(*want)).abs() <= 1);
            }
        }
        mix(&mut core, far, delay * 20);
        assert_eq!(core.stats().decoded, 1);
        for tick in 1..13 {
            mix(&mut core, far, (delay + tick) * 20);
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
    mix(&mut core, 0, 0);
    mix(&mut core, 1 << 3, 20);
    assert_eq!(core.stats().squelch_in, 0);
    assert_eq!(core.stats().decoded, 2);
}
/// A steady tone from slot 3, mixed frame by frame as `heard(tick)` says:
/// each frame's left and right.
fn heard_tone(ticks: u32, heard: impl Fn(u32) -> Heard) -> Vec<Stereo> {
    let mut encoder = Encoder::new().unwrap();
    let frames: Vec<Vec<u8>> = (0..ticks as usize + 4)
        .map(|i| encoder.encode(&tone(i, 500., 8000.)).unwrap())
        .collect();
    let mut core = Core::new().unwrap();
    feed(&mut core, &frames);
    (0..ticks)
        .map(|tick| {
            let mut hearing = Hearing::UNITY;
            hearing.set(3, heard(tick));
            core.mix(1., &hearing, tick * 20)
        })
        .collect()
}
fn ear_sums(frames: &[Stereo]) -> [f64; 2] {
    let mut ears = [0f64; 2];
    for [left, right] in frames.iter().flatten() {
        ears[0] += f64::from(*left).abs();
        ears[1] += f64::from(*right).abs();
    }
    ears
}
#[test]
fn a_plain_voice_is_mixed_by_the_gains_it_is_given() {
    // To the right (the engine's pan of a sound at the side: nothing in the
    // far ear): after the first frame's ramp from unity the left is silent.
    let right = heard_tone(8, |_| Heard::Plain(Ears::new(0., 1.)));
    let ears = ear_sums(&right);
    assert!(ears[1] > 0. && ears[0] < ears[1] / 20., "{ears:?}");
    assert!(right[1..].iter().flatten().all(|[left, _]| *left == 0));
    // Half as loud in both ears: half the samples of unity.
    let unity = heard_tone(8, |_| Heard::Plain(Ears::UNITY));
    let half = heard_tone(8, |_| Heard::Plain(Ears::new(0.5, 0.5)));
    for ([whole, _], [halved, _]) in unity[2..].iter().flatten().zip(half[2..].iter().flatten()) {
        assert!((i32::from(*whole) - 2 * i32::from(*halved)).abs() <= 2);
    }
    // Silent in both: nothing, and the utterance still plays out.
    let silent = heard_tone(8, |_| Heard::Plain(Ears::SILENT));
    assert!(silent[1..].iter().flatten().all(|ears| *ears == [0, 0]));
}
#[test]
fn the_switch_to_the_radio_ramps_over_a_frame() {
    // Plain for four frames, then the radio (the speaker walked past its
    // distance): the frame of the switch starts as the plain voice went on
    // and ends as the radio goes on, no step between two samples.
    let plain = heard_tone(8, |_| Heard::Plain(Ears::UNITY));
    let switched = heard_tone(8, |tick| {
        if tick < 4 {
            Heard::Plain(Ears::UNITY)
        } else {
            Heard::Radio
        }
    });
    assert_eq!(plain[..4], switched[..4]);
    let near = |a: i16, b: i16, most: i32| (i32::from(a) - i32::from(b)).abs() <= most;
    // The tone's own largest step between two samples (500 Hz at 8000).
    let step = 8000. * 2. * std::f32::consts::PI * 500. / RATE as f32;
    assert!(near(switched[4][0][0], plain[4][0][0], 80), "{:?} {:?}", switched[4][0], plain[4][0]);
    let all: Vec<i16> = switched.iter().flatten().map(|[left, _]| *left).collect();
    let largest = all.windows(2).map(|pair| (i32::from(pair[1]) - i32::from(pair[0])).abs()).max().unwrap();
    assert!((largest as f32) < 2.5 * step, "{largest} {step}");
    // From the next frame on the radio alone: both ears alike, not the tone.
    assert!(switched[5..].iter().flatten().all(|[left, right]| left == right));
    assert_ne!(switched[6], plain[6]);
}
#[test]
fn between_the_two_distances_the_plain_voice_and_the_radio_are_both_heard() {
    let right = Ears::new(0., 1.);
    let plain = ear_sums(&heard_tone(12, |_| Heard::Plain(right))[9..]);
    let radio = ear_sums(&heard_tone(12, |_| Heard::Radio)[9..]);
    let both = ear_sums(&heard_tone(12, |_| Heard::Both(right))[9..]);
    // The radio is centred: alone in the left ear; the right has the voice too.
    assert!(plain[0] == 0. && radio[0] > 0. && (both[0] - radio[0]).abs() < radio[0] / 100., "{plain:?} {radio:?} {both:?}");
    assert!(both[1] > radio[1] * 1.2 && both[1] > plain[1] * 1.2, "{plain:?} {radio:?} {both:?}");
}
#[test]
fn the_microphone_tests_playback_plays_under_a_voice_heard_in_the_world() {
    // The test's playback (a steady level) while slot 3 talks from the right:
    // the left ear has the playback alone, as it plays with no voice
    // (`centred`); the right has the voice over it.
    let mut encoder = Encoder::new().unwrap();
    let frames: Vec<Vec<u8>> = (0..12).map(|i| encoder.encode(&tone(i, 500., 8000.)).unwrap()).collect();
    let mut core = Core::new().unwrap();
    feed(&mut core, &frames);
    let under = [0.1_f32; FRAME];
    let alone = receiver::centred(&under);
    let mut hearing = Hearing::UNITY;
    hearing.set(3, Heard::Plain(Ears::new(0., 1.)));
    let mixed: Vec<Stereo> = (0..8).map(|tick| core.mix_over(Some(&under), 1., &hearing, tick * 20)).collect();
    assert!(mixed[1..].iter().flatten().all(|[left, _]| *left == alone[0][0]));
    let over = mixed[2..].iter().flatten().filter(|[_, right]| *right != alone[0][1]).count();
    assert!(over > 4 * FRAME, "{over}");
    // On the radio instead: both ears alike, over the playback.
    let mut core = Core::new().unwrap();
    feed(&mut core, &frames);
    hearing.set(3, Heard::Radio);
    let radio: Vec<Stereo> = (0..8).map(|tick| core.mix_over(Some(&under), 1., &hearing, tick * 20)).collect();
    assert!(radio[1..].iter().flatten().all(|[left, right]| left == right));
    assert!(radio[6..].iter().flatten().any(|[left, _]| *left != alone[0][0]));
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
    assert_eq!(core.talking(30), 1 << 3);
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
}
/// Speech-like input: a voice of gliding pitch with eight harmonics, in
/// syllables of 200 ms and pauses of 100 ms.
fn syllables(frame: usize, amplitude: f32) -> [i16; FRAME] {
    voiced(frame, amplitude, 8)
}
/// As [`syllables`], with `harmonics` harmonics falling 6 dB an octave (45:
/// up to 7.6 kHz, a wide-band voice).
pub(crate) fn voiced(frame: usize, amplitude: f32, harmonics: usize) -> [i16; FRAME] {
    std::array::from_fn(|i| {
        let t = (frame * FRAME + i) as f32 / RATE as f32;
        if (t * 1000.) as u32 % 300 >= 200 {
            return 0;
        }
        let phase = 2.
            * std::f32::consts::PI
            * (150. * t - 20. / 6. * (6. * t).cos() / std::f32::consts::PI);
        let v: f32 = (1..=harmonics)
            .map(|h| (phase * h as f32).sin() / h as f32)
            .sum();
        (v * amplitude) as i16
    })
}
/// How loud a sound is, its loudness summed over critical bands as Zwicker
/// sums it (each third-octave band's power, 100 Hz to 6.3 kHz, to the 0.23,
/// over blocks of 50 ms; no threshold or masking), given in dB: 20 log10 of
/// a gain moves it as much, so a difference is the gain that makes up for it.
fn loudness_db(samples: &[f32]) -> f32 {
    let block = RATE / 20;
    let blocks = samples.len() / block;
    let mut sum = 0.;
    let bands: Vec<Vec<f32>> = (0..19)
        .map(|k| {
            // RBJ band-pass, 0 dB at its centre, a third of an octave wide.
            let w = 2. * std::f32::consts::PI * 100. * 2f32.powf(k as f32 / 3.) / RATE as f32;
            let alpha = w.sin() / (2. * 4.318);
            let (b0, a1, a2) = (alpha, -2. * w.cos(), 1. - alpha);
            let a0 = 1. + alpha;
            let mut z = [0f32; 2];
            samples
                .iter()
                .map(|&x| {
                    let y = b0 / a0 * x + z[0];
                    z[0] = -a1 / a0 * y + z[1];
                    z[1] = -b0 / a0 * x - a2 / a0 * y;
                    y
                })
                .collect()
        })
        .collect();
    for b in 0..blocks {
        for band in &bands {
            let power = band[b * block..(b + 1) * block]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
                / block as f32;
            sum += power.powf(0.23);
        }
    }
    10. / 0.23 * (sum / blocks as f32).log10()
}
#[test]
fn a_radio_voice_is_as_loud_as_a_direct_one() {
    // 10 s of a voice through the automatic gain at the default sensitivity,
    // its words as the mix plays them near (as decoded) and far (through the
    // radio, its static apart): the last 5 s as loud within 2 dB, for a
    // narrow voice (to 1.3 kHz) and a wide one (to 7.6 kHz). Measured
    // 2026-11-13: the radio's words without `radio::VOICE` were 2.9 and 1.7
    // dB quieter (at the highest sensitivity 3.9 and 3.1).
    for harmonics in [8, 45] {
        let mut pre = Preprocessor::new().unwrap();
        let mut encoder = Encoder::new().unwrap();
        let mut decoder = Decoder::new().unwrap();
        let mut direct = Vec::new();
        for f in 0..500 {
            let mut pcm = voiced(f, 8000., harmonics);
            pre.run(&mut pcm, 0.5);
            let frame = encoder.encode(&pcm).unwrap();
            direct.extend(decoder.decode(Some(&frame)).unwrap());
        }
        let direct = &direct[RATE * 5..];
        let mut radio = direct.to_vec();
        Radio::new(0).process(&mut radio, 0.);
        let (near, far) = (loudness_db(direct), loudness_db(&radio));
        assert!((near - far).abs() < 2., "{harmonics}: {near} {far}");
    }
}
/// The level (dB of full scale) the preprocessor gives a speaker of this
/// amplitude, over the last of 4 s.
fn processed_db(amplitude: f32, sensitivity: f32) -> f32 {
    let mut pre = Preprocessor::new().unwrap();
    let seconds = 4 * RATE / FRAME;
    let last = RATE / FRAME;
    let mut sum = 0.;
    for frame in 0..seconds {
        let mut pcm = syllables(frame, amplitude);
        pre.run(&mut pcm, sensitivity);
        if frame >= seconds - last {
            sum += pcm
                .iter()
                .map(|&x| (f32::from(x) / 32768.).powi(2))
                .sum::<f32>();
        }
    }
    10. * (sum / RATE as f32).log10()
}
#[test]
fn automatic_gain_levels_quiet_and_loud_microphones() {
    // Speakers from -50 to -13 dB of full scale (amplitudes 150..10000) all
    // come out at one level (-29 dB measured), within 2 dB.
    let levels = [150., 600., 2500., 10000.].map(|amplitude| processed_db(amplitude, 0.5));
    let (low, high) = levels
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    assert!(high - low < 2., "{levels:?}");
    assert!((-32.0..-26.0).contains(&low), "{levels:?}");
    // The sensitivity slider moves it 6 dB either way.
    let quiet = processed_db(2500., 0.);
    let loud = processed_db(2500., 1.);
    assert!((10.0..14.0).contains(&(loud - quiet)), "{quiet} {loud}");
    assert_eq!(agc_level(0.5), AGC_LEVEL);
    assert_eq!(agc_level(f32::NAN), AGC_LEVEL / 2.);
    assert_eq!(agc_level(1.), AGC_LEVEL * 2.);
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
/// One well-formed datagram of each type the relay sends.
fn relayed() -> Vec<Vec<u8>> {
    [
        Message::Welcome { slot: 4 },
        Message::Roster {
            entries: vec![(0, 7), (4, 9)],
        },
        Message::RelayStart {
            slot: 4,
            utt: 2,
            ms: 9,
        },
        packet(0, &encoded(2)),
        Message::RelayEnd {
            slot: 4,
            utt: 2,
            reason: 0,
        },
    ]
    .iter()
    .map(|message| message.encode().unwrap())
    .collect()
}
#[test]
fn truncated_or_extended_datagrams_are_malformed() {
    use wire::Malformed;
    for bytes in relayed() {
        assert!(wire::parse(&bytes).is_ok());
        for len in 0..bytes.len() {
            assert!(wire::parse(&bytes[..len]).is_err(), "{:?}", &bytes[..len]);
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(wire::parse(&longer).is_err(), "{longer:?}");
    }
    assert_eq!(wire::parse(&[]), Err(Malformed::Short));
    assert_eq!(wire::parse(&[0x56, 1]), Err(Malformed::Short));
    assert_eq!(wire::parse(&[0x56, 1, 0x81]), Err(Malformed::Length));
    assert_eq!(wire::parse(&[0x56, 1, 0x81, 3, 0]), Err(Malformed::Length));
}
#[test]
fn oversize_bad_version_type_and_slot_are_malformed() {
    use wire::Malformed;
    let mut large = relayed().swap_remove(3);
    large.resize(wire::MAX_DATAGRAM + 1, 0);
    assert_eq!(wire::parse(&large), Err(Malformed::TooLarge));
    assert_eq!(wire::parse(&[0x57, 1, 0x81, 0]), Err(Malformed::NotVoice));
    assert_eq!(wire::parse(&[0x56, 2, 0x81, 0]), Err(Malformed::Version(2)));
    for kind in [0x00, 0x05, 0x85, 0x8F, 0xFF] {
        assert_eq!(wire::parse(&[0x56, 1, kind, 0]), Err(Malformed::Kind(kind)));
    }
    // Every relayed type's slot, out of range.
    for mut bytes in relayed() {
        let at = if bytes[2] == 0x90 { 4 } else { 3 };
        for slot in [SLOTS as u8, 31, 32, u8::MAX] {
            bytes[at] = slot;
            assert_eq!(wire::parse(&bytes), Err(Malformed::Slot(slot)));
        }
    }
    let roster = |entries: &[u8]| {
        let mut bytes = vec![0x56, 1, 0x90, (entries.len() / 5) as u8];
        bytes.extend_from_slice(entries);
        bytes
    };
    assert_eq!(
        wire::parse(&roster(&[3, 1, 0, 0, 0, 3, 2, 0, 0, 0])),
        Err(Malformed::Roster)
    );
    let mut many = roster(&[]);
    many[3] = SLOTS as u8 + 1;
    assert_eq!(wire::parse(&many), Err(Malformed::Length));
    // A slot out of range that reached the core anyway is ignored.
    let mut core = Core::new().unwrap();
    for slot in [SLOTS as u8, u8::MAX] {
        core.receive(
            &Message::RelayStart {
                slot,
                utt: 1,
                ms: 0,
            },
            0,
        );
        core.receive(
            &Message::RelayAudio {
                slot,
                utt: 1,
                seq: 0,
                frames: encoded(2),
            },
            0,
        );
    }
    assert_eq!(core.talking(0), 0);
}
#[test]
fn audio_frames_must_be_our_encoders() {
    use wire::Malformed;
    let audio = |frames: &[&[u8]]| {
        let mut bytes = vec![0x56, 1, 0x83, 3, 1, 0, 0, 0, frames.len() as u8];
        for frame in frames {
            bytes.push(frame.len() as u8);
            bytes.extend_from_slice(frame);
        }
        bytes
    };
    let frame: &[u8] = &[0x48, 1, 2];
    assert!(wire::parse(&audio(&[frame])).is_ok());
    assert!(wire::parse(&audio(&[frame; wire::MAX_FRAMES])).is_ok());
    assert_eq!(wire::parse(&audio(&[])), Err(Malformed::Frames));
    // Three frames: more than the sender sends and the receiver plays.
    assert_eq!(
        wire::parse(&audio(&[frame; wire::MAX_FRAMES + 1])),
        Err(Malformed::Frames)
    );
    assert_eq!(wire::parse(&audio(&[&[]])), Err(Malformed::NotOpus));
    // SILK narrow, medium and wide band 20 ms, mono, code 0: taken.
    for toc in wire::FRAME_TOCS {
        assert!(wire::parse(&audio(&[&[toc, 1]])).is_ok());
    }
    // 20 ms all, but not the encoder's: stereo, code 1 (two of 10 ms), code
    // 3 (one, padded), hybrid, CELT; and 10 ms SILK, 60 ms SILK.
    for toc in [0x4C, 0x01, 0x4B, 0x68, 0xF8, 0x00, 0x18] {
        assert_eq!(
            wire::parse(&audio(&[&[toc, 1, 1]])),
            Err(Malformed::NotOpus)
        );
    }
    // The longest frame the encoder may make, and one byte more.
    let longest = [0x48; wire::MAX_FRAME_BYTES];
    assert!(wire::parse(&audio(&[&longest, &longest])).is_ok());
    let longer = [0x48; wire::MAX_FRAME_BYTES + 1];
    assert_eq!(wire::parse(&audio(&[&longer])), Err(Malformed::NotOpus));
    // The encoder's frames are its own, and the sender refuses others.
    assert!(encoded(4).iter().all(|frame| wire::our_frame(frame)));
    let mut sender = Sender::new(1, 1);
    sender.start(0);
    assert!(sender.frame(&[0x00, 1]).is_none());
    assert!(sender.frame(&longer).is_none());
    assert_eq!(sender.end().map(|bytes| bytes[9]), Some(0));
}
#[test]
fn the_encoder_makes_only_what_the_relay_takes() {
    // Speech, and what no microphone after automatic gain gives: full-scale
    // noise, square waves and a sweep, with and without the preprocessor.
    let mut random = 0x1234_5678u32;
    let mut noise = move || {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (random >> 16) as i16
    };
    let square = |period: usize| {
        move |n: usize| {
            if (n / period).is_multiple_of(2) {
                i16::MAX
            } else {
                i16::MIN
            }
        }
    };
    let signals: [&mut dyn FnMut(usize) -> i16; 6] = [
        &mut |_| noise(),
        &mut square(80),
        &mut square(8),
        &mut square(3),
        &mut |n| {
            let t = n as f32 / RATE as f32;
            ((2. * std::f32::consts::PI * (100. * t + 400. * t * t)).sin() * 32767.) as i16
        },
        &mut |n| syllables(n / FRAME, 16000.)[n % FRAME],
    ];
    let mut largest = 0;
    for signal in signals {
        for preprocessed in [false, true] {
            let mut encoder = Encoder::new().unwrap();
            let mut pre = Preprocessor::new().unwrap();
            for f in 0..250 {
                let mut pcm: [i16; FRAME] = std::array::from_fn(|i| signal(f * FRAME + i));
                if preprocessed {
                    pre.run(&mut pcm, 1.);
                }
                let frame = encoder.encode(&pcm).unwrap();
                assert!(wire::our_frame(&frame), "{frame:x?}");
                largest = largest.max(frame.len());
            }
        }
    }
    // Its frames are well within the cap (93 bytes measured at most): the
    // cap never takes a frame of a game that has not changed its encoder.
    assert!(largest <= 100, "{largest}");
}
#[test]
fn opus_check_agrees_with_the_decoder() {
    let mut decoder = Decoder::new().unwrap();
    for toc in 0..=255u8 {
        // The shortest well-formed packet of each frame-count code (RFC 6716
        // 3.2): frames of no bytes, which the decoder conceals.
        let packets: Vec<Vec<u8>> = match toc & 3 {
            0 | 1 => vec![vec![toc]],
            2 => vec![vec![toc, 0]],
            _ => (0..=4).map(|count| vec![toc, count]).collect(),
        };
        for packet in packets {
            // What is taken is decoded; most of the rest would be too.
            if wire::our_frame(&packet) {
                assert!(decoder.decode(Some(&packet)).is_ok(), "{packet:?}");
            }
            assert_eq!(
                wire::our_frame(&packet),
                wire::FRAME_TOCS.contains(&toc),
                "{packet:?}"
            );
            decoder.reset();
        }
    }
}
#[test]
fn undecodable_opus_is_concealed_and_does_not_stop_another_speaker() {
    // 20 ms by its TOC, but code 3 with padding and no padding length: not
    // taken off the wire any more; were it to reach the decoder, concealed.
    let garbage = vec![0x4B, 0x41];
    assert!(!wire::our_frame(&garbage));
    assert!(Decoder::new().unwrap().decode(Some(&garbage)).is_err());
    let good = encoded(20);
    let mut core = Core::new().unwrap();
    for seq in 0..10u16 {
        let at = usize::from(seq) * 2;
        core.receive(&packet(seq, &good[at..at + 2]), 0);
        core.receive(
            &Message::RelayAudio {
                slot: 5,
                utt: 1,
                seq,
                frames: vec![garbage.clone(); 2],
            },
            0,
        );
    }
    for slot in [3, 5] {
        core.receive(
            &Message::RelayEnd {
                slot,
                utt: 1,
                reason: 0,
            },
            0,
        );
    }
    let mut energy = 0.;
    for tick in 0..24 {
        let pcm = mix(&mut core, 0, tick * 20);
        energy += pcm.iter().map(|&x| f32::from(x).powi(2)).sum::<f32>();
    }
    assert_eq!(core.stats().decoded, 20);
    assert_eq!(core.stats().undecodable, 20);
    assert!(energy > 0.);
    assert_eq!(core.talking(1000), 0);
}
#[test]
fn a_flood_of_random_datagrams_does_not_delay_a_good_speaker() {
    let good = encoded(20);
    let mut core = Core::new().unwrap();
    let mut random = 0x2468_ace1u32;
    let mut next = move || {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        random
    };
    for seq in 0..10u16 {
        // A thousand datagrams of junk between each of the good ones: some
        // pure noise, most of a valid header, any of them well formed.
        for _ in 0..1000 {
            let len = (next() % 480) as usize;
            let mut bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            if next() % 4 != 0 && len >= 3 {
                bytes[0] = 0x56;
                bytes[1] = 1;
                bytes[2] = [0x81, 0x82, 0x83, 0x84, 0x90][(next() % 5) as usize];
            }
            if let Ok(message) = wire::parse(&bytes) {
                // Junk that parses as another speaker's is that speaker's.
                let speaker = match &message {
                    Message::RelayStart { slot, .. }
                    | Message::RelayAudio { slot, .. }
                    | Message::RelayEnd { slot, .. } => Some(*slot),
                    _ => None,
                };
                if speaker != Some(3) {
                    core.receive(&message, 0);
                }
            }
        }
        let at = usize::from(seq) * 2;
        core.receive(&packet(seq, &good[at..at + 2]), 0);
    }
    core.receive(
        &Message::RelayEnd {
            slot: 3,
            utt: 1,
            reason: 0,
        },
        0,
    );
    let before = core.stats().decoded;
    for tick in 0..24 {
        mix(&mut core, 0, tick * 20);
    }
    // All twenty of the good speaker's frames played (others' may add).
    assert!(core.stats().decoded - before >= 20, "{:?}", core.stats());
}
#[test]
fn the_drop_log_is_one_line_at_most_every_ten_seconds() {
    use wire::Malformed;
    let mut log = runtime::DropLog::default();
    assert_eq!(log.line(0, 0), None);
    log.drop(Some(Malformed::Short));
    for _ in 0..9_999 {
        log.drop(Some(Malformed::Kind(0x40)));
    }
    assert_eq!(
        log.line(5, 0).as_deref(),
        Some("dropped 10000 datagrams (the first: truncated header)")
    );
    log.drop(None);
    assert_eq!(log.line(5 + runtime::DROP_LOG_MS - 1, 3), None);
    assert_eq!(
        log.line(5 + runtime::DROP_LOG_MS, 3).as_deref(),
        Some("dropped 1 datagrams (the first: a receive error); 3 frames the decoder refused")
    );
    // Nothing new: nothing written, however long.
    assert_eq!(log.line(100_000, 3), None);
    // The millisecond clock wraps.
    log.drop(Some(Malformed::NotOpus));
    assert!(log.line(u32::MAX, 3).is_some());
    log.drop(Some(Malformed::NotOpus));
    assert_eq!(log.line(5, 3), None);
    assert!(log.line(runtime::DROP_LOG_MS, 3).is_some());
}
#[test]
fn the_host_argument_is_read_no_further_than_its_bound() {
    use std::ffi::c_char;
    // SAFETY: each pointer is null or to a buffer of MAX_HOST + 1 bytes or
    // more, or NUL-terminated within it.
    unsafe {
        assert_eq!(runtime::host_arg(std::ptr::null()), None);
        assert_eq!(runtime::host_arg(c"127.0.0.1".as_ptr()), Some("127.0.0.1"));
        assert_eq!(runtime::host_arg(c"".as_ptr()), None);
        assert_eq!(runtime::host_arg(c"\xFF\xFE".as_ptr()), None);
        let mut longest = [b'a' as c_char; runtime::MAX_HOST + 1];
        longest[runtime::MAX_HOST] = 0;
        assert_eq!(
            runtime::host_arg(longest.as_ptr()).map(str::len),
            Some(runtime::MAX_HOST)
        );
        // No NUL in the first MAX_HOST + 1 bytes: refused, nothing past them read.
        let unterminated = [b'a' as c_char; runtime::MAX_HOST + 1];
        assert_eq!(runtime::host_arg(unterminated.as_ptr()), None);
    }
    // The game's settings: NaN and infinities are bounded.
    assert_eq!(radio::unit(f32::NAN), 0.);
    assert_eq!(radio::unit(f32::INFINITY), 1.);
    assert_eq!(radio::unit(f32::NEG_INFINITY), 0.);
    assert!(agc_level(f32::INFINITY).is_finite());
}
/// Plays `frames` (20 ms each) through a core as the relay sends them, near:
/// each mixed frame's RMS in dB of full scale, and the core.
fn played(frames: &[Vec<u8>]) -> (Vec<f32>, Core) {
    let mut core = Core::new().unwrap();
    let mut levels = Vec::new();
    let mut now = 0;
    for (seq, pair) in frames.chunks(2).enumerate() {
        core.receive(&packet(seq as u16, pair), now);
        for _ in 0..2 {
            let pcm = mix(&mut core, 0, now);
            let power = pcm
                .iter()
                .map(|&x| (f32::from(x) / 32768.).powi(2))
                .sum::<f32>();
            levels.push(10. * (power / FRAME as f32).max(1e-12).log10());
            now += 20;
        }
    }
    (levels, core)
}
#[test]
fn speech_through_the_automatic_gain_is_never_held() {
    // Voices from -50 to 0 dB of full scale, at the lowest, the default and
    // the highest sensitivity, and at the default a quiet voice that
    // suddenly shouts: the automatic gain keeps all of them under the
    // ceiling, so the listener plays them as decoded.
    let mut random = 0x2468_ace1u32;
    let mut hiss = move || {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (random >> 16) as i16 / 512
    };
    for (sensitivity, before, after) in [
        (0., 32000., 32000.),
        (0.5, 150., 150.),
        (0.5, 32000., 32000.),
        (0.5, 600., 20000.),
        (1., 2500., 2500.),
        (1., 32000., 32000.),
    ] {
        let mut pre = Preprocessor::new().unwrap();
        let mut encoder = Encoder::new().unwrap();
        let frames: Vec<Vec<u8>> = (0..500)
            .map(|f| {
                let mut pcm = syllables(f, if f < 250 { before } else { after });
                for sample in &mut pcm {
                    *sample = sample.saturating_add(hiss());
                }
                pre.run(&mut pcm, sensitivity);
                encoder.encode(&pcm).unwrap()
            })
            .collect();
        let (levels, core) = played(&frames);
        assert_eq!(core.stats().limited, 0, "{sensitivity} {before} {after}");
        assert!(levels.iter().any(|&db| db > -30.));
    }
}
#[test]
fn a_stream_that_skipped_the_automatic_gain_is_held_at_the_ceiling() {
    // A voice under the ceiling, played as decoded: what the mix adds to a
    // speaker's level (the volume and any gain of its own).
    let mut encoder = Encoder::new().unwrap();
    let voice: Vec<Vec<u8>> = (0..100)
        .map(|f| encoder.encode(&tone(f, 300., 6000.)).unwrap())
        .collect();
    let mean_db = |levels: &[f32]| {
        let power = levels.iter().map(|db| 10f32.powf(db / 10.)).sum::<f32>();
        10. * (power / levels.len() as f32).log10()
    };
    let mut decoder = Decoder::new().unwrap();
    let decoded: Vec<f32> = voice
        .iter()
        .map(|frame| {
            let pcm = decoder.decode(Some(frame)).unwrap();
            10. * (pcm.iter().map(|x| x * x).sum::<f32>() / FRAME as f32).log10()
        })
        .collect();
    let (alone, core) = played(&voice);
    assert_eq!(core.stats().limited, 0);
    let mix_gain = mean_db(&alone[20..]) - mean_db(&decoded[10..]);
    // A modified game's full-scale square wave: -0.3 dBFS decoded, held at
    // the ceiling (-12 dBFS) from its first frame on.
    let mut encoder = Encoder::new().unwrap();
    let frames: Vec<Vec<u8>> = (0..100)
        .map(|f| {
            let pcm = std::array::from_fn(|i| {
                if ((f * FRAME + i) / 8).is_multiple_of(2) {
                    i16::MAX
                } else {
                    i16::MIN
                }
            });
            encoder.encode(&pcm).unwrap()
        })
        .collect();
    let (levels, core) = played(&frames);
    let loudest = levels.iter().copied().fold(f32::MIN, f32::max) - mix_gain;
    assert!((-12.6..=-11.9).contains(&loudest), "{mix_gain} {levels:?}");
    assert!(
        levels[4..].iter().all(|&db| db - mix_gain > -14.),
        "{levels:?}"
    );
    assert!(core.stats().limited >= 95, "{:?}", core.stats().limited);
    // Then the voice from the same speaker comes back to its own level
    // within half a second (0.5 dB a frame).
    let mut after_square = frames[..50].to_vec();
    after_square.extend(voice.iter().cloned());
    let (after, _) = played(&after_square);
    for (a, b) in alone[50..].iter().zip(&after[100..]) {
        assert!((a - b).abs() < 0.5, "{a} {b}");
    }
}
#[test]
fn a_new_utterance_starts_at_unity() {
    // A loud utterance held at the ceiling, then a quiet one: the quiet one
    // plays as it would alone from its first frame, not at the gain the
    // loud one left.
    let mut encoder = Encoder::new().unwrap();
    let loud: Vec<Vec<u8>> = (0..20)
        .map(|f| {
            let pcm = std::array::from_fn(|i| {
                if ((f * FRAME + i) / 8).is_multiple_of(2) {
                    i16::MAX
                } else {
                    i16::MIN
                }
            });
            encoder.encode(&pcm).unwrap()
        })
        .collect();
    let mut encoder = Encoder::new().unwrap();
    let quiet: Vec<Vec<u8>> = (0..20)
        .map(|f| encoder.encode(&tone(f, 300., 6000.)).unwrap())
        .collect();
    let (alone, _) = played(&quiet);
    let mut core = Core::new().unwrap();
    let mut now = 0;
    for (utt, frames) in [(1, &loud), (2, &quiet)] {
        let mut levels = Vec::new();
        for (seq, pair) in frames.chunks(2).enumerate() {
            core.receive(
                &Message::RelayAudio {
                    slot: 3,
                    utt,
                    seq: seq as u16,
                    frames: pair.to_vec(),
                },
                now,
            );
            for _ in 0..2 {
                let pcm = mix(&mut core, 0, now);
                let power = pcm
                    .iter()
                    .map(|&x| (f32::from(x) / 32768.).powi(2))
                    .sum::<f32>();
                levels.push(10. * (power / FRAME as f32).max(1e-12).log10());
                now += 20;
            }
        }
        core.receive(
            &Message::RelayEnd {
                slot: 3,
                utt,
                reason: 0,
            },
            now,
        );
        for _ in 0..10 {
            mix(&mut core, 0, now);
            now += 20;
        }
        if utt == 1 {
            assert!(core.stats().limited >= 15);
        } else {
            let played: Vec<f32> = levels.into_iter().filter(|&db| db > -100.).collect();
            let alone: Vec<f32> = alone.iter().copied().filter(|&db| db > -100.).collect();
            assert!(played.len() >= 10);
            for (a, b) in alone.iter().zip(&played) {
                assert!((a - b).abs() < 0.1, "{a} {b}");
            }
        }
    }
}
