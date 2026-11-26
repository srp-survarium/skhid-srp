//! Null DXGI / Direct3D 11 device and the Win32 display queries around it: one adapter with one output and one
//! display mode (1920x1080, B8G8R8A8), D3D11CreateDevice at feature level 11_0, a device whose Create* methods hand
//! out "null children" and a context whose draw/state calls do nothing. GetData reports done, Map returns writable
//! memory. Every method's first call is logged.
//! Interface slot numbers: DXGI 1.0/1.1 and D3D11 headers (IDXGIObject 0..6, IDXGIFactory 7..11 (+12, 13 for
//! IDXGIFactory1), IDXGIAdapter 7..9 (+10), IDXGIOutput 7..18, ID3D11Device 0..42, ID3D11DeviceContext 0..114).
#![allow(non_snake_case)]

use crate::hlog;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

type A = u64;

const S_OK: A = 0;
const E_NOINTERFACE: A = 0x8000_4002;
const DXGI_ERROR_NOT_FOUND: A = 0x887A_0002;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
pub const HOST_MONITOR: u64 = 0x10020;

static CHILDREN: AtomicU64 = AtomicU64::new(0);

fn once(name: &str) {
    static SEEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let mut s = SEEN.lock().unwrap();
    if !s.iter().any(|x| x == name) {
        s.push(name.to_string());
        hlog!("null D3D: first call {name}");
    }
}

unsafe fn put(p: A, v: A) {
    if p != 0 {
        unsafe { (p as *mut u64).write_unaligned(v) };
    }
}
unsafe fn put32(p: A, v: u32) {
    if p != 0 {
        unsafe { (p as *mut u32).write_unaligned(v) };
    }
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
fn object(vt: u64, size: usize) -> u64 {
    unsafe {
        let o = crate::mem::heap_alloc(size, true) as u64;
        *(o as *mut u64) = vt;
        o
    }
}

// ---------------------------------------------------------------- IUnknown helpers
unsafe extern "win64" fn qi_self(this: A, _iid: A, out: A) -> A {
    unsafe { put(out, this) };
    S_OK
}
unsafe extern "win64" fn add_ref(_this: A) -> A {
    1
}
unsafe extern "win64" fn release(_this: A) -> A {
    0
}
unsafe extern "win64" fn not_found(_this: A) -> A {
    DXGI_ERROR_NOT_FOUND
}

// ---------------------------------------------------------------- null child (buffers, textures, views, shaders,
// states, queries, input layouts): {vtable, +8 kind, +0x10 creation desc copy (0x60 bytes), +0x70 Map scratch,
// +0x78 scratch size}
unsafe extern "win64" fn child_get_device(_this: A, out: A) -> A {
    once("child::GetDevice");
    unsafe { put(out, device()) };
    0
}
unsafe extern "win64" fn child_zero(_this: A) -> A {
    0
}
unsafe extern "win64" fn child_private(_this: A) -> A {
    DXGI_ERROR_NOT_FOUND
}
fn child_vt() -> u64 {
    static V: OnceLock<u64> = OnceLock::new();
    *V.get_or_init(|| {
        let mut v = vec![child_zero as *const () as usize; 16];
        v[0] = qi_self as *const () as usize;
        v[1] = add_ref as *const () as usize;
        v[2] = release as *const () as usize;
        v[3] = child_get_device as *const () as usize;
        v[4] = child_private as *const () as usize;
        table(&v)
    })
}
fn child(kind: u64, desc: A, desc_len: usize) -> u64 {
    CHILDREN.fetch_add(1, Ordering::Relaxed);
    let o = object(child_vt(), 0x80);
    unsafe {
        *((o + 8) as *mut u64) = kind;
        if desc != 0 && desc_len != 0 {
            std::ptr::copy_nonoverlapping(desc as *const u8, (o + 0x10) as *mut u8, desc_len.min(0x60));
        }
    }
    o
}

// ---------------------------------------------------------------- DXGI
unsafe extern "win64" fn f_enum_adapters(_this: A, i: A, out: A) -> A {
    once("IDXGIFactory::EnumAdapters");
    if i as u32 != 0 {
        unsafe { put(out, 0) };
        return DXGI_ERROR_NOT_FOUND;
    }
    unsafe { put(out, adapter()) };
    S_OK
}
unsafe extern "win64" fn f_ok(_this: A) -> A {
    once("IDXGIFactory (no-op method)");
    S_OK
}
unsafe extern "win64" fn f_fail(_this: A) -> A {
    once("IDXGIFactory (refused method)");
    0x8000_4005
}
unsafe extern "win64" fn f_is_current(_this: A) -> A {
    1
}
pub fn factory() -> u64 {
    static F: OnceLock<u64> = OnceLock::new();
    *F.get_or_init(|| {
        let mut v = vec![f_ok as *const () as usize; 14];
        v[0] = qi_self as *const () as usize;
        v[1] = add_ref as *const () as usize;
        v[2] = release as *const () as usize;
        v[5] = not_found as *const () as usize; // GetPrivateData
        v[6] = f_fail as *const () as usize; // GetParent
        v[7] = f_enum_adapters as *const () as usize;
        v[10] = f_fail as *const () as usize; // CreateSwapChain (window cook 0x77 only)
        v[11] = f_fail as *const () as usize; // CreateSoftwareAdapter
        v[12] = f_enum_adapters as *const () as usize; // EnumAdapters1
        v[13] = f_is_current as *const () as usize;
        object(table(&v), 0x10)
    })
}

const IID_ADAPTER3: [u8; 16] = [0xa4, 0x67, 0x59, 0x64, 0x92, 0x13, 0x10, 0x43, 0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd];
unsafe extern "win64" fn a_qi(this: A, iid: A, out: A) -> A {
    let g = if iid != 0 { unsafe { std::slice::from_raw_parts(iid as *const u8, 16) }.to_vec() } else { vec![] };
    if g == IID_ADAPTER3 {
        once("IDXGIAdapter::QueryInterface(IDXGIAdapter3) -> E_NOINTERFACE");
        unsafe { put(out, 0) };
        return E_NOINTERFACE;
    }
    unsafe { put(out, this) };
    S_OK
}
unsafe extern "win64" fn a_enum_outputs(_this: A, i: A, out: A) -> A {
    once("IDXGIAdapter::EnumOutputs");
    if i as u32 != 0 {
        unsafe { put(out, 0) };
        return DXGI_ERROR_NOT_FOUND;
    }
    unsafe { put(out, output()) };
    S_OK
}
/// DXGI_ADAPTER_DESC (0x130 bytes; DXGI_ADAPTER_DESC1 adds Flags at 0x130): Description WCHAR[128], VendorId 0x100,
/// DeviceId 0x104, SubSysId 0x108, Revision 0x10C, DedicatedVideoMemory 0x110, DedicatedSystemMemory 0x118,
/// SharedSystemMemory 0x120, LUID 0x128. Vendor 0x1AF4 (not 0x1414/0x8C = the skipped Basic Render Driver, not one of
/// the known vendors: score 9999, 0x20009ABE1).
unsafe extern "win64" fn a_get_desc(_this: A, d: A) -> A {
    once("IDXGIAdapter::GetDesc");
    unsafe {
        std::ptr::write_bytes(d as *mut u8, 0, 0x130);
        for (i, c) in "host null adapter".encode_utf16().enumerate() {
            ((d + 2 * i as u64) as *mut u16).write_unaligned(c);
        }
        put32(d + 0x100, 0x1AF4);
        put32(d + 0x104, 0x1);
        put(d + 0x110, 2 << 30);
        put(d + 0x120, 4 << 30);
        put32(d + 0x128, 0x1234);
    }
    S_OK
}
unsafe extern "win64" fn a_get_desc1(this: A, d: A) -> A {
    unsafe {
        a_get_desc(this, d);
        put32(d + 0x130, 0);
    }
    S_OK
}
unsafe extern "win64" fn a_check_interface(_this: A, _iid: A, umd: A) -> A {
    unsafe { put(umd, 0x0001_0000_0000_0000) };
    S_OK
}
pub fn adapter() -> u64 {
    static X: OnceLock<u64> = OnceLock::new();
    *X.get_or_init(|| {
        let mut v = vec![f_ok as *const () as usize; 11];
        v[0] = a_qi as *const () as usize;
        v[1] = add_ref as *const () as usize;
        v[2] = release as *const () as usize;
        v[5] = not_found as *const () as usize;
        v[6] = qi_self as *const () as usize; // GetParent(riid, out) -> the factory would be right; self is harmless
        v[7] = a_enum_outputs as *const () as usize;
        v[8] = a_get_desc as *const () as usize;
        v[9] = a_check_interface as *const () as usize;
        v[10] = a_get_desc1 as *const () as usize;
        object(table(&v), 0x10)
    })
}

/// DXGI_OUTPUT_DESC: DeviceName WCHAR[32], DesktopCoordinates RECT 0x40, AttachedToDesktop 0x50, Rotation 0x54,
/// Monitor 0x58.
unsafe extern "win64" fn o_get_desc(_this: A, d: A) -> A {
    once("IDXGIOutput::GetDesc");
    unsafe {
        std::ptr::write_bytes(d as *mut u8, 0, 0x60);
        for (i, c) in "\\\\.\\DISPLAY1".encode_utf16().enumerate() {
            ((d + 2 * i as u64) as *mut u16).write_unaligned(c);
        }
        put32(d + 0x48, WIDTH);
        put32(d + 0x4C, HEIGHT);
        put32(d + 0x50, 1);
        put32(d + 0x54, 1);
        put(d + 0x58, HOST_MONITOR);
    }
    S_OK
}
/// GetDisplayModeList(format, flags, *num, desc): one mode 1920x1080 60 Hz in the asked format (0x57).
unsafe extern "win64" fn o_mode_list(_this: A, format: A, _flags: A, num: A, desc: A) -> A {
    once("IDXGIOutput::GetDisplayModeList");
    unsafe {
        if desc == 0 {
            put32(num, 1);
            return S_OK;
        }
        let n = if num != 0 { (num as *const u32).read_unaligned() } else { 0 };
        if n >= 1 {
            put32(desc, WIDTH);
            put32(desc + 4, HEIGHT);
            put32(desc + 8, 60);
            put32(desc + 12, 1);
            put32(desc + 16, format as u32);
            put32(desc + 20, 1);
            put32(desc + 24, 0);
        }
        put32(num, 1);
    }
    S_OK
}
unsafe extern "win64" fn o_ok(_this: A) -> A {
    once("IDXGIOutput (no-op method)");
    S_OK
}
pub fn output() -> u64 {
    static X: OnceLock<u64> = OnceLock::new();
    *X.get_or_init(|| {
        let mut v = vec![o_ok as *const () as usize; 19];
        v[0] = qi_self as *const () as usize;
        v[1] = add_ref as *const () as usize;
        v[2] = release as *const () as usize;
        v[5] = not_found as *const () as usize;
        v[7] = o_get_desc as *const () as usize;
        v[8] = o_mode_list as *const () as usize;
        object(table(&v), 0x10)
    })
}

unsafe extern "win64" fn CreateDXGIFactory(_iid: A, out: A) -> A {
    let f = factory();
    hlog!("dxgi!CreateDXGIFactory -> null factory {f:#x}");
    unsafe { put(out, f) };
    S_OK
}
unsafe extern "win64" fn CreateDXGIFactory1(iid: A, out: A) -> A {
    unsafe { CreateDXGIFactory(iid, out) }
}

// ---------------------------------------------------------------- ID3D11Device
macro_rules! creator {
    ($name:ident, $label:literal, $kind:literal, $desc_idx:literal, $desc_len:literal, $out_idx:literal) => {
        #[allow(clippy::too_many_arguments)]
        unsafe extern "win64" fn $name(_this: A, a1: A, a2: A, a3: A, a4: A, a5: A, a6: A, a7: A, a8: A, a9: A) -> A {
            once($label);
            let args = [a1, a2, a3, a4, a5, a6, a7, a8, a9];
            let desc = args.get(($desc_idx as usize).wrapping_sub(1)).copied().unwrap_or(0);
            let out = args[$out_idx - 1];
            if out != 0 {
                unsafe { put(out, child($kind, desc, $desc_len)) };
            }
            S_OK
        }
    };
}
// (this, desc, init, out)
creator!(d_create_buffer, "ID3D11Device::CreateBuffer", 1, 1, 0x18, 3);
creator!(d_create_tex1d, "ID3D11Device::CreateTexture1D", 2, 1, 0x1C, 3);
creator!(d_create_tex2d, "ID3D11Device::CreateTexture2D", 3, 1, 0x2C, 3);
creator!(d_create_tex3d, "ID3D11Device::CreateTexture3D", 4, 1, 0x24, 3);
// (this, resource, desc, out)
creator!(d_create_view, "ID3D11Device::Create*View", 5, 2, 0x18, 3);
// (this, descs, num, bytecode, len, out)
creator!(d_create_layout, "ID3D11Device::CreateInputLayout", 6, 0, 0, 5);
// (this, bytecode, len, linkage, out)
creator!(d_create_shader, "ID3D11Device::Create*Shader", 7, 0, 0, 4);
// CreateGeometryShaderWithStreamOutput: out is the 10th parameter
creator!(d_create_gs_so, "ID3D11Device::CreateGeometryShaderWithStreamOutput", 7, 0, 0, 9);
// (this, out)
creator!(d_create_linkage, "ID3D11Device::CreateClassLinkage", 8, 0, 0, 1);
// (this, desc, out)
creator!(d_create_state, "ID3D11Device::Create*State/Query/Predicate/Counter", 9, 1, 0x40, 2);
// (this, flags, out)
creator!(d_create_deferred, "ID3D11Device::CreateDeferredContext", 10, 0, 0, 2);
// (this, handle, riid, out)
creator!(d_open_shared, "ID3D11Device::OpenSharedResource", 11, 0, 0, 3);

unsafe extern "win64" fn d_check_format(_this: A, _fmt: A, out: A) -> A {
    once("ID3D11Device::CheckFormatSupport");
    unsafe { put32(out, 0xFFFF_FFFF) };
    S_OK
}
unsafe extern "win64" fn d_check_msaa(_this: A, _fmt: A, _count: A, out: A) -> A {
    once("ID3D11Device::CheckMultisampleQualityLevels");
    unsafe { put32(out, 1) };
    S_OK
}
unsafe extern "win64" fn d_check_feature(_this: A, _f: A, data: A, size: A) -> A {
    once("ID3D11Device::CheckFeatureSupport");
    if data != 0 {
        unsafe { std::ptr::write_bytes(data as *mut u8, 0, size as u32 as usize) };
    }
    S_OK
}
unsafe extern "win64" fn d_feature_level(_this: A) -> A {
    0xB000
}
unsafe extern "win64" fn d_zero(_this: A) -> A {
    0
}
unsafe extern "win64" fn d_immediate(_this: A, out: A) -> A {
    unsafe { put(out, context()) };
    0
}
unsafe extern "win64" fn d_ok(_this: A) -> A {
    once("ID3D11Device (no-op method)");
    S_OK
}
pub fn device() -> u64 {
    static X: OnceLock<u64> = OnceLock::new();
    *X.get_or_init(|| {
        let mut v = vec![d_ok as *const () as usize; 43];
        v[0] = qi_self as *const () as usize;
        v[1] = add_ref as *const () as usize;
        v[2] = release as *const () as usize;
        v[3] = d_create_buffer as *const () as usize;
        v[4] = d_create_tex1d as *const () as usize;
        v[5] = d_create_tex2d as *const () as usize;
        v[6] = d_create_tex3d as *const () as usize;
        for s in 7..=10 {
            v[s] = d_create_view as *const () as usize;
        }
        v[11] = d_create_layout as *const () as usize;
        v[12] = d_create_shader as *const () as usize;
        v[13] = d_create_shader as *const () as usize;
        v[14] = d_create_gs_so as *const () as usize;
        for s in 15..=18 {
            v[s] = d_create_shader as *const () as usize;
        }
        v[19] = d_create_linkage as *const () as usize;
        for s in 20..=26 {
            v[s] = d_create_state as *const () as usize;
        }
        v[27] = d_create_deferred as *const () as usize;
        v[28] = d_open_shared as *const () as usize;
        v[29] = d_check_format as *const () as usize;
        v[30] = d_check_msaa as *const () as usize;
        v[33] = d_check_feature as *const () as usize;
        v[34] = not_found as *const () as usize;
        v[37] = d_feature_level as *const () as usize;
        v[38] = d_zero as *const () as usize;
        v[39] = d_zero as *const () as usize; // GetDeviceRemovedReason: S_OK
        v[40] = d_immediate as *const () as usize;
        v[42] = d_zero as *const () as usize;
        object(table(&v), 0x10)
    })
}

// ---------------------------------------------------------------- ID3D11DeviceContext
/// Map(this, resource, subresource, map type, flags, D3D11_MAPPED_SUBRESOURCE*): writable memory per resource
/// (buffers: ByteWidth from the creation desc; others: 16 MB), kept with the child and reused.
unsafe extern "win64" fn c_map(_this: A, res: A, _sub: A, _ty: A, _flags: A, out: A) -> A {
    once("ID3D11DeviceContext::Map");
    unsafe {
        let mut scratch = if res != 0 { *((res + 0x70) as *const u64) } else { 0 };
        if scratch == 0 {
            let kind = if res != 0 { *((res + 8) as *const u64) } else { 0 };
            let size = if kind == 1 { (*((res + 0x10) as *const u32) as usize).max(16) } else { 16 << 20 };
            scratch = crate::mem::heap_alloc(size, true) as u64;
            if res != 0 {
                *((res + 0x70) as *mut u64) = scratch;
                *((res + 0x78) as *mut u64) = size as u64;
            }
        }
        put(out, scratch);
        put32(out + 8, 16 << 10);
        put32(out + 12, 16 << 20);
    }
    S_OK
}
/// GetData(this, async, data, size, flags): always done (S_OK); data: a BOOL true / the timestamp-disjoint
/// {u64 frequency 1 000 000, BOOL 0} / zeros.
unsafe extern "win64" fn c_get_data(_this: A, _q: A, data: A, size: A, _flags: A) -> A {
    once("ID3D11DeviceContext::GetData");
    let size = size as u32 as usize;
    if data != 0 && size != 0 {
        unsafe {
            std::ptr::write_bytes(data as *mut u8, 0, size);
            match size {
                1..=4 => *(data as *mut u8) = 1,
                16 => put(data, 1_000_000),
                _ => {}
            }
        }
    }
    S_OK
}
unsafe extern "win64" fn c_get_device(_this: A, out: A) -> A {
    unsafe { put(out, device()) };
    0
}
unsafe extern "win64" fn c_nop(_this: A) -> A {
    0
}
unsafe extern "win64" fn c_get_type(_this: A) -> A {
    0 // D3D11_DEVICE_CONTEXT_IMMEDIATE
}
pub fn context() -> u64 {
    static X: OnceLock<u64> = OnceLock::new();
    *X.get_or_init(|| {
        let mut v = vec![c_nop as *const () as usize; 115];
        v[0] = qi_self as *const () as usize;
        v[1] = add_ref as *const () as usize;
        v[2] = release as *const () as usize;
        v[3] = c_get_device as *const () as usize;
        v[4] = not_found as *const () as usize;
        v[14] = c_map as *const () as usize;
        v[29] = c_get_data as *const () as usize;
        v[112] = c_get_type as *const () as usize;
        object(table(&v), 0x10)
    })
}

/// D3D11CreateDevice(adapter, driver type, software, flags, levels, n, sdk, &device, &level, &context).
#[allow(clippy::too_many_arguments)]
unsafe extern "win64" fn D3D11CreateDevice(_ad: A, _dt: A, _sw: A, flags: A, _lv: A, _n: A, _sdk: A, dev: A, level: A, ctx: A) -> A {
    hlog!("d3d11!D3D11CreateDevice(flags {flags:#x}) -> null device {:#x}, context {:#x}, feature level 0xB000", device(), context());
    unsafe {
        put(dev, device());
        put32(level, 0xB000);
        put(ctx, context());
    }
    S_OK
}

// ---------------------------------------------------------------- Win32 display queries
/// EnumDisplayMonitors(hdc, clip, callback, lparam): one monitor; the callback (0x200421140) gets it.
unsafe extern "win64" fn EnumDisplayMonitors(_hdc: A, _clip: A, cb: A, lp: A) -> A {
    once("USER32!EnumDisplayMonitors");
    if cb != 0 {
        let rect = unsafe { crate::mem::heap_alloc(16, true) } as u64;
        unsafe {
            put32(rect + 8, WIDTH);
            put32(rect + 12, HEIGHT);
            let f: extern "win64" fn(A, A, A, A) -> A = std::mem::transmute(cb);
            f(HOST_MONITOR, 0, rect, lp);
        }
    }
    1
}
/// GetMonitorInfoA(hmon, MONITORINFO(EX)A*): cbSize at 0, rcMonitor 4, rcWork 0x14, dwFlags 0x24 (1 = primary),
/// szDevice at 0x28 when cbSize >= 0x48.
unsafe extern "win64" fn GetMonitorInfoA(_m: A, mi: A) -> A {
    once("USER32!GetMonitorInfoA");
    unsafe {
        let cb = (mi as *const u32).read_unaligned();
        for (o, v) in [(4, 0), (8, 0), (12, WIDTH), (16, HEIGHT), (0x14, 0), (0x18, 0), (0x1C, WIDTH), (0x20, HEIGHT), (0x24, 1)] {
            put32(mi + o, v);
        }
        if cb >= 0x48 {
            let name = b"\\\\.\\DISPLAY1\0";
            std::ptr::copy_nonoverlapping(name.as_ptr(), (mi + 0x28) as *mut u8, name.len());
        }
    }
    1
}
/// EnumDisplaySettingsA(device, mode, DEVMODEA*): mode 0 / current (-1) / registry (-2) = 1920x1080x32 at 60 Hz.
unsafe extern "win64" fn EnumDisplaySettingsA(_dev: A, mode: A, dm: A) -> A {
    once("USER32!EnumDisplaySettingsA");
    let m = mode as u32;
    if m != 0 && m != 0xFFFF_FFFF && m != 0xFFFF_FFFE {
        return 0;
    }
    unsafe {
        put32(dm + 40, 0x5C_0000); // dmFields: BITSPERPEL | PELSWIDTH | PELSHEIGHT | DISPLAYFREQUENCY (+flags)
        put32(dm + 104, 32);
        put32(dm + 108, WIDTH);
        put32(dm + 112, HEIGHT);
        put32(dm + 116, 0);
        put32(dm + 120, 60);
    }
    1
}
unsafe extern "win64" fn EnumDisplayDevicesW(_dev: A, _i: A, _dd: A, _flags: A) -> A {
    once("USER32!EnumDisplayDevicesW (none)");
    0
}
unsafe extern "win64" fn RegOpenKeyExW(_k: A, _sub: A, _o: A, _sam: A, out: A) -> A {
    once("ADVAPI32!RegOpenKeyExW (not found)");
    unsafe { put(out, 0) };
    2
}
unsafe extern "win64" fn RegQueryValueExW(_k: A) -> A {
    2
}
unsafe extern "win64" fn RegCloseKey(_k: A) -> A {
    0
}

pub fn children() -> u64 {
    CHILDREN.load(Ordering::Relaxed)
}

pub fn table_entries() -> Vec<(&'static str, &'static str, usize)> {
    vec![
        ("dxgi.dll", "CreateDXGIFactory", CreateDXGIFactory as *const () as usize),
        ("dxgi.dll", "CreateDXGIFactory1", CreateDXGIFactory1 as *const () as usize),
        ("d3d11.dll", "D3D11CreateDevice", D3D11CreateDevice as *const () as usize),
        ("user32.dll", "EnumDisplayMonitors", EnumDisplayMonitors as *const () as usize),
        ("user32.dll", "GetMonitorInfoA", GetMonitorInfoA as *const () as usize),
        ("user32.dll", "EnumDisplaySettingsA", EnumDisplaySettingsA as *const () as usize),
        ("user32.dll", "EnumDisplayDevicesW", EnumDisplayDevicesW as *const () as usize),
        ("advapi32.dll", "RegOpenKeyExW", RegOpenKeyExW as *const () as usize),
        ("advapi32.dll", "RegQueryValueExW", RegQueryValueExW as *const () as usize),
        ("advapi32.dll", "RegCloseKey", RegCloseKey as *const () as usize),
    ]
}
