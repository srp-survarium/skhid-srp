//! A stand-in for the game and for the launcher in the updater's tests.
//! It writes `<its own path>.started.txt` with its command line and its
//! working folder (UTF-16, a line each) and ends. It carries the mark the
//! launcher finds the game's version by.
//!
//! Two more ways to start it, for the tests under Wine, where Python cannot
//! hold a Windows file or make a Windows pipe:
//!
//! - `fake-exe.exe hold <file> <held> <release>`: as the game holds
//!   resources.db, or an antivirus a file it scans: opens `<file>` (as soon
//!   as it can) for reading, others may read and write it but not delete or
//!   rename it, makes the file `<held>`, and lets go once the file
//!   `<release>` is there.
//! - `fake-exe.exe launcher <name> <updater> <out> <stop>`: the launcher's
//!   end of its pipe: makes `\\.\pipe\vostok_updater_pipe-<name>`, starts
//!   `<updater> update <pipe>`, writes every byte the updater sends into
//!   `<out>`, says stop (the launcher's abort) once the file `<stop>` is
//!   there, and ends with the updater's exit code once it is gone.
#![windows_subsystem = "windows"]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows_sys::Win32::System::Environment::{GetCommandLineW, GetCurrentDirectoryW};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, ExitProcess, GetExitCodeProcess, INFINITE, PROCESS_INFORMATION, STARTUPINFOW,
    Sleep, WaitForSingleObject,
};

/// What `get_local_version` of the launcher looks for in the game exe: the
/// 32 bytes of the mark, then the version.
#[used]
#[unsafe(no_mangle)]
static VERSION_MARK: [u8; 48] = *b"survarium_version_header_mark!!\x000.26g0-fake\0\0\0\0\0";

// A panic ends the program with 3, as the updater's does.
fn fatal_panic(_: &std::panic::PanicHookInfo) {
    unsafe { ExitProcess(3) }
}

unsafe fn length(text: *const u16) -> usize {
    let mut length = 0;
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    length
}

unsafe fn write(file: HANDLE, bytes: *const u8, size: usize) {
    let mut written = 0;
    unsafe { WriteFile(file, bytes, size as u32, &mut written, null_mut()) };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}

fn exists(path: &str) -> bool {
    unsafe { GetFileAttributesW(wide(path).as_ptr()) != INVALID_FILE_ATTRIBUTES }
}

fn create(path: &str, share: u32) -> HANDLE {
    unsafe {
        CreateFileW(
            wide(path).as_ptr(),
            GENERIC_WRITE,
            share,
            null(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    }
}

/// The game's resources.db, or an antivirus's scan, until told to let go.
fn hold(path: &str, held: &str, release: &str) -> u32 {
    let name = wide(path);
    let mut file = INVALID_HANDLE_VALUE;
    for _ in 0..60_000 {
        file = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if file != INVALID_HANDLE_VALUE {
            break;
        }
        unsafe { Sleep(1) };
    }
    if file == INVALID_HANDLE_VALUE {
        return 4;
    }
    unsafe { CloseHandle(create(held, 0)) };
    for _ in 0..12_000 {
        if exists(release) {
            break;
        }
        unsafe { Sleep(10) };
    }
    unsafe { CloseHandle(file) };
    0
}

/// The launcher's end of the pipe, and the updater it starts.
fn launcher(name: &str, updater: &str, out: &str, stop: &str) -> u32 {
    let pipe_name = alloc::format!("\\\\.\\pipe\\vostok_updater_pipe-{name}");
    let pipe = unsafe {
        CreateNamedPipeW(
            wide(&pipe_name).as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            0,
            1,
            0x10000,
            0x10000,
            0,
            null(),
        )
    };
    if pipe == INVALID_HANDLE_VALUE {
        return 5;
    }
    let folder = match updater.rfind('\\') {
        Some(at) => &updater[..at],
        None => ".",
    };
    let mut command = wide(&alloc::format!("\"{updater}\" \"update\" \"{pipe_name}\""));
    let folder = wide(folder);
    let mut startup: STARTUPINFOW = unsafe { core::mem::zeroed() };
    startup.cb = core::mem::size_of::<STARTUPINFOW>() as u32;
    let mut process: PROCESS_INFORMATION = unsafe { core::mem::zeroed() };
    let started = unsafe {
        CreateProcessW(
            null(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            0,
            null(),
            folder.as_ptr(),
            &startup,
            &mut process,
        )
    };
    if started == 0 {
        return 6;
    }
    if unsafe { ConnectNamedPipe(pipe, null_mut()) } == 0
        && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED
    {
        return 7;
    }
    let output = create(out, FILE_SHARE_READ);
    let mut stopped = false;
    let mut buffer = [0_u8; 4096];
    loop {
        let mut waiting = 0_u32;
        let ok =
            unsafe { PeekNamedPipe(pipe, null_mut(), 0, null_mut(), &mut waiting, null_mut()) };
        if ok == 0 {
            break;
        }
        if waiting > 0 {
            let mut read = 0_u32;
            let want = waiting.min(buffer.len() as u32);
            let ok = unsafe { ReadFile(pipe, buffer.as_mut_ptr(), want, &mut read, null_mut()) };
            if ok == 0 {
                break;
            }
            unsafe { write(output, buffer.as_ptr(), read as usize) };
            continue;
        }
        if !stopped && exists(stop) {
            // The launcher's abort: a frame of one byte, 0x1B.
            let abort = [1_u8, 0, 0x1b];
            unsafe { write(pipe, abort.as_ptr(), abort.len()) };
            stopped = true;
        }
        unsafe { Sleep(5) };
    }
    unsafe {
        CloseHandle(output);
        CloseHandle(pipe);
    }
    let mut code = 8;
    unsafe {
        if WaitForSingleObject(process.hProcess, INFINITE) == WAIT_OBJECT_0 {
            GetExitCodeProcess(process.hProcess, &mut code);
        }
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }
    code
}

fn main() {
    std::panic::set_hook(Box::new(fatal_panic));
    unsafe {
        let command = GetCommandLineW();
        let line = String::from_utf16_lossy(core::slice::from_raw_parts(command, length(command)));
        let arguments = updater_logic::args::split(&line);
        let argument = |at: usize| arguments.get(at).map_or("", String::as_str);
        match argument(1) {
            "hold" => ExitProcess(hold(argument(2), argument(3), argument(4))),
            "launcher" => ExitProcess(launcher(argument(2), argument(3), argument(4), argument(5))),
            _ => {}
        }
        let mut path = [0_u16; 1024];
        let used = GetModuleFileNameW(null_mut(), path.as_mut_ptr(), 1000) as usize;
        for (index, unit) in ".started.txt\0".encode_utf16().enumerate() {
            path[used + index] = unit;
        }
        let file = CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            0,
            null(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        );
        if file == INVALID_HANDLE_VALUE {
            ExitProcess(2);
        }
        let line_end = [0x0D_u16, 0x0A];
        let mark = [0xFEFF_u16];
        write(file, mark.as_ptr().cast(), 2);
        write(file, command.cast(), length(command) * 2);
        write(file, line_end.as_ptr().cast(), 4);
        let mut folder = [0_u16; 1024];
        let used = GetCurrentDirectoryW(1024, folder.as_mut_ptr()) as usize;
        write(file, folder.as_ptr().cast(), used * 2);
        write(file, line_end.as_ptr().cast(), 4);
        CloseHandle(file);
        // The mark must stay in the file.
        ExitProcess(u32::from(VERSION_MARK[0] != b's'));
    }
}
