//! `updater_skhid.exe seed <pipe>`: the game's sharing
//! (`srp-026g/docs/notes/torrent-downloads.md`, "Sharing from the game").
//! The patched game, while its `enable_torrent_seeding` ("Allow torrent",
//! user.cfg) is on, starts it on its login screen and in its lobby and closes the pipe when
//! a match starts or the game ends; this shares the folder's torrent while
//! the pipe is open (`torrent::share_while`, the one call into the engine),
//! and answers the game's stats switch with ticks (`updater_logic::seed`).
//!
//! No window, no box, no lock, nothing installed or fetched; its own log,
//! so that it never mixes with a launcher's updater's.

use alloc::format;

use updater_logic::pipe::port_number;
use updater_logic::seed::{Event, FromGame, GamePipe, Meter, Stats, Tick, frame};

use crate::install::Layout;
use crate::log::log;
use crate::pipe::Pipe;
use crate::{exit, log as logging, torrent, win};

const LOG_FILE: &str = "updater_skhid.seed.log";

/// Shares until the game closes `pipe`. `pipe`: `None` when the command
/// line was not the game's.
pub fn run(pipe: Option<GamePipe>) -> u32 {
    let Ok(layout) = Layout::find() else {
        return exit::FAILED;
    };
    logging::start(format!("{}\\{LOG_FILE}", layout.exe_dir));
    log!("---- updater {} seeding for the game, in {}", env!("CARGO_PKG_VERSION"), layout.exe_dir);
    let Some(pipe) = pipe else {
        log!("seed: not the game's command line: nothing done");
        return exit::FAILED;
    };
    let mut channel = match Pipe::connect(pipe.name()) {
        Ok(channel) => channel,
        Err(code) => {
            log!("seed: pipe {}: {}", pipe.name(), win::error_text(code));
            return exit::NO_PIPE;
        }
    };
    // The port is the launcher's setting; "Allow torrent" was the game's
    // to decide, which started this to share.
    let (_, port) = win::launcher_torrent_setting();
    let port = port_number(&port);
    let mut stats = Stats::Hidden;
    let mut meter = Meter::new(win::ticks(), 0);
    let shared = torrent::share_while(&layout, port, &mut |sharing| {
        let now = win::ticks();
        let mut events: alloc::vec::Vec<Event> = channel
            .payloads()
            .iter()
            .filter_map(|payload| FromGame::parse(payload))
            .map(Event::Game)
            .collect();
        events.push(Event::Clock);
        for event in events {
            let (next, tick) = stats.step(now, event);
            if next != stats {
                log!("seed: the game's stats {}", if matches!(next, Stats::Shown { .. }) { "shown" } else { "hidden" });
            }
            stats = next;
            if tick {
                let tick = Tick {
                    upload: meter.rate(now, sharing.sent),
                    limit: u32::try_from(sharing.rate).unwrap_or(u32::MAX),
                    peers: u32::try_from(sharing.peers).unwrap_or(u32::MAX),
                };
                channel.send_bytes(&frame(tick));
            }
        }
        channel.is_open()
    });
    match shared {
        Ok(()) => log!("seed: the game closed the pipe: done"),
        Err(torrent::NotShared::PortBusy) => {
            log!("seed: port {port:?} is held (another updater shares): not shared");
        }
        Err(torrent::NotShared::Nothing) => log!("seed: nothing to share"),
    }
    channel.close();
    log!("exit 0");
    exit::OK
}
