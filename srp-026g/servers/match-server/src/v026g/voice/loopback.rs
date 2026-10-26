//! Two headless clients send an encoded tone through the actual UDP relay.
//! No microphone, fake codec, fake relay or game process participates.
use super::*;
use crate::v026g::registry::Seat;
use crate::v026g::tests::{MAP_TDM, session_of, setup};
use skhid_voice::{Core, Encoder, FRAME, RATE, Sender, wire};
use std::sync::Arc;

struct Server {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<io::Result<()>>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap().unwrap();
    }
}

fn receive(game: &UdpSocket) -> wire::Message {
    let mut buffer = [0; MAX_DATAGRAM];
    let size = game
        .recv(&mut buffer)
        .expect("relay response within timeout");
    wire::parse(&buffer[..size]).expect("the relay speaks the client's protocol")
}

fn tone_at(sample: usize, hz: f64) -> f64 {
    (sample as f64 * hz * std::f64::consts::TAU / RATE as f64).sin()
}

#[test]
fn two_clients_pass_opus_audio_through_the_udp_relay() {
    let registry = Arc::new(MatchRegistry::new("127.0.0.1", 1));
    let ticket = registry.create(setup(4, MAP_TDM)).unwrap();
    for place in [0, 2] {
        registry.set_connection(
            Seat {
                match_id: ticket.match_id,
                place,
            },
            "127.0.0.1".parse().unwrap(),
        );
    }
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let _server = Server {
        stop,
        thread: Some(std::thread::spawn(move || {
            run(&socket, registry, [7; 32], &worker_stop)
        })),
    };
    let games: Vec<_> = (0..2)
        .map(|_| {
            let game = UdpSocket::bind("127.0.0.1:0").unwrap();
            game.connect(address).unwrap();
            game.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            game
        })
        .collect();
    let mut cores = [Core::new().unwrap(), Core::new().unwrap()];
    for (index, place) in [0, 2].into_iter().enumerate() {
        games[index]
            .send(&wire::hello(session_of(place), true, 0))
            .unwrap();
        let welcome = receive(&games[index]);
        assert!(matches!(welcome, wire::Message::Welcome { slot } if slot == place as u8));
        cores[index].receive(&welcome, 0);
        let roster = receive(&games[index]);
        assert!(matches!(&roster, wire::Message::Roster { entries } if entries.len() == 2));
        cores[index].receive(&roster, 0);
    }
    let mut encoders = [Encoder::new().unwrap(), Encoder::new().unwrap()];
    let mut senders = [Sender::new(session_of(0), 1), Sender::new(session_of(2), 1)];
    let mut output = [Vec::<i16>::new(), Vec::<i16>::new()];
    for index in 0..2 {
        games[index].send(&senders[index].start(0)).unwrap();
        cores[1 - index].receive(&receive(&games[1 - index]), 0);
    }
    // 0.8s, 20 audio datagrams each: comfortably inside the relay's 60/s cap,
    // even when this deterministic test runs faster than the real audio clock.
    let frequencies = [600.0, 900.0];
    for frame in 0..40 {
        let now = frame as u32 * 20;
        for index in 0..2 {
            let samples = std::array::from_fn(|i| {
                (tone_at(frame * FRAME + i, frequencies[index]) * 12000.0) as i16
            });
            let encoded = encoders[index].encode(&samples).unwrap();
            if let Some(packet) = senders[index].frame(&encoded) {
                games[index].send(&packet).unwrap();
                cores[1 - index].receive(&receive(&games[1 - index]), now);
            }
        }
        for index in 0..2 {
            output[index].extend_from_slice(&cores[index].mix(1.0, 0, now));
        }
    }
    for index in 0..2 {
        if let Some(packet) = senders[index].end() {
            games[index].send(&packet).unwrap();
            cores[1 - index].receive(&receive(&games[1 - index]), 800);
        }
    }
    for index in 0..2 {
        // Exclude startup's jitter delay and codec lookahead. A phase-free
        // frequency projection proves we received the other client's tone,
        // while RMS rejects silence and a numerically tiny false positive.
        let sound = &output[index][FRAME * 12..];
        let energy: f64 = sound.iter().map(|&x| f64::from(x).powi(2)).sum();
        assert!(
            (energy / sound.len() as f64).sqrt() > 1000.0,
            "client {index}: audible decoded tone"
        );
        let hz = frequencies[1 - index];
        let (sin, cos) = sound
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(sin, cos), (i, &sample)| {
                let phase = i as f64 * hz * std::f64::consts::TAU / RATE as f64;
                (
                    sin + f64::from(sample) * phase.sin(),
                    cos + f64::from(sample) * phase.cos(),
                )
            });
        let projection = (sin * sin + cos * cos).sqrt() / (energy * sound.len() as f64).sqrt();
        assert!(
            projection > 0.4,
            "client {index}: received the teammate's {hz}Hz tone ({projection})"
        );
    }
}
