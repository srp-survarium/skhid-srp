//! Before the launcher's PLAY starts the game, the games an earlier start of
//! this install left running are closed (`updater_logic::leftover`): every
//! process of this session whose image is this install's
//! `survarium_skhid.exe` (its whole path). Their visible windows are sent
//! WM_CLOSE (the game quits; its crash box, BugTrap's dialog in the game's
//! own process, closes and lets the process end); what still runs after
//! `ASK_MS` is terminated and waited for. Games of other folders and other
//! programs are never opened beyond reading their path.

use alloc::vec::Vec;

use updater_logic::leftover::{ASK_MS, END_MS, Event, Failure, GameExe, Leftover, Step};
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
    WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
};

use crate::log::log;
use crate::win;

/// An earlier game, open to be waited for and terminated.
struct Game {
    id: u32,
    handle: HANDLE,
}

impl Drop for Game {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Closes the earlier games of `exe` (the game exe PLAY is about to start).
/// `false`: one of them may still run.
pub fn close(exe: &str) -> bool {
    let exe = GameExe::new(&long_path(exe));
    let mut closed = true;
    let mut games = Vec::new();
    for found in find(&exe) {
        match found {
            Ok(game) => games.push(game),
            Err((id, code)) => {
                log!("play: earlier game, process {id}: {:?}", Leftover::Failed(Failure::Refused(code)));
                closed = false;
            }
        }
    }
    if games.is_empty() {
        return closed;
    }
    let windows = ask(&games);
    log!("play: {} earlier game(s) of this folder still run; {windows} window(s) asked to close", games.len());
    let asked_until = win::ticks() + u64::from(ASK_MS);
    for game in &games {
        let (mut state, mut step) = Leftover::asked();
        loop {
            let event = match step {
                Step::AwaitClose => wait(game, asked_until.saturating_sub(win::ticks()) as u32),
                Step::Terminate => terminate(game),
                Step::AwaitEnd => wait(game, END_MS),
                Step::Done => break,
            };
            (state, step) = state.on(event);
        }
        log!("play: earlier game, process {}: {state:?}", game.id);
        closed &= !matches!(state, Leftover::Failed(_));
    }
    closed
}

/// `path` with its folders' long names, as Windows gives a process's image.
fn long_path(path: &str) -> alloc::string::String {
    let short = win::wide(path);
    let mut long = alloc::vec![0_u16; 32768];
    let length = unsafe { GetLongPathNameW(short.as_ptr(), long.as_mut_ptr(), long.len() as u32) } as usize;
    if length == 0 || length >= long.len() {
        return path.into();
    }
    win::from_wide(&long[..length])
}

/// The processes of this session whose image is `exe`: open, or the error
/// Windows refused one with.
fn find(exe: &GameExe) -> Vec<Result<Game, (u32, u32)>> {
    let mut found = Vec::new();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        log!("play: cannot list the running programs ({})", win::error_text(win::last_error()));
        return found;
    }
    let own = unsafe { GetCurrentProcessId() };
    let session = session_of(own);
    let mut entry: PROCESSENTRY32W = unsafe { core::mem::zeroed() };
    entry.dwSize = core::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while more {
        let id = entry.th32ProcessID;
        if id != own && exe.may_be(&win::from_wide(&entry.szExeFile)) && session_of(id) == session {
            found.extend(open(exe, id));
        }
        more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe {
        CloseHandle(snapshot);
    }
    found
}

fn session_of(id: u32) -> Option<u32> {
    let mut session = 0;
    (unsafe { ProcessIdToSessionId(id, &mut session) } != 0).then_some(session)
}

/// Process `id`, when its image is `exe`.
fn open(exe: &GameExe, id: u32) -> Option<Result<Game, (u32, u32)>> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, 0, id) };
    if !handle.is_null() {
        let game = Game { id, handle };
        return image_is(exe, game.handle).then_some(Ok(game));
    }
    // Not to be terminated by this user (another's, or run as
    // administrator): a failure only when it is this game.
    let refused = win::last_error();
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, id) };
    if handle.is_null() {
        log!("play: cannot open process {id} ({}), which may be the game", win::error_text(refused));
        return None;
    }
    let ours = image_is(exe, handle);
    unsafe {
        CloseHandle(handle);
    }
    ours.then_some(Err((id, refused)))
}

fn image_is(exe: &GameExe, process: HANDLE) -> bool {
    let mut image = alloc::vec![0_u16; 32768];
    let mut length = image.len() as u32;
    let read = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, image.as_mut_ptr(), &mut length) } != 0;
    read && exe.is(&win::from_wide(&image[..length as usize]))
}

/// WM_CLOSE to the visible windows of `games`; how many.
fn ask(games: &[Game]) -> usize {
    struct Asking {
        ids: Vec<u32>,
        windows: usize,
    }
    unsafe extern "system" fn each(window: HWND, asking: LPARAM) -> i32 {
        let asking = unsafe { &mut *(asking as *mut Asking) };
        let mut id = 0;
        unsafe { GetWindowThreadProcessId(window, &mut id) };
        if asking.ids.contains(&id) && unsafe { IsWindowVisible(window) } != 0 {
            unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
            asking.windows += 1;
        }
        1
    }
    let mut asking = Asking { ids: games.iter().map(|game| game.id).collect(), windows: 0 };
    unsafe { EnumWindows(Some(each), &mut asking as *mut Asking as LPARAM) };
    asking.windows
}

fn wait(game: &Game, milliseconds: u32) -> Event {
    match unsafe { WaitForSingleObject(game.handle, milliseconds) } {
        WAIT_OBJECT_0 => Event::Ended,
        WAIT_TIMEOUT => Event::Running,
        _ => Event::Refused(win::last_error()),
    }
}

fn terminate(game: &Game) -> Event {
    if unsafe { TerminateProcess(game.handle, 1) } != 0 {
        return Event::Terminated;
    }
    let code = win::last_error();
    // Ended between the wait and now: Windows refuses a process that has.
    match wait(game, 0) {
        Event::Ended => Event::Ended,
        _ => Event::Refused(code),
    }
}
