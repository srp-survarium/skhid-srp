//! Extend the native match table without changing existing message metadata.
//! The lobby has its own table and never reaches these match-only sites.
use super::Object;
use crate::raw;
use crate::vostok::{Context, Reg};

pub const REQUEST_ID: u32 = 0xD7;
pub const RELAY_ID: u32 = 0xF7;

pub fn extend_ranges(ctx: &Context<'_>) {
    let table = Object::at(ctx.reg(Reg::Esi)).expect("missing constructed match table");
    // SAFETY: 2792DC follows the four bound stores into this live getter;
    // extending these two bytes leaves its vtable and roster width untouched.
    unsafe {
        raw::match_message_table::received_max(table.ptr()).write(RELAY_ID as u8);
        raw::match_message_table::sending_min(table.ptr()).write(REQUEST_ID as u8);
    }
}

/// Set a pickup message's fixed length and checked availability, or leave the
/// native switch untouched. No payload bytes are read or cursor advanced.
pub fn set_length(ctx: &mut Context<'_>) -> bool {
    let length = match ctx.reg(Reg::Esi) {
        REQUEST_ID => weapon_pickup::input::BYTES as u32,
        RELAY_ID => weapon_pickup::input::BYTES as u32 + 1,
        _ => return false,
    };
    let frame = Object::at(ctx.reg(Reg::Ebp)).expect("missing message-length frame");
    // SAFETY: 279694's EBP frame contains the by-value reader and output
    // pointer; native code has just written that output at 279691.
    let (begin, cursor, size, ok) = unsafe {
        (
            *raw::message_length_frame::begin(frame.ptr()),
            *raw::message_length_frame::cursor(frame.ptr()),
            *raw::message_length_frame::size(frame.ptr()),
            *raw::message_length_frame::ok(frame.ptr()),
        )
    };
    let available = begin
        .checked_add(size)
        .is_some_and(|end| cursor >= begin && cursor <= end && end - cursor >= length);
    assert_ne!(ok, 0, "missing message-length output");
    // SAFETY: the native function's required output pointer, borrowed here.
    unsafe { (ok as *mut u8).write(u8::from(available)) };
    ctx.set_reg(Reg::Eax, length);
    true
}
