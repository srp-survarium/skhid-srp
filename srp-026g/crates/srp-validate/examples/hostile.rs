//! Bounded malformed-player search and message flood microbenchmark.
//! Run with --target i686-unknown-linux-gnu and the arena's size optimization.
//! This measures validator CPU, not a network or OS DDoS defence.
use std::{hint::black_box, time::Instant};
use srp_validate::{messages, world::{self, Block, Context}};

fn main() {
    let context = Context {
        time: 4500, places: 20, game_mode: 4, batteries: 0,
        respawn_points: Some(3), profiles: None, artefacts: None,
        objects: None, inventory: None,
    };
    let mut timings = Vec::new();
    let mut refused = 0;
    for len in [64, 128, 512, 1024, 2048, 4096] {
        for word in [0u32, 1, 16, 64, 255, 256, 455, 0xff00, 0x01000000, 0xff000000, u32::MAX] {
            for shift in 0..4 {
                let pattern = word.to_le_bytes();
                let mut bytes: Vec<u8> = (0..len).map(|i| pattern[(i + shift) % 4]).collect();
                bytes[..31].fill(0); bytes[2] = 8;
                let block = Block { place: 0, at: 0, state: &bytes };
                refused += usize::from(world::player_tail(&block, &context).is_err());
                let start = Instant::now();
                for _ in 0..1000 { black_box(world::player_tail(black_box(&block), black_box(&context))).ok(); }
                timings.push(start.elapsed().as_nanos() as f64 / 1000.0);
            }
        }
    }
    timings.sort_by(f64::total_cmp);
    println!("{} bounded player-state patterns ({} refused), ns/player: median {:.0}, p99 {:.0}, max {:.0}",
        timings.len(), refused, timings[timings.len()/2], timings[timings.len()*99/100], timings[timings.len()-1]);
    for (name, bytes) in [("valid input", [0u8;10]), ("unused action bit", [0,0,0,0,0,0,0,0,0,0x40])] {
        let start = Instant::now();
        for _ in 0..1_000_000 { black_box(messages::check(black_box(0xeb), black_box(&bytes))).ok(); }
        println!("{name}: {:.1} ns/message over 1,000,000 calls", start.elapsed().as_nanos() as f64 / 1_000_000.0);
    }
}
