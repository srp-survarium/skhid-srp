//! The helper's own Vulkan device, on the game's GPU (matched by deviceUUID),
//! and the game's shared images and timeline semaphore imported into it.

use crate::ipc::{SLOTS, Setup, Slot};
use crate::log;
use crate::platform::wide;
use ash::{khr, vk};
use std::ffi::{CStr, c_char};

pub const NVIDIA: u32 = 0x10DE;

pub struct Gpu {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub pd: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub family: u32,
    pub props: vk::PhysicalDeviceProperties,
    pub uuid: [u8; 16],
    pool: vk::CommandPool,
    pub cmds: [vk::CommandBuffer; 3],
    pub queries: vk::QueryPool,
    budget: bool,
    sem_win32: khr::external_semaphore_win32::Device,
}

fn has(list: &[vk::ExtensionProperties], name: &CStr) -> bool {
    list.iter().any(|e| e.extension_name_as_c_str() == Ok(name))
}

/// Why a device cannot be made.
pub enum GpuError {
    NotNvidia { name: String, vendor: u32 },
    Other(String),
}
impl GpuError {
    pub fn text(&self) -> String {
        match self {
            GpuError::NotNvidia { name, vendor } => format!("not an NVIDIA GPU: {name} (vendor {vendor:#06x})"),
            GpuError::Other(s) => s.clone(),
        }
    }
}
impl From<String> for GpuError {
    fn from(s: String) -> Self {
        GpuError::Other(s)
    }
}
fn vkerr(what: &str) -> impl Fn(vk::Result) -> GpuError + '_ {
    move |r| GpuError::Other(format!("{what}: {r:?}"))
}

/// The game's GPU, found by its deviceUUID, before a device is made on it.
pub struct Adapter {
    entry: ash::Entry,
    instance: ash::Instance,
    pd: vk::PhysicalDevice,
    props: vk::PhysicalDeviceProperties,
    uuid: [u8; 16],
    pub instance_extensions: Vec<&'static CStr>,
}

impl Adapter {
    /// The physical device whose deviceUUID is `uuid`; an NVIDIA one, or the error says which it is.
    pub fn find(uuid: [u8; 16]) -> Result<Adapter, GpuError> {
        // SAFETY: vulkan-1.dll's vkGetInstanceProcAddr, the loader's entry point.
        let entry = unsafe {
            let module = windows_sys::Win32::System::LibraryLoader::LoadLibraryW(wide("vulkan-1.dll").as_ptr());
            if module.is_null() {
                return Err("vulkan-1.dll cannot be loaded".to_string().into());
            }
            let gipa = windows_sys::Win32::System::LibraryLoader::GetProcAddress(module, c"vkGetInstanceProcAddr".as_ptr().cast())
                .ok_or_else(|| "vulkan-1.dll has no vkGetInstanceProcAddr".to_string())?;
            ash::Entry::from_static_fn(ash::StaticFn { get_instance_proc_addr: std::mem::transmute::<unsafe extern "system" fn() -> isize, vk::PFN_vkGetInstanceProcAddr>(gipa) })
        };
        let available = unsafe { entry.enumerate_instance_extension_properties(None) }.map_err(vkerr("instance extensions"))?;
        // NGX asks for VK_KHR_get_physical_device_properties2 alone (main.rs logs anything more).
        let mut iexts: Vec<&'static CStr> = Vec::new();
        for name in [khr::get_physical_device_properties2::NAME, khr::external_memory_capabilities::NAME, khr::external_semaphore_capabilities::NAME] {
            if has(&available, name) {
                iexts.push(name);
            } else {
                log!("instance extension missing: {name:?}");
            }
        }
        let iptrs: Vec<*const c_char> = iexts.iter().map(|n| n.as_ptr()).collect();
        let app = vk::ApplicationInfo::default().application_name(c"dlss_helper").api_version(vk::API_VERSION_1_3);
        let instance = unsafe { entry.create_instance(&vk::InstanceCreateInfo::default().application_info(&app).enabled_extension_names(&iptrs), None) }
            .map_err(vkerr("vkCreateInstance"))?;
        let pds = unsafe { instance.enumerate_physical_devices() }.map_err(vkerr("vkEnumeratePhysicalDevices"))?;
        let mut found = None;
        for pd in pds {
            let mut id = vk::PhysicalDeviceIDProperties::default();
            let props = {
                let mut p2 = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
                unsafe { instance.get_physical_device_properties2(pd, &mut p2) };
                p2.properties
            };
            if id.device_uuid == uuid {
                found = Some((pd, props));
            }
        }
        let (pd, props) = found.ok_or_else(|| "no Vulkan device with the game's deviceUUID".to_string())?;
        let name = props.device_name_as_c_str().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        log!("device: {name}, vendor {:#06x}, driver {:#x}, Vulkan {}.{}.{}", props.vendor_id, props.driver_version,
            vk::api_version_major(props.api_version), vk::api_version_minor(props.api_version), vk::api_version_patch(props.api_version));
        if props.vendor_id != NVIDIA {
            unsafe { instance.destroy_instance(None) };
            return Err(GpuError::NotNvidia { name, vendor: props.vendor_id });
        }
        Ok(Adapter { entry, instance, pd, props, uuid, instance_extensions: iexts })
    }

    /// A device on it with the sharing's extensions and those NGX asks for.
    pub fn into_gpu(self, ngx_device: &[&CStr]) -> Result<Gpu, GpuError> {
        let Adapter { entry, instance, pd, props, uuid, .. } = self;
        let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
        let family = families
            .iter()
            .position(|f| f.queue_flags.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE))
            .ok_or_else(|| "no graphics and compute queue".to_string())? as u32;
        let davail = unsafe { instance.enumerate_device_extension_properties(pd) }.map_err(vkerr("device extensions"))?;
        let mut dexts: Vec<&CStr> = Vec::new();
        for name in [khr::external_memory::NAME, khr::external_memory_win32::NAME, khr::external_semaphore::NAME, khr::external_semaphore_win32::NAME, khr::dedicated_allocation::NAME, khr::get_memory_requirements2::NAME] {
            if !has(&davail, name) {
                return Err(format!("the device lacks {name:?}").into());
            }
            dexts.push(name);
        }
        let budget = has(&davail, ash::ext::memory_budget::NAME);
        if budget {
            dexts.push(ash::ext::memory_budget::NAME);
        }
        for &name in ngx_device {
            if dexts.contains(&name) {
                continue;
            }
            if has(&davail, name) {
                dexts.push(name);
            } else {
                log!("device extension NGX asks for is missing: {name:?}");
            }
        }
        // Every supported 1.1/1.2/1.3 feature (NGX's kernels may use any), less robustness.
        let mut f13 = vk::PhysicalDeviceVulkan13Features::default();
        let mut f12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut f11 = vk::PhysicalDeviceVulkan11Features::default();
        let mut base = {
            let mut f2 = vk::PhysicalDeviceFeatures2::default().push_next(&mut f13).push_next(&mut f12).push_next(&mut f11);
            unsafe { instance.get_physical_device_features2(pd, &mut f2) };
            f2.features
        };
        base.robust_buffer_access = vk::FALSE;
        if f12.timeline_semaphore == vk::FALSE {
            return Err("no timeline semaphores".to_string().into());
        }
        f13.robust_image_access = vk::FALSE;
        f13.p_next = std::ptr::null_mut();
        f12.p_next = std::ptr::null_mut();
        f11.p_next = std::ptr::null_mut();
        let mut f2 = vk::PhysicalDeviceFeatures2::default().features(base).push_next(&mut f13).push_next(&mut f12).push_next(&mut f11);
        let dptrs: Vec<*const c_char> = dexts.iter().map(|n| n.as_ptr()).collect();
        let prio = [1.0f32];
        let qci = [vk::DeviceQueueCreateInfo::default().queue_family_index(family).queue_priorities(&prio)];
        let dci = vk::DeviceCreateInfo::default().queue_create_infos(&qci).enabled_extension_names(&dptrs).push_next(&mut f2);
        let device = unsafe { instance.create_device(pd, &dci, None) }.map_err(vkerr("vkCreateDevice"))?;
        let queue = unsafe { device.get_device_queue(family, 0) };
        let pool = unsafe {
            device.create_command_pool(&vk::CommandPoolCreateInfo::default().flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER).queue_family_index(family), None)
        }
        .map_err(vkerr("vkCreateCommandPool"))?;
        let cmds = unsafe { device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(3)) }
            .map_err(vkerr("vkAllocateCommandBuffers"))?;
        let queries = unsafe { device.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(8), None) }
            .map_err(vkerr("vkCreateQueryPool"))?;
        let sem_win32 = khr::external_semaphore_win32::Device::new(&instance, &device);
        Ok(Gpu { entry, instance, pd, device, queue, family, props, uuid, pool, cmds: [cmds[0], cmds[1], cmds[2]], queries, budget, sem_win32 })
    }
}

impl Gpu {

    pub fn gipa(&self) -> vk::PFN_vkGetInstanceProcAddr {
        self.entry.static_fn().get_instance_proc_addr
    }
    pub fn gdpa(&self) -> vk::PFN_vkGetDeviceProcAddr {
        self.instance.fp_v1_0().get_device_proc_addr
    }

    /// This process's use of the device-local heaps (VK_EXT_memory_budget), bytes.
    pub fn vram(&self) -> u64 {
        if !self.budget {
            return 0;
        }
        let mut b = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let mut m = vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut b);
        unsafe { self.instance.get_physical_device_memory_properties2(self.pd, &mut m) };
        let mp = m.memory_properties;
        (0..mp.memory_heap_count as usize)
            .filter(|&i| mp.memory_heaps[i].flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|i| b.heap_usage[i])
            .sum()
    }

    /// Records into `cmds[2]`, submits it and waits for the queue.
    pub fn one_time(&self, record: impl FnOnce(vk::CommandBuffer)) -> Result<(), vk::Result> {
        let cmd = self.cmds[2];
        unsafe {
            self.device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            record(cmd);
            self.device.end_command_buffer(cmd)?;
            self.device.queue_submit(self.queue, &[vk::SubmitInfo::default().command_buffers(&[cmd])], vk::Fence::null())?;
            self.device.queue_wait_idle(self.queue)
        }
    }

    /// Imports the game's images and timeline semaphore. Each handle is
    /// closed once imported (or refused).
    pub fn import(&self, setup: &Setup) -> Result<Shared, String> {
        let mut shared = Shared { images: Vec::new(), timeline: vk::Semaphore::null(), bytes: 0 };
        let close_all = |from: usize| {
            for s in &setup.img[from..] {
                unsafe { windows_sys::Win32::Foundation::CloseHandle(s.handle as _) };
            }
            unsafe { windows_sys::Win32::Foundation::CloseHandle(setup.semaphore as _) };
        };
        for (i, slot) in SLOTS.iter().enumerate() {
            let s = &setup.img[i];
            match self.import_image(*slot, s) {
                Ok(img) => {
                    unsafe { windows_sys::Win32::Foundation::CloseHandle(s.handle as _) };
                    shared.bytes += s.alloc_size;
                    shared.images.push(img);
                }
                Err(e) => {
                    close_all(i);
                    self.destroy(shared);
                    return Err(format!("shared image {i} ({slot:?}): {e}"));
                }
            }
        }
        let mut ty = vk::SemaphoreTypeCreateInfo::default().semaphore_type(vk::SemaphoreType::TIMELINE);
        let sem = unsafe { self.device.create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut ty), None) };
        let sem = match sem {
            Ok(s) => s,
            Err(e) => {
                unsafe { windows_sys::Win32::Foundation::CloseHandle(setup.semaphore as _) };
                self.destroy(shared);
                return Err(format!("timeline semaphore: {e:?}"));
            }
        };
        shared.timeline = sem;
        let info = vk::ImportSemaphoreWin32HandleInfoKHR::default()
            .semaphore(sem)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_WIN32)
            .handle(setup.semaphore as isize);
        let r = unsafe { self.sem_win32.import_semaphore_win32_handle(&info) };
        unsafe { windows_sys::Win32::Foundation::CloseHandle(setup.semaphore as _) };
        if let Err(e) = r {
            self.destroy(shared);
            return Err(format!("importing the timeline semaphore: {e:?}"));
        }
        Ok(shared)
    }

    fn import_image(&self, slot: Slot, s: &crate::ipc::SharedImage) -> Result<SharedImage, String> {
        let format = vk::Format::from_raw(s.format);
        let mut ext = vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32);
        let ci = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width: s.size.0, height: s.size.1, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::from_raw(s.usage))
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut ext);
        let image = unsafe { self.device.create_image(&ci, None) }.map_err(|e| format!("vkCreateImage: {e:?}"))?;
        let req = unsafe { self.device.get_image_memory_requirements(image) };
        let fail = |msg: String| {
            unsafe { self.device.destroy_image(image, None) };
            Err(msg)
        };
        if req.memory_type_bits & (1 << s.memory_type) == 0 {
            return fail(format!("memory type {} not allowed here (bits {:#x})", s.memory_type, req.memory_type_bits));
        }
        if req.size > s.alloc_size {
            return fail(format!("needs {} bytes, the game's allocation has {}", req.size, s.alloc_size));
        }
        let mut ded = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut imp = vk::ImportMemoryWin32HandleInfoKHR::default().handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32).handle(s.handle as isize);
        let ai = vk::MemoryAllocateInfo::default().allocation_size(s.alloc_size).memory_type_index(s.memory_type).push_next(&mut imp).push_next(&mut ded);
        let memory = match unsafe { self.device.allocate_memory(&ai, None) } {
            Ok(m) => m,
            Err(e) => return fail(format!("importing the memory: {e:?}")),
        };
        if let Err(e) = unsafe { self.device.bind_image_memory(image, memory, 0) } {
            unsafe { self.device.free_memory(memory, None) };
            return fail(format!("vkBindImageMemory: {e:?}"));
        }
        let aspect = aspect(format);
        let vci = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(range(aspect));
        let view = match unsafe { self.device.create_image_view(&vci, None) } {
            Ok(v) => v,
            Err(e) => {
                unsafe {
                    self.device.destroy_image(image, None);
                    self.device.free_memory(memory, None);
                }
                return Err(format!("vkCreateImageView: {e:?}"));
            }
        };
        log!("imported {slot:?}: {}x{} format {}, {} bytes, memory type {}", s.size.0, s.size.1, s.format, s.alloc_size, s.memory_type);
        Ok(SharedImage { image, memory, view, format, size: s.size, aspect })
    }

    /// Frees what `import` made; the device must be idle.
    pub fn destroy(&self, shared: Shared) {
        unsafe {
            for img in shared.images {
                self.device.destroy_image_view(img.view, None);
                self.device.destroy_image(img.image, None);
                self.device.free_memory(img.memory, None);
            }
            if shared.timeline != vk::Semaphore::null() {
                self.device.destroy_semaphore(shared.timeline, None);
            }
        }
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_query_pool(self.queries, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

/// The aspect DLSS reads: depth of a depth(-stencil) format, colour of any other
/// (a depth the game wrote into an R32_SFLOAT image is a colour image).
pub fn aspect(format: vk::Format) -> vk::ImageAspectFlags {
    match format {
        vk::Format::D16_UNORM | vk::Format::X8_D24_UNORM_PACK32 | vk::Format::D32_SFLOAT | vk::Format::D16_UNORM_S8_UINT | vk::Format::D24_UNORM_S8_UINT | vk::Format::D32_SFLOAT_S8_UINT => vk::ImageAspectFlags::DEPTH,
        _ => vk::ImageAspectFlags::COLOR,
    }
}
pub fn range(aspect: vk::ImageAspectFlags) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange { aspect_mask: aspect, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }
}

pub struct SharedImage {
    pub image: vk::Image,
    memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub size: (u32, u32),
    pub aspect: vk::ImageAspectFlags,
}

/// The game's images (in `SLOTS` order) and timeline, in this device.
pub struct Shared {
    pub images: Vec<SharedImage>,
    pub timeline: vk::Semaphore,
    pub bytes: u64,
}

impl Shared {
    pub fn get(&self, slot: Slot) -> &SharedImage {
        &self.images[slot as usize]
    }
}
