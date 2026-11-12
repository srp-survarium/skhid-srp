# The DLSS helper's protocol (game x86 <-> dlss_helper.exe x64)

The game's side is exe patch 83 with the Vulkan/DXVK renderer
(`srp-026g/docs/notes/dlss-option.md`); the helper's is this crate. The records are
`src/ipc.rs` (the C layout: the probe's `probe_ipc.h`); `tests/ipc_tests.rs` pins sizes and
parsing. Proven with a 32-bit D3D11 producer on DXVK 3.1.1 x32 under Wine 11.8 (README.md,
Tested).

## 1. Processes and the pipe

- The game creates a named pipe `\\.\pipe\<any name>` (byte mode, duplex, 64 KiB each way),
  then starts `binaries\x86\dlss\dlss_helper.exe <pipe name>` (CreateProcess; the working
  folder does not matter). The helper opens the pipe as a client (it retries for 2 s). The game
  keeps the helper's process handle: it is the target of DuplicateHandle.
- The helper shows nothing. It ends when the game sends quit, when the pipe closes (a game
  that exits or crashes: within 100 ms, also while a frame waits on the GPU), or after a reply
  with `ok = 0`. Exit codes: 0 done, 3 refused (the reason was in the reply), 2 broken.
- All messages are fixed-size records of fixed-width little-endian fields, 8-byte handle
  fields, the same layout in the x86 and x64 builds. Order:

  ```
  game -> helper  HelloMsg            helper -> game  HelloReply
  game -> helper  SetupMsg            helper -> game  SetupReply
  per frame N:    game -> helper FrameMsg;  helper -> game FrameStat
  a new size:     game -> helper FrameMsg{quit = 2}, then HelloMsg, SetupMsg as above
  end:            game -> helper FrameMsg{quit = 1}
  ```

## 2. Hello: which GPU, what output size, what quality

- `HelloMsg` (48 bytes): magic `"DLSS"` (0x53534C44), version 3, `device_uuid[16]` and
  `device_luid[8]` of DXVK's VkPhysicalDevice (vkGetPhysicalDeviceProperties2 +
  VkPhysicalDeviceIDProperties on the instance of `IDXGIVkInteropDevice::GetVulkanHandles`),
  `out_w`, `out_h`, `quality` (NGX's PerfQuality value: 0 Performance, 1 Balanced, 2 Quality,
  3 Ultra Performance, 5 DLAA), `helper_mode` (0 DLSS; 1 the round trip without DLSS, for
  timing).
- The helper, at the first Hello: finds the Vulkan device with that deviceUUID (it must be
  NVIDIA's), loads NvAPI and NGX (README.md, Wine), makes its own VkInstance (1.3) and VkDevice
  there, starts NGX and asks whether DLSS is available. At every Hello: DLSS's optimal render
  size for the output size and quality.
- `HelloReply` (272 bytes): `ok`, `ngx_result` (NGX's code when it was NGX that refused),
  `render_w`, `render_h`, `text[256]` (UTF-8, NUL-ended: why not, when `ok = 0`; the game logs
  it). The game sizes its render targets from this reply. After `ok = 0` the helper exits.
- The first Hello takes 0.3 s (no DLSS) to 1.0 s (NGX, DLSS's kernels); a later one a few ms.

## 3. Setup: what is shared (made and exported by the game, on DXVK's VkDevice)

Four images, each its own dedicated allocation, `VK_IMAGE_TILING_OPTIMAL`, 1 mip, 1 layer,
exclusive sharing, `VK_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_WIN32_BIT` on both
VkExternalMemoryImageCreateInfo and VkExportMemoryAllocateInfo, plus
VkMemoryDedicatedAllocateInfo, a DEVICE_LOCAL memory type:

| index | what | size | format (the probe's) | usage |
|---|---|---|---|---|
| 0 | colour | render | R8G8B8A8_UNORM | TRANSFER_DST, SAMPLED |
| 1 | depth | render | D32_SFLOAT (aspect DEPTH) | TRANSFER_DST, SAMPLED |
| 2 | motion | render | R16G16_SFLOAT | TRANSFER_DST, SAMPLED |
| 3 | output | output | R8G8B8A8_UNORM | STORAGE, SAMPLED, TRANSFER_SRC |

- The helper takes each image's format, size and usage from the Setup and creates the same
  image, so the formats are the game's choice: an image of a depth format (D16, D24, D32, with
  or without stencil) is read by its depth aspect, any other by colour (a depth written by a
  compute pass into an R32_SFLOAT image is a colour image).
- Import: VkImportMemoryWin32HandleInfoKHR(OPAQUE_WIN32, handle) + dedicated info,
  `allocationSize` = the exporter's size, `memoryTypeIndex` = the exporter's index (same
  physical device and driver, so the indices agree; the helper checks it against its own
  memoryTypeBits). The helper closes each handle once imported or refused.
- One timeline semaphore (VkSemaphoreTypeCreateInfo TIMELINE, VkExportSemaphoreCreateInfo
  OPAQUE_WIN32) made by the game on DXVK's device; the helper imports it
  (vkImportSemaphoreWin32HandleKHR, OPAQUE_WIN32, not temporary). The same semaphore serves
  every size: at a new size the game exports a new handle of it and its values go on.
- Handles: vkGetMemoryWin32HandleKHR / vkGetSemaphoreWin32HandleKHR (NT handles) in the game,
  `DuplicateHandle(game, h, helper process, DUPLICATE_SAME_ACCESS)`, the game closes its own
  copy, and the value in the helper's handle table goes into `SetupMsg`.
- `SetupMsg` (176 bytes): `img[4]` of {`handle` u64, `alloc_size` u64, `memory_type`,
  `format` (VkFormat), `width`, `height`, `usage` (VkImageUsageFlags), pad}, `semaphore` u64,
  `dlss_flags` (NVSDK_NGX_DLSS_Feature_Flags; the probe: MVLowRes = 2: LDR colour, depth not
  inverted, motion at the render size), pad. The three render-size images must have one size.
- `SetupReply` (288 bytes): `ok`, `ngx_result`, the helper's VRAM before and after the DLSS
  feature (VK_EXT_memory_budget, device-local heaps), the imported bytes, `text[256]`.
  After `ok = 0` the helper exits.

## 4. Frames: the only synchronisation is the one timeline

```
value 2N+1 = the game's inputs of frame N are in the shared images
value 2N+2 = the helper's DLSS output of frame N is in the shared output
```

(Any increasing values do: each `FrameMsg` names its own `wait_value` and `signal_value`,
`signal_value > wait_value`, above every value used before.)

Game, all on DXVK's queue (GetSubmissionQueue1, family F):

1. D3D11 renders colour, motion vectors and depth at the render size.
2. Pipe: `FrameMsg{frame, wait_value, signal_value, jitter_x, jitter_y, mv_scale_x, mv_scale_y,
   reset, quit = 0}` (48 bytes), before the game's own submit, so the helper's submit is not
   late. **Every FrameMsg must be followed by the game's signal of `wait_value`**: the helper's
   submit waits for it.
3. `FlushRenderingCommands()`; `GetVulkanImageInfo` of the D3D11 textures (VkImage and the
   layout DXVK keeps them in).
4. `LockSubmissionQueue()`; vkQueueSubmit(copy-in, signal `wait_value`); vkQueueSubmit(copy-back,
   wait `signal_value` at ALL_COMMANDS); `ReleaseSubmissionQueue()`. No CPU wait anywhere.
   - copy-in: DXVK's images to TRANSFER_SRC_OPTIMAL (src ALL_COMMANDS, MEMORY_READ|WRITE); the
     shared inputs acquired from `VK_QUEUE_FAMILY_EXTERNAL` to F, GENERAL to GENERAL (the first
     frame of a Setup: UNDEFINED to GENERAL without an ownership transfer, and the output too,
     UNDEFINED to GENERAL, then released to EXTERNAL); vkCmdCopyImage x3 (or the game's own
     passes writing them); DXVK's images back to their layout with dst ALL_COMMANDS (this
     barrier orders DXVK's later writes after our reads); the shared inputs released F to
     EXTERNAL, GENERAL.
   - copy-back: the shared output acquired EXTERNAL to F; the D3D11 target to
     TRANSFER_DST_OPTIMAL; vkCmdCopyImage; the target back to its layout with src
     TRANSFER_WRITE, dst ALL_COMMANDS / MEMORY_READ|WRITE (DXVK's following commands, the HUD
     and the present, see the copy and run after the wait, with no CPU wait); the shared output
     released F to EXTERNAL.
5. D3D11 draws the HUD on the target and presents.

Helper (its own device, a graphics+compute queue family H): reads the FrameMsg; records:
acquire all four EXTERNAL to H (GENERAL), DLSS's evaluate, release all four H to EXTERNAL;
vkQueueSubmit(wait `wait_value` at ALL_COMMANDS, signal `signal_value`). Then it waits for its
own signal on the CPU (to read its GPU timestamps; the game does not wait for this) and sends
`FrameStat` (56 bytes: frame, four raw GPU timestamps: begin, evaluate begin, evaluate end, end;
`tick_ns`; `record_ms`, its CPU time to record and submit). The game may read the FrameStats
late or drop them, but must read them off the pipe (64 KiB hold about 1100).

All images stay in VK_IMAGE_LAYOUT_GENERAL in both processes; ownership goes through
VK_QUEUE_FAMILY_EXTERNAL at every hand-over.

A new size: once the old size's last frame is through (the game saw its `signal_value`), the
game sends `FrameMsg{quit = 2}`, then a Hello with the new output size and a Setup with new
images (and a new handle of the timeline semaphore). The helper frees the old images and DLSS
feature and keeps its device and NGX: 36 ms measured, against 0.8 s for a new helper.

## 5. DLSS's inputs (per frame, the helper's side)

- Colour, depth, motion vectors, output: the four images' views (output read-write).
- `jitter_x`, `jitter_y`: the sample's offset from the pixel centre, in render pixels, the
  value the game shifted its projection by, in [-0.5, 0.5]. The sign is the game's to settle:
  the probe scored the negated offset better at Quality (21.9 against 19.2 dB PSNR).
- Motion vectors: every pixel, current to previous position (`previous = current + mv`), in
  render pixels with `mv_scale` 1, 1 and the MVLowRes flag (another unit: its scale in
  `mv_scale_x/y`).
- `reset` = 1 on the first frame of a Setup and on a cut. Sharpness 0, pre-exposure 1,
  exposure scale 1, the render subrect the whole render size.
- The feature: render size to output size, the Hello's quality, the Setup's flags.
