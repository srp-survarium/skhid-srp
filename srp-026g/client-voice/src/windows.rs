//! Owned WinMM devices. A header and its PCM storage stay at stable heap
//! addresses until the driver returns them and unprepares the header. And
//! the capture devices as Windows lists them, for the options' Microphone
//! list.
use crate::{
    FRAME, RATE, Stereo,
    microphone::{Device, Found},
    runtime::BufferOrder,
};
use std::{cell::UnsafeCell, ffi::c_void, mem::size_of, ptr, ptr::NonNull};
use voice_abi::MicrophoneId;
use windows_sys::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::{
            Audio::*,
            Multimedia::{DRV_QUERYFUNCTIONINSTANCEID, DRV_QUERYFUNCTIONINSTANCEIDSIZE},
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, STGM_READ,
                StructuredStorage::{PROPVARIANT, PropVariantClear},
            },
            Variant::VT_LPWSTR,
        },
    },
    core::{GUID, HRESULT},
};

const BUFFERS: usize = 4;
const HEADER_BYTES: u32 = size_of::<WAVEHDR>() as u32;

// windows-sys models WAVEHDR as packed. Put it first at an explicitly
// aligned address so its u32 completion flag is safe to read volatile.
#[repr(C, align(8))]
struct Buffer<S> {
    header: UnsafeCell<WAVEHDR>,
    samples: UnsafeCell<S>,
    prepared: bool,
    queued: bool,
}
impl<S: Copy> Buffer<S> {
    fn new(silence: S) -> Box<Self> {
        let buffer = Box::new(Self {
            // SAFETY: all-zero WAVEHDR is the documented initial state.
            header: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            samples: UnsafeCell::new(silence),
            prepared: false,
            queued: false,
        });
        // SAFETY: the allocation is stable and not yet given to a driver.
        unsafe {
            (*buffer.header.get()).lpData = buffer.samples.get().cast();
            (*buffer.header.get()).dwBufferLength = size_of::<S>() as u32;
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
/// 16-bit PCM at [`RATE`]: the microphone mono, the mix stereo.
fn format(channels: u16) -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: channels,
        nSamplesPerSec: RATE as u32,
        nAvgBytesPerSec: RATE as u32 * 2 * u32::from(channels),
        nBlockAlign: 2 * channels,
        wBitsPerSample: 16,
        cbSize: 0,
    }
}

pub struct Output {
    handle: HWAVEOUT,
    buffers: [Option<Box<Buffer<Stereo>>>; BUFFERS],
    order: BufferOrder,
}
impl Output {
    pub fn open() -> Result<Self, u32> {
        let mut handle = ptr::null_mut();
        // SAFETY: output and format are valid for the call; no callback is used.
        let error =
            unsafe { waveOutOpen(&mut handle, WAVE_MAPPER, &format(2), 0, 0, CALLBACK_NULL) };
        if error != 0 {
            return Err(error);
        }
        let mut device = Self {
            handle,
            buffers: std::array::from_fn(|_| None),
            order: BufferOrder::new(0),
        };
        for index in 0..BUFFERS {
            let mut buffer = Buffer::new([[0; 2]; FRAME]);
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
    pub fn pump(&mut self, mut mix: impl FnMut() -> Stereo) -> Result<(), u32> {
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
    buffers: [Option<Box<Buffer<[i16; FRAME]>>>; BUFFERS],
    order: BufferOrder,
}
impl Input {
    /// Opens WinMM's device of that number, or Windows' default.
    pub fn open(device: Device) -> Result<Self, u32> {
        let number = match device {
            Device::Default => WAVE_MAPPER,
            Device::Number(number) => number,
        };
        let mut handle = ptr::null_mut();
        // SAFETY: valid output pointer and PCM format, no callback.
        let error = unsafe { waveInOpen(&mut handle, number, &format(1), 0, 0, CALLBACK_NULL) };
        if error != 0 {
            return Err(error);
        }
        let mut device = Self {
            handle,
            buffers: std::array::from_fn(|_| None),
            order: BufferOrder::new(0),
        };
        for index in 0..BUFFERS {
            let mut buffer = Buffer::new([0; FRAME]);
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
impl crate::mic_test::Microphone for Input {
    fn capture(&mut self, frame: impl FnMut([i16; FRAME])) -> Result<(), u32> {
        self.pump(frame)
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

/// WinMM's capture devices now, by their numbers: each one's endpoint ID.
pub fn endpoint_ids() -> Vec<Option<MicrophoneId>> {
    // SAFETY: no arguments.
    let count = unsafe { waveInGetNumDevs() };
    (0..count).map(endpoint_id).collect()
}

/// The capture devices now, for the options' list: WinMM's, each with its
/// endpoint ID, WinMM's name and the endpoint's whole name. Run on a thread
/// of its own: COM is started on it here for MMDevice's names, and ended.
pub fn microphones() -> Vec<Found> {
    let names = Names::new();
    // SAFETY: no arguments.
    let count = unsafe { waveInGetNumDevs() };
    (0..count)
        .map(|number| {
            let id = endpoint_id(number);
            let full = names.as_ref().zip(id.as_ref()).and_then(|(names, id)| names.of(id));
            Found { id, short: short_name(number), full }
        })
        .collect()
}

/// The text before the first zero (all of it when none).
fn until_zero(wide: &[u16]) -> &[u16] {
    &wide[..wide.iter().position(|&unit| unit == 0).unwrap_or(wide.len())]
}

/// WinMM's endpoint ID of its capture device `number` (Windows from Vista
/// on, and Wine, answer DRV_QUERYFUNCTIONINSTANCEID): None when it gives
/// none, or one that is not an ID.
fn endpoint_id(number: u32) -> Option<MicrophoneId> {
    // The device is named by its number in the handle's place.
    let device = number as usize as HWAVEIN;
    let mut bytes = 0_u32;
    // SAFETY: WinMM writes the ID's size in bytes, its zero counted, into
    // the u32 (a ULONG) it is given.
    let error = unsafe {
        waveInMessage(device, DRV_QUERYFUNCTIONINSTANCEIDSIZE, (&raw mut bytes) as usize, 0)
    };
    let units = (bytes as usize).div_ceil(2);
    if error != 0 || units == 0 || units > 4 * voice_abi::MICROPHONE_ID {
        return None;
    }
    let mut wide = vec![0_u16; units];
    // SAFETY: room for `bytes` bytes, the ID and its zero.
    let error = unsafe {
        waveInMessage(device, DRV_QUERYFUNCTIONINSTANCEID, wide.as_mut_ptr() as usize, units * 2)
    };
    if error != 0 {
        return None;
    }
    let text = String::from_utf16(until_zero(&wide)).ok()?;
    MicrophoneId::parse(text.as_bytes())
}

/// WinMM's name of its capture device `number` (cut at 31 characters).
fn short_name(number: u32) -> String {
    // SAFETY: all zero is a valid WAVEINCAPSW.
    let mut caps: WAVEINCAPSW = unsafe { std::mem::zeroed() };
    // SAFETY: room of the size given.
    let error = unsafe {
        waveInGetDevCapsW(number as usize, &mut caps, size_of::<WAVEINCAPSW>() as u32)
    };
    if error != 0 {
        return String::new();
    }
    let name = caps.szPname;
    String::from_utf16_lossy(until_zero(&name))
}

/// MMDevice's enumerator (Windows' audio endpoints), which knows each
/// endpoint's whole name.
const ENUMERATOR: GUID = MMDeviceEnumerator;
const IMM_DEVICE_ENUMERATOR: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
/// PKEY_Device_FriendlyName: "Microphone (Realtek(R) Audio)".
const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

/// IUnknown's slots, the first of every COM interface's.
#[repr(C)]
struct Unknown {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}
/// IMMDeviceEnumerator's slots up to GetDevice.
#[repr(C)]
struct EnumeratorSlots {
    unknown: Unknown,
    enum_audio_endpoints: usize,
    get_default_audio_endpoint: usize,
    get_device: unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> HRESULT,
}
/// IMMDevice's slots up to OpenPropertyStore.
#[repr(C)]
struct DeviceSlots {
    unknown: Unknown,
    activate: usize,
    open_property_store: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
}
/// IPropertyStore's slots up to GetValue.
#[repr(C)]
struct StoreSlots {
    unknown: Unknown,
    get_count: usize,
    get_at: usize,
    get_value:
        unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> HRESULT,
}

/// A COM interface of ours, of the slots `S`: released when it goes.
struct Com<S> {
    this: NonNull<*const S>,
}
impl<S> Com<S> {
    /// # Safety
    /// `this` is null or an interface whose slots begin as `S`'s, with a
    /// reference that is now this one's.
    unsafe fn new(this: *mut c_void) -> Option<Com<S>> {
        NonNull::new(this.cast()).map(|this| Com { this })
    }
    fn this(&self) -> *mut c_void {
        self.this.as_ptr().cast()
    }
    fn slots(&self) -> &S {
        // SAFETY: an interface's first field is its slots, alive while it is.
        unsafe { &**self.this.as_ptr() }
    }
}
impl<S> Drop for Com<S> {
    fn drop(&mut self) {
        // SAFETY: every interface's slots begin with IUnknown's; ours is
        // given back once.
        unsafe {
            let unknown = (*self.this.as_ptr()).cast::<Unknown>();
            ((*unknown).release)(self.this());
        }
    }
}

/// COM on this thread, ended when it goes.
struct ComThread;
impl ComThread {
    fn start() -> Option<ComThread> {
        // SAFETY: no reserved pointer; a free-threaded apartment. A thread
        // already in another (not ours) is left as it is.
        let result = unsafe { CoInitializeEx(ptr::null(), COINIT_MULTITHREADED as u32) };
        (result >= 0).then_some(ComThread)
    }
}
impl Drop for ComThread {
    fn drop(&mut self) {
        // SAFETY: balances the start.
        unsafe { CoUninitialize() }
    }
}

/// The endpoints' whole names: MMDevice's enumerator, on a thread COM was
/// started on for it.
struct Names {
    // Released before COM ends: fields drop in this order.
    enumerator: Com<EnumeratorSlots>,
    _com: ComThread,
}
impl Names {
    fn new() -> Option<Names> {
        let com = ComThread::start()?;
        let mut enumerator = ptr::null_mut();
        // SAFETY: the class and interface IDs of MMDevice's enumerator.
        let result = unsafe {
            CoCreateInstance(&ENUMERATOR, ptr::null_mut(), CLSCTX_ALL, &IMM_DEVICE_ENUMERATOR, &mut enumerator)
        };
        if result < 0 {
            return None;
        }
        // SAFETY: an IMMDeviceEnumerator, its reference ours.
        let enumerator = unsafe { Com::new(enumerator) }?;
        Some(Names { enumerator, _com: com })
    }

    /// The endpoint's friendly name, whole.
    fn of(&self, id: &MicrophoneId) -> Option<String> {
        let wide: Vec<u16> = id.as_bytes().iter().map(|&byte| u16::from(byte)).chain([0]).collect();
        let mut device = ptr::null_mut();
        // SAFETY: a zero-terminated ID, an interface pointer's room.
        let result = unsafe {
            (self.enumerator.slots().get_device)(self.enumerator.this(), wide.as_ptr(), &mut device)
        };
        if result < 0 {
            return None;
        }
        // SAFETY: an IMMDevice, its reference ours.
        let device: Com<DeviceSlots> = unsafe { Com::new(device) }?;
        let mut store = ptr::null_mut();
        // SAFETY: read access, an interface pointer's room.
        let result =
            unsafe { (device.slots().open_property_store)(device.this(), STGM_READ, &mut store) };
        if result < 0 {
            return None;
        }
        // SAFETY: an IPropertyStore, its reference ours.
        let store: Com<StoreSlots> = unsafe { Com::new(store) }?;
        let mut value = PROPVARIANT::default();
        // SAFETY: a key and an empty PROPVARIANT, which the store fills.
        if unsafe { (store.slots().get_value)(store.this(), &FRIENDLY_NAME, &mut value) } < 0 {
            return None;
        }
        // SAFETY: a string's PROPVARIANT holds a zero-terminated wide string
        // until it is cleared; anything else is cleared unread.
        unsafe {
            let fields = value.Anonymous.Anonymous;
            let text = fields.Anonymous.pwszVal;
            let name = (fields.vt == VT_LPWSTR && !text.is_null()).then(|| {
                let mut len = 0;
                while len < 1024 && *text.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
            });
            PropVariantClear(&mut value);
            name
        }
    }
}
