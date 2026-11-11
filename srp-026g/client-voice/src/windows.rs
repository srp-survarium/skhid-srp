//! Owned WinMM devices. A header and its PCM storage stay at stable heap
//! addresses until the driver returns them and unprepares the header.
use crate::{FRAME, RATE, runtime::BufferOrder};
use std::{cell::UnsafeCell, mem::size_of, ptr};
use windows_sys::Win32::Media::Audio::*;

const BUFFERS: usize = 4;
const HEADER_BYTES: u32 = size_of::<WAVEHDR>() as u32;

// windows-sys models WAVEHDR as packed. Put it first at an explicitly
// aligned address so its u32 completion flag is safe to read volatile.
#[repr(C, align(8))]
struct Buffer {
    header: UnsafeCell<WAVEHDR>,
    samples: UnsafeCell<[i16; FRAME]>,
    prepared: bool,
    queued: bool,
}
impl Buffer {
    fn new() -> Box<Self> {
        let buffer = Box::new(Self {
            // SAFETY: all-zero WAVEHDR is the documented initial state.
            header: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            samples: UnsafeCell::new([0; FRAME]),
            prepared: false,
            queued: false,
        });
        // SAFETY: the allocation is stable and not yet given to a driver.
        unsafe {
            (*buffer.header.get()).lpData = buffer.samples.get().cast();
            (*buffer.header.get()).dwBufferLength = (FRAME * size_of::<i16>()) as u32;
        }
        buffer
    }
    fn done(&self) -> bool {
        // SAFETY: the driver owns a prepared, live header. Only its completion
        // flag is read until it has returned ownership of the PCM buffer.
        unsafe { ptr::read_volatile(ptr::addr_of!((*self.header.get()).dwFlags)) & WHDR_DONE != 0 }
    }
    fn ready(&self) -> bool {
        !self.queued || self.done()
    }
}
fn format() -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 1,
        nSamplesPerSec: RATE as u32,
        nAvgBytesPerSec: RATE as u32 * 2,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    }
}

pub struct Output {
    handle: HWAVEOUT,
    buffers: [Option<Box<Buffer>>; BUFFERS],
    order: BufferOrder,
}
impl Output {
    pub fn open() -> Result<Self, u32> {
        let mut handle = ptr::null_mut();
        // SAFETY: output and format are valid for the call; no callback is used.
        let error =
            unsafe { waveOutOpen(&mut handle, WAVE_MAPPER, &format(), 0, 0, CALLBACK_NULL) };
        if error != 0 {
            return Err(error);
        }
        let mut device = Self {
            handle,
            buffers: std::array::from_fn(|_| None),
            order: BufferOrder::new(0),
        };
        for index in 0..BUFFERS {
            let mut buffer = Buffer::new();
            // SAFETY: the boxed header and PCM outlive the device preparation.
            let error = unsafe { waveOutPrepareHeader(handle, buffer.header.get(), HEADER_BYTES) };
            if error != 0 {
                return Err(error);
            }
            buffer.prepared = true;
            device.buffers[index] = Some(buffer);
        }
        // Three quiet frames allow the device to start without an underrun.
        for buffer in device.buffers.iter_mut().flatten().take(3) {
            // SAFETY: prepared header and stable PCM are owned by this device.
            let error = unsafe { waveOutWrite(handle, buffer.header.get(), HEADER_BYTES) };
            if error != 0 {
                return Err(error);
            }
            buffer.queued = true;
        }
        device.order = BufferOrder::new(3);
        Ok(device)
    }
    pub fn pump(&mut self, mut mix: impl FnMut() -> [i16; FRAME]) -> Result<(), u32> {
        for _ in 0..BUFFERS {
            let Some(index) = self.order.next_ready(BUFFERS, |index| {
                self.buffers[index]
                    .as_ref()
                    .is_some_and(|buffer| buffer.ready())
            }) else {
                break;
            };
            let buffer = self.buffers[index]
                .as_mut()
                .expect("all output buffers prepared");
            buffer.queued = false;
            // SAFETY: WHDR_DONE (or never queued) returns the PCM to us. The
            // next write lends the same allocation back to WinMM.
            unsafe {
                *buffer.samples.get() = mix();
            }
            let error = unsafe { waveOutWrite(self.handle, buffer.header.get(), HEADER_BYTES) };
            if error != 0 {
                return Err(error);
            }
            buffer.queued = true;
        }
        Ok(())
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: the handle belongs to this device, and every header remains
        // allocated until reset and unprepare return ownership to us.
        unsafe {
            waveOutReset(self.handle);
        }
        for buffer in self.buffers.iter_mut().filter_map(Option::take) {
            if buffer.prepared
                && unsafe { waveOutUnprepareHeader(self.handle, buffer.header.get(), HEADER_BYTES) }
                    != 0
            {
                // A broken driver may still hold these addresses. Leaking its
                // allocation is safer than freeing memory while it plays it.
                Box::leak(buffer);
            }
        }
        unsafe {
            waveOutClose(self.handle);
        }
    }
}

pub struct Input {
    handle: HWAVEIN,
    buffers: [Option<Box<Buffer>>; BUFFERS],
    order: BufferOrder,
}
impl Input {
    pub fn open() -> Result<Self, u32> {
        let mut handle = ptr::null_mut();
        // SAFETY: valid output pointer and PCM format, no callback.
        let error = unsafe { waveInOpen(&mut handle, WAVE_MAPPER, &format(), 0, 0, CALLBACK_NULL) };
        if error != 0 {
            return Err(error);
        }
        let mut device = Self {
            handle,
            buffers: std::array::from_fn(|_| None),
            order: BufferOrder::new(0),
        };
        for index in 0..BUFFERS {
            let mut buffer = Buffer::new();
            // SAFETY: headers and PCM are at stable addresses until Drop.
            let error = unsafe { waveInPrepareHeader(handle, buffer.header.get(), HEADER_BYTES) };
            if error != 0 {
                return Err(error);
            }
            buffer.prepared = true;
            device.buffers[index] = Some(buffer);
            let buffer = device.buffers[index]
                .as_mut()
                .expect("just inserted a capture buffer");
            let error = unsafe { waveInAddBuffer(handle, buffer.header.get(), HEADER_BYTES) };
            if error != 0 {
                return Err(error);
            }
            buffer.queued = true;
        }
        // SAFETY: every capture header is prepared and queued.
        let error = unsafe { waveInStart(handle) };
        if error != 0 {
            return Err(error);
        }
        Ok(device)
    }
    pub fn pump(&mut self, mut capture: impl FnMut([i16; FRAME])) -> Result<(), u32> {
        for _ in 0..BUFFERS {
            let Some(index) = self.order.next_ready(BUFFERS, |index| {
                self.buffers[index]
                    .as_ref()
                    .is_some_and(|buffer| buffer.done())
            }) else {
                break;
            };
            let buffer = self.buffers[index]
                .as_mut()
                .expect("all input buffers prepared");
            // SAFETY: completion returns both the header and samples to us.
            let recorded = unsafe { (*buffer.header.get()).dwBytesRecorded };
            if recorded == (FRAME * size_of::<i16>()) as u32 {
                capture(unsafe { *buffer.samples.get() });
            }
            unsafe {
                (*buffer.header.get()).dwBytesRecorded = 0;
            }
            let error = unsafe { waveInAddBuffer(self.handle, buffer.header.get(), HEADER_BYTES) };
            if error != 0 {
                return Err(error);
            }
        }
        Ok(())
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        // SAFETY: reset returns the queued headers before unpreparing them.
        unsafe {
            waveInReset(self.handle);
        }
        for buffer in self.buffers.iter_mut().filter_map(Option::take) {
            if buffer.prepared
                && unsafe { waveInUnprepareHeader(self.handle, buffer.header.get(), HEADER_BYTES) }
                    != 0
            {
                Box::leak(buffer);
            }
        }
        unsafe {
            waveInClose(self.handle);
        }
    }
}
