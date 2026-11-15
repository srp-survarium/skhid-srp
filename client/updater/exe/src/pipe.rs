//! This end of the launcher's named pipe (and of the game's, in seed mode:
//! the same framing, `crate::seed`).
//!
//! The launcher makes the pipe and passes its name; the updater opens it
//! once. Everything here is done by the one thread the program has: it
//! writes a message when it has one, and between its steps it looks whether
//! the launcher has written (`PeekNamedPipe` never waits).

use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use updater_logic::pipe::{FromLauncher, Reader, ToLauncher, frame};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows_sys::Win32::System::Pipes::{PeekNamedPipe, WaitNamedPipeW};

use crate::win::{Code, last_error, wide};

pub struct Pipe {
    handle: HANDLE,
    reader: Reader,
    /// `false` once a read or write failed: the launcher is gone.
    open: bool,
}

impl Pipe {
    /// Opens the launcher's pipe. If its one instance is taken for a moment,
    /// waits for it (five seconds at most).
    pub fn connect(name: &str) -> Result<Self, Code> {
        let name = wide(name);
        for _ in 0..2 {
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    null(),
                    OPEN_EXISTING,
                    0,
                    null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                return Ok(Self {
                    handle,
                    reader: Reader::default(),
                    open: true,
                });
            }
            let code = last_error();
            if code != ERROR_PIPE_BUSY || unsafe { WaitNamedPipeW(name.as_ptr(), 5000) } == 0 {
                return Err(code);
            }
        }
        Err(ERROR_PIPE_BUSY)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Writes one message. `false` when the launcher is gone.
    pub fn send(&mut self, message: ToLauncher) -> bool {
        self.send_bytes(&frame(message))
    }

    /// Writes one framed message. `false` when the other end is gone.
    pub fn send_bytes(&mut self, bytes: &[u8]) -> bool {
        if !self.open {
            return false;
        }
        let mut sent = 0;
        while sent < bytes.len() {
            let mut written = 0_u32;
            let ok = unsafe {
                WriteFile(
                    self.handle,
                    bytes[sent..].as_ptr(),
                    (bytes.len() - sent) as u32,
                    &mut written,
                    null_mut(),
                )
            };
            if ok == 0 || written == 0 {
                self.open = false;
                return false;
            }
            sent += written as usize;
        }
        true
    }

    /// The messages the launcher has written since the last look.
    pub fn receive(&mut self) -> Vec<FromLauncher> {
        self.fill();
        let mut messages = Vec::new();
        while let Some(message) = self.reader.message() {
            messages.push(message);
        }
        messages
    }

    /// The bytes of each message written since the last look (the id
    /// first).
    pub fn payloads(&mut self) -> Vec<Vec<u8>> {
        self.fill();
        core::iter::from_fn(|| self.reader.payload()).collect()
    }

    /// Takes what the other end wrote, without waiting.
    fn fill(&mut self) {
        while self.open {
            let mut waiting = 0_u32;
            let ok = unsafe {
                PeekNamedPipe(
                    self.handle,
                    null_mut(),
                    0,
                    null_mut(),
                    &mut waiting,
                    null_mut(),
                )
            };
            if ok == 0 {
                self.open = false;
                break;
            }
            if waiting == 0 {
                break;
            }
            let mut buffer = [0_u8; 512];
            let mut read = 0_u32;
            let want = (waiting as usize).min(buffer.len()) as u32;
            let ok = unsafe {
                ReadFile(
                    self.handle,
                    buffer.as_mut_ptr(),
                    want,
                    &mut read,
                    null_mut(),
                )
            };
            if ok == 0 || read == 0 {
                self.open = false;
                break;
            }
            self.reader.push(&buffer[..read as usize]);
        }
    }

    /// Waits until the launcher has read everything written, then closes.
    pub fn close(mut self) {
        if self.open {
            unsafe {
                FlushFileBuffers(self.handle);
            }
        }
        self.open = false;
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
