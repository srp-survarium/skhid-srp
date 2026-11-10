//! Texture streaming's per-frame bookkeeping as patch 137 (texture_streaming) reads and writes
//! it (client_patch.md 3.79): the scene's streamable textures and the instances drawn with each,
//! and the resource manager's texture registry.
//!
//! - The scene (+126158h) keeps a vector of streamable textures (148h bytes each); each has a
//!   list of the objects drawn with it (+110h), one record per object, whose proxy (a
//!   streaming_texture_instance_proxy_base) answers for the object. scene::update_streaming_data
//!   (0x155980) rewrites every record from its proxy each frame, through four virtual calls:
//!   bound_sphere, last_render_time, texel_factor and +14h. The exe has three classes of proxy
//!   (its RTTI): a model's surface, a grass patch and a decal. [`Proxy`] is what each reports,
//!   read once here from its object by its vtable; [`update_streaming_data`] walks the records
//!   in the native's order and writes back what the patch makes of it.
//! - The resource manager's texture registry (+8809Ch) is an STLport map from a texture's name
//!   to it. resource_manager::get_texture_video_memory_size (0x3BE280) sums the video memory of
//!   every texture whose name lacks "$user$" (strstr each), for renderer::render's statistics
//!   each frame. [`texture_registry`] walks it in the map's order and answers the name test
//!   with SSE2, 16 bytes at a time.
//!
//! The objects are the render thread's: both functions run on it, from process_streaming and
//! renderer::render, and nothing else writes the records or the registry while they run.

use core::arch::x86::{__m128i, _mm_cmpeq_epi8, _mm_load_si128, _mm_movemask_epi8, _mm_set1_epi8, _mm_storeu_si128};
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::world::{floats, read, write_floats};
use crate::raw;

/// A model surface's view of its instance, what its proxy's virtuals read
/// (render_surface_texture_instance_proxy, vtable 9F7060h).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    /// The instance's bound sphere (+10h): centre x, y, z, radius.
    pub sphere: [f32; 4],
    /// When it was last drawn (+48h).
    pub last_render_time: f32,
    /// The first three rows of its world matrix (+50h), x, y, z of each.
    pub rows: [[f32; 3]; 3],
    /// Its render surface's streaming_texture_factor (+38h, +4Ch).
    pub factor: f32,
}

/// A decal's box (+74h): min x, y, z, then max.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// What a texture instance's proxy answers for, by its class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Proxy {
    /// A model's surface (vtable 9F7060h).
    RenderSurface(Surface),
    /// A grass patch (vtable 9F7080h): its answers are constants.
    GrassPatch,
    /// A decal (vtable 9F70A0h).
    Decal(Aabb),
}

/// What a streaming record keeps of its proxy each frame: the bound sphere (centre, radius),
/// last_render_time and texel_factor, as the record's f32 (+0Ch, +1Ch, +20h).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub sphere: [f32; 4],
    pub last_render_time: f32,
    pub texel_factor: f32,
}

/// A proxy read: what it answers for, and the surface whose streaming mark its slot 14h
/// clears (a model's surface only; the other classes' slot 14h is a bare `ret`, 0x1AF330).
struct Read {
    proxy: Proxy,
    marked: Option<raw::RenderSurfaceInstance>,
}

/// Reads a proxy by its class. The exe has no other class of proxy: another vtable is a
/// broken record, a crash as the native's call through it would be.
///
/// # Safety
/// `proxy` is a live record's proxy, on the render thread.
unsafe fn read_proxy(proxy: raw::TextureInstanceProxy) -> Read {
    // SAFETY: the caller's: a live proxy; each class's fields are read behind its vtable only,
    // the pointers there as the native's methods follow them (none is 0 in a live proxy: the
    // native would fault on it).
    unsafe {
        match read(proxy.vtable()) {
            raw::image::RENDER_SURFACE_PROXY_VTABLE => {
                let instance = proxy.render_surface().instance().expect("a surface proxy's instance");
                let matrix = instance.transform();
                assert!(!matrix.is_null(), "a drawn surface's world matrix");
                let m: [f32; 12] = floats(matrix);
                let surface = instance.surface().expect("a surface instance's render surface");
                Read {
                    proxy: Proxy::RenderSurface(Surface {
                        sphere: floats(instance.sphere()),
                        last_render_time: read(instance.last_render_time()),
                        rows: [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]],
                        factor: read(surface.streaming_texture_factor()),
                    }),
                    marked: Some(instance),
                }
            }
            raw::image::GRASS_PATCH_PROXY_VTABLE => Read { proxy: Proxy::GrassPatch, marked: None },
            raw::image::DECAL_PROXY_VTABLE => {
                let decal = proxy.decal().decal().expect("a decal proxy's decal");
                let b: [f32; 6] = floats(decal.aabb());
                Read { proxy: Proxy::Decal(Aabb { min: [b[0], b[1], b[2]], max: [b[3], b[4], b[5]] }), marked: None }
            }
            _ => panic!("a streaming record's proxy of no known class"),
        }
    }
}

/// scene::update_streaming_data at its start (`streaming.update`): every record of every
/// streamable texture, the vector's order and each list's, rewritten with what `record` makes
/// of its proxy and the proxy's owner (+4), then the proxy's slot 14h (a surface's streaming
/// mark cleared). The caller leaves by the site's `returned` exit.
pub fn update_streaming_data(ctx: &Context<'_>, mut record: impl FnMut(Proxy) -> Record) {
    use raw::site::streaming_update as site;
    // SAFETY: the site's EAX, process_streaming's scene, alive through the frame.
    let scene = unsafe { site::scene(ctx) }.expect("update_streaming_data's scene");
    // SAFETY: the scene's vector of streamable textures, begin and end (an empty one may be
    // two zeros).
    let (first, end) = unsafe { (scene.streamable(), scene.streamable_end()) };
    let count = match (first, end) {
        (Some(first), Some(end)) => end.index_from(first).expect("the streamable textures' vector"),
        (None, None) => 0,
        _ => panic!("the streamable textures' vector"),
    };
    for i in 0..count {
        // SAFETY: below the vector's length; its list of records, each live while listed.
        let mut next = unsafe { first.expect("counted").at(i).instances() };
        while let Some(at) = next {
            // SAFETY: a listed record and its proxy (the native calls it unchecked); the record
            // is the scene's, written on the render thread only.
            unsafe {
                let proxy = at.proxy().expect("a streaming record's proxy");
                let Read { proxy: answers, marked } = read_proxy(proxy);
                let made = record(answers);
                write_floats(at.sphere(), made.sphere);
                at.last_render_time().write_volatile(made.last_render_time);
                at.texel_factor().write_volatile(made.texel_factor);
                at.owner().write_volatile(read(proxy.owner()));
                if let Some(instance) = marked {
                    instance.streaming_mark().write_volatile(0);
                }
                next = at.next();
            }
        }
    }
}

/// The resource manager's texture registry at get_texture_video_memory_size's start
/// (`streaming.texture_memory`), in the map's order (its in-order successor,
/// _Rb_global::_M_increment 0x405490, from the header's first to the header).
pub struct TextureRegistry<'t> {
    header: raw::TextureRegistryNode,
    at: raw::TextureRegistryNode,
    thread: PhantomData<&'t Thread>,
}

/// One registered texture.
pub struct RegisteredTexture<'t> {
    node: raw::TextureRegistryNode,
    thread: PhantomData<&'t Thread>,
}

pub fn texture_registry<'t>(ctx: &'t Context<'_>) -> TextureRegistry<'t> {
    use raw::site::streaming_texture_memory as site;
    // SAFETY: the site's EAX, the render's resource manager (renderer::render reads the global
    // 0x226047C into it); its registry's header and first node.
    unsafe {
        let manager = site::manager(ctx).expect("get_texture_video_memory_size's resource manager");
        let header = manager.textures();
        TextureRegistry { header, at: header.left().expect("the registry's first"), thread: PhantomData }
    }
}

impl<'t> Iterator for TextureRegistry<'t> {
    type Item = RegisteredTexture<'t>;

    fn next(&mut self) -> Option<RegisteredTexture<'t>> {
        let node = self.at;
        if node.address() == self.header.address() {
            return None;
        }
        // SAFETY: a node of the live map (the walk starts at its first and stops at its header):
        // its links, as _M_increment follows them.
        self.at = unsafe { successor(node) };
        Some(RegisteredTexture { node, thread: PhantomData })
    }
}

/// The node after `x` in the map's order (_M_increment): the leftmost of its right subtree;
/// else up while it is a right child, and that parent, unless the climb ended at the header
/// (the root's parent is the header, whose right is the last node).
///
/// # Safety
/// `x` is a node of a live map other than its header.
unsafe fn successor(x: raw::TextureRegistryNode) -> raw::TextureRegistryNode {
    // SAFETY: the caller's; every link followed is the map's.
    unsafe {
        if let Some(mut y) = x.right() {
            while let Some(left) = y.left() {
                y = left;
            }
            return y;
        }
        let mut x = x;
        let mut y = x.parent().expect("a map node's parent");
        while y.right().map(|r| r.address()) == Some(x.address()) {
            x = y;
            y = y.parent().expect("a map node's parent");
        }
        if x.right().map(|r| r.address()) != Some(y.address()) { y } else { x }
    }
}

impl RegisteredTexture<'_> {
    /// Whether `needle` occurs in the texture's name (strstr's answer, before its NUL).
    pub fn name_contains(&self, needle: &Needle) -> bool {
        // SAFETY: the node's key, a NUL-terminated buffer string.
        let name = unsafe { self.node.name() };
        assert!(!name.is_null(), "a registered texture's name");
        let start = name as usize;
        let base = start & !15;
        // SAFETY: an aligned 16 bytes never cross a page, so a block holding a byte of the name
        // is readable whole, before its first byte and past its NUL; find_in_blocks asks for a
        // block only while none before it held the NUL, so each one asked holds a byte of it.
        find_in_blocks(needle, start - base, |i| unsafe { _mm_load_si128((base + 16 * i) as *const __m128i) })
    }

    /// Its video memory, bytes (m_mem_usage).
    pub fn memory(&self) -> u32 {
        // SAFETY: the node's value, the registered texture (the native reads it unchecked).
        unsafe { read(self.node.texture().expect("a registered texture").memory()) }
    }
}

/// get_texture_video_memory_size's answer (EAX), for its `returned` exit.
pub fn texture_memory_result(ctx: &mut Context<'_>, bytes: u32) {
    ctx.set_reg(Reg::Eax, bytes);
}

/// A string to find in a name: 1 to 16 bytes, none of them NUL (a match must end before the
/// name's NUL, which no byte of the needle can stand for). Made at compile time.
pub struct Needle {
    bytes: [u8; 16],
    len: usize,
}

impl Needle {
    pub const fn new(text: &[u8]) -> Needle {
        assert!(!text.is_empty() && text.len() <= 16, "a needle of 1 to 16 bytes");
        let mut bytes = [0; 16];
        let mut i = 0;
        while i < text.len() {
            assert!(text[i] != 0, "a needle without NUL");
            bytes[i] = text[i];
            i += 1;
        }
        Needle { bytes, len: text.len() }
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// The positions of `byte` in a block, a bit each (SSE2: pcmpeqb, pmovmskb).
fn positions(block: __m128i, byte: u8) -> u32 {
    // SAFETY: SSE2 is the build's baseline (i686, target-cpu pentium4: link_026g.py; the host
    // tests' i686 too).
    unsafe { _mm_movemask_epi8(_mm_cmpeq_epi8(block, _mm_set1_epi8(byte as i8))) as u32 }
}

/// A block's bytes.
fn bytes(block: __m128i) -> [u8; 16] {
    let mut out = [0; 16];
    // SAFETY: SSE2 (as `positions`); writes the 16 bytes of `out`.
    unsafe { _mm_storeu_si128(out.as_mut_ptr().cast(), block) };
    out
}

/// Whether `needle` occurs in a NUL-terminated text before its NUL, the text given as the
/// aligned 16-byte blocks of its memory: `block(0)` holds its first byte at `skip`, and
/// `block(i)` is asked only while no block before it held the NUL. A block with neither the
/// needle's first byte nor a NUL is passed over whole; in another, each place of the first
/// byte before the NUL is a candidate, and one near the block's end reads on into the next
/// block, which then holds a byte of the text (no NUL in this one: the candidate is before it,
/// and the needle has none to match one).
fn find_in_blocks(needle: &Needle, skip: usize, mut block: impl FnMut(usize) -> __m128i) -> bool {
    let needle = needle.bytes();
    let mut index = 0;
    let mut from = !0u32 << skip;
    loop {
        let current = block(index);
        let nul = positions(current, 0) & from;
        let first = positions(current, needle[0]) & from;
        if nul | first != 0 {
            let before = if nul == 0 { !0 } else { (1u32 << nul.trailing_zeros()) - 1 };
            let mut candidates = first & before;
            let here = bytes(current);
            while candidates != 0 {
                let at = candidates.trailing_zeros() as usize;
                candidates &= candidates - 1;
                let split = needle.len().min(16 - at);
                if here[at..at + split] == needle[..split]
                    && (split == needle.len() || bytes(block(index + 1))[..needle.len() - split] == needle[split..])
                {
                    return true;
                }
            }
            if nul != 0 {
                return false;
            }
        }
        index += 1;
        from = !0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: Needle = Needle::new(b"$user$");

    fn load(block: &[u8; 16]) -> __m128i {
        // SAFETY: SSE2; reads the 16 bytes of `block`.
        unsafe { core::arch::x86::_mm_loadu_si128(block.as_ptr().cast()) }
    }

    /// `text` (its NUL added) placed `skip` bytes into aligned blocks, the rest filled with
    /// `fill`; the blocks asked are counted, and none may be asked past the NUL's.
    fn find(needle: &Needle, text: &[u8], skip: usize, fill: u8) -> bool {
        let mut memory = [[fill; 16]; 8];
        for (i, &b) in text.iter().chain([0u8].iter()).enumerate() {
            memory[(skip + i) / 16][(skip + i) % 16] = b;
        }
        let last = (skip + text.len()) / 16;
        find_in_blocks(needle, skip, |i| {
            assert!(i <= last, "a block past the NUL's asked");
            load(&memory[i])
        })
    }

    fn strstr(text: &[u8], needle: &[u8]) -> bool {
        text.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn as_strstr_at_every_place() {
        let names: &[&[u8]] = &[
            b"",
            b"$",
            b"$user",
            b"$user$",
            b"$user$rt_0",
            b"textures/terrain/grass_01",
            b"textures/$user$/skin",
            b"$$user$$",
            b"$use$user$",
            b"$us$user",
            b"textures/very/long/path/to/some/texture_name_$user$",
            b"textures/very/long/path/to/some/texture_name_$user",
            b"user$user$user",
            b"$user$$user$",
        ];
        for name in names {
            for skip in 0..16 {
                for fill in [0u8, b'$', b'u', b'x', 0xFF] {
                    assert_eq!(find(&USER, name, skip, fill), strstr(name, b"$user$"),
                               "{:?} at {skip}, fill {fill}", core::str::from_utf8(name));
                }
            }
        }
    }

    #[test]
    fn a_match_across_a_block() {
        for skip in 0..16 {
            for pad in 0..20 {
                let mut text = [b'a'; 40];
                text[pad..pad + 6].copy_from_slice(b"$user$");
                assert!(find(&USER, &text[..pad + 6], skip, 0));
                assert!(!find(&USER, &text[..pad + 5], skip, b'$'), "cut by its NUL");
            }
        }
    }

    #[test]
    fn nothing_after_the_nul() {
        let mut memory = [[0u8; 16]; 2];
        memory[0][..4].copy_from_slice(b"abc\0");
        memory[0][4..10].copy_from_slice(b"$user$");
        assert!(!find_in_blocks(&USER, 0, |i| load(&memory[i])));
    }

    // The patch's x87 model (texture_streaming's `record`) against this CPU's x87: the host
    // tests are i686 code, so the native's own instruction sequences run here.

    /// The render surface proxy's texel_factor (0x1C8B90) as its instructions compute it, the
    /// sqrt call (0x38960: fld, fsqrt) inlined, under the x87 control word `control`.
    fn native_texel_factor(matrix: &[f32; 16], factor: f32, control: u16) -> f32 {
        let (mut out, mut temps, mut saved) = (0f32, [0f32; 4], 0u16);
        // SAFETY: reads `matrix` and `factor`, writes `temps`, `out` and `saved`; the control
        // word is put back; every register it uses is declared.
        unsafe {
            core::arch::asm!(
                "fnstcw word ptr [{saved}]",
                "fldcw word ptr [{control}]",
                "movss xmm1, dword ptr [{m} + 4]",
                "movss xmm0, dword ptr [{m} + 8]",
                "movss xmm2, dword ptr [{m}]",
                "movaps xmm3, xmm1", "mulss xmm3, xmm1", "movaps xmm1, xmm0", "mulss xmm1, xmm0",
                "movaps xmm0, xmm2", "addss xmm3, xmm1", "mulss xmm0, xmm2", "addss xmm3, xmm0",
                "movss dword ptr [{t}], xmm3", "fld dword ptr [{t}]", "fsqrt", "fstp dword ptr [{t} + 4]",
                "movss xmm2, dword ptr [{m} + 16]",
                "movss xmm1, dword ptr [{m} + 20]",
                "movss xmm0, dword ptr [{m} + 24]",
                "movaps xmm3, xmm2", "mulss xmm3, xmm2", "movaps xmm2, xmm1", "mulss xmm2, xmm1",
                "movaps xmm1, xmm0", "addss xmm3, xmm2", "mulss xmm1, xmm0", "addss xmm3, xmm1",
                "movss dword ptr [{t}], xmm3", "fld dword ptr [{t}]", "fsqrt", "fstp dword ptr [{t} + 8]",
                "movss xmm2, dword ptr [{m} + 36]",
                "movss xmm1, dword ptr [{m} + 40]",
                "movss xmm0, dword ptr [{m} + 32]",
                "movaps xmm3, xmm2", "mulss xmm3, xmm2", "movaps xmm2, xmm1", "mulss xmm2, xmm1",
                "movaps xmm1, xmm0", "addss xmm3, xmm2", "mulss xmm1, xmm0", "addss xmm3, xmm1",
                "movss dword ptr [{t}], xmm3", "fld dword ptr [{t}]", "fsqrt", "fst dword ptr [{t} + 12]",
                "fld dword ptr [{t} + 8]",
                "fcomip st, st(1)",
                "fstp st(0)",
                "jbe 2f",
                "movss xmm0, dword ptr [{t} + 8]",
                "jmp 3f",
                "2:",
                "movss xmm0, dword ptr [{t} + 12]",
                "3:",
                "movss xmm1, dword ptr [{t} + 4]",
                "comiss xmm1, xmm0",
                "jbe 4f",
                "fld dword ptr [{f}]", "movss dword ptr [{t}], xmm1", "fmul dword ptr [{t}]",
                "jmp 5f",
                "4:",
                "fld dword ptr [{f}]", "movss dword ptr [{t}], xmm0", "fmul dword ptr [{t}]",
                "5:",
                "fstp dword ptr [{o}]",
                "fldcw word ptr [{saved}]",
                saved = in(reg) &mut saved,
                control = in(reg) &control,
                m = in(reg) matrix.as_ptr(),
                t = in(reg) temps.as_mut_ptr(),
                f = in(reg) &factor,
                o = in(reg) &mut out,
                out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _,
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            );
        }
        out
    }

    /// The surface proxy's last_render_time (0x1C8B80: fld) as update_streaming_data stores it
    /// (fstp).
    fn native_load(x: f32) -> f32 {
        let mut out = 0f32;
        // SAFETY: reads `x`, writes `out`; the x87 stack is left as it was.
        unsafe {
            core::arch::asm!("fld dword ptr [{x}]", "fstp dword ptr [{o}]", x = in(reg) &x, o = in(reg) &mut out,
                             out("st(0)") _);
        }
        out
    }

    const SPECIAL: [u32; 18] = [
        0x0000_0000, 0x8000_0000, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0000, 0xFFC0_0000, 0x7FC1_2345, 0xFFC0_0001,
        0x7F80_0001, 0xFF80_ABCD, 0x7FA0_0000, 0x0000_0001, 0x807F_FFFF, 0x0040_0000, 0x7F7F_FFFF, 0x5F80_0000,
        0x1F80_0000, 0x9F00_0001,
    ];

    /// A float of the specials or a random one, from a xorshift state.
    fn a_float(state: &mut u32, special: u32) -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        if *state % 100 < special {
            f32::from_bits(SPECIAL[(*state >> 8) as usize % SPECIAL.len()])
        } else {
            ((*state >> 8) as f32 / 16_777_216.0 - 0.5) * 200.0
        }
    }

    #[test]
    fn the_x87_model_is_this_cpus_x87() {
        use crate::texture_streaming::record;
        let mut state = 0x1234_5678u32;
        for round in 0..200_000 {
            let special = [0, 10, 50][round % 3];
            let mut matrix = [0f32; 16];
            for value in &mut matrix {
                *value = a_float(&mut state, special);
            }
            let factor = a_float(&mut state, special);
            let time = a_float(&mut state, special);
            let rows = [[matrix[0], matrix[1], matrix[2]], [matrix[4], matrix[5], matrix[6]],
                        [matrix[8], matrix[9], matrix[10]]];
            let made = record(Proxy::RenderSurface(Surface { sphere: [0.0; 4], last_render_time: time, rows, factor }));
            for control in [0x27F, 0x37F] {
                let native = native_texel_factor(&matrix, factor, control);
                assert_eq!(made.texel_factor.to_bits(), native.to_bits(), "{matrix:?} {factor:?} {control:#x}");
            }
            assert_eq!(made.last_render_time.to_bits(), native_load(time).to_bits(), "{time:?}");
        }
    }
}
