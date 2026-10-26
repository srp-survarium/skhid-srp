//! The game's configs (vostok::configs::binary_config_value): a material's
//! settings, read by key. A node is asked whether it has a key
//! (value_exists 0xF6C00) and then for that key's node (operator[] 0xF6B10,
//! which assumes the key is there: ask first); what a key holds is its
//! child "value"'s node, as the game's own reads (effect_fstage_default_
//! materials::compile 0x1D9003..0x1D9016) do.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::hook::Thread;
use super::object::{Object, read};
use crate::raw;

/// A key's name: a C string of the exe's (its texts, `raw::image`) or one
/// of ours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigKey(*const u8);

impl ConfigKey {
    /// A key the exe has no text for.
    pub const fn ours(name: &'static CStr) -> ConfigKey {
        ConfigKey(name.as_ptr().cast())
    }

    /// A key named by the exe's own text at `va`.
    pub(in crate::vostok) const fn exe(va: u32) -> ConfigKey {
        ConfigKey(va as usize as *const u8)
    }

    /// The key's text, for the game's lookups.
    pub(in crate::vostok) fn raw(self) -> *const u8 {
        self.0
    }
}

/// "value": a key's node holds what it says in this child.
const VALUE: ConfigKey = ConfigKey::exe(raw::image::VALUE_KEY);

/// A config node the game gave a hook (a material).
pub struct ConfigNode<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ConfigNode<'t> {
    /// # Safety
    /// `raw` is a config node of the game's, live for the hook (`'t`).
    pub(in crate::vostok) unsafe fn at(_: &'t Thread, raw: u32) -> Option<ConfigNode<'t>> {
        Object::at(raw).map(|this| ConfigNode { this, thread: PhantomData })
    }

    /// What the node's `key` holds: its "value" node, none when the node
    /// has no such key (or the game's lookup gives none).
    pub fn value(&self, key: ConfigKey) -> Option<ConfigValue<'t>> {
        // SAFETY: `this` is a live config node (at's contract), `key` a C
        // string; operator[] is asked only for a key value_exists said the
        // node has, and its result's "value", as the game's reads do.
        unsafe {
            if raw::engine_cfg_has(self.this.ptr(), key.0) & 0xFF == 0 {
                return None;
            }
            let node = raw::engine_cfg_child(self.this.ptr(), key.0);
            let value = raw::engine_cfg_child(node, VALUE.0);
            Object::at(value as usize as u32).map(|this| ConfigValue { this, thread: PhantomData })
        }
    }
}

/// A key's "value" node.
pub struct ConfigValue<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl ConfigValue<'_> {
    fn raw_value(&self) -> u32 {
        // SAFETY: a value node operator[] gave (ConfigNode::value); its
        // first dword is the value.
        unsafe { read(raw::binary_config_value::value(self.this.ptr())) }
    }

    /// A switch: on when its dword is not 0 (compile's own read, 0x1D9016).
    pub fn switch(&self) -> bool {
        self.raw_value() != 0
    }

    /// A number: a float as it is, an integer made a float (cvtsi2ss), as
    /// compile reads its numbers (0x1D9249).
    pub fn number(&self) -> Number {
        // SAFETY: as raw_value; the type word is at +14h.
        let kind = unsafe { read(raw::binary_config_value::kind(self.this.ptr())) };
        if kind == FLOAT {
            Number(self.raw_value())
        } else {
            Number::from_int(self.raw_value() as i32)
        }
    }
}

/// The type word of a float value.
const FLOAT: u16 = 2;

/// A config's number, an f32 kept as its bits: moved and stored, never
/// computed on, so the game gets the bits the config gave (an f32 an i686
/// function returns goes through the x87's ST0, where a signaling NaN
/// would be quieted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Number(u32);

impl Number {
    pub const ZERO: Number = Number(0);

    /// An integer made a float: cvtsi2ss's rounding (to nearest, ties to
    /// even), which Rust's `as` is.
    pub fn from_int(value: i32) -> Number {
        Number((value as f32).to_bits())
    }

    pub fn to_bits(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_are_made_floats_as_cvtsi2ss() {
        assert_eq!(Number::from_int(-2).to_bits(), (-2.0f32).to_bits());
        assert_eq!(Number::from_int(16_777_217).to_bits(), 16_777_216.0f32.to_bits()); // ties to even
        assert_eq!(Number::from_int(i32::MIN).to_bits(), (-2_147_483_648.0f32).to_bits());
        assert_eq!(Number::ZERO.to_bits(), 0);
    }
}
