# The sound follows the audio output device (patch 100)

Players' report (Discord backlog 2026-10-21, B43): when the audio output device changes or goes
(a Bluetooth headset connects or disconnects, the default device is switched, a USB DAC is
unplugged), the game has no sound until it is restarted. Survarium 0.59g1's changelog fixed it;
0.69 has a device monitor and a mastering-voice restart.

## Spec

- A lost output device (XAudio2's critical error): the sound comes back on the default device,
  within one sound-thread tick of a device being there.
- A new default render device (Windows' console or multimedia default): the sound moves to it.
  The communications default is not followed. The same device again: nothing happens.
- No device at all: silence, as before; tried again every 2 s, and at once on the next device
  event. The failures are logged once, not at every retry.
- What plays goes on: every voice is made again as it was (sends, effects and their
  parameters, volumes, matrices, frequency ratio, started or not), and a source voice's buffers
  that had not ended are queued again in their order. Each buffer still ends once.
- A flushed buffer is never played again. The game flushes at every detach_voice (Stop, then
  FlushSourceBuffers, 0x5CB8E..0x5CBB5: a sound's end, an HDR cull, #1247's hand-on), and XAudio2
  ends the flushed buffers on a later pass, which a dead or stopped engine never runs: the rebuild
  gives their OnBufferEnd to the game itself (so 0x5DDE0's order lets the decoded chunk go), with
  the old engine gone and no source voice on the new one yet.
- The device is picked by the game's own rule (`initialize_xaudio`): the last device of the
  DefaultMultimediaDevice role, else device 0; the mastering voice gets the game's own
  arguments (default channels, 44100 Hz).
- Each event is a line in `srp_patches.log` (`srp: audio_device: ...`).
- Sound only: nothing a match computes. No setting.

## 0.26g (static RE of the original exe)

- XAudio2 **2.7** (DirectX SDK June 2010): `XAudio2Create` inlined at 0x5A640 (cdecl, the engine
  pointer and the flags) is CoCreateInstance (the delay-loaded slot 0xAF0064) of CLSID_XAudio2
  {5a508685-...} (debug {db05ea35-...} for flag bit 0; 0x9E0C88 / 0x9E0C78), IID_IXAudio2
  {8bcf1f58-...} (0x9E0C98), then `Initialize` (vtable +14h).
- `sound_world::initialize_xaudio` 0x59080 (`ret 4`, the world at ESP+4; the name is its log
  string 0x9E0820): XAudio2Create into world+0C0h (0x59187, again without the debug flag at
  0x591C8), GetDeviceCount (+0Ch), the device loop (GetDeviceDetails +10h, role bit 2 kept,
  0x594A0..0x59698), `CreateMasteringVoice` (+28h) into world+0C4h with 0 channels, 44100 Hz,
  flags 0, that device, no effects (0x596AF..0x596C3). Nothing registers an engine callback and
  nothing watches the devices (no IXAudio2EngineCallback or IMMNotificationClient in the exe).
- `create_submix_voice` 0x59B80: 2 channels, 44100 Hz, stage 2, then `SetOutputVoices(NULL)` (to
  the master). The 0.10b source: a source voice is made with no sends (to the master) and sent to
  its scene's submix when used, back to the master when let go (voice_factory::delete_voice); the
  scenes' environment submixes (stage 1, a reverb XAPO each) send to the scene's submix.
- `sound_world::tick` 0x59A60 (thiscall, the sound thread): `push edi / call 0x59DD0`
  (process_orders, stdcall, `ret 4`) at 0x59AAB, the scenes' SetVolume (+30h) and the engine's
  CommitChanges (+34h) with the world's operation set at its end.

So after a critical error XAudio2 is stopped for good (Microsoft: "the only way to recover is to
release the XAudio2 instance and create a new one"), and a new default device is never used.

## 0.69d0 (the retail x64 exe, static)

- `sound_world::create_mastering_voice(bool)` 0x200626B00: the default render endpoint
  (IMMDeviceEnumerator::GetDefaultAudioEndpoint, state active), its id compared with the
  mastering voice's: the same is "Sound device change detected, but master device was not
  affected. Ignoring."; another: `stop_mastering_voice` 0x200626610 (every voice of the pools and
  every submix destroyed, then the master) and the master made again.
- `restart_mastering_voice` 0x2006268E0 makes the submixes and the pools again; the device
  monitor (IMMNotificationClient, OnDefaultDeviceChanged LIKELY 0x20061FDB0) posts "Audio device
  change has been detected." (`on_audio_device_changed` LIKELY 0x200626100).

## The patch

`client-patches/audio_device_026g.c` (the core, plain C), `audio_device_engine_026g.c` (its
platform in the game: the engine-function library) in the arena's blocks `audio_device.code` and
`.state`; three call sites (`AUDIO_DEVICE_HOOKS`):

- 0x59187, 0x591C8 (XAudio2Create): `audio_device_create` makes the engine and gives the game our
  proxy of it. Every voice the game makes through it is a proxy too (a vtable of ours whose
  methods forward to the real voice and keep what the call set). Our engine callback is
  registered on the real engine (OnCriticalError: an event bit), and our IMMNotificationClient
  with the MMDevice enumerator (OnDefaultDeviceChanged of a render device, not communications:
  an event bit).
- 0x59AAC (tick's process_orders): `audio_device_orders`, on the sound thread, before the orders:
  an event (or a retry due) rebuilds.

The rebuild: a new engine (the game's XAudio2Create, its flags) and a mastering voice on the
default device first; if either fails nothing changed (the old graph stays, tried again later).
Then the old graph goes in XAudio2's order (the sources, the submixes up their stages, the
master, the engine), and every voice is made again on the new engine (the submixes down their
stages, so a voice's destinations are there first, then the sources) with what was kept; a
source's pending buffers are queued again, then it is started if it was. An effect (the reverb
XAPO) keeps a reference of ours, so the same object goes into the new voice once the old one
let it go.

One owner: the engine proxy's binding (no master / bound to a device id / lost with a retry
time) is changed only by the sound thread's step; the other threads only raise the event bits.
A successful FlushSourceBuffers marks the queue's entries flushed (all of a stopped voice, all
but the playing one of a started one), kept so a late OnBufferEnd still finds its entry by
context; a rebuild ends the flushed ones (`ad_end_flushed`) and queues only the others again.
The pending queue of a source voice is a ring under a spin lock that is never held across an
XAudio2 or game call (XAudio2's thread takes a buffer off it in OnBufferEnd). Two engine
callback objects alternate, so a late critical error of the engine a rebuild replaced is not
taken for the new one's.

What is not made again (logged): a property too big to keep (more than 4 sends or effects, an
effect's parameters over 128 bytes, a matrix over 64 coefficients, a non-PCM format over 64
bytes); a voice whose remaking fails (it stays silent; its pending buffers do not end).

## Checks

- `client-patches/test_audio_device_026g.c`: the core, i686 (stdcall), under ASan/UBSan, against
  a made-up XAudio2 that keeps its rules (one master an engine; sends to later stages of the same
  engine; a destination and the master not destroyed while sent to / while voices live; an effect
  in one voice; buffers ending in order, one OnBufferEnd each). The game's graph (master, a stage
  2 submix, a stage 1 submix with a reverb, four sources with matrices, volumes, ratios, three
  buffers each, two started): a default change to the same device (nothing), a lost device (the
  same graph on the new default, the pending buffers in order, each ending once), a late critical
  error of the old engine (ignored), no device (kept, retried 2 s later), a new default (moved),
  a communications default (ignored), a failed engine creation (kept, retried), a flush on a live
  engine (ended once, by its pass), flushes on a dead engine while lost (the stopped voice's and
  a started voice's but its playing one: ended by the rebuild, the new owner's buffers alone
  queued again; this case fails on the code before the fix), the game's end
  (every voice, engine, effect reference and the device client given back).
- The exe built; the three sites disassembled (calls to the two entries; `audio_device_orders`
  realigns its stack and returns `ret 4`); `engine_026g.py`, `hooks_026g.py`, `link_026g.py
  --check` current.
- **Not run in a game.** Windows XAudio2 2.7's behaviour (a new engine and master after a
  critical error; an XAPO moved to a new engine's voice) is from Microsoft's documentation and
  0.69's code, not tried.
