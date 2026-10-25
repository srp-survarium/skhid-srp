# Voice chat (being built: PR #357)

The owner asked for voice chat in matches (2026-10-23). This note is the design and what the
0.26g exe and the later builds already have. Nothing here is built yet: 0.26g has no capture
path and no codec, so the work is about two weeks (estimate at the end), and it waits for the
owner's go on the open points below.

## What 0.26g already has

All of it is the unused front end of a feature the 0.26g engine never had. PROVEN unless marked.

- **Options, sound tab.** The tab has four voice controls: `_sound_chat`, `_use_mik`, `_mic_sens`
  and `_ptt` (`flash/main_menu/src/survarium/menu/Settings.as`, `_element_map[3]`). Their labels
  are in the stock English and Russian localization: `st_voice_chat_volume_option`,
  `st_use_microphone_option`, `st_microphone_sensitivity_option`, `st_ptt_button_option` and
  `st_options_voice`.
  - `options_tab::options_tab` (0x262300) binds them by console-variable name at 0x262DE8..0x262EC9:
    `s_chat_volume` (float, item 3), `s_use_microphone` (bool, 4), `s_mic_sens` (float, 5) and
    `s_ptt_button` (bool, 6). It calls `options_item_base::options_item_base` (0x260E90), which
    looks the name up with `console_commands::find` (0xEB7C0, at 0x260EF0).
  - No variable of those names is registered: each name string is used once, at that call. So the
    lookup gives NULL and the controls do nothing.
  - The engine source shows the same: `vostok/game/sources/options_items.cpp` builds the items,
    and no code registers the variables.
  - Registering the variables, as patch 58 did for `g_private_messages_in_game`, makes these
    controls work and saves them in `user.cfg`.
- **A push-to-talk key.** The action table (0xA74AF0) has the entry 0xA74F8C
  `{"ptt", action 47h, kind 8 (chat keys), default kP}`. The options' key list labels it
  `st_ptt_action` (the entry `{47h, "st_ptt_action", 1}` at 0xA9EB00). So it is already listed,
  can be rebound in the options, and is saved as `bind ptt <key>` in `user.cfg`.
  - Nothing uses action 47h. The chat handler `chat_handler::on_keyboard_action` (0x208870) asks
    the kind-8 bindings (`key_binder::get_binded_action` 0x207D50, EDI = 8) and acts on 45h
    (send) and 46h (send to) only.
  - The player's `on_keyboard_action` (0x23FE00) never asks kind 8.
- **P is free in a match.** P is also the default of the debug action 38h
  `deserialize_player_state` (0xA74E24). On key down the player's handler runs it as a console
  command (0x23FE58 -> `console_commands::execute` 0xEB980). No console command of that name
  exists: its string is referenced only at the two execute sites, 0x23FE5C and 0x24951C. LIKELY:
  the command does nothing but log. The input builder (0x241250) does not ask action 38h.
  - So the stock binding `ptt = P` can stay as it is.
  - Keys free in every group, if the owner wants another: B, H, J, L, U, Y, Z, 9, 0, F4, F7-F10,
    F12, Caps Lock.
- **A lobby message, dead.** Server-to-client lobby id 0xA0 has the statistics name "connect to
  MyTeamVoice room" (0x215A7C, string 0x9FF0C4) and a NULL handler: sending it crashes the game
  (lobby.md). Not used.
- **Sound output.** XAudio2 2.7 through COM (`CoCreateInstance`; ole32 is delay-loaded):
  `sound_world::initialize_xaudio` 0x59080, `create_sound_voice` 0x59E70,
  `create_submix_voice` 0x59B80.
- **No capture and no codec.**
  - The delay imports (17 DLLs) have no `waveIn*`, no DirectSound and no capture API.
  - There is no Opus, Speex or CELT. Vorbis is a decoder only, for the game's sounds.
  - `winmm.dll` is delay-loaded (`timeGetTime`), and `LoadLibraryA` / `GetProcAddress` are
    imported, so `waveIn` and `waveOut` can be reached at run time.
  - `ws2_32.dll` is delay-loaded with `socket`, `sendto` and `recvfrom`.

## What the later builds had

- **0.46c2** (exe in the weapon survey): the same dead options and `ptt` action. **Quick
  commands** are new:
  - the config `gameplay/sound_commands.options`;
  - 896 phrase collections, each recorded twice: `<phrase>.sound_collection` and
    `<phrase>_radio.sound_collection`;
  - the setting `sound_commands_radio_distance`.
- **0.53** brought voice chat: "Voice Chat" in the 0.53 news (`st_whats_new_053_voice_chat`).
- **0.69a0** (x64 exe from `/mnt/toshiba/surv-resources/0.69a0`, BCJ-decoded; addresses are
  in that file):
  - **Capture:** `vostok::sound::voice_recorder`, through WASAPI with DirectSoundCapture as a
    fallback (`IMMDevice::Activate failed`, `DirectSoundCapture CreateCaptureBuffer failed`).
  - **Codec and processing:** "libopus 1.3-beta", and speexdsp for resampling and preprocessing
    (VAD, echo cancel).
  - **Settings:** `voice_chat_enabled`, `voice_chat_volume`, `voice_chat_open_mic`,
    `voice_chat_push_to_talk_hotkey`, `voice_chat_mic_device`, `voice_chat_bitrate_sent`,
    `voice_chat_speech_low/high`, `voice_chat_echo_cancel_a/b`.
  - **Playback:** a pool of 20 `voice_player`s (0x200628130). Each has an Opus decoder at
    48 kHz, a resampler to 44.1 kHz and an XAudio2 source voice (0x20062FBF0): mono, 44.1 kHz,
    16-bit, no send list, no effect chain, volume 1.0. Each goes straight to the mastering voice.
  - **The game side:** `survarium::voice_chat_client`, player mutes (`player_mute_voice`), and a
    room option `full_voice_chat` ("Common voice chat": both teams in one channel). The HUD calls
    `root.voice_chat_set_players_talking(_icons)`, `root.set_local_player_talking` and
    `root.set_voice_indicator_level`.
  - So in 0.69, **voice is team-only by default and not positional**. Enemies never hear it,
    near or far. A custom room can make it one channel for everyone.
- **The radio effect** the owner remembers is the **quick commands'**, not voice chat's.
  - A phrase heard from beyond `sound_commands_radio_distance` plays its `_radio` recording;
    from nearer, the plain one.
  - The 0.69 config holds only the phrase lists; the distance's default was not recovered.
  - 0.26g has no quick commands.
  - Voice chat in 0.69 has no filter of any kind (above).

## The rules (v1)

- **Who hears whom.** Team-only, in a match, as in 0.69. Enemies never hear, near or far. No
  voice in the lobby, no squad channel, no "common" channel (0.69 had that only as a room
  option; a later rooms feature).
- **The radio effect** (an owner decision, default on):
  - A teammate within `VOICE_NEAR` (placeholder 25 m) is heard plain. One farther away is heard
    through a radio filter: band-pass 300-3400 Hz, light clipping and a little noise.
  - This is the quick commands' idea applied to voice. 0.69's voice had no such effect.
  - Each client decides it from its own simulation (it knows every player's position). The
    server does not take part.
  - The volume stays the same near and far; it is not proximity chat.
- **Push-to-talk** on the stock `ptt` action. The key is P by default, rebindable in the
  options' chat keys, and kept in `user.cfg`.
  - Hold to talk.
  - It does not work while typing in the chat (the chat handler takes the keys first).
- **Options.** The stock controls start working:
  - "Voice chat" volume (`s_chat_volume`, 0..1, default 1; 0 also tells the server not to send);
  - "Use microphone" (`s_use_microphone`, default on): the off switch for talking;
  - "Microphone sensitivity" (`s_mic_sens`, 0..1, default 0.5): the mic gain.
  - The "PTT" checkbox (`s_ptt_button`, open mic when off) stays disabled: v1 is push-to-talk only.
- **Speaking indicator.** On the HUD, a short list of who is talking: a speaker mark and the name,
  above the chat on the left. Your own name shows while you transmit.
- **Who can talk.** Dead players talk to their team too (0.69's rule is not known). Bots never
  talk. Players who join late are heard from the moment their client says hello.
- **Never kept.** The server only forwards audio (Privacy, below).

## The path

    talker's game                   srp (voice port, UDP)              teammate's game
    +-------------------+           +------------------------+         +-------------------+
    | P held            |           | session -> seat        |         | jitter buffer     |
    | waveIn 16 kHz     |  START    | (match, team, slot)    |  START  | Opus decode       |
    | Opus 16 kbps      | --------> | IP == match conn's IP? | ------> | radio filter if   |
    | 2 x 20 ms / dgram |  AUDIO    | forward to teammates   |  AUDIO  |   > VOICE_NEAR    |
    | P released        | --------> | with a live HELLO      | ------> | waveOut           |
    |                   |  END      | tap (none by default)  |  END    | HUD: name talking |
    +-------------------+ --------> +------------------------+ ------> +-------------------+

- **A separate UDP channel to srp, not the match transport.**
  - The match transport has fixed tables of message ids and lengths in the exe (match.md,
    transport.md section 4). A new id means patching those tables and the dispatcher.
  - It would also share the 8-bit sequence window and the 128-unacked limit with the
    simulation's inputs.
  - So voice goes on its own socket: the client's DLL to a new srp port, `SRP_VOICE_PORT`
    (default: the match port + 1).
  - 0.69 kept voice apart too (`voice_chat_client`).
- **Who a datagram is from.** The `u32 session` of the client's 0xD8 connect (match.md),
  checked two ways: the session holds a seat in a running match, and the datagram's IP is the
  IP of that seat's match connection. A datagram that fails either is dropped silently. Ports
  may differ (NAT).

## The wire (voice port)

Little-endian. Every datagram starts with `56 01 <type>`: the magic, the version, then the type.
**A receiver ignores a type it does not know.** That is the rule that lets later types
(subtitles, translated audio) come without breaking older games.

| type | dir | body | when |
|---|---|---|---|
| 01 HELLO | C->S | `u32 session, u8 flags, u32 muted` (bit 0: hears, i.e. volume > 0 and voice not disabled; bit n of muted: slot n muted; the mask may be left out) | on match connect, every 2 s (keeps NAT open, tells srp where to send), and at once on a change |
| 02 START | C->S | `u32 session, u16 utt, u32 client_ms` | P down |
| 03 AUDIO | C->S | `u32 session, u16 utt, u16 seq, u8 n, n x (u8 len, Opus frame)` | every 40 ms while held (n = 2) |
| 04 END | C->S | `u32 session, u16 utt, u16 frames` | P up; sent 3 times, 30 ms apart |
| 81 WELCOME | S->C | `u8 slot` | answer to HELLO |
| 82 START | S->C | `u8 speaker_slot, u16 utt, u32 server_ms` | relayed |
| 83 AUDIO | S->C | `u8 speaker_slot, u16 utt, u16 seq, u8 n, frames` | relayed |
| 84 END | S->C | `u8 speaker_slot, u16 utt, u8 reason` (0 released, 1 timeout, 2 too long) | relayed |
| 85 TEXT | S->C | reserved: `u8 speaker_slot, u16 utt, u8 final, u8 lang[2], u8 len, utf8` | later: subtitles |
| 86-8F | S->C | reserved: an AUDIO-like stream with a language | later: translated audio |
| 90 ROSTER | S->C | `u8 count, count x (u8 slot, u32 voice_id)` | with each WELCOME: the listener's team |

- **speaker_slot** is the speaker's player index in the match: the order of the match-start
  player list (0xE4, static_info.md). The client gets the name and the team from it without new
  data.
- **utt** counts up per talker. **seq** counts AUDIO datagrams within the utterance. Lost and late
  datagrams are skipped (Opus packet-loss concealment), never resent.

## The relay (srp)

- **Utterances.** srp keeps one open utterance per speaker:
  `{match, team, speaker slot, utt, started_at (server), last_audio_at}`.
  - START opens it and is forwarded at once.
  - Each AUDIO goes to every teammate with a HELLO in the last 6 s, except the speaker, a
    listener whose HELLO mutes the speaker's slot, and a
    listener whose flags say it does not hear.
  - END closes it.
  - No AUDIO for 1 s closes it as a timeout. After 30 s it is cut (END reason 2); the talker's
    next START opens a new one.
  - AUDIO for an utterance srp did not see start opens it (a lost START).
- **Limits.** 60 datagrams/s and 400 bytes a datagram per talker; one open utterance per talker.
  The excess is dropped and counted.
- **The seam for later steps** (subtitles, translation; another design, `feat/chat-translation`):
  the relay calls an `UtteranceTap` for every utterance, `NoTap` by default.

      trait UtteranceTap {
          fn started(&mut self, u: &Utterance);                 // match, team, speaker, utt, started_at
          fn audio(&mut self, u: &Utterance, seq: u16, frames: &[&[u8]]);
          fn ended(&mut self, u: &Utterance, reason: EndReason);
      }

  - Frames are borrowed for the call only.
  - A tap that needs a whole utterance buffers it itself, in memory, and drops it at `ended`.
  - What a tap produces goes back through `RelayOut::text(match, team, speaker, utt, lang, text)`
    and later an audio stream (types 85-8F). The wire above does not change.
- **The voice socket** runs beside the match server's (`servers/match-server/src/v026g`). It
  reads the seats from the match registry the lobby fills.
- **Built** (`servers/match-server/src/v026g/voice.rs`, srp's thread `voice`):
  - the port is always the match port + 1 (`SRP_VOICE=off` turns it off); Docker publishes it
    (`SRP26_VOICE_PORT`), and the host's firewall must let it in;
  - the registry keeps each place's match-connection IP (set at its 0xD8) and answers
    `voice_seat(session)`: match, place, team, IP; bots' places have none;
  - a released seat (left by 0xE0, the match's result) neither talks nor hears from the next
    second on;
  - tests: `cargo test -p match-server --lib voice` (17: team-only, mutes, IP and seat checks,
    malformed and large datagrams, lost START, timeout, the 30 s cut, the rate limit, the tap's
    text, real sockets over the registry, the 0xD8 setting the IP).

## The client

- **`skhid_voice.dll` beside the exe.** C, built with mingw32 (nixpkgs `pkgsCross.mingw32`), with
  libopus 1.3 linked in (fixed point; about 250 KB).
  - It owns capture (waveIn, 16 kHz mono), encoding (Opus VOIP 16 kbps), the socket, a 60 ms
    jitter buffer per speaker, decoding, the radio filter, mixing, and playback (waveOut, 16 kHz
    mono).
  - Exports: `sv_start(ip, port, session)`, `sv_stop()`, `sv_ptt(int down)`,
    `sv_settings(volume, mic_on, mic_gain)`, `sv_radio(slot, int far)`,
    `sv_talking(u32 *mask_out)`.
  - Why a DLL and not the arena: libopus is a C library of about 40k lines with its own memory
    and build. The arena is freestanding `gcc -m32` objects linked into an exe section
    (`link_026g.py`).
  - Why waveIn/waveOut and not XAudio2: the exe has no capture API at all, and waveOut keeps
    COM calls into the engine's XAudio2 objects off a second thread. A DLL can also be rebuilt
    without touching the exe's patches.
- **The exe patch** (arena, C, engine calls through `engine_026g.tsv`):
  - At start, `LoadLibraryA("skhid_voice.dll")`. When it is missing, voice is off and the log
    says so. The updater keeps it in place like `bugtrap.dll`.
  - Registers `s_chat_volume`, `s_use_microphone` and `s_mic_sens` (the `cc_value` constructors,
    as patch 58 and `uro_fov` do), and enables their controls in `Settings.as`.
  - Match connect (`match_client::connect` 0x27A1B0, the 0xD8) calls `sv_start` with the
    server's address and the session. Leaving (0x27A790, the 0xE0) and the end of the match call
    `sv_stop`.
  - Push-to-talk: the player's `on_keyboard_action` (0x23FE00) also asks
    `get_binded_action(key, 8)`. Action 47h down calls `sv_ptt(1)`, up calls `sv_ptt(0)`. Nothing
    else sees the key.
  - Once a frame: `sv_talking` and, for the radio effect, each talking teammate's distance from
    the local player (where the game world keeps positions: still to RE). On a change it calls the
    HUD's `root.srp_voice_talking(names)`.
- **HUD movie:** `flash/hud/src/HUD.as` gets `srp_voice_talking(names:Array)`: a list of names
  with a speaker mark, under the minimap or above the chat, and nothing when empty.

## As built (2026-10-25)

Where the build differs from or adds to the design above:

- **The exe patch is 66** (`client-patches/voice_026g.asm`, `voice_026g.c`; README patch 66,
  client_patch.md 3.55). The sites: game::game (the variables), match_client::connect
  (`sv_start(host, match port + 1, session)`), on_finish_match and on_deactivate (`sv_stop`),
  game_world::tick (every frame), on_keyboard_action's Tab-test call (the keys), the Esc menu.
- **Options:** `s_chat_volume` and `s_mic_sens` are 0..100 (the game's float slider steps by 1),
  defaults 100 and 50; `s_use_microphone` default on. **"Disable voice chat"** is the gameplay
  tab's dead option 5 (`g_hide_spam`, "Hide spam"), relabelled by `voice-chat-1.json`, next to
  "Hide chat in match" (patch 58's dead option 4, the same way).
- **Keys:** PTT is the stock `ptt` (P, the chat keys' group; relabelled "Push to talk"). **"Mute
  the last speaker"** is the action table's dead entry 37h renamed `mute_last_speaker`, a 42nd
  Controls row, **no default key**: M is `set_next_ammo_type` in 0.26g, so it waits for the owner's
  pick (free in every group: B, H, J, U, Y, Z, 9, 0, F4, F7-F10, F12, Caps Lock).
- **The Esc menu's "Voice" panel** (`main_menu`: `VoicePanel`, beside the menu's buttons): the
  teammates whose voice id is known, a mute checkbox each, "Mute all". Empty and hidden in the
  lobby. It works in Blackout (the Esc menu frees the cursor; the Tab list is not used).
- **The HUD list** (`hud`: `VoiceSpeakers`, left of the screen above the middle): the player's own
  name in gold while transmitting, then each teammate talking; a dimmed speaker mark for the radio.
  Muted players are left out. The mute key shows "Muted: name" / "Unmuted: name" under it for 3 s.
- **The radio** (the DLL): a teammate farther than 25 m, or either player dead, through a filter
  fitted to 0.69's `_radio` quick-command recordings (high-pass 405 Hz, peaks and dips up to 6.3
  kHz), retail static under it (`tutorial_interference_02`, -44 dBFS), and retail's squelch at the
  start and end of a far utterance (the static lead-in and tail of 0.69's `_radio` phrases: 0.69 has
  no separate squelch file). Near: plain, no squelch. The sounds are compiled into the DLL
  (`client-voice/make_radio_sounds.py`).
- **The DLL:** libopus 1.6.1 in floating point (its fixed-point x86 build does not link), static;
  about 550 KB; imports kernel32, msvcrt, winmm and ws2_32 only. The microphone opens on PTT and
  closes 1 s after. One press transmits 30 s at most.

## Bandwidth

- Talking: Opus 16 kbps, 2 frames of 20 ms per datagram (about 80 bytes of audio + 10 header +
  28 IP/UDP), 25 datagrams/s, about **24 kbit/s up** for the talker.
- Each teammate gets **24 kbit/s per teammate talking**. srp sends the speaker's stream to each
  of up to 9 teammates (10v10): about 216 kbit/s per talker. Two talking at once in each team:
  under 1 Mbit/s for a match.
- Idle: a HELLO every 2 s (about 40 bytes). The match transport's traffic is unchanged.

## Desync, resync, replays

- Voice never touches the simulation:
  - another socket on both ends;
  - not the match transport, the input relay, the hash checks or the world uploads.
- A resync (0xE5), a world restore, or a late join changes nothing for voice. A resynced player
  keeps talking and hearing.
- Replays (`replay.rs`) and the match recordings (`record.rs`, `SRP_RECORD_DIR`) are made from
  the match socket's datagrams only, so **no voice is ever in a replay or a recording**.
- What a match computes does not change: no `simulation_changes.tsv` row.

## Privacy

- srp forwards each datagram to the listeners and keeps nothing:
  - no audio is buffered past the forward;
  - nothing is written to disk;
  - no audio, text or speaker list goes in the logs.
- The logs keep counts only: per match, the utterances, the seconds talked and the datagrams
  dropped.
- `NoTap` is the default. A tap that reads audio (subtitles, translation) is a separate decision
  by the owner, with a notice to the players before it is turned on. It holds an utterance in
  memory only while it works on it.

## Open for the owner (decided 2026-10-25)

- **Radio effect: on.** A teammate within 25 m (`VOICE_NEAR`) is heard plain; farther, through
  the radio filter (band-pass 300-3400 Hz, light clipping, a little noise). Same volume near and
  far: not proximity chat. Each client decides from its own simulation.
- **Push-to-talk: P**, the stock `ptt` action, rebindable in Controls.
- **Dead players talk to their team: yes.**

## Added by the owner (2026-10-25)

- **Muting**, without the Tab list (Tab frees no cursor; Blackout hides the list):
  - the Esc menu gets a "Voice" panel: the teammates, a mute toggle each, and "Mute all";
  - a key "mute the last speaker" in Controls (default M only if M is free in every group);
  - mutes are per account, kept across matches; the HUD list leaves the muted out;
  - kept by a **voice id**, not the name (accounts can be renamed; the owner, 2026-10-25): srp's
    ROSTER gives each teammate's slot the first four bytes of SHA-256(key, account id), the key
    `<SRP_DATA_DIR>/voice-id.key` (32 random bytes made once). Stable, and no account id goes
    to other players. The game keeps the muted ids in `user.cfg` (`srp_voice_mutes`);
  - client-side: a muted slot is not played; HELLO also carries the mask of muted slots, and
    the relay sends that listener nothing of them.
- **"Disable voice chat"** in the options, beside "Hide chat in match": nothing played, no
  microphone, PTT does nothing, the HUD list empty, and HELLO says "does not hear", so the relay
  sends nothing. Saved as the other options, at once, no restart.

## Estimate and tests

About **8-10 working days** plus in-game rounds, because both the capture path and a codec have
to be added:

| part | days |
|---|---|
| srp: voice socket, seats and IP check, utterances, tap seam, limits, tests | 1 |
| `skhid_voice.dll`: waveIn/waveOut, Opus, jitter buffer, radio filter, socket; a Linux loopback test (encode, lose, decode) | 3 |
| exe patch: load, cvars, connect/leave hooks, PTT, HUD poll; selftest and a Unicorn check | 2-3 |
| positions for the radio effect (RE) | 0.5-1 |
| HUD list (`HUD.as`), options enabled (`Settings.as`) | 0.5 |
| shipping a new DLL: `build-zip`, the updater's client files | 0.5-1 |

The in-game tests need the owner: a real microphone on at least one machine.

1. Two clients on one team and one on the other.
2. Hold P on one: the teammate hears it and sees the name. The enemy hears nothing.
3. Walk apart past `VOICE_NEAR`: the radio sound starts.
4. Set the options volume to 0: nothing heard, and srp stops sending (capture).
5. Turn "Use microphone" off: P sends nothing.
6. A resync during speech: voice goes on.
7. Watch the match's replay: no voice.
