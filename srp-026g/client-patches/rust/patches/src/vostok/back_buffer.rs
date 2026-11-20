//! The game's back buffer read back: the picture a frame has drawn so far,
//! for patch 152's screenshots (docs/notes/screenshots.md).
//!
//! The frame's draw (0x13D010) writes the scene to the swap chain's back
//! buffer (the present stage, 0x13DB36), draws the interface's movies over
//! it (render_flash_ui 0x13CED0, called at 0x13DB3D) and presents
//! (res_render_output::present 0x3CF7F0, at 0x13DD46). At either site
//! ([`Output::before_interface`], [`Output::before_present`]) the back
//! buffer is copied to a staging texture of its size and mapped
//! ([`Output::read`]): a [`Picture`], whose rows are read until it is dropped
//! (unmapped, the staging released).
//!
//! Direct3D's own calls, by the interfaces' declarations (dxgi.h, d3d11.h),
//! on the thread that draws: the immediate context is its alone.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::{null, null_mut, NonNull};

use super::context::Context;
use crate::raw;

/// IID_ID3D11Texture2D, {6F15AAF2-D208-4E89-9AB4-489535D34F9C}.
const TEXTURE2D_IID: [u8; 16] =
    [0xF2, 0xAA, 0x15, 0x6F, 0x08, 0xD2, 0x89, 0x4E, 0x9A, 0xB4, 0x48, 0x95, 0x35, 0xD3, 0x4F, 0x9C];

/// D3D11_USAGE_STAGING.
const STAGING: u32 = 3;
/// D3D11_CPU_ACCESS_READ.
const CPU_READ: u32 = 1 << 17;
/// D3D11_MAP_READ.
const MAP_READ: u32 = 1;
/// The widest and highest texture Direct3D 11 has.
const MOST: u32 = 16384;

/// A D3D11_TEXTURE2D_DESC.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Desc {
    width: u32,
    height: u32,
    mip_levels: u32,
    array_size: u32,
    format: u32,
    samples: u32,
    quality: u32,
    usage: u32,
    bind: u32,
    cpu_access: u32,
    misc: u32,
}

/// A D3D11_MAPPED_SUBRESOURCE.
#[repr(C)]
struct MappedData {
    data: *mut c_void,
    row_pitch: u32,
    depth_pitch: u32,
}

/// Which of a pixel's four bytes is red: the first (R8G8B8A8) or the third
/// (B8G8R8A8); green is the second, the fourth is not colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channels {
    Rgb,
    Bgr,
}

impl Channels {
    /// Of a DXGI format of four bytes a pixel (R8G8B8A8 27..29, B8G8R8A8 and
    /// B8G8R8X8 87..93), typeless or not, sRGB or not: the bytes are the
    /// screen's either way.
    fn of(format: u32) -> Option<Channels> {
        match format {
            27..=29 => Some(Channels::Rgb),
            87 | 88 | 90..=93 => Some(Channels::Bgr),
            _ => None,
        }
    }
}

/// The Direct3D call that refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    GetBuffer,
    CreateTexture2D,
    Map,
}

/// Why a back buffer was not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// No swap chain or no render device (before the renderer is made).
    NoDevice,
    /// The back buffer is not one this reads: its DXGI format, its samples a
    /// pixel (only four bytes a pixel and one sample are read), its size.
    Unread { format: u32, samples: u32, width: u32, height: u32 },
    /// Direct3D refused a call: its HRESULT.
    Refused(Call, u32),
}

/// A reference of ours to a Direct3D texture, released when dropped.
struct Held(raw::HwResource);

impl Drop for Held {
    fn drop(&mut self) {
        // SAFETY: a reference GetBuffer or CreateTexture2D gave us, released once.
        unsafe { raw::engine_d3d_resource_release(Some(self.0)) };
    }
}

/// The render output a frame is drawn to, at one of the frame's sites: on
/// the thread that draws, alive for the hook.
pub struct Output<'t> {
    output: raw::ResRenderOutput,
    site: PhantomData<&'t ()>,
}

impl<'t> Output<'t> {
    /// At `render.flash_ui`: the scene is on the back buffer, the
    /// interface's movies not yet.
    pub fn before_interface(ctx: &'t Context<'_>) -> Option<Output<'t>> {
        // SAFETY: the context is this site's: render_flash_ui's start, its
        // second argument the output window the frame's draw presents to,
        // alive for the frame, on the thread that draws.
        let output = unsafe { raw::site::render_flash_ui::frame(ctx)?.window()?.output() }?;
        Some(Output { output, site: PhantomData })
    }

    /// At `render_output.present`: everything the frame draws is on the back
    /// buffer, about to be presented.
    pub fn before_present(ctx: &'t Context<'_>) -> Option<Output<'t>> {
        // SAFETY: the context is this site's: ESI the render output present
        // runs on, alive while it runs, on the thread that draws.
        let output = unsafe { raw::site::render_output_present::output(ctx) }?;
        Some(Output { output, site: PhantomData })
    }

    /// The back buffer as it is now, copied and mapped. Waits for the GPU to
    /// draw what the frame has asked of it so far.
    pub fn read(&self) -> Result<Picture<'t>, ReadError> {
        // SAFETY: the render output of the site, alive; the render device's
        // global, a pointer the game sets once, and its immediate context.
        let (chain, context) = unsafe {
            let context = match raw::image::device_global().device() {
                Some(device) => device.context(),
                None => None,
            };
            (self.output.swap_chain(), context)
        };
        let (Some(chain), Some(context)) = (chain, context) else { return Err(ReadError::NoDevice) };

        let mut buffer = None;
        // SAFETY: IDXGISwapChain::GetBuffer of the game's live swap chain:
        // buffer 0 as an ID3D11Texture2D, a reference added, into `buffer`.
        let got = unsafe { raw::engine_swap_chain_get_buffer(Some(chain), 0, TEXTURE2D_IID.as_ptr().cast(), &mut buffer) };
        let Some(buffer) = buffer.filter(|_| succeeded(got)).map(Held) else {
            return Err(ReadError::Refused(Call::GetBuffer, got));
        };

        let mut desc = Desc::default();
        // SAFETY: ID3D11Texture2D::GetDesc of the texture held above: it
        // writes a D3D11_TEXTURE2D_DESC, which `desc` is.
        unsafe { raw::engine_texture2d_get_desc(Some(buffer.0), (&raw mut desc).cast()) };
        let channels = Channels::of(desc.format).filter(|_| {
            desc.samples == 1 && (1..=MOST).contains(&desc.width) && (1..=MOST).contains(&desc.height)
        });
        let Some(channels) = channels else {
            return Err(ReadError::Unread {
                format: desc.format,
                samples: desc.samples,
                width: desc.width,
                height: desc.height,
            });
        };

        let staging_desc = Desc {
            mip_levels: 1,
            array_size: 1,
            usage: STAGING,
            bind: 0,
            cpu_access: CPU_READ,
            misc: 0,
            ..desc
        };
        let mut device = null_mut();
        let mut staging = None;
        // SAFETY: ID3D11DeviceChild::GetDevice of the texture (its device, a
        // reference added, never 0 for a live child), that device's
        // CreateTexture2D with a desc of ours and no initial data, the
        // texture into `staging`, and the device's reference let go.
        let made = unsafe {
            raw::engine_device_child_get_device(Some(buffer.0), &mut device);
            if device.is_null() {
                return Err(ReadError::NoDevice);
            }
            let made =
                raw::engine_device_create_texture2d(device, (&raw const staging_desc).cast(), null(), &mut staging);
            raw::engine_d3d_device_release(device);
            made
        };
        let Some(staging) = staging.filter(|_| succeeded(made)).map(Held) else {
            return Err(ReadError::Refused(Call::CreateTexture2D, made));
        };

        let mut mapped = MappedData { data: null_mut(), row_pitch: 0, depth_pitch: 0 };
        // SAFETY: the immediate context on its thread: CopyResource between
        // two textures of one size and format, both held; Map of the staging
        // (CPU read) for reading, waiting for the copy, into `mapped`.
        let got = unsafe {
            raw::engine_context_copy_resource(Some(context), Some(staging.0), Some(buffer.0));
            raw::engine_context_map(Some(context), Some(staging.0), 0, MAP_READ, 0, (&raw mut mapped).cast())
        };
        let data = NonNull::new(mapped.data.cast::<u8>()).filter(|_| succeeded(got));
        // a row holds its pixels (a driver's pitch is never less)
        let data = data.filter(|_| mapped.row_pitch / 4 >= desc.width);
        let Some(data) = data else { return Err(ReadError::Refused(Call::Map, got)) };
        Ok(Picture {
            context,
            staging,
            data,
            pitch: mapped.row_pitch,
            width: desc.width,
            height: desc.height,
            channels,
            site: PhantomData,
        })
    }
}

/// Whether an HRESULT says the call succeeded.
fn succeeded(result: u32) -> bool {
    result & 1 << 31 == 0
}

/// A back buffer's copy, mapped: its rows, top to bottom, until it is
/// dropped (unmapped through the context it was mapped with, the staging
/// released).
pub struct Picture<'t> {
    context: raw::ImmediateContext,
    staging: Held,
    data: NonNull<u8>,
    pitch: u32,
    width: u32,
    height: u32,
    channels: Channels,
    site: PhantomData<&'t ()>,
}

impl Picture<'_> {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn channels(&self) -> Channels {
        self.channels
    }

    /// Row `y`'s pixels, four bytes each; empty past the last row.
    pub fn row(&self, y: u32) -> &[u8] {
        if y >= self.height {
            return &[];
        }
        // SAFETY: a mapped staging of `height` rows holds `pitch` bytes a
        // row (D3D11_MAPPED_SUBRESOURCE), the pitch at least the pixels'
        // bytes (read checked it), alive until the unmap (our drop).
        unsafe {
            core::slice::from_raw_parts(self.data.as_ptr().add(y as usize * self.pitch as usize), self.width as usize * 4)
        }
    }
}

impl Drop for Picture<'_> {
    fn drop(&mut self) {
        // SAFETY: the staging mapped by read, through the context that
        // mapped it, on its thread (a Picture does not leave the hook).
        unsafe { raw::engine_context_unmap(Some(self.context), Some(self.staging.0), 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_formats_read_are_four_bytes_a_pixel() {
        assert_eq!(Channels::of(28), Some(Channels::Rgb), "R8G8B8A8_UNORM, the game's");
        assert_eq!(Channels::of(29), Some(Channels::Rgb), "R8G8B8A8_UNORM_SRGB");
        assert_eq!(Channels::of(87), Some(Channels::Bgr), "B8G8R8A8_UNORM");
        assert_eq!(Channels::of(24), None, "R10G10B10A2_UNORM");
        assert_eq!(Channels::of(10), None, "R16G16B16A16_FLOAT");
        assert_eq!(Channels::of(89), None, "R10G10B10_XR_BIAS_A2_UNORM");
    }

    #[test]
    fn an_hresult_succeeds_without_its_top_bit() {
        assert!(succeeded(0));
        assert!(succeeded(1), "S_FALSE");
        assert!(!succeeded(0x887A_0005), "DXGI_ERROR_DEVICE_REMOVED");
    }
}
