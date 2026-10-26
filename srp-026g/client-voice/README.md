# Voice client

`skhid_voice.dll` is Rust. It handles Opus, the UDP voice connection, WinMM
capture/playback, per-speaker jitter buffers, mutes and the retail radio effect.
The arena patch loads its optional interface from `crates/voice-abi`; missing or
incompatible DLLs leave voice disabled.

Build the 32-bit Windows 7 DLL with the same pinned Rust compiler and static
Microsoft C runtime as BugTrap:

```sh
nix develop ./srp-026g -c bash srp-026g/client-voice/build.sh
```

The result is `srp-026g/target-voice/i686-win7-windows-msvc/release/skhid_voice.dll`.
The build checks its exports against the shared ABI and refuses non-system DLL
imports. Opus is static; its COFF archive and GCC integer/stack helpers are
prepared by the pinned Nix `voice-opus` package. No GNU runtime DLL is required.
`/Brepro`, trimmed paths and no PDB make the DLL reproducible.

Portable tests and the real server relay loopback:

```sh
nix develop ./srp-026g -c bash -c 'cd srp-026g; cargo test -p skhid-voice; cargo test -p match-server voice'
```

`nix build ./srp-026g#skhid-voice-smoke` builds the independent C ABI probe.
Run its `smoke_voice.exe` under Windows or Wine with the DLL path as its argument.
It exercises start/stop, settings, roster, mute and PTT session boundaries over a
local UDP peer. Audio-device availability is reported separately. The real game
still needs two clients and a microphone to check capture, playback and controls.

To listen to the codec/jitter/radio path without hardware, use a PCM 16-bit mono
16 kHz WAV:

```sh
nix develop ./srp-026g -c bash -c 'cd srp-026g; cargo run -p skhid-voice --bin radio_demo -- input.wav output.wav far'
```

The radio's lead-in, tail and static samples come from the hash-checked retail
recordings named by `make_radio_sounds.py`. That script regenerates
`src/sounds.rs`; the portable tests check the filter against retail's measured
frequency response and the static's level.
