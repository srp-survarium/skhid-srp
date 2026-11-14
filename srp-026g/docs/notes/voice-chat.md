# Voice chat (PR #357: built, not played, not released)

The owner asked for voice chat in matches (2026-10-23). This note is the design and what the
0.26g exe and the later builds already have; "As built" below says where the build differs
(ported to main's named hook sites 2026-11-06).

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
  `create_submix_voice` 0x59B80. How it plays a sound in the world: "The game's sound engine",
  below.
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

## The game's sound engine (study, 2026-11-14)

For the plain voice in the world ("As built"). 0.26g, static, base 0x10000; names from the re-kit
(0.21d's PDB) and 0.10b's sources (`vostok/sound/sources`). PROVEN (read in the exe, the pan also
run on its bytes in Unicorn) unless marked.

- **From `emit_point_sound` to a voice.**
  - The game's thread: `sound_emitter::emit_point_sound` 0x65670 takes a proxy of the scene's pool
    (`new_point_sound_instance_proxy` 0x64F80; 144 bytes, +80h its sound_type: 0 point, 1 cone, 2
    volumetric, 3 hud). Playing it is an order to the sound thread (LIKELY: the world user's
    `sound_order` channel, by its names; `play` 0x5ABA0 was not read).
  - The sound thread: `sound_world::tick` 0x59A60 runs the orders, then every active scene's tick
    (0x57B80), which calls `notify_listener2` 0x5C280 once the scene has a listener (+2FAh,
    0x57C9C). At its end the scenes' submix volumes and one CommitChanges (audio_device.md).
  - The listener is three atomic `half3` in the scene (+120h place, +128h front, +130h top: 16-bit
    floats). The game's thread writes them every tick (`game_world::on_after_tick` 0x1F7620:
    `set_listener_properties` 0x587D0 with the game world's +3Ch, +2Ch, +1Ch, for its sound scene
    +94h); every reader reads them with a locked `cmpxchg8b` that leaves them as they are
    (`get_listenet_position` 0x57560, `..._orient_front` 0x575F0, `..._orient_top` 0x57680).
  - `notify_listener2` first has `calculate_hdr_audio_frame` 0x5C7A0 give every propagator its
    gain by distance and ask for voices, then gives each voice its pan and its volume.
- **The gain by distance** (0x5C9B5..0x5CA37) is the sound's options' (`<sound>.raw_options`):
  - with an `spl_preset`: that preset's `volume` curve at the distance (`sounds/spl/<preset>.spl`:
    keys by metres, linear or cubic; the curve at the preset's +128h, `curve_line_points<float>::
    evaluate` 0x25F30);
  - with none (`default`: 2042 of the stock sounds): 1 to 2 m, 2 / d past it (2.0 at 0x9CC87C);
  - held to 0..1, kept at the propagator's +4Ch; the distance is never under 1 m
    (weapon-reverb-layers.md). A voice is asked for only above 0.01 (0xA2D774).
  - The voice's volume is that gain times `volume_k` (0x5C72D..0x5C743), held to 0..1
    (`voice_bridge::set_volume_impl` 0x633F0).
- **The pan** is `sound_scene::calculate_3d_pan` 0x57D70 (the scene, the world's panning table,
  the proxy, the sound's place, the distance; out: two gains and a low-pass coefficient; pops 28).
  - It is 0.10b's `calculate_channel_matrix` (OpenAL Soft's table panning). The table
    (`panning_lut`, inside the sound world at +0CCh) is 512 directions of 9 speaker gains, filled
    once by its constructor 0x5E150 for two speakers at the sides.
  - Run on the exe's bytes (`check_voice_pan_026g.py`): constant power; ahead and behind 0.707 in
    both ears, 30 degrees to the right 0.499 / 0.867, at the side 0 / 1, above towards the centre;
    no gain by distance in it. The low-pass coefficient is 1 ahead and falls to 0.80 straight
    behind.
  - Of a point sound it reads the proxy's +80h, the place, the table and the scene's three
    listener vectors, and writes its two gains and the coefficient: nothing else outside its stack.
  - A mono sound's two gains are its output matrix into its scene's two-channel submix
    (`voice_bridge::set_output_matrix_impl` 0x5DC40). A hud sound (type 3) is not panned: 1 and 1.
- **The volumes.** A scene's submix gets its fade volume times the general volume
  (`s_general_volume`; fields_026g.tsv, the scene's +2E4h and +2E8h); the master is at unity.
- **Threads.** Every XAudio2 call is the sound thread's. The game's thread writes the listener's
  atomics and queues orders. With patch 100 the engine and every voice the game makes are its
  proxies, whose list only the sound thread's step changes (audio_device.md).
- **The characters' own voices.** 0.26g has no speech. Its voices are the death scream
  (`sounds/single/animations/death/male_scream/death_0N_m`, as others hear it: preset
  `death_scream_close`, `volume_k` 1.3; the player's own `death_0N`: `default`) and the breath of
  an exhausted player (`human/stamina/*_m`: preset `steps`, `volume_k` 0.2 to 0.5). They fall fast:

  | the gain at | 1 m | 5 m | 9 m | 10 m | ends |
  |---|---|---|---|---|---|
  | `death_scream_close` | 0.86 | 0.38 (-7 dB) | 0.10 (-19 dB) | 0.07 (-21 dB) | 15 m |
  | `steps` | 0.67 | 0.23 (-9 dB) | 0.10 (-17 dB) | 0.08 (-18 dB) | 17 m |
  | none (`default`: 2 / d) | 1 | 0.40 (-8 dB) | 0.22 (-13 dB) | 0.20 (-14 dB) | never |
  | `material_hit` (a bullet's hit) | 0.92 | 0.64 (-3.2 dB) | 0.44 (-6.5 dB) | 0.40 (-7.3 dB) | 35 m |
  | `weapon_echo_loud` | 0.96 | 0.74 (-2.3 dB) | 0.51 (-5.6 dB) | 0.46 (-6.4 dB) | 40 m |
  | `weapon_shot_mosin` | 0.92 | 0.64 (-3.1 dB) | 0.47 (-5.8 dB) | 0.43 (-6.6 dB) | 100 m |
  | `weapon_shot_40` | 0.97 | 0.84 (-1.2 dB) | 0.71 (-2.7 dB) | 0.67 (-3.1 dB) | 45 m |
  | `weapon_echo_close_mid` | 0.97 | 0.85 (-1.2 dB) | 0.72 (-2.6 dB) | 0.69 (-3.0 dB) | 85 m |

  (the stock db's 52 presets, their `volume` keys read linearly between the keys: within a few
  tenths of a dB of the engine's cubic keys, LIKELY; the dB are against the gain at 1 m).
- **Can a live voice be a sound of the scene?**
  - Not a sound instance. One plays an emitter resource: a single sound with its Ogg, its `.rms`
    and its options, of a known length, decoded into the voice's buffers as they end
    (`sound_voice::refill_buffers` 0x632A0, `ogg_encoded_sound_interface::decompress` 0x64ED0;
    LIKELY, from the names and 0.10b's sources). A stream of no known length is none of that
    without made-up engine objects.
  - A source voice of ours on the engine's XAudio2 (the sound world's +0C0h; `IXAudio2::
    CreateSourceVoice`, +20h by XAudio2 2.7's declaration): possible, and the engine-function library has COM calls. It would
    be made, fed and placed on the sound thread (`sound_world.tick`, patch 88's site), sent to the
    scene's submix so the game's volume and device apply, its matrix the same `calculate_3d_pan`.
    What it needs and nothing here can run: the decoded frames handed from the DLL's worker to the
    sound thread and kept until XAudio2 ends them; the voices gone before their scene's submix and
    the engine go (the match's end, the game's exit, patch 100's rebuild of every voice on a
    device change); XAudio2 2.7's and Wine's behaviour with a 16 kHz streamed voice. A mistake
    there is a crash or a silent sound thread for every player. **Not built blind** (2026-11-14);
    the next step when it can be played.
  - Built instead: the DLL keeps playing, and the gains it mixes a plain voice by are the
    engine's (below, "Plain in the world").

## The rules (v1)

- **Who hears whom.** Team-only, in a match, as in 0.69. Enemies never hear, near or far. No
  voice in the lobby, no squad channel, no "common" channel (0.69 had that only as a room
  option; a later rooms feature).
- **The radio effect** (an owner decision, default on):
  - A teammate within `VOICE_NEAR` (25 m; 15 m since 2026-11-13; 10 m since 2026-11-14) is heard
    plain (since 2026-11-13 from its side, since 2026-11-14 in the world, falling with its
    distance: "As built"). One farther away is heard through a radio
    filter: band-pass 300-3400 Hz, light clipping and a little noise.
  - This is the quick commands' idea applied to voice. 0.69's voice had no such effect.
  - Each client decides it from its own simulation (it knows every player's position). The
    server does not take part.
  - The volume stays the same near and far; it is not proximity chat (until 2026-11-14: the plain
    voice now falls with its distance, gently; the radio's level is as it was).
- **Push-to-talk** on the stock `ptt` action. The key is P by default, rebindable in the
  options' chat keys, and kept in `user.cfg`.
  - Hold to talk.
  - It does not work while typing in the chat (the chat handler takes the keys first).
- **Options.** The stock controls start working:
  - "Voice chat" volume (`s_chat_volume`, 0..1, default 1; 0 also tells the server not to send);
  - "Use microphone" (`s_use_microphone`, default on): the off switch for talking;
  - "Microphone sensitivity" (`s_mic_sens`, 0..1, default 0.5): the mic gain (as built: the
    aim of the automatic gain, below).
  - The "PTT" checkbox (`s_ptt_button`, open mic when off) stays disabled: v1 is push-to-talk only.
    Removed from the tab since (the owner, 2026-11-14, told it is the stock game's push-to-talk or
    open microphone switch, kept greyed because it does nothing: "let's remove the button then"):
    the movie hides it after each bind of the tab (the exe's options still bind its option 6 by
    name, so it stays in the timeline), and the voice column closes up under the sensitivity
    slider. The push-to-talk key and its Controls row are unchanged.
- **Speaking indicator.** On the HUD, a short list of who is talking, as 0.69's (below, "As
  built").
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
  IP of that seat's match connection. A datagram that fails either, or is malformed, is dropped
  and logged (a line per source IP at most every 10 s, with its count; "As built"). Ports may
  differ (NAT).

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
- **utt** counts up per sending session. The relay assigns an ordered counter per match slot
  on the listening wire, so a restarted game can reuse its account and slot without listeners
  rejecting its reset counter. **seq** counts AUDIO datagrams within the utterance. Lost and
  late datagrams are skipped (Opus packet-loss concealment), never resent.

## The relay (srp)

- **Utterances.** srp keeps one open utterance per speaker:
  `{match, team, speaker slot, utt, started_at (server), last_audio_at}`.
  - START opens it and is forwarded at once.
  - Each AUDIO goes to every teammate with a HELLO in the last 6 s, except the speaker, a
    listener whose HELLO mutes the speaker's slot, and a
    listener whose flags say it does not hear.
  - END closes it.
  - No AUDIO for 1 s closes it as a timeout. After 31 s it is cut (END reason 2), a second
    past the games' own cap of a press (30 s), so their END comes first; the talker's next START
    opens a new one. A modified game that ends and restarts can talk on: that is left to the
    teammates' mutes (2026-11-13).
  - AUDIO for an utterance srp did not see start opens it (a lost START).
- **Limits.** 60 datagrams/s and 400 bytes a datagram per talker; one open utterance per talker.
  The excess is dropped and counted. Hardened 2026-11-13 against a modified game, to what the
  games' own sender makes:
  - an AUDIO carries 1 or 2 frames (the sender sends 2, 1 at an odd end; receivers play 2);
  - a frame's TOC is 0x08, 0x28 or 0x48 (SILK 20 ms mono code 0, narrow, medium or wide band:
    the encoder, 16 kHz speech at 16 kbit/s, makes only 0x48) and it is 120 bytes at most (the
    encoder averages 40 and was measured at 93 at most on full-scale squares and noise; it is
    given only 120 bytes of room, so it cannot make more); no CELT, hybrid, stereo or padded
    frame reaches a teammate's decoder;
  - a talker's AUDIO: 30 a second past a burst of 20 (the games send 25; 20 is more than a
    receiver's ring of 16 AUDIO, what a network hold releases at once);
  - each source IP: 2000 datagrams a second, a flood's cap only (many players share an exit,
    Cloudflare WARP or carrier NAT: 40 talking send under 1400);
  - seat lookups that fail (no seat, another IP) have a budget of their own per source IP, 20 a
    second: a session not yet known from that IP (its HELLO) is looked up only when the budget
    has room, one that is known always; a refused session is not looked up again from that IP
    for 1 s. So guessing sessions costs the registry's lock (which the match threads share) 20
    lookups a second, and a refused game never stops the others behind its exit. 4096 sources
    apart, the rest share one.
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
    (`SRP26_VOICE_PORT`, which must be the match port + 1: release.sh's preflight stops
    otherwise), and the host's firewall must let it in (UDP);
  - the voice ids' key (`voice-id.key`) is made only when there is none, mode 0600; a file that
    cannot be read or is not 32 bytes turns voice off with an error and is left alone (the
    players' mutes are kept by it);
  - the registry keeps each place's match-connection IP (set at its 0xD8) and answers
    `voice_seat(session)`: match, place, team, IP; bots' places have none;
  - a released seat (left by 0xE0, the match's result) neither talks nor hears from the next
    second on;
  - tests: `cargo test -p match-server --lib voice` (39: team-only, mutes, IP and seat checks,
    every malformed shape with its reason, frames not the games' encoder's, a flood not delaying
    a good sender, the drop log's bounds, lost START, timeout, the 31 s cut after the games' END,
    the rate limits (talker, audio, source, failed lookups; 40 players behind one exit), the
    tap's text, real sockets over the registry, the
    0xD8 setting the IP).

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

## As built (2026-10-25; ported 2026-11-06)

Where the build differs from or adds to the design above:

- **The exe patch is 66**, the safe Rust module `client-patches/rust/patches/src/voice.rs` over
  its boundary `vostok/voice.rs`. Its names-only `voice.toml` chains eight lohooks (three more
  since the microphone test, below) of
  `sites_026g.tsv`: `configs.start` (the variables, before user.cfg), `match_client.connect`
  (the options told, no mutes and no radio, then `sv_start(host, match port + 1, session)`),
  `game_world.finish` and `game_world.deactivate` (`sv_stop`, the talking list emptied),
  `game_world.after_move` (every frame), `tab_cursor.key` and `game_world.mouse_key`
  (push-to-talk on a key or a mouse button) and `hud.callback` (the Tab menu's voice entries).
  README patch 66, client_patch.md 3.71.
- **Options:** `s_chat_volume` and `s_mic_sens` are 0..100 (the game's float slider steps by 1),
  defaults 100 and 50; `s_use_microphone` default on. **"Disable voice chat"** is the gameplay
  tab's dead option 5 (`g_hide_spam`, "Hide spam"), relabelled in `localization/*.tsv`, next to
  "Hide chat in match" (patch 58's dead option 4, the same way).
- **Keys:** PTT is the stock `ptt` (P; relabelled "Push to talk"), the one voice key. The owner,
  2026-11-11: "P push to talk is not part of chat, it is part of general controls", "and it should
  be rebindable": its Controls row (the game's is the chat group's, its key button disabled) is in
  the game group and can be set (patch 62's `controls`); the action stays the binder's chat kind,
  as the game made it. A key or a mouse button: the game world's keys (`tab_cursor.key`) and its
  mouse buttons (`game_world.mouse_key`, past patch 31's jump) both ask the binder. A held PTT is
  let go when its release comes, when user32's GetAsyncKeyState says it is up (so a release the
  chat box took is seen; the numpad's keys by both the key they map to and the digit NumLock gives,
  NumLock and Pause by their own, a mouse button by its VK_*BUTTON; the buttons past the side ones
  only by their release), or when the game loses the keyboard. A "mute the last speaker" key (the
  action table's dead entry 37h renamed, a 43rd Controls row) was built and removed (the owner,
  2026-11-09: "that mute last speaker button can be removed, since now this can be done in UI");
  the exe's action table stays the game's.
- **The options from the start:** at the match's connect the DLL is told the game's options, no
  mutes and no radio before `sv_start`, so the loading plays and says HELLO as the options are (the
  DLL kept the last match's, by place, or its defaults; the review of 2026-11-11); the match's
  first frame then tells the mutes and the radio of its places.
- **Muting** (the owner, 2026-11-06: in the Tab list's player menu): a right-click on a teammate's
  row (Ctrl for the cursor, patch 86) offers "Mute voice" or "Unmute voice" after the chat mute's
  entries, for a teammate whose voice id the relay sent; the menu shows "Voice muted", "Voice
  unmuted" or "Voice chat unavailable". 0.69 had a team panel with the chat input instead (its
  `VoiceChatEditPanel`: a text-mute and a voice-mute button per row, mutes for the match only).
- **Who is talking** (the owner, 2026-11-06: as 0.69's `VoiceChatTalkingPanel`): `hud`'s
  `VoiceSpeakers`, a row per teammate talking, a light-blue speaker (0x73B9FF) and the name, 25 px
  apart, the newest on top; a name fades over 1 s, 0.5 s after its player stops. The muted do not
  show. The owner, 2026-11-11: "when I speak I want to see this with an icon somewhere" (the
  player's own name shows while its microphone is sent: the relay answered and the capture runs)
  and "usually you want those on the right" (the list at the right edge, under mid-height, each
  row ending at the margin, the speaker left of the name; 0.69's was at the left). The names are
  text fields in DefaultLabel3's embedded font (a Label drew its text lower than its field in the
  game, cut). The list is emptied when the match ends or is left (`sv_stop`'s sites). In Blackout
  (the interface forced hidden) it shows only while Tab is held, with the Tab list (the owner,
  2026-11-11: "we need to also show in blackout only when tabbed that someone is speaking";
  HUD.keptOnTab). 0.69's speaker over heads was not picked.
- **The radio** (the DLL): a teammate from 10 m on (15 m 2026-11-13, 25 m before), or either player
  dead (a spectator too: no body in the world), through a filter
  fitted to 0.69's `_radio` quick-command recordings (high-pass 405 Hz, peaks and dips up to 6.3
  kHz), retail static under it (`tutorial_interference_02`, -44 dBFS; half of it in the mix from
  #1635 until 2026-11-14, whole again since: below), and retail's squelch at the
  start and end of a far utterance (the static lead-in and tail of 0.69's `_radio` phrases: 0.69 has
  no separate squelch file). Near: plain, no squelch. The sounds are compiled into the DLL
  (`client-voice/make_radio_sounds.py`). The switch at 10 m takes one 20 ms frame: the plain voice
  ramps out and the radio in over it (until 2026-11-14 the filter went on or off from the next
  frame, only the ears' gains ramped). An utterance that starts on the radio has its squelch, one
  that starts plain none, as before.
  **Loudness** (2026-11-13): #1635's +6 dB on speech is undone for near voices, which play as
  decoded, as on revision 58 (the owner, after playing revision 58: "sound close by right now was
  very good in game"). The radio's words are raised 3 dB (`radio::VOICE`; the owner: "static is
  mostly fine, it is just that the general sound is a bit quiet"), its static kept where #1635
  put it in the mix (half of retail's; `RADIO_BED` 0.5 now that the mix adds no gain). Measured
  with a voice through the automatic gain at the default sensitivity, a narrow one (to 1.3 kHz)
  and a wide one (to 7.6 kHz): by ITU-R BS.1770's K weighting the radio's fitted makeup already
  gave its words the near level (narrow -28.6 near, -29.4 far; wide -27.7, -26.8 dB of full
  scale), but a band-limited voice of the same energy is heard quieter: its loudness summed over
  critical bands as Zwicker sums it (third-octave bands to the 0.23) needed 2.9 and 1.7 dB more
  (3.9 and 3.1 at the highest sensitivity), so +3 dB. With it the far words are within 0.1 and
  1.3 dB of the near (`a_radio_voice_is_as_loud_as_a_direct_one` holds 2). The static alone
  -47.7 (K), about 21 dB under the far words. Near speech peaks at 0.15-0.22 of full scale at the
  default sensitivity, 0.3-0.45 at the highest, under the soft limit's knee (0.5); full to one side
  (+3 dB in that ear) only the highest sensitivity's loudest reaches into the knee (0.63).
  **Louder, the static as live** (2026-11-14, the owner in the local test of the plain voice and
  the radio): "voice chat in general can be louder by x1.5 for people who can always tune it
  down", then "make radio effect like it is currently on main" and "static like live". The plain
  voice plays 1.5 times as decoded at the full "Voice chat" volume (`LOUDER`, +3.5 dB; the
  slider still turns it down to nothing); the radio's filter, words and squelch stay as they
  were; its static is retail's level again (`RADIO_BED` 1.0, as revision 58 played it; half of
  it could not be heard in the test). With it a plain voice at 9.99 m (-3.6 dB of itself) is
  within about a dB of the radio's words it turns into, so the step at 10 m below is
  gone. Near speech now peaks at 0.22-0.33 of full scale at the default sensitivity, still under
  the soft limit's knee (0.5); the highest sensitivity's loudest reaches into it.
- **Plain in the world** (2026-11-14; before it, 2026-11-13, "directional near": plain within 15 m
  at one loudness, panned by a law of the DLL's, equal power by the sine of the bearing).
  - The owner, 2026-11-14, of the merged and unreleased #1782:
    - "let's do directional in 10 m and it should also be really directional and fall the further
      from 10 meters you go", "like normal voice does";
    - "the game already has audio engine", "maybe we can tie into it?";
    - "no the voice fall of start immediately and drops completely at 10 meters, radio starts at 10
      too" (they floated "maybe drop completely at 15 and radio starts at 10" and withdrew it:
      "no", "this isnot good");
    - "should be still fine at 9m";
    - "fall off should not be big", and, told that a voice 6 dB down at 9 m would come back
      full on the radio at 10 m, "so you always hear normally".
  - So:
    - under 10 m (`PLAIN_TO_M`) a speaker is heard plain, as a sound in the game's world: from its
      side, and quieter the farther it is, gently: 0.7 of itself (3 dB down) at 9 m (`AT_9_M`,
      the one number of the fall; 0.5, 6 dB, in the first build of the same day);
    - at 10 m the plain voice is cut and the radio starts (`RADIO_FROM_M`): a switch, as it was
      at 15 m. Nothing of the radio nearer, nothing of the plain voice farther;
    - the two distances are two constants (patch 66, `voice.rs`) and may differ: with the plain
      voice reaching past the radio's start both are heard between them, with the radio starting
      past the plain voice's end neither;
    - dead or a spectator (either of the two): the radio only, as before. Free for all: the same
      rule for everyone heard. Team modes: teammates only. The relay and the wire: unchanged.
  - **The engine's part.** Each frame (`game_world.after_move`) the exe asks the engine's own pan
    of a point sound, `sound_scene::calculate_3d_pan` (0x57D70: the function every point sound's
    voice gets its output matrix from at each sound tick), for each speaker: with the game world's
    sound scene (the game's listener: the camera's place, front and top, as every game sound has
    it), the sound world's panning table, a proxy of ours of 144 zero bytes (a point sound) and the
    speaker's place: its offset from the listener, feet to feet (so one on the same floor is at the
    ears' height), added to the listener's place as the engine keeps it. On the game's thread: the
    function reads only atomics and the table ("The game's sound engine"). No listener yet (the
    world's first tick): the centre.
  - **Not the engine's** (said plainly):
    - the gain by distance is the patch's own arithmetic, in the form of the engine's distance
      presets (a `volume` curve, the distance not under 1 m, held to 0..1): two linear keys, 1 to
      1 m and `AT_9_M` at 9 m, on to the cut. With 0.7 it is the stock `weapon_shot_40` within
      0.3 dB and `weapon_echo_close_mid` within 0.4 dB from 1 to 10 m (the table above; with 0.5 it
      was `weapon_echo_loud` within 0.5 dB). No stock preset
      is loaded and the engine's curve reader (0x25F30) is not called: that needs a `.spl` asked
      of the resource manager, and a number in the patch is turned by ear sooner;
    - the characters' own voice presets are not used: a scream is 19 dB down at 9 m, the engine's
      law for a sound with no preset 13 dB (the owner: "fall off should not be big");
    - the output: the DLL still plays through waveOut (the default device, stereo, its own
      volume option), not through the game's XAudio2: no surround layout, no game volume, no
      low-pass behind the listener (the engine's coefficient is not used);
    - `LEVEL` (the square root of 2): the engine pans a point sound at constant power, 0.707 in
      each ear ahead, and holds a voice's volume to 1; the plain voice ahead and beside the
      listener is raised to 1 in both ears, as decoded: revision 58's near level (the owner then:
      "sound close by right now was very good in game") and the radio's.
  - **What a listener gets** (the plain voice straight ahead, both ears, against the voice as
    decoded: until now every near voice's level, and the radio's words' loudness; at the side the
    same power, 3 dB more in that ear and nothing in the other):

    | the speaker at | 1 m | 5 m | 8 m | 9 m | 9.5 m | 9.99 m | 10 m and past |
    |---|---|---|---|---|---|---|---|
    | until now (#1782) | plain 0 dB | 0 | 0 | 0 | 0 | 0 | plain 0 dB to 15 m, then the radio |
    | now | plain 0 dB | -1.4 | -2.6 | -3.1 | -3.3 | -3.6 | the radio, 0 dB |

    (Since `LOUDER`, above, the plain column is 3.5 dB higher and the step is gone.) So the voice
    came back about 3.6 dB louder where it turns into the radio. Both levels stay
    as the owner left them (the near voice beside the listener, the radio): `LEVEL` or the
    radio's `VOICE` would move a level that was tuned by ear. `AT_9_M` is what sets the step: 0.5
    (6 dB down at 9 m) made it 7.2 dB, which the owner's "so you always hear normally" ruled out;
    1 would be no fall and no step.
  - **The interface** (ABI 3 in #1801; 4 with the microphone test): `sv_hear(place, left, right, radio)` tells a place's whole state in
    one call (`voice_abi::Heard`: plain with its two gains, the radio, or both), in place of the
    radio's mask and a pan a place (`sv_radio`, `sv_direction`), which could disagree. The DLL
    has no law of its own left: it mixes the plain voice by the two gains and the radio at unity,
    each ramped over a frame from the last frame's. The mix is stereo (waveOut 16 kHz 16-bit
    stereo, since 2026-11-13); the radio and the squelch are centred.
- **Free for all: voice for all** (the owner, 2026-11-13: "in FFA I would enable voice chat for
  all"): in a free-for-all every player hears every other of the match (each is a side of its
  own, so by teams nobody heard anyone), by the same rule of near and far. One decision a side,
  from the match's mode: the relay's `registry::VoiceChannel::of` (a team's places, or the
  match's), which routes the forwards and the ROSTER (so voice ids, and the mutes, cover every
  player); the game's `channel` in patch 66 (patch 142's free-for-all, read from the options),
  which the roster's `mate` follows for the hearing, the mutes, the Tab menu's voice entries and
  the talking list. Team modes as before: teammates only.
- **Automatic gain** (the owner, 2026-11-07: as 0.69): before Opus, the microphone goes through
  speexdsp's preprocessor, noise suppression and automatic gain on, as 0.69d0's voice_recorder
  turns them on (0x2006281CA; its level `voice_chat_preprocess_gain`, 12000 of 1..32768, set
  every frame at 0x200621B6D). A quiet microphone is brought up and a loud one down to one
  loudness; "Microphone sensitivity" moves that aim 6 dB either way (0.5 is 0.69's level). One
  preprocessor a match, so the gain learnt carries from one press to the next. 0.69's voice
  detection (open mic only) and echo cancellation are not used: push-to-talk only. The listener's
  side plays each speaker as decoded, as in 0.69 (each `voice_player` at volume 1.0), but for a
  ceiling (2026-11-13, against a modified game that skips the gain: a full-scale square wave
  decodes at -0.3 dBFS): a speaker's 20 ms frame over -12 dBFS RMS is brought down to it at once,
  the gain coming back 0.5 dB a frame (and at unity for each new utterance). Speech through the gain never reaches it (measured -13.6
  dBFS at most, at the highest sensitivity and a full-scale voice; -19 at the default); only the
  gain's first fifth of a second after a sudden shout at the highest sensitivity (up to -8 dBFS)
  is held. The mix's soft limit is #1635's (its +6 dB undone for near voices 2026-11-13: the radio, above).
- **The DLL is Rust** (`client-voice/src`), with static libopus for the codec and static speexdsp
  (its own FFT, not FFTW) for the preprocessor. A worker owns
  its socket, codec, jitter buffers and WinMM capture/playback devices. The game communicates
  through the small typed C interface defined once in `crates/voice-abi`; both sides compile
  against it. All exports and the ABI version must match before the game enables voice. A
  missing or incompatible DLL leaves voice off. Capture opens when PTT starts and closes on
  release. One press transmits 30 s at most, then requires release before another starts.
- **Build and packaging:** `nix develop ./srp-026g -c bash srp-026g/client-voice/build.sh` builds
  the Windows x86 DLL with the pinned Rust/MSVC toolchain and checks its ABI. `release/build-zip`
  runs that build and includes the DLL through `make_client_026g.py --voice`; the updater installs
  it with the rest of the ZIP. No separate updater allowlist is needed.
- **Push-to-talk never outlives a match:** the exe releases it at every stop and before every
  start, and the DLL starts and stops released (a key held when a match ended once opened the
  microphone in the next: the review of 7b653ac9).
- **Malformed input** (the owner, 2026-11-06: "dropped and logged", never stopping the game for
  those who receive it): the relay and the DLL read each datagram once into a typed message; a
  truncated, oversized, extended or unknown one, an AUDIO of more than 2 frames or with a frame
  the games' encoder does not make (its TOC byte, RFC 6716 3.1, and its length: "Limits" above),
  a session with no seat or from another IP, or a source or a talker over its rate is dropped, never forwarded, and counted: the relay logs a line
  per source IP at most every 10 s (1024 sources apart, the rest summed), the DLL one line at most
  every 10 s (its first drop at once). A frame the decoder still refuses is concealed. A receive
  error no longer ends the DLL's tick; `sv_start` reads the host within 256 bytes. The saved
  mutes (`srp_voice_mutes`) are read once a match: a record not 8 hex digits is left out and
  logged. A datagram of a seat whose IP the match server does not know yet (the game starts
  voice as it connects, before its 0xD8) is too early, not malformed: ignored, neither counted
  nor logged; the DLL says HELLO every 250 ms until WELCOME (then every 2 s).

## Bandwidth

- Talking: Opus 16 kbps, 2 frames of 20 ms per datagram (about 80 bytes of audio + 10 header +
  28 IP/UDP), 25 datagrams/s, about **24 kbit/s up** for the talker.
- Each teammate gets **24 kbit/s per teammate talking**. srp sends the speaker's stream to each
  of up to 9 teammates (10v10): about 216 kbit/s per talker. Two talking at once in each team:
  under 1 Mbit/s for a match.
- Idle: a HELLO every 2 s (about 40 bytes). The match transport's traffic is unchanged.
- The most a modified game can push into each teammate (2026-11-13): 30 AUDIO a second of 2
  frames of 120 bytes, about 68 kbit/s with IP/UDP (3 times a real talker); it was 60 of 400
  bytes, about 205 kbit/s.

## Desync, resync, replays

- Voice never touches the simulation:
  - another socket on both ends;
  - not the match transport, the input relay, the hash checks or the world uploads.
- A resync (0xE5), a world restore, or a late join changes nothing for voice. A resynced player
  keeps talking and hearing.
- Replays (`replay.rs`) and the match recordings (`record.rs`, `SRP_RECORD_DIR`) are made from
  the match socket's datagrams only, so **no voice is ever in a replay or a recording**.
- Releasing or reseating a voice seat stops its old team routing immediately, without waiting
  for housekeeping. An utterance cannot migrate with the session to another team.
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

- **Radio effect: on.** A teammate within 25 m (`VOICE_NEAR`; 15 m and directional since 2026-11-13; 10 m and in the world since 2026-11-14) is heard plain; farther, through
  the radio filter (band-pass 300-3400 Hz, light clipping, a little noise). Same volume near and
  far: not proximity chat. Each client decides from its own simulation.
- **Push-to-talk: P**, the stock `ptt` action, rebindable in Controls.
- **Dead players talk to their team: yes.**

## Added by the owner (2026-10-25)

- **Muting** (moved to the Tab list's player menu 2026-11-06, above; the Esc menu's panel of
  2026-10-25 is gone):
  - a key "mute the last speaker" in Controls (default M only if M is free in every group;
    removed 2026-11-09, the Tab menu does it);
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

## Microphone test (the owner, 2026-11-14; built, not played)

The owner: "can we add feature to test your microphone? you press button, it records for 10
seconds, then plays it back". Until it, a player could not tell whether the microphone works, or
how it sounds, without asking a teammate in a match.

- **Where.** Options, Sound tab, the last of the voice controls (under "Microphone sensitivity"; under
  the stock PTT checkbox until it was removed, 2026-11-14): one button, "Test microphone".
- **A press.** The microphone is recorded for 10 s, then the recording plays back once, then the
  button is as before. The button itself shows the state and the seconds left ("Recording 7",
  "Playing 4"). A press while it records or plays stops it.
- **What is heard** is what a teammate standing beside the player would hear: the recording goes
  through a transmission's path (noise suppression, the automatic gain at the sensitivity, the
  codec) and is played plain, centred, no radio, at the "Voice chat" volume. The sensitivity and
  the volume are the sliders' as they stand on the screen at the press, also before they are
  accepted: move a slider, test again.
- **Wherever the options open:** in the lobby with no match, and in a match.
- **Whatever "Use microphone" and "Disable voice chat" say:** it is local, and how one checks
  before turning them on.
- **Nothing of it leaves the game:** not sent to the relay or to anyone, not written to disk,
  dropped from memory when the playback ends or the test is stopped.
- **In a match talking wins.** A push-to-talk press stops the test and transmits as usual; the
  test cannot start while push-to-talk is held. Voices from others keep playing during the test.
- **It stops by itself** when the options close (the Settings window, or the whole menu), when a
  match starts loading and when a match ends. As built, also when the options go to another tab:
  its button is not seen there.
- **No microphone, or it cannot be opened:** the button says so for a moment ("No microphone",
  2 s) and returns. The same for no playback device ("No sound output").
- **Not asked, not built:** a level meter, a device list (built since: "Microphone", below), a saved
  recording.

As built:

- **The DLL owns the test** (`client-voice/src/mic_test.rs`): one machine, `Test`, and one
  transition, `Test::step(state, event, now, devices)`. The exe and the movie only show what it
  reports.

  | state, and what it holds | `Press` | `Stop` | `Talk` | `Pass` | `Play` |
  |---|---|---|---|---|---|
  | `Idle` | push-to-talk held: stays. Else the microphone is opened: `Recording`; none, or no playback device: `Missing` | stays | stays | stays | nothing |
  | `Recording`: the open capture device, the press's sliders, when its 10 s end, a preprocessor and an encoder of its own, the frames recorded so far | `Idle` | `Idle` | `Idle` | the frames captured since the last pass are preprocessed, encoded and kept (500 at most); the device failed: `Missing`; the 10 s over: `Playing`, or `Missing` when not one frame came | nothing |
  | `Playing`: the press's volume, a decoder, the frames left | `Idle` | `Idle` | `Idle` | no playback device: `Missing` | the next frame decoded, at the volume; none left: `Idle` |
  | `Missing`: the microphone or the playback device, until when | as `Idle` | `Idle` | stays | its 2 s over: `Idle` | nothing |

  `Press` is the button (with the two sliders, and whether push-to-talk is held), `Stop` the
  options closed or on another tab or a match loading or over, `Talk` push-to-talk starting a
  transmission, `Pass` a pass of the worker (every 5 ms), `Play` the playback device taking its
  next 20 ms. A state that goes takes what it holds with it: the capture device is closed, the
  recording freed.
- **The capture device and push-to-talk.** `Recording` owns the open device and nothing else has
  it. A transmission's start is the test's `Talk`: the test ends and its device is closed before
  the transmission opens its own, so nothing the test captured is ever sent. A press while
  push-to-talk is held starts nothing.
- **Without a match.** The DLL used to run only between `sv_start` and `sv_stop`. A press with no
  worker now starts one for the test alone (`runtime.rs`, `run_test`): the playback device and
  the test, no socket and none of the relay's code. It leaves by itself when the test is over
  (100 ms after, for the device to play out what it was handed), closing its devices on its own
  thread. In a match the match's worker runs the test in its loop and mixes the playback under
  the others' voices, before the mix's soft limit. The game's asks reach the worker through a
  `Door` (the last one stands); a worker that leaves shuts it, so a press is never left with a
  worker that has gone: the DLL starts another. `sv_start` and `sv_stop` end a test's own worker
  as they end a match's.
- **A teammate's path.** Each captured frame goes through a preprocessor and an encoder made for
  the test (as a match's first press finds them), and is kept as the frame a transmission would
  have sent (one the relay would not forward is not kept): about 20 kB for 10 s, in memory only.
  The playback decodes them as a speaker heard directly is decoded (the ceiling of a loud
  speaker too), at the press's volume, centred. The DLL's tests hold it sample for sample to what
  a teammate's game plays of the same microphone (the relay's messages, the jitter buffer, the
  mix).
- **The seconds.** The recording lasts 10 s of the worker's clock; its seconds left are the
  clock's, counted up (10 at the press, 1 in the last). The playback's are those of the frames
  left.
- **ABI 3** (in #1803; 4 with `sv_hear`) (`voice-abi`): `sv_test_press(volume, sensitivity)` (each 0..1), `sv_test_stop()`,
  `sv_test_state() -> u32`, the word of `MicTest`: `Idle`, `Recording(seconds)`,
  `Playing(seconds)`, `NoMicrophone`, `NoPlayback`.
- **The exe** (patch 66, three more lohooks). `options.callback`: the sound tab shown asks
  `srp_mic_test`, answered `root.srp_mic_test(text)`, the button's whole text in the running
  language; `srp_mic_test("<volume> <sensitivity>")` (the sliders' steps, 0..100) is a press,
  `srp_mic_test("stop")` the Settings window closed (the menu behind it stays);
  the DLL is loaded here when no match loaded it yet (none of this ABI: no answer, and no
  button); the game's own `options_tab_changed` stops the test. `game.tick` (every frame, in a
  match or not), while the main menu is shown: the button is told its text again when the DLL's
  state, or its second, is no longer the one it shows; the DLL is asked nothing while the menu is
  closed, or before the sound tab first asked.
  `options.deactivate` (the menu's Resume or key, the lobby left for a match's loading, a match's
  end): the test stopped. `match_client.connect` and the match's end stop it too.
- **The movie** (`flash/main_menu`, `SrpMicTest.as`): the tab's own button (`SetingsButton`, as
  the Graphics tab's), one width whatever it says. It shows the exe's text and keeps no state and
  no clock; a press is no change for Apply; the window closing says stop.
- **Texts:** `st_srp_mic_test`, `_recording`, `_playing` (`%1$s` the seconds), `_no_microphone`,
  `_no_playback` in every dictionary, read by the exe.

## Microphone (the owner, 2026-11-14; built, not played)

The owner named a microphone list the next voice patch (2026-11-11), then asked: "did we create
selector for microphone?" (2026-11-14). Until it, voice always used Windows' default recording
device.

- **Where.** Options, Sound tab, with the voice controls: a list "Microphone", under "Microphone
  sensitivity" and over "Test microphone", in the look of the movie's other lists.
- **What it lists.** "Default" first, then each capture device by its name as Windows shows it,
  whole ("Microphone (Realtek(R) Audio)", not WinMM's 31 characters).
- **A pick takes effect at once**, without Apply (the owner, 2026-11-14: "does microphone chooser
  work when you switch mic but not apply it? — it should"): from the pick on, "Test microphone"
  and push-to-talk open the picked device, from their next press, as the test takes the two
  sliders as they stand on the screen.
- **Saved** as the other options are: Apply keeps it in `user.cfg`, across restarts; Cancel, or
  leaving without Apply, puts the saved one back, and the saved device is used again.
- **"Default"** is Windows' default recording device: what the DLL opened before the list (WinMM's
  mapper, the console role; not the default communications device, which `waveInOpen`'s
  `WAVE_MAPPED_DEFAULT_COMMUNICATION_DEVICE` would open instead).
- **Unplugged.** A saved device that is not there: the default is used, and the list shows
  "Default" (nothing to apply); the pick stays saved, so the device is used again once it is back.
- **Plugged in while the game runs:** it is in the list the next time the list is opened (the tab
  shown again, or the list pressed open); nothing more.
- **Not asked, not built:** a level meter, a choice of the output device.

As built:

- **Found again by its endpoint ID**, the ID WinMM reports for its device
  (`DRV_QUERYFUNCTIONINSTANCEID`, Windows from Vista on and Wine: `{0.0.1.00000000}.{<a GUID>}`):
  the same while the device is unplugged and plugged in again, whatever WinMM's number for it.
  `user.cfg` keeps it as `srp_microphone <id>` (empty: the default); a value that is not an ID (a
  hand-edited file) is the default. IDs are compared whatever the case of their letters.
- **The DLL** (`client-voice/src/microphone.rs`, the part its tests run with a made-up list; Windows'
  part in `windows.rs`):
  - the list (`sv_microphones`): WinMM's capture devices in its order, each with its endpoint ID
    and its endpoint's whole friendly name (MMDevice's `PKEY_Device_FriendlyName`, asked by that
    ID); without the whole name WinMM's (cut at 31 characters), without either the ID; a device
    with no ID is not listed (nothing to save it by). Windows is asked on a thread of the DLL's
    own, which starts COM for MMDevice and ends it, so the game's thread is never in COM;
  - the choice (`sv_choose_microphone`): kept by the DLL; at each opening of the capture device (a
    push-to-talk press, the test's start) the WinMM device with that ID is looked for, and opened;
    none (unplugged), or no choice: WinMM's mapper, as before.
- **ABI 5** (`voice-abi`): `sv_microphones(room, size) -> written` (a `Microphone`: its ID, 128 bytes
  with the NUL, and its name, UTF-8, 256; read once by the exe with `Microphone::read`) and
  `sv_choose_microphone(id or null)`; `MicrophoneId` is the one type of an ID on both sides.
- **The exe** (patch 66, no new hook): `options.callback`, `srp_microphone` (the tab shown, the list
  pressed open) is answered `root.srp_microphone(label, default, shown, id, name, ...)`, at most 16
  devices: shown is the saved device's ID as listed while it is there, else empty (the default);
  `srp_microphone(id)` (empty: the default) is Apply's pick, saved in `user.cfg` at once and told to
  the DLL, and answered the same; `srp_microphone_use(id)` is a pick, told to the DLL at once and
  not saved, and `srp_microphone_use` with nothing tells it the saved pick again (not answered;
  with no DLL loaded nothing). The DLL is told the saved pick as it is loaded (the first match, or
  the tab's first showing). No DLL: no answer, no list.
- **The movie** (`flash/main_menu`, `SrpMicrophone.as`): the movie's own list
  (`DropdownMenuSkinned`, as the Language list), its label in the sensitivity slider's label's look,
  in the row the PTT checkbox had; as wide as the slider's row, the open list at most 9 rows (more
  scroll by the wheel); a name wider than the list is cut at its edge. A pick in the open list is
  sent at once (`srp_microphone_use`, its id; the saved one picked again: nothing, so a saved device
  that is unplugged and shown as Default stays the one used once it is back), and is a change for
  Apply, as the Language list's; the list's revert (Cancel, the Settings closed or the tab changed
  without Apply) sends `srp_microphone_use` with nothing. A press on the closed list asks the exe
  again first, keeping a pick not applied yet while its device is listed; a pick whose device is
  gone is dropped, and the saved one used again.
- **Texts:** `st_srp_microphone` ("Microphone"), `st_srp_microphone_default` ("Default") in every
  dictionary, read by the exe.

## Validation

- The DLL's Rust tests cover protocol bounds, Opus round trips, jitter and packet loss/reorder,
  the fitted radio response, retail squelch, muting, timeouts and the press/release lifecycle,
  and the automatic gain: speech-like input from -50 to -13 dB of full scale comes out within
  2 dB of one level (-29 dB), the sensitivity's ends 12 dB apart; a plain voice mixed by the
  gains it is given (to the right: nothing in the left ear; half: half), the switch to the radio
  ramped over a frame with no step between two samples, the plain voice and the radio both heard
  when told so, and a radio voice's words within 2 dB of a plain one's (loudness summed over
  critical bands). `voice-abi`'s own test: a place's state through its one call.
- The match server's headless loopback test runs its actual UDP relay with two Rust client cores.
  Each sends a different synthetic tone through Opus, the wire, routing and jitter playback.
  The received audio's energy and frequency must match the other client. No microphone or fake
  codec participates. Relay tests also cover team isolation (and a free-for-all heard by all), mutes, authentication, rate limits,
  released seats, immediate reseats, stale utterances and process restarts in the same slot
  while the listener misses the new START packet.
- `scripts/tab-row-menu/check.sh` (Ruffle) runs the Tab menu with the voice entries: offered,
  muted, unmuted, not done, none without the exe's answer.
- `cargo test` in `client-patches/rust/`: the saved mutes' text (malformed records), plain under
  10 m (9.9 m plain, 10 m and 50 m the radio), the radio for the dead and for a broken place, the
  pan asked of the speaker's offset and its gains raised and lowered by the distance (1 to 1 m,
  `AT_9_M` at 9 m), the two distances apart (both between them, or neither).
- `check_voice_pan_026g.py` (Unicorn, `tools/check voice-pan`): the original exe's
  `calculate_3d_pan` called as the patch calls it, on a scene whose listener the engine's own
  `set_listener_properties` set and with the table its own constructor filled: the law (above),
  a listener moved and turned, what it reads and writes; and `get_listenet_position` (0x57560)
  as the patch calls it beside the pan every frame (the scene in EAX, the three floats' room in
  ESI): the place the engine keeps, nothing read but the scene's +120h, nothing written but the
  three floats. `check_engine_026g.py` holds the two new
  engine functions' thunks to the re-kit's rows. The old `check_voice_026g.py` (Unicorn over the 2026-10-25 hooks) went with
  those hooks; the lohooks' handlers are not run in Unicorn.
- The Windows/Wine DLL smoke test covers loading, the ABI, relay handshake, mute/volume updates,
  and stopping/restarting with PTT held. It uses a fake relay; the Rust integration above uses
  the real relay.
- The microphone test: the DLL's `mic_test` tests run the machine with a made-up microphone and
  clock, no device (10 s recorded through the clock's wrap and played back once, the seconds
  shown, the playback sample for sample a teammate's, the sliders' levels, a press stopping
  either part, talking winning, the stops, no microphone in its three ways, no playback device,
  a microphone ahead of the clock, the door); `cargo test` in `client-patches/rust/` the press's
  two sliders and each state's text; `scripts/mic-test/check.sh` (Ruffle) the button: none until
  the exe answers, its place, a press sending the sliders as they stand and no change for Apply,
  every dictionary's texts on its one line, the other tabs, the PTT checkbox hidden (also after a
  tab's change). The smoke test starts and stops the test with no match and in one, with whatever
  devices it finds.
- The built DLL's microphone test was run 2026-11-14 with the smoke test under Wine 11.8, in a
  sandbox with none of the machine's audio (no `/dev/snd`, no user sound server; a private
  PulseAudio server instead). With no device: "no microphone" for its 2 s, twice, in a match too.
  With a microphone and no playback device: "no playback device". With a null sink as the
  playback device and its monitor as the microphone, a made-up voice played into it for the
  recording: "recording" 10 to 1 a second apart, "playing" 10 to 1, idle at 19.99 s; the sink was
  given the made-up voice for its 9.5 s, then 10 s of the DLL's playback at the automatic gain's
  level (-23 dB of full scale falling to -28), then silence. In a match there: no START or AUDIO
  from the test, a push-to-talk ends it and sends its START, held it keeps the test from starting,
  the match's end stops it. A source that comes in bursts longer than the capture's four 20 ms
  buffers (the null sink's monitor behind a slow stream: 8.1 s of the 10 came through) loses the
  rest, for a transmission as for the test: not changed here.
- The Microphone list: the DLL's `microphone` tests with a made-up device list (Windows' order, the
  whole name over WinMM's, WinMM's without one, the ID without either, a device without an ID left
  out, the room's size; the chosen device found by its ID after another is unplugged, the default
  when it is not there or none is chosen, the ID whatever its case); `voice-abi`'s test: a
  `Microphone` reads back as made, a long name cut at a character, a broken one refused; `cargo
  test` in `client-patches/rust/` the pick's text and the list's shown pick (the saved device
  while listed, else the default); `scripts/mic-test/check.sh` (Ruffle) the list: none until the
  exe answers, Default and the devices by name, its place between the slider and the button, a
  pick pending, Cancel, Apply sending the ID and the empty ID for Default, an unplugged device
  shown as Default, the list asked again as it is pressed open with a pick not applied kept,
  every dictionary's two texts fitting; `client-voice/build.sh` built the DLL and `check_abi.py`
  passed (15 exports, `ole32.dll` added to its system imports). The smoke test prints the list
  and chooses a device that is not there (not run for this change: it needs Wine).

Real microphone capture and audible playback still require an in-game test with two games, a
microphone on one, and both near (walking round the speaker and away from it: the voice from its
side, falling to 10 m) and beyond 10 m (the radio). **Nobody has heard the plain voice in the
world yet** (2026-11-14): the patch's handler is not run in Unicorn, only the engine's function it
calls. One player with one microphone cannot test it as it is: push-to-talk is let go when its
window loses the keyboard, so the window that talks cannot be left to walk round it in the other.
What would do it (not built): a local switch of the DLL, an environment variable read once at
`sv_start` naming a 16 kHz mono WAV, with which the worker talks by itself, the file as the
microphone, in utterances with a pause between (so the squelch is heard too), whatever the key.
The microphone test needs one game and a microphone: in the lobby and in a match,
the button's countdown, how the playback sounds at a few sensitivities, a push-to-talk during it,
closing the options during it. Also check the Tab menu's voice mute, the
talking list's fade, the automatic gain with a quiet and a loud microphone, disable voice chat,
PTT rebinding after restarting, death/respawn and crossing a match boundary while PTT is held.
The Microphone list needs Windows (or Wine) with two capture devices: the list's names, a pick
heard in the test and in a match, the pick after a restart, one unplugged before the start
(Default shown, the default used) and plugged in while the game runs.
This PR is kept out of v35 and the following port-only release.
