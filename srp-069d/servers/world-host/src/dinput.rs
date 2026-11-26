//! Null DirectInput 8: a world host has no local keyboard/mouse. DirectInput8Create hands out an
//! IDirectInput8 whose CreateDevice returns a device that reports "nothing pressed": GetDeviceState zero-fills,
//! GetDeviceData returns 0 events, every other method returns S_OK. Every method call is counted/logged once.
#![allow(non_snake_case)]

use crate::hlog;
use std::sync::OnceLock;

type A = u64;

unsafe extern "win64" fn ok0(_this: A) -> A {
    0
}
unsafe extern "win64" fn add_ref(_this: A) -> A {
    1
}
unsafe extern "win64" fn create_device(_this: A, guid: A, out: A, _outer: A) -> A {
    let g = if guid != 0 { unsafe { std::slice::from_raw_parts(guid as *const u8, 16) }.to_vec() } else { vec![] };
    hlog!("null DirectInput: CreateDevice({:02x?}) -> null device", g);
    unsafe { (out as *mut u64).write_unaligned(device()) };
    0
}
unsafe extern "win64" fn get_device_state(_this: A, size: A, buf: A) -> A {
    if buf != 0 {
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, size as u32 as usize) };
    }
    0
}
unsafe extern "win64" fn get_device_data(_this: A, _cb: A, _data: A, count: A, _flags: A) -> A {
    if count != 0 {
        unsafe { (count as *mut u32).write_unaligned(0) };
    }
    0
}

fn make_object(methods: &[(usize, usize)], n: usize) -> u64 {
    unsafe {
        let vt = crate::mem::heap_alloc(n * 8, true) as u64;
        for i in 0..n {
            *((vt + 8 * i as u64) as *mut u64) = ok0 as *const () as u64;
        }
        *((vt + 8) as *mut u64) = add_ref as *const () as u64;
        *((vt + 16) as *mut u64) = add_ref as *const () as u64;
        for &(i, f) in methods {
            *((vt + 8 * i as u64) as *mut u64) = f as u64;
        }
        let obj = crate::mem::heap_alloc(0x40, true) as u64;
        *(obj as *mut u64) = vt;
        obj
    }
}

fn device() -> u64 {
    static D: OnceLock<u64> = OnceLock::new();
    // IDirectInputDevice8: 9 GetDeviceState, 10 GetDeviceData (32 methods)
    *D.get_or_init(|| make_object(&[(9, get_device_state as *const () as usize), (10, get_device_data as *const () as usize)], 32))
}

unsafe extern "win64" fn DirectInput8Create(_inst: A, _ver: A, _iid: A, out: A, _outer: A) -> A {
    static DI: OnceLock<u64> = OnceLock::new();
    // IDirectInput8: 3 CreateDevice (11 methods)
    let di = *DI.get_or_init(|| make_object(&[(3, create_device as *const () as usize)], 11));
    hlog!("null DirectInput: DirectInput8Create -> {di:#x} (no devices)");
    unsafe { (out as *mut u64).write_unaligned(di) };
    0
}

pub fn table() -> Vec<(&'static str, &'static str, usize)> {
    vec![("dinput8.dll", "DirectInput8Create", DirectInput8Create as *const () as usize)]
}
