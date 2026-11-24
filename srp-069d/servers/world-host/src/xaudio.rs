//! Null XAudio2 2.7: a world host has no audio device, but the client treats a failed XAudio2 creation as fatal
//! ("Sound initialization FAILED. Can not create XAudio2 interface." -> MessageBoxA -> exit). This object is a
//! working, silent XAudio2: one device (stereo, 48 kHz), voices that accept every call and play nothing.
//! Interface layouts: XAudio2 2.7 (DirectX SDK June 2010, CLSID 5a508685-a254-4fba-9b82-9a24b00306af):
//! IXAudio2 = QueryInterface, AddRef, Release, GetDeviceCount, GetDeviceDetails, Initialize, RegisterForCallbacks,
//! UnregisterForCallbacks, CreateSourceVoice, CreateSubmixVoice, CreateMasteringVoice, StartEngine, StopEngine,
//! CommitChanges, GetPerformanceData, SetDebugConfiguration. IXAudio2Voice = 19 methods (GetVoiceDetails ..
//! DestroyVoice); IXAudio2SourceVoice adds Start, Stop, SubmitSourceBuffer, FlushSourceBuffers, Discontinuity,
//! ExitLoop, GetState, SetFrequencyRatio, GetFrequencyRatio. Every method's first call is logged.
#![allow(non_snake_case)]

use crate::hlog;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

type A = u64;

pub const CLSID_XAUDIO2_27: [u8; 16] = [0x85, 0x86, 0x50, 0x5a, 0x54, 0xa2, 0xba, 0x4f, 0x9b, 0x82, 0x9a, 0x24, 0xb0, 0x03, 0x06, 0xaf];
pub const CLSID_XAUDIO2_27_DEBUG: [u8; 16] = [0x34, 0xa3, 0xdb, 0xdb, 0x4d, 0xf0, 0x2a, 0x45, 0x8d, 0xa5, 0x5c, 0x89, 0x3e, 0x96, 0x8a, 0x5f];

const CHANNELS: u32 = 2;
const RATE: u32 = 48000;
static VOICES: AtomicU64 = AtomicU64::new(0);

fn once(name: &str) {
    static SEEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let mut s = SEEN.lock().unwrap();
    if !s.iter().any(|x| x == name) {
        s.push(name.to_string());
        hlog!("null XAudio2: first call {name}");
    }
}

unsafe extern "win64" fn x_query_interface(this: A, _iid: A, ppv: A) -> A {
    once("IXAudio2::QueryInterface");
    if ppv != 0 {
        unsafe { (ppv as *mut u64).write_unaligned(this) };
    }
    0
}
unsafe extern "win64" fn x_add_ref(_this: A) -> A {
    1
}
unsafe extern "win64" fn x_release(_this: A) -> A {
    once("IXAudio2::Release");
    0
}
unsafe extern "win64" fn x_get_device_count(_this: A, count: A) -> A {
    once("IXAudio2::GetDeviceCount");
    if count != 0 {
        unsafe { (count as *mut u32).write_unaligned(1) };
    }
    0
}
/// XAUDIO2_DEVICE_DETAILS: DeviceID WCHAR[256], DisplayName WCHAR[256], Role u32 at 0x400, WAVEFORMATEXTENSIBLE at
/// 0x404 (40 bytes).
unsafe extern "win64" fn x_get_device_details(_this: A, index: A, d: A) -> A {
    once("IXAudio2::GetDeviceDetails");
    if index != 0 {
        return 0x8876_0004; // XAUDIO2_E_INVALID_CALL
    }
    unsafe {
        std::ptr::write_bytes(d as *mut u8, 0, 0x404 + 40);
        for (i, c) in "host-null-audio".encode_utf16().enumerate() {
            ((d + 2 * i as u64) as *mut u16).write_unaligned(c);
            ((d + 0x200 + 2 * i as u64) as *mut u16).write_unaligned(c);
        }
        ((d + 0x400) as *mut u32).write_unaligned(0xF); // GlobalDefaultDevice (all default roles)
        let f = d + 0x404;
        (f as *mut u16).write_unaligned(0xFFFE); // WAVE_FORMAT_EXTENSIBLE
        ((f + 2) as *mut u16).write_unaligned(CHANNELS as u16);
        ((f + 4) as *mut u32).write_unaligned(RATE);
        ((f + 8) as *mut u32).write_unaligned(RATE * CHANNELS * 4);
        ((f + 12) as *mut u16).write_unaligned((CHANNELS * 4) as u16);
        ((f + 14) as *mut u16).write_unaligned(32);
        ((f + 16) as *mut u16).write_unaligned(22);
        ((f + 18) as *mut u16).write_unaligned(32);
        ((f + 20) as *mut u32).write_unaligned(3); // SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT
        // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT 00000003-0000-0010-8000-00aa00389b71
        let g: [u8; 16] = [3, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71];
        std::ptr::copy_nonoverlapping(g.as_ptr(), (f + 24) as *mut u8, 16);
    }
    0
}
unsafe extern "win64" fn x_ok(_this: A) -> A {
    once("IXAudio2 (no-op method)");
    0
}
unsafe extern "win64" fn x_create_source_voice(_this: A, out: A, fmt: A) -> A {
    once("IXAudio2::CreateSourceVoice");
    let (ch, rate) = if fmt != 0 { unsafe { (((fmt + 2) as *const u16).read_unaligned() as u32, ((fmt + 4) as *const u32).read_unaligned()) } } else { (CHANNELS, RATE) };
    unsafe { (out as *mut u64).write_unaligned(voice(true, ch, rate)) };
    0
}
unsafe extern "win64" fn x_create_submix_voice(_this: A, out: A, ch: A, rate: A) -> A {
    once("IXAudio2::CreateSubmixVoice");
    unsafe { (out as *mut u64).write_unaligned(voice(false, ch as u32, rate as u32)) };
    0
}
unsafe extern "win64" fn x_create_mastering_voice(_this: A, out: A, ch: A, rate: A) -> A {
    once("IXAudio2::CreateMasteringVoice");
    let ch = if ch as u32 == 0 { CHANNELS } else { ch as u32 };
    let rate = if rate as u32 == 0 { RATE } else { rate as u32 };
    unsafe { (out as *mut u64).write_unaligned(voice(false, ch, rate)) };
    0
}
unsafe extern "win64" fn x_get_performance_data(_this: A, p: A) -> A {
    once("IXAudio2::GetPerformanceData");
    if p != 0 {
        unsafe { std::ptr::write_bytes(p as *mut u8, 0, 0x50) };
    }
    0
}

// ---- voices: object = {vtable, InputChannels u32 +8, InputSampleRate u32 +0xC}
unsafe extern "win64" fn v_ok(_this: A) -> A {
    once("IXAudio2Voice (no-op method)");
    0
}
unsafe extern "win64" fn v_get_voice_details(this: A, d: A) -> A {
    once("IXAudio2Voice::GetVoiceDetails");
    unsafe {
        (d as *mut u32).write_unaligned(0);
        ((d + 4) as *mut u32).write_unaligned(((this + 8) as *const u32).read());
        ((d + 8) as *mut u32).write_unaligned(((this + 0xC) as *const u32).read());
    }
    0
}
unsafe extern "win64" fn v_get_volume(_this: A, v: A) -> A {
    once("IXAudio2Voice::GetVolume");
    if v != 0 {
        unsafe { (v as *mut f32).write_unaligned(1.0) };
    }
    0
}
unsafe extern "win64" fn v_get_channel_volumes(_this: A, n: A, v: A) -> A {
    once("IXAudio2Voice::GetChannelVolumes");
    for i in 0..n as u32 as u64 {
        unsafe { ((v + 4 * i) as *mut f32).write_unaligned(1.0) };
    }
    0
}
unsafe extern "win64" fn v_get_output_matrix(_this: A, _dst: A, src_ch: A, dst_ch: A, m: A) -> A {
    once("IXAudio2Voice::GetOutputMatrix");
    for i in 0..(src_ch as u32 as u64 * dst_ch as u32 as u64) {
        unsafe { ((m + 4 * i) as *mut f32).write_unaligned(0.0) };
    }
    0
}
unsafe extern "win64" fn v_destroy(_this: A) -> A {
    once("IXAudio2Voice::DestroyVoice");
    0
}
/// XAUDIO2_VOICE_STATE (2.7): pCurrentBufferContext, BuffersQueued u32, SamplesPlayed u64: nothing queued.
unsafe extern "win64" fn v_get_state(_this: A, s: A) -> A {
    once("IXAudio2SourceVoice::GetState");
    if s != 0 {
        unsafe { std::ptr::write_bytes(s as *mut u8, 0, 0x18) };
    }
    0
}
unsafe extern "win64" fn v_get_frequency_ratio(_this: A, r: A) -> A {
    once("IXAudio2SourceVoice::GetFrequencyRatio");
    if r != 0 {
        unsafe { (r as *mut f32).write_unaligned(1.0) };
    }
    0
}

fn table(fns: &[usize]) -> u64 {
    unsafe {
        let vt = crate::mem::heap_alloc(8 * fns.len(), true) as u64;
        for (i, f) in fns.iter().enumerate() {
            *((vt + 8 * i as u64) as *mut u64) = *f as u64;
        }
        vt
    }
}

fn voice(source: bool, ch: u32, rate: u32) -> u64 {
    static VT_SRC: OnceLock<u64> = OnceLock::new();
    static VT_OTHER: OnceLock<u64> = OnceLock::new();
    let base = |n: usize| {
        let mut v = vec![v_ok as *const () as usize; n];
        v[0] = v_get_voice_details as *const () as usize;
        v[13] = v_get_volume as *const () as usize;
        v[15] = v_get_channel_volumes as *const () as usize;
        v[17] = v_get_output_matrix as *const () as usize;
        v[18] = v_destroy as *const () as usize;
        v
    };
    let vt = if source {
        *VT_SRC.get_or_init(|| {
            let mut v = base(28);
            v[25] = v_get_state as *const () as usize;
            v[27] = v_get_frequency_ratio as *const () as usize;
            table(&v)
        })
    } else {
        *VT_OTHER.get_or_init(|| table(&base(19)))
    };
    VOICES.fetch_add(1, Ordering::Relaxed);
    unsafe {
        let o = crate::mem::heap_alloc(0x20, true) as u64;
        *(o as *mut u64) = vt;
        *((o + 8) as *mut u32) = ch;
        *((o + 0xC) as *mut u32) = rate;
        o
    }
}

/// The IXAudio2 object handed out by CoCreateInstance.
pub fn xaudio2() -> u64 {
    static X: OnceLock<u64> = OnceLock::new();
    *X.get_or_init(|| {
        let mut v = vec![x_ok as *const () as usize; 16];
        v[0] = x_query_interface as *const () as usize;
        v[1] = x_add_ref as *const () as usize;
        v[2] = x_release as *const () as usize;
        v[3] = x_get_device_count as *const () as usize;
        v[4] = x_get_device_details as *const () as usize;
        v[8] = x_create_source_voice as *const () as usize;
        v[9] = x_create_submix_voice as *const () as usize;
        v[10] = x_create_mastering_voice as *const () as usize;
        v[14] = x_get_performance_data as *const () as usize;
        let vt = table(&v);
        unsafe {
            let o = crate::mem::heap_alloc(0x10, true) as u64;
            *(o as *mut u64) = vt;
            o
        }
    })
}

/// Voices created so far (for the report).
pub fn voices() -> u64 {
    VOICES.load(Ordering::Relaxed)
}

// ---- IMMDeviceEnumerator (MMDeviceEnumerator CLSID bcde0395-e52f-467c-8e3d-c4579291692e): the sound world keeps it
// at +0x1998 and asks it for the default render endpoint (slot 4). The null one has no endpoints: the client then
// takes its no-device path.
pub const CLSID_MMDEVICE_ENUMERATOR: [u8; 16] = [0x95, 0x03, 0xde, 0xbc, 0x2f, 0xe5, 0x7c, 0x46, 0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e];
const E_NOTFOUND: u64 = 0x8007_0490;
unsafe extern "win64" fn m_no_device(_this: A, _a: A, _b: A, out: A) -> A {
    once("IMMDeviceEnumerator::GetDefaultAudioEndpoint");
    if out != 0 {
        unsafe { (out as *mut u64).write_unaligned(0) };
    }
    E_NOTFOUND
}
/// EnumAudioEndpoints succeeds with an empty IMMDeviceCollection: the caller releases the collection without
/// checking the result (0x20061F8D7).
unsafe extern "win64" fn m_enum(_this: A, _flow: A, _mask: A, out: A) -> A {
    once("IMMDeviceEnumerator::EnumAudioEndpoints");
    if out != 0 {
        unsafe { (out as *mut u64).write_unaligned(empty_collection()) };
    }
    0
}
unsafe extern "win64" fn c_get_count(_this: A, n: A) -> A {
    once("IMMDeviceCollection::GetCount");
    if n != 0 {
        unsafe { (n as *mut u32).write_unaligned(0) };
    }
    0
}
unsafe extern "win64" fn c_item(_this: A, _i: A, out: A) -> A {
    once("IMMDeviceCollection::Item");
    if out != 0 {
        unsafe { (out as *mut u64).write_unaligned(0) };
    }
    0x8007_0057 // E_INVALIDARG
}
fn empty_collection() -> u64 {
    static C: OnceLock<u64> = OnceLock::new();
    *C.get_or_init(|| {
        let v = vec![
            x_query_interface as *const () as usize,
            x_add_ref as *const () as usize,
            x_release as *const () as usize,
            c_get_count as *const () as usize,
            c_item as *const () as usize,
        ];
        let vt = table(&v);
        unsafe {
            let o = crate::mem::heap_alloc(0x10, true) as u64;
            *(o as *mut u64) = vt;
            o
        }
    })
}
unsafe extern "win64" fn m_get_device(_this: A, _id: A, out: A) -> A {
    once("IMMDeviceEnumerator::GetDevice");
    if out != 0 {
        unsafe { (out as *mut u64).write_unaligned(0) };
    }
    E_NOTFOUND
}
unsafe extern "win64" fn m_ok(_this: A) -> A {
    once("IMMDeviceEnumerator (no-op method)");
    0
}
pub fn mm_device_enumerator() -> u64 {
    static M: OnceLock<u64> = OnceLock::new();
    *M.get_or_init(|| {
        let mut v = vec![m_ok as *const () as usize; 7];
        v[0] = x_query_interface as *const () as usize;
        v[1] = x_add_ref as *const () as usize;
        v[2] = x_release as *const () as usize;
        v[3] = m_enum as *const () as usize;
        v[4] = m_no_device as *const () as usize;
        v[5] = m_get_device as *const () as usize;
        let vt = table(&v);
        unsafe {
            let o = crate::mem::heap_alloc(0x10, true) as u64;
            *(o as *mut u64) = vt;
            o
        }
    })
}
