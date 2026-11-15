//! `updater_skhid.exe seed <pipe>`: sharing while the game keeps the pipe
//! open (spec: `srp-026g/docs/notes/torrent-downloads.md`, "Sharing from the
//! game"). No window, no lock, nothing fetched or installed.

use alloc::format;

use updater_logic::seed::{Event, FromGame, GamePipe, Meter, Stats, Tick, frame};

use crate::install::Layout;
use crate::log::log;
use crate::pipe::Pipe;
use crate::{exit, log as logging, torrent, win};

/// Its own: a seed runs at every lobby and would push the launcher's
/// updater's logs out of the kept 100.
const LOG_FOLDER: &str = "updater_skhid.seed-logs";
/// How often "Allow torrent" is read again while it shares.
const RECHECK_MS: u64 = 10_000;

/// `pipe`: `None` when the command line was not the game's.
pub fn run(pipe: Option<GamePipe>) -> u32 {
    let Ok(layout) = Layout::find() else {
        return exit::FAILED;
    };
    let logs = format!("{}\\{LOG_FOLDER}", layout.exe_dir);
    let _ = win::create_folders(&logs);
    logging::start(&logs);
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
    // A folder the Skhid launcher never reset has the original game's
    // default `on` saved, which no player chose.
    if !win::launcher_torrent_allowed() {
        log!("seed: \"Allow torrent\" is not on (user.cfg, after the launcher's reset): nothing shared");
        channel.close();
        log!("exit 0");
        return exit::OK;
    }
    let mut stats = Stats::Hidden;
    let mut meter = Meter::new(win::ticks(), 0);
    let mut looked = win::ticks();
    let mut allowed = true;
    let shared = torrent::share_while(&layout, None, &mut |sharing| {
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
        // The launcher may untick the box while the game runs.
        if now.saturating_sub(looked) >= RECHECK_MS {
            looked = now;
            allowed = win::launcher_torrent_allowed();
            if !allowed {
                log!("seed: \"Allow torrent\" was turned off: stopped");
            }
        }
        allowed && channel.is_open()
    });
    match shared {
        Ok(()) => log!("seed: the game closed the pipe: done"),
        Err(torrent::NotShared::PortBusy) => {
            log!("seed: the kept port is held (another updater shares): not shared");
        }
        Err(torrent::NotShared::Nothing) => log!("seed: nothing to share"),
    }
    channel.close();
    log!("exit 0");
    exit::OK
}
