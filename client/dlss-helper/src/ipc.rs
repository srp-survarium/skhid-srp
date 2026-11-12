//! The pipe's messages (PROTOCOL.md): fixed-size little-endian records, the
//! same layout in the 32-bit game and this 64-bit program (fixed-width
//! fields, 8-byte handles). Raw records are read here and turned into typed
//! values once; nothing past this module sees a raw field.

use std::io::{self, Read, Write};
use std::mem::size_of;

pub const MAGIC: u32 = 0x5353_4c44; // "DLSS"
pub const VERSION: u32 = 3;

/// The shared images, in the order of `SetupMsg::img`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Color = 0,
    Depth = 1,
    Motion = 2,
    Output = 3,
}
pub const SLOTS: [Slot; 4] = [Slot::Color, Slot::Depth, Slot::Motion, Slot::Output];

/// A record that crosses the pipe as its bytes.
///
/// # Safety
/// Only for `#[repr(C)]` structs of integers and floats, without padding
/// that matters: every byte pattern is a value.
pub unsafe trait Wire: Sized + Copy {
    fn read_from(r: &mut impl Read) -> io::Result<Self> {
        let mut value = std::mem::MaybeUninit::<Self>::zeroed();
        // SAFETY: Wire types are plain data; zeroed is a value, and so is any byte pattern read over it.
        let bytes = unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast::<u8>(), size_of::<Self>()) };
        r.read_exact(bytes)?;
        Ok(unsafe { value.assume_init() })
    }
    fn write_to(&self, w: &mut impl Write) -> io::Result<()> {
        // SAFETY: Wire types are plain data.
        let bytes = unsafe { std::slice::from_raw_parts((self as *const Self).cast::<u8>(), size_of::<Self>()) };
        w.write_all(bytes)?;
        w.flush()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HelloMsg {
    pub magic: u32,
    pub version: u32,
    pub device_uuid: [u8; 16],
    pub device_luid: [u8; 8],
    pub out_w: u32,
    pub out_h: u32,
    pub quality: i32,
    pub helper_mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HelloReply {
    pub ok: u32,
    pub ngx_result: i32,
    pub render_w: u32,
    pub render_h: u32,
    pub text: [u8; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SharedImageMsg {
    pub handle: u64,
    pub alloc_size: u64,
    pub memory_type: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub usage: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SetupMsg {
    pub img: [SharedImageMsg; 4],
    pub semaphore: u64,
    pub dlss_flags: i32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SetupReply {
    pub ok: u32,
    pub ngx_result: i32,
    pub vram_before_feature: u64,
    pub vram_after_feature: u64,
    pub import_bytes: u64,
    pub text: [u8; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameMsg {
    pub frame: u64,
    pub wait_value: u64,
    pub signal_value: u64,
    pub jitter_x: f32,
    pub jitter_y: f32,
    pub mv_scale_x: f32,
    pub mv_scale_y: f32,
    pub reset: u32,
    /// 0: a frame; 1: quit; 2: a new size (a HelloMsg and a SetupMsg follow).
    pub quit: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FrameStat {
    pub frame: u64,
    pub ts_begin: u64,
    pub ts_eval_begin: u64,
    pub ts_eval_end: u64,
    pub ts_end: u64,
    pub tick_ns: f64,
    pub record_ms: f64,
}

// The layouts of src/probe_ipc.h (PROTOCOL.md), on x86 and x64 alike.
const _: () = assert!(size_of::<HelloMsg>() == 48);
const _: () = assert!(size_of::<HelloReply>() == 272);
const _: () = assert!(size_of::<SharedImageMsg>() == 40);
const _: () = assert!(size_of::<SetupMsg>() == 176);
const _: () = assert!(size_of::<SetupReply>() == 288);
const _: () = assert!(size_of::<FrameMsg>() == 48);
const _: () = assert!(size_of::<FrameStat>() == 56);

unsafe impl Wire for HelloMsg {}
unsafe impl Wire for HelloReply {}
unsafe impl Wire for SetupMsg {}
unsafe impl Wire for SetupReply {}
unsafe impl Wire for FrameMsg {}
unsafe impl Wire for FrameStat {}

/// A reply's text field: UTF-8, cut to 255 bytes on a character boundary, NUL-ended.
pub fn text(s: &str) -> [u8; 256] {
    let mut out = [0u8; 256];
    let mut end = s.len().min(255);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    out[..end].copy_from_slice(&s.as_bytes()[..end]);
    out
}

/// What the helper does with a Hello.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dlss,
    /// The round trip without DLSS (timing only).
    Pass,
}

/// NGX's NVSDK_NGX_PerfQuality_Value (its MaxPerf, Balanced, MaxQuality, UltraPerformance, DLAA).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // DLSS's own name of the mode
pub enum Quality {
    Performance = 0,
    Balanced = 1,
    Quality = 2,
    UltraPerformance = 3,
    Dlaa = 5,
}

impl Quality {
    /// Each side's scale when NGX gives no optimal size.
    pub fn scale(self) -> f64 {
        match self {
            Quality::Performance => 0.5,
            Quality::Balanced => 0.58,
            Quality::Quality => 1.0 / 1.5,
            Quality::UltraPerformance => 1.0 / 3.0,
            Quality::Dlaa => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hello {
    pub device_uuid: [u8; 16],
    pub out: (u32, u32),
    pub quality: Quality,
    pub mode: Mode,
}

impl TryFrom<HelloMsg> for Hello {
    type Error = String;
    fn try_from(m: HelloMsg) -> Result<Self, String> {
        if m.magic != MAGIC || m.version != VERSION {
            return Err(format!("not this protocol (magic {:#x}, version {}; expected {MAGIC:#x}, {VERSION})", m.magic, m.version));
        }
        let quality = match m.quality {
            0 => Quality::Performance,
            1 => Quality::Balanced,
            2 => Quality::Quality,
            3 => Quality::UltraPerformance,
            5 => Quality::Dlaa,
            q => return Err(format!("unknown quality {q}")),
        };
        let mode = match m.helper_mode {
            0 => Mode::Dlss,
            1 => Mode::Pass,
            n => return Err(format!("unknown helper mode {n}")),
        };
        if m.out_w == 0 || m.out_h == 0 || m.out_w > 16384 || m.out_h > 16384 {
            return Err(format!("output size {}x{} out of range", m.out_w, m.out_h));
        }
        Ok(Hello { device_uuid: m.device_uuid, out: (m.out_w, m.out_h), quality, mode })
    }
}

/// One shared image as the game made it.
#[derive(Clone, Copy, Debug)]
pub struct SharedImage {
    pub handle: usize,
    pub alloc_size: u64,
    pub memory_type: u32,
    pub format: i32,
    pub size: (u32, u32),
    pub usage: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Setup {
    pub img: [SharedImage; 4],
    pub semaphore: usize,
    pub dlss_flags: i32,
}

impl TryFrom<SetupMsg> for Setup {
    type Error = String;
    fn try_from(m: SetupMsg) -> Result<Self, String> {
        let mut img = [SharedImage { handle: 0, alloc_size: 0, memory_type: 0, format: 0, size: (0, 0), usage: 0 }; 4];
        for (i, s) in m.img.iter().enumerate() {
            if s.handle == 0 || s.width == 0 || s.height == 0 || s.width > 16384 || s.height > 16384 || s.memory_type >= 32 {
                return Err(format!("shared image {i}: handle {:#x}, {}x{}, memory type {}", s.handle, s.width, s.height, s.memory_type));
            }
            img[i] = SharedImage {
                handle: s.handle as usize,
                alloc_size: s.alloc_size,
                memory_type: s.memory_type,
                format: s.format as i32,
                size: (s.width, s.height),
                usage: s.usage,
            };
        }
        let (color, depth, motion) = (img[0].size, img[1].size, img[2].size);
        if color != depth || color != motion {
            return Err(format!("render-size images differ: colour {color:?}, depth {depth:?}, motion {motion:?}"));
        }
        if m.semaphore == 0 {
            return Err("no timeline semaphore handle".into());
        }
        Ok(Setup { img, semaphore: m.semaphore as usize, dlss_flags: m.dlss_flags })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub number: u64,
    pub wait: u64,
    pub signal: u64,
    pub jitter: (f32, f32),
    pub mv_scale: (f32, f32),
    pub reset: bool,
}

/// What the game asks between frames.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    Frame(Frame),
    Quit,
    /// A new output size: a Hello and a Setup follow.
    Resize,
}

impl TryFrom<FrameMsg> for Command {
    type Error = String;
    fn try_from(m: FrameMsg) -> Result<Self, String> {
        match m.quit {
            1 => Ok(Command::Quit),
            2 => Ok(Command::Resize),
            0 if m.signal_value > m.wait_value => Ok(Command::Frame(Frame {
                number: m.frame,
                wait: m.wait_value,
                signal: m.signal_value,
                jitter: (m.jitter_x, m.jitter_y),
                mv_scale: (m.mv_scale_x, m.mv_scale_y),
                reset: m.reset != 0,
            })),
            0 => Err(format!("frame {}: signal value {} not above the wait value {}", m.frame, m.signal_value, m.wait_value)),
            q => Err(format!("unknown command {q}")),
        }
    }
}
