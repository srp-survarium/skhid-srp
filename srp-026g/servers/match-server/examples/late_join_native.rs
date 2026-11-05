//! Loopback harness: [port] [map] [newcomer loadout index or starter] [initial players].
//! Enter `join` to add a player.

use std::io::{self, BufRead};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use match_server::v026g::{self, MatchPlayer, MatchProfile, MatchRegistry, MatchSetup};
use session::{SessionStore, persist::MemoryRepository};
use survarium::v026g::{dictionary::Dictionary, loadouts, skills::MODIFIERS};

struct Logger;
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Debug
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{} {}", record.level(), record.args());
        }
    }
    fn flush(&self) {}
}

fn player(place: u32, loadout: &loadouts::Loadout) -> MatchPlayer {
    MatchPlayer {
        session_id: 0x1000 + place,
        account_id: 100 + place,
        profile: MatchProfile::from_loadout(
            &format!("native-{place}"),
            (place % 2) as u8,
            loadout,
            [0.0; MODIFIERS],
            Dictionary::shared(),
            1 + place * 100,
        ),
        bot: false,
        driven: false,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    log::set_logger(&Logger).map_err(|e| e.to_string())?;
    log::set_max_level(log::LevelFilter::Debug);
    let port: u16 = std::env::args().nth(1).unwrap_or("2249".into()).parse()?;
    let map_id: u8 = std::env::args().nth(2).unwrap_or("61".into()).parse()?;
    let newcomer_loadout = match std::env::args().nth(3) {
        Some(index) if index != "starter" => loadouts::Loadouts::shared()
            .usable()
            .nth(index.parse()?)
            .ok_or("unknown usable loadout index")?,
        _ => loadouts::starter(),
    };
    let initial: u32 = std::env::args().nth(4).unwrap_or("2".into()).parse()?;
    if !(2..=20).contains(&initial) {
        return Err("initial players must be 2..=20".into());
    }
    let socket = UdpSocket::bind(("127.0.0.1", port))?;
    let registry = Arc::new(MatchRegistry::new("127.0.0.1", port));
    let ticket = registry.create_growing(MatchSetup {
        map_id,
        match_level: 3,
        players: (0..initial)
            .map(|place| player(place, loadouts::starter()))
            .collect(),
        score_weights: None,
        vs_bots: false,
        private: false,
        room_minimum: None,
        blackout: false,
    })?;
    let store = Arc::new(SessionStore::with_repository(
        Duration::from_secs(600),
        Box::new(MemoryRepository::default()),
    )?);
    let (results, _finished) = mpsc::channel::<v026g::MatchResult>();
    let stop = Arc::new(AtomicBool::new(false));
    let server = {
        let (registry, stop) = (registry.clone(), stop.clone());
        std::thread::spawn(move || v026g::run_on(&socket, registry, store, results, &stop))
    };
    eprintln!(
        "READY match={} port={port} initial={initial} sessions=0x1000..={:#x}",
        ticket.match_id,
        0x1000 + initial - 1
    );
    let mut next = initial;
    for line in io::stdin().lock().lines() {
        if line?.trim() != "join" {
            break;
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match registry.join_running(ticket.match_id, player(next, newcomer_loadout)) {
                Ok(_) => {
                    eprintln!("RESERVED place={next} session={:#x}", 0x1000 + next);
                    next += 1;
                    break;
                }
                Err(v026g::JoinError::Unavailable) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    eprintln!("RESERVATION FAILED: {error}");
                    break;
                }
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    server.join().expect("server thread")?;
    Ok(())
}
