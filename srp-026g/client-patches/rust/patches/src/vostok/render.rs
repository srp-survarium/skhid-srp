//! The renderer: an effect's compile (its shader configuration and the
//! constants it sends) and the constants the renderer binds to values of
//! ours.
//!
//! At `forward_compile.switch` and `forward_compile.constants`
//! (sites_026g.tsv), inside effect_fstage_default_materials::compile
//! (0x1D8F40, esp 16-aligned by its prologue, no push pending at either
//! site): EBX is the material's config node, the 16-byte shader
//! configuration is at [esp+50h] of the site, EBP is compile's frame (its
//! first argument, the effect compiler, at [ebp+8]). What those registers
//! hold is said here, once.

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::config::{ConfigKey, ConfigNode, Number};
use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, read, write};
use crate::raw;

/// The shader configuration's place in compile's frame, by the site's ESP.
const CONFIGURATION: u32 = 0x50;
/// Its bit use_sequence (in `raw::shader_configuration::sequence`).
const USE_SEQUENCE: u8 = 0x10;
/// Its bit CONFIG_USE_UV_SCROLLING (in `raw::shader_configuration::scrolling`).
const USE_UV_SCROLLING: u8 = 0x40;

impl ConfigKey {
    /// The material's switch of UV scrolling.
    pub const USE_UV_SCROLLING: ConfigKey = ConfigKey::exe(raw::image::USE_UV_SCROLLING_KEY);
    /// The emissive map's scrolling, u and v (the exe names them for
    /// g_stage; the transparency map's are not in the exe).
    pub const EMISSIVE_SCROLL_U: ConfigKey = ConfigKey::exe(raw::image::EMISSIVE_SCROLL_U_KEY);
    pub const EMISSIVE_SCROLL_V: ConfigKey = ConfigKey::exe(raw::image::EMISSIVE_SCROLL_V_KEY);
}

/// compile at one of its two sites.
pub struct ForwardCompile<'c, 't> {
    ctx: &'c mut Context<'t>,
}

impl<'c, 't> ForwardCompile<'c, 't> {
    pub fn at(ctx: &'c mut Context<'t>) -> ForwardCompile<'c, 't> {
        ForwardCompile { ctx }
    }

    /// The material being compiled (EBX).
    pub fn material(&self) -> Option<ConfigNode<'_>> {
        // SAFETY: at both sites EBX is the material's config node, live for
        // the hook.
        unsafe { ConfigNode::at(self.ctx.thread(), self.ctx.reg(Reg::Ebx)) }
    }

    fn configuration(&self) -> *mut c_void {
        self.ctx.site_esp().wrapping_add(CONFIGURATION) as usize as *mut c_void
    }

    /// Whether the material plays a sequence (use_sequence, which compile
    /// has read into the configuration before the first site).
    pub fn uses_sequence(&self) -> bool {
        // SAFETY: the configuration in compile's frame (CONFIGURATION).
        unsafe { read(raw::shader_configuration::sequence(self.configuration())) & USE_SEQUENCE != 0 }
    }

    /// Whether the configuration asks for UV scrolling.
    pub fn uv_scrolling(&self) -> bool {
        // SAFETY: as uses_sequence.
        unsafe { read(raw::shader_configuration::scrolling(self.configuration())) & USE_UV_SCROLLING != 0 }
    }

    /// Asks for UV scrolling: CONFIG_USE_UV_SCROLLING, which
    /// fill_shader_configuration_macros makes a macro of.
    pub fn set_uv_scrolling(&mut self) {
        // SAFETY: as uses_sequence; compile gives this byte to compile_begin
        // after the site, and writes it only where it zeroes it (0x1D8FA3).
        unsafe {
            let at = raw::shader_configuration::scrolling(self.configuration());
            write(at, read(at) | USE_UV_SCROLLING)
        }
    }

    /// The effect compiler, compile's first argument.
    pub fn compiler(&self) -> Option<EffectCompiler<'_>> {
        let frame = self.ctx.reg(Reg::Ebp) as usize as *mut c_void;
        // SAFETY: EBP is compile's frame at both sites; [ebp+8] its first
        // argument, the effect compiler, live through compile.
        let this = unsafe { Object::at(read(raw::effect_compile_frame::compiler(frame))) }?;
        Some(EffectCompiler { this, thread: PhantomData })
    }
}

/// A constant's name, a text of the exe's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstantName(u32);

impl ConstantName {
    pub const UV_SCROLLING_PARAMETERS0: ConstantName = ConstantName(raw::image::UV_SCROLLING_PARAMETERS0_NAME);
    pub const UV_SCROLLING_PARAMETERS1: ConstantName = ConstantName(raw::image::UV_SCROLLING_PARAMETERS1_NAME);
}

/// The effect compiler an effect's compile is given.
pub struct EffectCompiler<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl EffectCompiler<'_> {
    /// Sends the constant `name` = `value`, four floats (set_constant<float4>,
    /// 0x1A8900, which copies them).
    pub fn set_float4(&self, name: ConstantName, value: [Number; 4]) {
        // SAFETY: the compiler compile was given, a text of the exe's, four
        // floats on our stack (Number is an f32's bits).
        unsafe {
            raw::engine_set_constant_float4(value.as_ptr().cast(), self.this.ptr(), name.0 as usize as *const u8)
        }
    }
}

/// The renderer while its context is made (renderer_context's constructor,
/// at `renderer_context.scene_time_bound`).
pub struct RendererConstruction<'t> {
    thread: PhantomData<&'t Thread>,
}

/// A constant binding, as renderer_context's constructor makes one on its
/// stack for register_constant_binding (0x1498A6..0x1498EB): the value's
/// address, its size, the name (a shared string), 0, the size again.
#[repr(C)]
struct ConstantBinding {
    value: *const f32,
    size: u32,
    name: u32,
    zero: u32,
    size_again: u32,
}

impl<'t> RendererConstruction<'t> {
    pub fn at(ctx: &'t Context<'_>) -> RendererConstruction<'t> {
        let _ = ctx.thread();
        RendererConstruction { thread: PhantomData }
    }

    /// Binds the shader constant `name` to the float `value` for the
    /// renderer's life, with the game's own calls for scene_time: the name's
    /// shared string held while register_constant_binding is given the
    /// binding and the resource manager, then let go.
    pub fn bind_constant(&self, name: &'static CStr, value: &'static f32) {
        let mut binding = ConstantBinding { value, size: 4, name: 0, zero: 0, size_again: 4 };
        // SAFETY: a C string of ours; the result a shared string or 0.
        let text = unsafe { raw::engine_shared_string(name.as_ptr().cast()) } as usize as u32;
        if let Some(shared) = SharedString::at(text) {
            binding.name = text;
            shared.hold();
        }
        // SAFETY: the binding on our stack, as the constructor's; the
        // renderer's resource manager from its pointer (raw::image), as the
        // constructor passes it.
        unsafe {
            let manager = read(raw::image::RESOURCE_MANAGER as usize as *mut u32);
            raw::engine_register_constant_binding((&raw const binding).cast(), manager as usize as *mut c_void);
        }
        // The name as the binding holds it after the call (read again, as
        // the constructor does at 0x1498F6).
        // SAFETY: our own binding.
        if let Some(shared) = SharedString::at(unsafe { read(&raw mut binding.name) }) {
            shared.release();
        }
    }
}

/// A shared string (vostok's pooled text): its count at +0.
struct SharedString(Object);

impl SharedString {
    fn at(raw: u32) -> Option<SharedString> {
        Object::at(raw).map(SharedString)
    }

    fn count(&self) -> &AtomicU32 {
        // SAFETY: a live shared string: its count is an aligned dword the
        // game changes with lock xadd only.
        unsafe { AtomicU32::from_ptr(raw::shared_string::count(self.0.ptr())) }
    }

    /// Takes a reference (lock inc, as the constructor's 0x1498D4).
    fn hold(&self) {
        self.count().fetch_add(1, Ordering::SeqCst);
    }

    /// Lets one go; the last destroys it (as the constructor's 0x149900).
    fn release(self) {
        if self.count().fetch_sub(1, Ordering::SeqCst) == 1 {
            // SAFETY: a shared string whose count is now 0.
            unsafe { raw::engine_shared_string_destroy(self.0.ptr()) }
        }
    }
}
