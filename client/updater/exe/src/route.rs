//! The player's route (`updater_logic::route`) and why the server cannot be
//! reached (`updater_logic::diagnosis`). This module is the one owner of the
//! choice: read once from `<Documents>\survarium\launcher_skhid.route`, the
//! first time decided by the Windows region and saved at once (so that it is
//! the player's from then on, shown and changed in the launcher's "Relay" row), and
//! changed only by that row or a block found ([`set`]). Beside it, the
//! route PLAY last started a game by after its check
//! (`launcher_skhid.route_played`, [`played`]): the next check's evidence.

use alloc::format;
use alloc::string::String;
use core::ffi::c_void;
use core::ptr::null_mut;
use std::net::ToSocketAddrs;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use relay_client::{Config, Selection};
use updater_logic::diagnosis::{Diagnosis, Internet, Reach, Via, diagnose};
use updater_logic::route::{self as routing, CHOICE_FILE, Choice, Event, PLAYED_FILE, Probed, Shown};
use updater_logic::url::Url;

use crate::log::log;
use crate::{http, win};

static CHOICE: Mutex<Option<Choice>> = Mutex::new(None);

fn folder() -> Option<String> {
    win::documents().map(|documents| format!("{documents}\\survarium"))
}

/// The player's choice. Only asked in a download with a relay.
pub fn choice() -> Choice {
    let mut held = CHOICE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(choice) = *held {
        return choice;
    }
    let folder = folder();
    let saved = folder
        .as_deref()
        .and_then(|folder| win::read_file(&format!("{folder}\\{CHOICE_FILE}"), 64).ok())
        .and_then(|text| Choice::parse(&text));
    let choice = saved.unwrap_or_else(|| {
        let nation = region();
        let first = Choice::first(nation);
        log!("route: none chosen yet; Windows region {nation}: {}", first.word());
        save(folder.as_deref(), first);
        first
    });
    *held = Some(choice);
    choice
}

/// The row's pick.
pub fn set(choice: Choice) {
    save(folder().as_deref(), choice);
    *CHOICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(choice);
}

/// The direct route is blocked while the relay answers, and no row shows
/// it (PLAY, the updater by hand): the choice the row would move to, saved.
pub fn blocked() {
    if let Some(choice) = Shown::start(choice()).shown.on(Event::Blocked).save {
        set(choice);
    }
}

fn save(folder: Option<&str>, choice: Choice) {
    let Some(folder) = folder else {
        log!("route: no Documents folder: {} not saved", choice.word());
        return;
    };
    let written = win::create_folders(folder).and_then(|()| {
        win::write_file(&format!("{folder}\\{CHOICE_FILE}"), format!("{}\r\n", choice.word()).as_bytes())
    });
    match written {
        Ok(()) => log!("route: {} saved", choice.word()),
        Err(code) => log!("route: {} not saved: {}", choice.word(), win::error_text(code)),
    }
}

/// The route PLAY last started a game by after its check.
pub fn played() -> Option<Via> {
    let text = win::read_file(&format!("{}\\{PLAYED_FILE}", folder()?), 64).ok()?;
    routing::played(&text)
}

/// PLAY starts a game by `via`, which its check found.
pub fn set_played(via: Via) {
    if played() == Some(via) {
        return;
    }
    let word = routing::played_word(via);
    let Some(folder) = folder() else {
        log!("route: no Documents folder: played {word} not saved");
        return;
    };
    let written = win::create_folders(&folder)
        .and_then(|()| win::write_file(&format!("{folder}\\{PLAYED_FILE}"), format!("{word}\r\n").as_bytes()));
    match written {
        Ok(()) => log!("route: played {word} saved"),
        Err(code) => log!("route: played {word} not saved: {}", win::error_text(code)),
    }
}

/// The check of the routes (`relay_client::select`), with the last game's
/// route as its evidence; `who` asked, for the log, which gets every round
/// and each probe that failed.
pub fn check(config: &Config, who: &str) -> Selection {
    let last = played();
    let report = relay_client::select(config, last);
    log!(
        "{who}: route check (last game {}): {report} -> {:?}",
        last.map_or("none", routing::played_word),
        report.selection
    );
    report.selection
}

/// Whether the game's ports answer now by the player's route: one round,
/// what a wait for a server that restarts asks.
pub fn game_answers(config: &Config) -> bool {
    let up = |via| relay_client::answers(config, via);
    match choice() {
        Choice::Direct => up(Via::Direct),
        Choice::Relay => up(Via::Relay),
        Choice::Automatic => up(Via::Direct) || up(Via::Relay),
    }
}

/// Windows' region of this user (`GetUserGeoID(GEOCLASS_NATION)`): what
/// Settings > Time & language > Region says, under Wine the prefix's.
fn region() -> i32 {
    use windows_sys::Win32::Globalization::{GEOCLASS_NATION, GetUserGeoID};
    unsafe { GetUserGeoID(GEOCLASS_NATION) }
}

/// The probe of PLAY's route, in a thread of its own: what the row shows.
pub fn probe(config: &'static Config) -> mpsc::Receiver<Probed> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let probed = match check(config, "route") {
            Selection::Direct(_) => Probed::Direct,
            Selection::Relay(_) => Probed::Relay,
            Selection::Unreachable => Probed::Neither,
        };
        let _ = send.send(probed);
    });
    receive
}

/// The route of the update's HTTPS: the player's; direct without a relay.
fn site_route() -> Choice {
    match crate::play::config() {
        Ok(Some(_)) => choice(),
        _ => Choice::Direct,
    }
}

/// Why the server's site did not give what the update asked (its HTTPS, by
/// the player's route). `None`: it answers now.
pub fn diagnose_site(server: &Url) -> Option<Diagnosis> {
    let direct = http::reach(server, Via::Direct).unwrap_or(Reach::Silent);
    let relayed = http::reach(server, Via::Relay);
    log!("route: the site answers: direct {direct:?}, through the relay {relayed:?}");
    told(site_route(), direct, relayed, &server.host)
}

/// Whether the site answers now by the player's route: what a wait for a
/// server that restarts asks.
pub fn site_answers(server: &Url) -> bool {
    let up = |via| http::reach(server, via) == Some(Reach::Up);
    match site_route() {
        Choice::Direct => up(Via::Direct),
        Choice::Relay => up(Via::Relay),
        Choice::Automatic => up(Via::Direct) || up(Via::Relay),
    }
}

/// Why PLAY's probe did not get the game's ports by the player's route
/// (direct, or Automatic), given what the probe found.
pub fn diagnose_game(config: &Config, selection: &Selection) -> Diagnosis {
    let server = Url {
        secure: true,
        host: config.direct.host().into(),
        port: 443,
        prefix: String::new(),
    };
    // The game's ports did not answer: the site tells the rest.
    let of_site = |reach: Option<Reach>| match reach {
        Some(Reach::Up) => Reach::Partly,
        reach => reach.unwrap_or(Reach::Silent),
    };
    let direct = match selection {
        Selection::Direct(_) => Reach::Up,
        _ => of_site(http::reach(&server, Via::Direct)),
    };
    let relayed = match selection {
        Selection::Relay(_) => Reach::Up,
        _ => of_site(http::reach(&server, Via::Relay)),
    };
    log!("route: the game answers: direct {direct:?}, through the relay {relayed:?}");
    // `None` (the route answers) is not what the probe just found.
    told(choice(), direct, Some(relayed), &server.host).unwrap_or(Diagnosis::Unclear)
}

fn told(route: Choice, direct: Reach, relayed: Option<Reach>, host: &str) -> Option<Diagnosis> {
    let diagnosis = diagnose(route, direct, relayed, || internet(host));
    log!("route: diagnosis {diagnosis:?}");
    diagnosis
}

/// Whether this PC is online: not when Windows says it is not (Network
/// List Manager, the network icon's test), nor when `host`'s name is not
/// found. Where Windows cannot say (Wine may not), the name alone tells.
fn internet(host: &str) -> Internet {
    let windows = windows_online();
    if windows == Some(false) {
        log!("route: Windows says this PC is offline");
        return Internet::Offline;
    }
    let found = name_found(host);
    log!("route: Windows says online: {windows:?}; {host} found by DNS: {found}");
    if found { Internet::Online } else { Internet::Offline }
}

fn name_found(host: &str) -> bool {
    let host = String::from(host);
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = send.send((host.as_str(), 443).to_socket_addrs().is_ok_and(|mut found| found.next().is_some()));
    });
    receive.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
}

/// `INetworkListManager::GetConnectivity`: IPv4 or IPv6 internet. `None`
/// when Windows cannot say (no COM, no Network List Manager: Wine may have
/// none).
fn windows_online() -> Option<bool> {
    use windows_sys::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    };
    use windows_sys::core::GUID;
    const CLSID_NETWORK_LIST_MANAGER: GUID = GUID::from_u128(0xDCB00C01_570F_4A9B_8D69_199FDBA5723B);
    const IID_INETWORK_LIST_MANAGER: GUID = GUID::from_u128(0xDCB00000_570F_4A9B_8D69_199FDBA5723B);
    const IPV4_INTERNET: i32 = 0x40;
    const IPV6_INTERNET: i32 = 0x400;
    // IUnknown (3), IDispatch (4), GetNetworks, GetNetwork,
    // GetNetworkConnections, GetNetworkConnection, IsConnectedToInternet,
    // IsConnected, then GetConnectivity.
    const GET_CONNECTIVITY: usize = 13;
    const RELEASE: usize = 2;
    type GetConnectivity = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
    // A thread of its own: COM as it likes it, whatever this one has.
    std::thread::spawn(|| unsafe {
        let initialized = CoInitializeEx(null_mut(), COINIT_MULTITHREADED as u32) >= 0;
        let mut manager: *mut c_void = null_mut();
        let mut online = None;
        if CoCreateInstance(&CLSID_NETWORK_LIST_MANAGER, null_mut(), CLSCTX_ALL,
            &IID_INETWORK_LIST_MANAGER, &mut manager) >= 0 && !manager.is_null()
        {
            let table = *(manager as *const *const usize);
            let get: GetConnectivity = core::mem::transmute(*table.add(GET_CONNECTIVITY));
            let mut connectivity = 0;
            if get(manager, &mut connectivity) >= 0 {
                online = Some(connectivity & (IPV4_INTERNET | IPV6_INTERNET) != 0);
            }
            let release: Release = core::mem::transmute(*table.add(RELEASE));
            release(manager);
        }
        if initialized {
            CoUninitialize();
        }
        online
    })
    .join()
    .ok()
    .flatten()
}

/// A message box's words: what the player is told, in the launcher's language.
pub fn words(diagnosis: Diagnosis, language: Option<u32>) -> (String, String) {
    let language = language.filter(|&n| n < 5).unwrap_or(0);
    (
        format!("Skhid: {}", diagnosis.title(0)),
        format!("{}\n{}", diagnosis.title(language), diagnosis.advice(language)),
    )
}
