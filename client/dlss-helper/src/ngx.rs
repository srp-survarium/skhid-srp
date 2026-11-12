//! NVIDIA NGX (DLSS) through the SDK's own static library, nvsdk_ngx_s.lib
//! (build.rs links it): it finds the driver's NGX core (beside the exe, the
//! driver store, or the NGXCore registry key Wine/Proton set) and loads
//! nvngx_dlss.dll from this exe's folder.
//!
//! This software contains source code provided by NVIDIA Corporation: the
//! declarations below follow the DLSS SDK's headers (nvsdk_ngx*.h, 310.9.16).

use ash::vk;
use ash::vk::Handle;
use std::ffi::{CStr, c_char, c_void};

pub type NgxResult = u32;
const RESULT_SUCCESS: NgxResult = 0x1;
pub fn failed(r: NgxResult) -> bool {
    r & 0xFFF0_0000 == 0xBAD0_0000
}
pub fn ok(r: NgxResult) -> bool {
    !failed(r) && r == RESULT_SUCCESS
}

/// NGX's name for a result.
pub fn describe(r: NgxResult) -> String {
    let name = match r {
        0x1 => "Success",
        0xBAD00000 => "Fail",
        0xBAD00001 => "FeatureNotSupported",
        0xBAD00002 => "PlatformError",
        0xBAD00003 => "FeatureAlreadyExists",
        0xBAD00004 => "FeatureNotFound",
        0xBAD00005 => "InvalidParameter",
        0xBAD00006 => "ScratchBufferTooSmall",
        0xBAD00007 => "NotInitialized",
        0xBAD00008 => "UnsupportedInputFormat",
        0xBAD00009 => "RWFlagMissing",
        0xBAD0000A => "MissingInput",
        0xBAD0000B => "UnableToInitializeFeature",
        0xBAD0000C => "OutOfDate",
        0xBAD0000D => "OutOfGPUMemory",
        0xBAD0000E => "UnsupportedFormat",
        0xBAD0000F => "UnableToWriteToAppDataPath",
        0xBAD00010 => "UnsupportedParameter",
        0xBAD00011 => "Denied",
        0xBAD00012 => "NotImplemented",
        _ => "",
    };
    format!("{r:#x} {name}")
}

const VERSION_API: u32 = 0x15;
const FEATURE_SUPER_SAMPLING: u32 = 1;
const ENGINE_TYPE_CUSTOM: u32 = 0;
const LOGGING_LEVEL_ON: u32 = 1;
/// Skhid's NGX project id (a GUID made for it, 2026-11-12), with engine type
/// CUSTOM: how a title without an NVIDIA application id initializes NGX.
pub const PROJECT_ID: &CStr = c"c7bbddb5-39f3-4445-9b6b-e2659022068c";
pub const ENGINE_VERSION: &CStr = c"0.26g";

#[repr(C)]
pub struct Parameter {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct Handle_ {
    _opaque: [u8; 0],
}

#[repr(C)]
struct PathListInfo {
    path: *const *const u16,
    length: u32,
}
#[repr(C)]
struct LoggingInfo {
    callback: Option<extern "C" fn(*const c_char, u32, u32)>,
    minimum_level: u32,
    disable_other_sinks: bool,
}
#[repr(C)]
struct FeatureCommonInfo {
    path_list: PathListInfo,
    internal: *mut c_void,
    logging: LoggingInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ImageViewInfo {
    view: vk::ImageView,
    image: vk::Image,
    range: vk::ImageSubresourceRange,
    format: vk::Format,
    width: u32,
    height: u32,
}
/// NVSDK_NGX_Resource_VK: an image view (the buffer arm of its union is smaller).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ResourceVk {
    info: ImageViewInfo,
    kind: u32,
    read_write: bool,
}

impl ResourceVk {
    pub fn image(view: vk::ImageView, image: vk::Image, aspect: vk::ImageAspectFlags, format: vk::Format, size: (u32, u32), read_write: bool) -> Self {
        ResourceVk {
            info: ImageViewInfo {
                view,
                image,
                range: vk::ImageSubresourceRange { aspect_mask: aspect, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 },
                format,
                width: size.0,
                height: size.1,
            },
            kind: 0, // NVSDK_NGX_RESOURCE_VK_TYPE_VK_IMAGEVIEW
            read_write,
        }
    }
}

unsafe extern "C" {
    fn NVSDK_NGX_VULKAN_RequiredExtensions(ni: *mut u32, ie: *mut *const *const c_char, nd: *mut u32, de: *mut *const *const c_char) -> NgxResult;
    #[allow(clippy::too_many_arguments)]
    fn NVSDK_NGX_VULKAN_Init_with_ProjectID(
        project_id: *const c_char,
        engine_type: u32,
        engine_version: *const c_char,
        data_path: *const u16,
        instance: u64,
        pd: u64,
        device: u64,
        gipa: vk::PFN_vkGetInstanceProcAddr,
        gdpa: vk::PFN_vkGetDeviceProcAddr,
        feature_info: *const FeatureCommonInfo,
        version: u32,
    ) -> NgxResult;
    fn NVSDK_NGX_VULKAN_Shutdown1(device: u64) -> NgxResult;
    fn NVSDK_NGX_VULKAN_GetCapabilityParameters(out: *mut *mut Parameter) -> NgxResult;
    fn NVSDK_NGX_VULKAN_AllocateParameters(out: *mut *mut Parameter) -> NgxResult;
    fn NVSDK_NGX_VULKAN_DestroyParameters(p: *mut Parameter) -> NgxResult;
    fn NVSDK_NGX_VULKAN_CreateFeature1(device: u64, cmd: u64, feature: u32, p: *mut Parameter, out: *mut *mut Handle_) -> NgxResult;
    fn NVSDK_NGX_VULKAN_ReleaseFeature(h: *mut Handle_) -> NgxResult;
    fn NVSDK_NGX_VULKAN_EvaluateFeature_C(cmd: u64, h: *const Handle_, p: *const Parameter, callback: *const c_void) -> NgxResult;
    fn NVSDK_NGX_Parameter_SetI(p: *mut Parameter, name: *const c_char, v: i32);
    fn NVSDK_NGX_Parameter_SetUI(p: *mut Parameter, name: *const c_char, v: u32);
    fn NVSDK_NGX_Parameter_SetF(p: *mut Parameter, name: *const c_char, v: f32);
    fn NVSDK_NGX_Parameter_SetVoidPointer(p: *mut Parameter, name: *const c_char, v: *mut c_void);
    fn NVSDK_NGX_Parameter_GetI(p: *mut Parameter, name: *const c_char, v: *mut i32) -> NgxResult;
    fn NVSDK_NGX_Parameter_GetUI(p: *mut Parameter, name: *const c_char, v: *mut u32) -> NgxResult;
    fn NVSDK_NGX_Parameter_GetVoidPointer(p: *mut Parameter, name: *const c_char, v: *mut *mut c_void) -> NgxResult;
}

/// Instance and device extensions NGX asks for.
pub fn required_extensions() -> (Vec<&'static CStr>, Vec<&'static CStr>) {
    let (mut ni, mut nd) = (0u32, 0u32);
    let (mut ie, mut de): (*const *const c_char, *const *const c_char) = (std::ptr::null(), std::ptr::null());
    // SAFETY: NGX fills the counts and points at its own static arrays of C strings.
    let r = unsafe { NVSDK_NGX_VULKAN_RequiredExtensions(&mut ni, &mut ie, &mut nd, &mut de) };
    let list = |n: u32, p: *const *const c_char| -> Vec<&'static CStr> {
        if failed(r) || p.is_null() {
            return Vec::new();
        }
        (0..n as usize).map(|i| unsafe { CStr::from_ptr(*p.add(i)) }).collect()
    };
    (list(ni, ie), list(nd, de))
}

/// NGX initialized on one VkDevice; shut down when dropped (before the device).
pub struct Ngx {
    device: vk::Device,
    caps: *mut Parameter,
}

/// What NGX says of DLSS on this PC.
pub enum Availability {
    Available,
    DriverTooOld { min: (u32, u32) },
    NotAvailable { init_result: NgxResult },
}

impl Ngx {
    /// # Safety
    /// The handles are live, of one device, and outlive the returned value.
    pub unsafe fn init(
        data_path: &[u16],
        instance: vk::Instance,
        pd: vk::PhysicalDevice,
        device: vk::Device,
        gipa: vk::PFN_vkGetInstanceProcAddr,
        gdpa: vk::PFN_vkGetDeviceProcAddr,
        log: extern "C" fn(*const c_char, u32, u32),
    ) -> Result<Ngx, NgxResult> {
        let info = FeatureCommonInfo {
            path_list: PathListInfo { path: std::ptr::null(), length: 0 },
            internal: std::ptr::null_mut(),
            logging: LoggingInfo { callback: Some(log), minimum_level: LOGGING_LEVEL_ON, disable_other_sinks: true },
        };
        // SAFETY: the caller's handles; `info` and the strings live through the call.
        let r = unsafe {
            NVSDK_NGX_VULKAN_Init_with_ProjectID(
                PROJECT_ID.as_ptr(),
                ENGINE_TYPE_CUSTOM,
                ENGINE_VERSION.as_ptr(),
                data_path.as_ptr(),
                instance.as_raw(),
                pd.as_raw(),
                device.as_raw(),
                gipa,
                gdpa,
                &info,
                VERSION_API,
            )
        };
        if !ok(r) {
            return Err(r);
        }
        let mut caps = std::ptr::null_mut();
        // SAFETY: NGX is up; it owns the returned parameter block.
        let r = unsafe { NVSDK_NGX_VULKAN_GetCapabilityParameters(&mut caps) };
        if failed(r) || caps.is_null() {
            unsafe { NVSDK_NGX_VULKAN_Shutdown1(device.as_raw()) };
            return Err(r);
        }
        Ok(Ngx { device, caps })
    }

    pub fn dlss_availability(&self) -> Availability {
        let (mut avail, mut needs, mut maj, mut min, mut init) = (0i32, 0i32, 0u32, 0u32, 0i32);
        // SAFETY: the capability block lives as long as self.
        unsafe {
            NVSDK_NGX_Parameter_GetI(self.caps, c"SuperSampling.Available".as_ptr(), &mut avail);
            NVSDK_NGX_Parameter_GetI(self.caps, c"SuperSampling.NeedsUpdatedDriver".as_ptr(), &mut needs);
            NVSDK_NGX_Parameter_GetUI(self.caps, c"SuperSampling.MinDriverVersionMajor".as_ptr(), &mut maj);
            NVSDK_NGX_Parameter_GetUI(self.caps, c"SuperSampling.MinDriverVersionMinor".as_ptr(), &mut min);
            NVSDK_NGX_Parameter_GetI(self.caps, c"SuperSampling.FeatureInitResult".as_ptr(), &mut init);
        }
        if needs != 0 {
            Availability::DriverTooOld { min: (maj, min) }
        } else if avail != 1 {
            Availability::NotAvailable { init_result: init as u32 }
        } else {
            Availability::Available
        }
    }

    /// DLSS's render size for an output size at a quality (its optimal-settings callback).
    pub fn optimal_render_size(&self, out: (u32, u32), quality: i32) -> Option<(u32, u32)> {
        let mut cb: *mut c_void = std::ptr::null_mut();
        // SAFETY: the capability block lives as long as self; the callback is NGX's, taking that block.
        unsafe {
            NVSDK_NGX_Parameter_GetVoidPointer(self.caps, c"DLSSOptimalSettingsCallback".as_ptr(), &mut cb);
            if cb.is_null() {
                return None;
            }
            NVSDK_NGX_Parameter_SetUI(self.caps, c"Width".as_ptr(), out.0);
            NVSDK_NGX_Parameter_SetUI(self.caps, c"Height".as_ptr(), out.1);
            NVSDK_NGX_Parameter_SetI(self.caps, c"PerfQualityValue".as_ptr(), quality);
            NVSDK_NGX_Parameter_SetI(self.caps, c"RTXValue".as_ptr(), 0);
            let f: extern "C" fn(*mut Parameter) -> NgxResult = std::mem::transmute(cb);
            if failed(f(self.caps)) {
                return None;
            }
            let (mut w, mut h) = (0u32, 0u32);
            NVSDK_NGX_Parameter_GetUI(self.caps, c"OutWidth".as_ptr(), &mut w);
            NVSDK_NGX_Parameter_GetUI(self.caps, c"OutHeight".as_ptr(), &mut h);
            (w > 0 && h > 0).then_some((w, h))
        }
    }
}

impl Drop for Ngx {
    fn drop(&mut self) {
        // SAFETY: every feature is released before (Dlss borrows self); the device is live.
        unsafe { NVSDK_NGX_VULKAN_Shutdown1(self.device.as_raw()) };
    }
}

/// The per-frame inputs of an evaluation.
pub struct Eval<'a> {
    pub color: &'a ResourceVk,
    pub depth: &'a ResourceVk,
    pub motion: &'a ResourceVk,
    pub output: &'a ResourceVk,
    pub jitter: (f32, f32),
    pub mv_scale: (f32, f32),
    pub reset: bool,
    pub render: (u32, u32),
}

/// A DLSS feature (one render size to one output size); released when dropped.
pub struct Dlss<'n> {
    _ngx: &'n Ngx,
    handle: *mut Handle_,
    params: *mut Parameter,
}

pub struct DlssSize {
    pub render: (u32, u32),
    pub output: (u32, u32),
    pub quality: i32,
    pub flags: i32,
}

impl<'n> Dlss<'n> {
    /// Records the feature's creation into `cmd` (the caller submits it and waits).
    ///
    /// # Safety
    /// `cmd` is recording, of NGX's device.
    pub unsafe fn create(ngx: &'n Ngx, cmd: vk::CommandBuffer, size: &DlssSize) -> Result<Self, NgxResult> {
        let mut params = std::ptr::null_mut();
        let r = unsafe { NVSDK_NGX_VULKAN_AllocateParameters(&mut params) };
        if failed(r) || params.is_null() {
            return Err(r);
        }
        let mut handle = std::ptr::null_mut();
        // SAFETY: `params` is NGX's; names are NUL-ended.
        let r = unsafe {
            NVSDK_NGX_Parameter_SetUI(params, c"CreationNodeMask".as_ptr(), 1);
            NVSDK_NGX_Parameter_SetUI(params, c"VisibilityNodeMask".as_ptr(), 1);
            NVSDK_NGX_Parameter_SetUI(params, c"Width".as_ptr(), size.render.0);
            NVSDK_NGX_Parameter_SetUI(params, c"Height".as_ptr(), size.render.1);
            NVSDK_NGX_Parameter_SetUI(params, c"OutWidth".as_ptr(), size.output.0);
            NVSDK_NGX_Parameter_SetUI(params, c"OutHeight".as_ptr(), size.output.1);
            NVSDK_NGX_Parameter_SetI(params, c"PerfQualityValue".as_ptr(), size.quality);
            NVSDK_NGX_Parameter_SetI(params, c"DLSS.Feature.Create.Flags".as_ptr(), size.flags);
            NVSDK_NGX_Parameter_SetI(params, c"DLSS.Enable.Output.Subrects".as_ptr(), 0);
            NVSDK_NGX_VULKAN_CreateFeature1(ngx.device.as_raw(), cmd.as_raw(), FEATURE_SUPER_SAMPLING, params, &mut handle)
        };
        if failed(r) || handle.is_null() {
            unsafe { NVSDK_NGX_VULKAN_DestroyParameters(params) };
            return Err(r);
        }
        Ok(Dlss { _ngx: ngx, handle, params })
    }

    /// Records one evaluation into `cmd`.
    ///
    /// # Safety
    /// `cmd` is recording; the resources are live and in the layouts NGX expects (GENERAL).
    pub unsafe fn evaluate(&self, cmd: vk::CommandBuffer, e: &Eval) -> NgxResult {
        let p = self.params;
        let set_res = |name: &CStr, r: &ResourceVk| unsafe {
            NVSDK_NGX_Parameter_SetVoidPointer(p, name.as_ptr(), r as *const ResourceVk as *mut c_void)
        };
        set_res(c"Color", e.color);
        set_res(c"Output", e.output);
        set_res(c"Depth", e.depth);
        set_res(c"MotionVectors", e.motion);
        // SAFETY: `p` is this feature's parameter block.
        unsafe {
            NVSDK_NGX_Parameter_SetF(p, c"Jitter.Offset.X".as_ptr(), e.jitter.0);
            NVSDK_NGX_Parameter_SetF(p, c"Jitter.Offset.Y".as_ptr(), e.jitter.1);
            NVSDK_NGX_Parameter_SetF(p, c"Sharpness".as_ptr(), 0.0);
            NVSDK_NGX_Parameter_SetI(p, c"Reset".as_ptr(), e.reset as i32);
            NVSDK_NGX_Parameter_SetF(p, c"MV.Scale.X".as_ptr(), e.mv_scale.0);
            NVSDK_NGX_Parameter_SetF(p, c"MV.Scale.Y".as_ptr(), e.mv_scale.1);
            NVSDK_NGX_Parameter_SetUI(p, c"DLSS.Render.Subrect.Dimensions.Width".as_ptr(), e.render.0);
            NVSDK_NGX_Parameter_SetUI(p, c"DLSS.Render.Subrect.Dimensions.Height".as_ptr(), e.render.1);
            NVSDK_NGX_Parameter_SetF(p, c"DLSS.Pre.Exposure".as_ptr(), 1.0);
            NVSDK_NGX_Parameter_SetF(p, c"DLSS.Exposure.Scale".as_ptr(), 1.0);
            NVSDK_NGX_VULKAN_EvaluateFeature_C(cmd.as_raw(), self.handle, p, std::ptr::null())
        }
    }
}

impl Drop for Dlss<'_> {
    fn drop(&mut self) {
        // SAFETY: the caller waited for the device to go idle before dropping (Session::end).
        unsafe {
            NVSDK_NGX_VULKAN_ReleaseFeature(self.handle);
            NVSDK_NGX_VULKAN_DestroyParameters(self.params);
        }
    }
}
