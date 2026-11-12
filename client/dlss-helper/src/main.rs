//! dlss_helper.exe: NVIDIA DLSS for the 32-bit game, in a 64-bit process.
//!
//! The game (exe patch 83, with its Vulkan/DXVK renderer) starts this program
//! with the name of a pipe and speaks PROTOCOL.md: it shares each frame's
//! colour, depth and motion vectors (exported Vulkan images) and one timeline
//! semaphore; the helper runs DLSS on its own VkDevice on the same GPU and
//! signals when the upscaled image is in the shared output.
//!
//! It never shows anything. Whatever stops it is said in its reply to the
//! game (which logs it) and in `dlss_helper.log` beside the exe. It ends when
//! the game says so or the pipe closes.

#![windows_subsystem = "windows"]

mod gpu;
mod ipc;
mod ngx;
mod platform;

use ash::vk;
use gpu::{Gpu, GpuError, Shared};
use ipc::{Command, Frame, FrameMsg, FrameStat, Hello, HelloMsg, HelloReply, Mode, Setup, SetupMsg, SetupReply, Slot, Wire};
use platform::System;
use std::ffi::{CStr, c_char};
use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

/// Why the helper ends.
enum End {
    /// The game said quit, or closed the pipe.
    Done,
    /// DLSS cannot run here; the reason went to the game in a reply.
    Refused(String),
    /// The pipe or the device failed mid-way.
    Broken(String),
}

impl From<io::Error> for End {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe => End::Done,
            _ => End::Broken(format!("pipe: {e}")),
        }
    }
}

fn main() {
    let dir = platform::exe_dir();
    platform::open_log(&dir);
    let code = match run(&dir) {
        End::Done => {
            log!("done");
            0
        }
        End::Refused(why) => {
            log!("refused: {why}");
            3
        }
        End::Broken(why) => {
            log!("stopped: {why}");
            2
        }
    };
    std::process::exit(code);
}

fn run(dir: &std::path::Path) -> End {
    let Some(name) = std::env::args().nth(1) else {
        return End::Broken("usage: dlss_helper.exe <pipe name> (started by the game)".into());
    };
    let system = platform::system();
    log!("dlss_helper {} on {system:?}, pipe {name}", env!("CARGO_PKG_VERSION"));
    let mut pipe = match connect(&name) {
        Ok(p) => p,
        Err(e) => return End::Broken(format!("cannot open the pipe {name}: {e}")),
    };
    match serve(&mut pipe, system, dir) {
        Ok(end) | Err(end) => end,
    }
}

/// The game made the pipe before it started us; a busy or not-yet-listening
/// pipe is tried again for two seconds.
fn connect(name: &str) -> io::Result<File> {
    let start = Instant::now();
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(name) {
            Ok(f) => return Ok(f),
            Err(e) if start.elapsed() < Duration::from_secs(2) => {
                let _ = e;
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
}

fn pipe_alive(pipe: &File) -> bool {
    let mut avail = 0u32;
    // SAFETY: the pipe's own handle; only the byte count is asked.
    unsafe {
        windows_sys::Win32::System::Pipes::PeekNamedPipe(pipe.as_raw_handle() as _, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut()) != 0
    }
}

/// The device and NGX, made at the first Hello and kept over resizes.
/// (Fields drop in this order: NGX before its device.)
struct Machine {
    ngx: Option<ngx::Ngx>,
    gpu: Gpu,
}

extern "C" fn ngx_log(message: *const c_char, _level: u32, _feature: u32) {
    if !message.is_null() {
        // SAFETY: NGX passes a NUL-ended line.
        let line = unsafe { CStr::from_ptr(message) }.to_string_lossy();
        platform::log(&format!("ngx: {}", line.trim_end()));
    }
}

/// Everything a Hello needs before the device exists, then the device and NGX.
fn start(hello: &Hello, system: System, dir: &std::path::Path) -> Result<Machine, (String, u32)> {
    let no = |s: String| (s, 0u32);
    let adapter = gpu::Adapter::find(hello.device_uuid).map_err(|e| match e {
        GpuError::NotNvidia { name, vendor } => no(format!("not an NVIDIA GPU: {name} (vendor {vendor:#06x})")),
        GpuError::Other(s) => no(s),
    })?;
    if hello.mode == Mode::Pass {
        let gpu = adapter.into_gpu(&[]).map_err(|e| no(e.text()))?;
        return Ok(Machine { ngx: None, gpu });
    }
    if !dir.join("nvngx_dlss.dll").is_file() {
        return Err(no(format!("nvngx_dlss.dll is not beside the helper ({})", dir.display())));
    }
    if system == System::Wine {
        let core = platform::wine_ngx_core().map_err(no)?;
        log!("NGX core folder (NGXCore key): {}", core.display());
    }
    let nvapi = platform::load_nvapi(system, dir).map_err(no)?;
    log!("nvapi: {}", nvapi.display());
    let (ngx_instance, ngx_device) = ngx::required_extensions();
    log!("NGX asks for instance {ngx_instance:?}, device {ngx_device:?}");
    for name in &ngx_instance {
        if !adapter.instance_extensions.contains(name) {
            log!("instance extension NGX asks for is not enabled: {name:?}");
        }
    }
    let gpu = adapter.into_gpu(&ngx_device).map_err(|e| no(e.text()))?;
    let data = platform::data_dir();
    // SAFETY: the device's own handles; `gpu` outlives the Ngx (Machine's field order).
    let ngx = unsafe { ngx::Ngx::init(&data, gpu.instance.handle(), gpu.pd, gpu.device.handle(), gpu.gipa(), gpu.gdpa(), ngx_log) }.map_err(|r| {
        let hint = match (r, system) {
            (0xBAD00002, System::Wine) => ": NGX found no NVIDIA driver through NvAPI (dxvk-nvapi) in this prefix",
            (0xBAD00002, System::Windows) => ": the driver gave NGX no NVIDIA GPU (driver too old or not installed properly)",
            (0xBAD0000C, _) => ": the NVIDIA driver is too old for this DLSS",
            _ => "",
        };
        (format!("NGX did not start ({}){hint}", ngx::describe(r)), r)
    })?;
    match ngx.dlss_availability() {
        ngx::Availability::Available => {}
        ngx::Availability::DriverTooOld { min } => {
            return Err(no(format!("the NVIDIA driver is too old for DLSS: it needs {}.{} or later", min.0, min.1)));
        }
        ngx::Availability::NotAvailable { init_result } => {
            return Err((format!("DLSS is not available on this GPU ({})", ngx::describe(init_result)), init_result));
        }
    }
    Ok(Machine { ngx: Some(ngx), gpu })
}

fn serve(pipe: &mut File, system: System, dir: &std::path::Path) -> Result<End, End> {
    let mut machine: Option<Machine> = None;
    loop {
        // ---- Hello: can DLSS run, and at which render size
        let hello = match Hello::try_from(HelloMsg::read_from(pipe)?) {
            Ok(h) => h,
            Err(why) => return Err(refuse_hello(pipe, why, 0)),
        };
        log!("hello: output {}x{}, {:?}, {:?}", hello.out.0, hello.out.1, hello.quality, hello.mode);
        if machine.is_none() {
            let t = Instant::now();
            match start(&hello, system, dir) {
                Ok(m) => machine = Some(m),
                Err((why, ngx_result)) => return Err(refuse_hello(pipe, why, ngx_result)),
            }
            log!("device{} up in {} ms", if hello.mode == Mode::Dlss { " and NGX" } else { "" }, t.elapsed().as_millis());
        }
        let m = machine.as_ref().expect("made above");
        if m.gpu.uuid != hello.device_uuid {
            return Err(refuse_hello(pipe, "another GPU than this helper was started for".into(), 0));
        }
        if (hello.mode == Mode::Dlss) != m.ngx.is_some() {
            return Err(refuse_hello(pipe, "the helper's mode cannot change between sizes".into(), 0));
        }
        let quality = hello.quality as i32;
        let render = m.ngx.as_ref().and_then(|n| n.optimal_render_size(hello.out, quality)).unwrap_or_else(|| {
            let s = hello.quality.scale();
            (((hello.out.0 as f64) * s + 0.5) as u32, ((hello.out.1 as f64) * s + 0.5) as u32)
        });
        log!("render size {}x{}", render.0, render.1);
        HelloReply { ok: 1, ngx_result: 1, render_w: render.0, render_h: render.1, text: ipc::text("") }.write_to(pipe)?;

        // ---- Setup: the game's images and semaphore, and the DLSS feature
        let setup = match Setup::try_from(SetupMsg::read_from(pipe)?) {
            Ok(s) => s,
            Err(why) => return Err(refuse_setup(pipe, why, 0)),
        };
        let shared = match m.gpu.import(&setup) {
            Ok(s) => s,
            Err(why) => return Err(refuse_setup(pipe, why, 0)),
        };
        let vram_before = m.gpu.vram();
        let render = shared.get(Slot::Color).size;
        let output = shared.get(Slot::Output).size;
        let dlss = match &m.ngx {
            None => None,
            Some(ngx) => {
                let size = ngx::DlssSize { render, output, quality, flags: setup.dlss_flags };
                let mut made = None;
                // SAFETY: the command buffer is recording, of NGX's device.
                let submitted = m.gpu.one_time(|cmd| made = Some(unsafe { ngx::Dlss::create(ngx, cmd, &size) }));
                match (submitted, made) {
                    (Ok(()), Some(Ok(d))) => Some(d),
                    (Err(e), _) => {
                        m.gpu.destroy(shared);
                        return Err(refuse_setup(pipe, format!("the DLSS feature's commands failed: {e:?}"), 0));
                    }
                    (_, Some(Err(r))) => {
                        m.gpu.destroy(shared);
                        return Err(refuse_setup(pipe, format!("DLSS refused {}x{} -> {}x{} ({})", render.0, render.1, output.0, output.1, ngx::describe(r)), r));
                    }
                    (Ok(()), None) => unreachable!("one_time ran the closure"),
                }
            }
        };
        let vram_after = m.gpu.vram();
        log!("setup: {}x{} -> {}x{}, {} bytes shared, VRAM {} -> {} MiB", render.0, render.1, output.0, output.1, shared.bytes, vram_before >> 20, vram_after >> 20);
        SetupReply { ok: 1, ngx_result: 1, vram_before_feature: vram_before, vram_after_feature: vram_after, import_bytes: shared.bytes, text: ipc::text("") }
            .write_to(pipe)?;

        // ---- frames of this size
        let end = frames(pipe, &m.gpu, &shared, dlss.as_ref());
        // SAFETY (of the drops below): nothing of this size runs on the device any more.
        unsafe { m.gpu.device.device_wait_idle().ok() };
        drop(dlss);
        m.gpu.destroy(shared);
        match end {
            Ok(Next::Resize) => log!("a new size"),
            Ok(Next::Quit) => return Ok(End::Done),
            Err(e) => return Err(e),
        }
    }
}

fn refuse_hello(pipe: &mut File, why: String, ngx_result: u32) -> End {
    let _ = HelloReply { ok: 0, ngx_result: ngx_result as i32, render_w: 0, render_h: 0, text: ipc::text(&why) }.write_to(pipe);
    End::Refused(why)
}

fn refuse_setup(pipe: &mut File, why: String, ngx_result: u32) -> End {
    let _ = SetupReply { ok: 0, ngx_result: ngx_result as i32, vram_before_feature: 0, vram_after_feature: 0, import_bytes: 0, text: ipc::text(&why) }
        .write_to(pipe);
    End::Refused(why)
}

enum Next {
    Quit,
    Resize,
}

fn frames(pipe: &mut File, gpu: &Gpu, shared: &Shared, dlss: Option<&ngx::Dlss>) -> Result<Next, End> {
    let res = |slot: Slot| {
        let i = shared.get(slot);
        ngx::ResourceVk::image(i.view, i.image, i.aspect, i.format, i.size, slot == Slot::Output)
    };
    let resources = [res(Slot::Color), res(Slot::Depth), res(Slot::Motion), res(Slot::Output)];
    let mut count = 0u64;
    loop {
        let frame = match Command::try_from(FrameMsg::read_from(pipe)?) {
            Ok(Command::Frame(f)) => f,
            Ok(Command::Quit) => return Ok(Next::Quit),
            Ok(Command::Resize) => return Ok(Next::Resize),
            Err(why) => return Err(End::Broken(why)),
        };
        let stat = one_frame(pipe, gpu, shared, dlss, &resources, &frame, (count & 1) as u32)?;
        stat.write_to(pipe)?;
        count += 1;
    }
}

/// Records and submits one frame's work between the game's signal (`wait`)
/// and ours (`signal`), waits for it and reads its GPU timestamps.
fn one_frame(pipe: &File, gpu: &Gpu, shared: &Shared, dlss: Option<&ngx::Dlss>, resources: &[ngx::ResourceVk; 4], frame: &Frame, slot: u32) -> Result<FrameStat, End> {
    let vk_err = |what: &'static str| move |e: vk::Result| End::Broken(format!("frame {}: {what}: {e:?}", frame.number));
    let t0 = Instant::now();
    let d = &gpu.device;
    let cmd = gpu.cmds[slot as usize];
    let q0 = slot * 4;
    let general = vk::ImageLayout::GENERAL;
    let rw = vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;
    // Every image comes from the game through VK_QUEUE_FAMILY_EXTERNAL and goes back the same way.
    let barriers = |acquire: bool| -> Vec<vk::ImageMemoryBarrier<'static>> {
        shared
            .images
            .iter()
            .map(|i| {
                let b = vk::ImageMemoryBarrier::default().old_layout(general).new_layout(general).image(i.image).subresource_range(gpu::range(i.aspect));
                if acquire {
                    b.dst_access_mask(rw).src_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL).dst_queue_family_index(gpu.family)
                } else {
                    b.src_access_mask(rw).src_queue_family_index(gpu.family).dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
                }
            })
            .collect()
    };
    // SAFETY: this frame's command buffer (its previous use ended: we waited for it), the device's own objects.
    unsafe {
        d.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty()).map_err(vk_err("reset"))?;
        d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(vk_err("begin"))?;
        d.cmd_reset_query_pool(cmd, gpu.queries, q0, 4);
        d.cmd_write_timestamp(cmd, vk::PipelineStageFlags::TOP_OF_PIPE, gpu.queries, q0);
        d.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::ALL_COMMANDS, vk::DependencyFlags::empty(), &[], &[], &barriers(true));
        d.cmd_write_timestamp(cmd, vk::PipelineStageFlags::ALL_COMMANDS, gpu.queries, q0 + 1);
        if let Some(dlss) = dlss {
            let eval = ngx::Eval {
                color: &resources[Slot::Color as usize],
                depth: &resources[Slot::Depth as usize],
                motion: &resources[Slot::Motion as usize],
                output: &resources[Slot::Output as usize],
                jitter: frame.jitter,
                mv_scale: frame.mv_scale,
                reset: frame.reset,
                render: shared.get(Slot::Color).size,
            };
            let r = dlss.evaluate(cmd, &eval);
            if ngx::failed(r) {
                return Err(End::Broken(format!("frame {}: DLSS evaluate: {}", frame.number, ngx::describe(r))));
            }
        }
        d.cmd_write_timestamp(cmd, vk::PipelineStageFlags::ALL_COMMANDS, gpu.queries, q0 + 2);
        d.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::ALL_COMMANDS, vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::DependencyFlags::empty(), &[], &[], &barriers(false));
        d.cmd_write_timestamp(cmd, vk::PipelineStageFlags::BOTTOM_OF_PIPE, gpu.queries, q0 + 3);
        d.end_command_buffer(cmd).map_err(vk_err("end"))?;
        let (wait, signal) = ([frame.wait], [frame.signal]);
        let mut values = vk::TimelineSemaphoreSubmitInfo::default().wait_semaphore_values(&wait).signal_semaphore_values(&signal);
        let (sems, stages, cmds) = ([shared.timeline], [vk::PipelineStageFlags::ALL_COMMANDS], [cmd]);
        let submit = vk::SubmitInfo::default().wait_semaphores(&sems).wait_dst_stage_mask(&stages).command_buffers(&cmds).signal_semaphores(&sems).push_next(&mut values);
        d.queue_submit(gpu.queue, &[submit], vk::Fence::null()).map_err(vk_err("submit"))?;
    }
    let record_ms = t0.elapsed().as_secs_f64() * 1000.0;
    // Our signal comes once the game's has; a game that is gone never signals.
    let (sems, values) = ([shared.timeline], [frame.signal]);
    let wait = vk::SemaphoreWaitInfo::default().semaphores(&sems).values(&values);
    let mut said = false;
    loop {
        // SAFETY: the imported timeline semaphore.
        match unsafe { d.wait_semaphores(&wait, 100_000_000) } {
            Ok(()) => break,
            Err(vk::Result::TIMEOUT) => {
                if !pipe_alive(pipe) {
                    return Err(End::Done);
                }
                if !said && t0.elapsed() > Duration::from_secs(5) {
                    log!("frame {}: the game has not signalled {} for 5 s", frame.number, frame.wait);
                    said = true;
                }
            }
            Err(e) => return Err(vk_err("waiting for the frame")(e)),
        }
    }
    let mut ts = [0u64; 4];
    // SAFETY: the four queries this frame wrote; its commands are done.
    unsafe { d.get_query_pool_results(gpu.queries, q0, &mut ts, vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT) }.map_err(vk_err("timestamps"))?;
    Ok(FrameStat {
        frame: frame.number,
        ts_begin: ts[0],
        ts_eval_begin: ts[1],
        ts_eval_end: ts[2],
        ts_end: ts[3],
        tick_ns: gpu.props.limits.timestamp_period as f64,
        record_ms,
    })
}
