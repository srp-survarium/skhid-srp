# Chat translation

The owner's of 2026-10-23: "auto-translate what players say into each reader's own language, using
a local model or a cheap cloud one"; the slang "after we collect enough data points"; voice later,
by speech to text, this text path, then text to speech. Code: `servers/lobby-server/src/v026g/`
`translate/` (backends, detection, glossary), `chat_out.rs` (who gets what, when),
`chat_sample.rs` (the slang sample), `src/bin/srp-chat-glossary.rs` (the glossary tools).

## Facts (0.26g, srp)

- **Every chat line passes through srp, in the lobby.** 0.26g carries chat on the lobby's UDP link
  (`login_chat.md` §5, `social.md` §1): the client sends `4C`, srp sends each reader `A2`. That is
  every channel: general (1), clan (3), private (4), match (5), team (6, 7), squad (8), clan team
  (9) and srp's room chat (10). Match and team chat too: a player in a match keeps its lobby link,
  and the match server sees no chat at all. srp's code: `social.rs` `chat`, `room_chat.rs`.
- **Who reads a line is decided by srp** (`social.rs`): the general channel to every subscriber
  (one room for every language: the owner's rule, `social.md` §9.6), private to one name, match
  and team to the match's players, squad, clan and room to their members; an ignoring player gets
  nothing.
- **srp knows each reader's game language.** The client's `4F` (subscription) ends with a language
  byte from its `-localization`: english 0, russian 1, ukrainian 2, german 3, polish 4, anything
  else 1 (0x26C36E); `45` carries it too. srp keeps it (`Player::language`). No client patch, no
  setting.
- **What a reader's game can show** (`social.md` §1.2):
  - `A2`'s body is a `u8`-length UTF-8 string read into a 256-byte stack buffer: **255 bytes**.
    Cyrillic is 2 bytes a letter, so 127 letters.
  - The chat inputs take **120 characters** (`SrvChatTab.as`, `SrvGameChat.as` `maxChars`): a
    Cyrillic line is up to 240 bytes and a translation into Cyrillic can pass 255. srp cuts it on a
    character and ends it with `...`.
  - The body is shown as it is after the client's own swear filter; never localized.
  - German and Polish letters: the game has German and Polish localizations, so its fonts have them
    [LIKELY; to be seen in a game].
  - Original plus translation does not fit in general (two 120-letter texts), so the reader gets
    the translation alone, marked: `[ru] go to the base, there's a sniper`.
- **The sender** sees its own line from its own client (0x26AD30 echoes it before it is sent): it
  never sees a translation of itself.
- **Where chat is written today:** nowhere with its text. The log has `Chat from <name> on channel
  <n> reached <k> player(s)`; a room keeps its last 20 lines in memory. `SRP_PACKET_LOG` (a debug
  switch, off) would hold raw datagrams.

## The rules

- **Off by default** (`SRP_CHAT_TRANSLATE`). Off, chat is byte for byte as before.
- **Each reader gets the line in its game language** (`4F`'s byte), on every channel above.
- **The original goes, at once,** to:
  - a reader of the line's own language;
  - a reader whose language byte is none of the five;
  - every reader of a line with no language: only shorthand, laughter, numbers or emoticons
    (`gg`, `ez`, `+1`, `хахаха`, `))`);
  - every reader of the server's own lines (sender id 0: "Alice joined the room"; the movie puts
    them in the player's language).
- **The line's language** is guessed on the server (`translate/detect.rs`): its letters (і ї є ґ
  Ukrainian, ы э ъ ё Russian, ą ę ł ś ż ... Polish, ä ö ü ß German) and its commonest words; a
  tie goes to the writer's game language. No model, microseconds a line. A wrong guess costs
  little: the backend says what the line is really in, and a reader of that language gets the
  original.
- **A translation is marked** `[<source>] <text>`, one line, at most 255 bytes.
- **Nobody waits long.** A reader waits for a translation at most `SRP_CHAT_TRANSLATE_TIMEOUT_MS`
  (2000 ms); then, or at once on any error, it gets the original. A backend that failed is not
  asked for 15 s: lines go as they are, at once.
- **Order.** Each reader gets its lines in the order they were said: a line still waiting holds
  back the later ones to that reader (at most the timeout).
- **Never in the lobby's way.** The backend runs on 4 worker threads; the lobby's thread only
  queues a job (64 at most; past them, the original) and picks up the answers on its next pass.
- **One call a line** for all the languages its readers need; a cache of 10 000 (text, language)
  answers, so a line said again, or "gg wp all", is asked once.
- **No name leaves srp.** The backend gets the text, the guessed language, the languages wanted
  and the glossary's terms of the line; no account, name, clan or address.

## How it flows

    talker's game          srp lobby thread                      worker threads
    4C line ------------>  social::chat: who reads it
                           chat_out::say:
                             guess language (detect)
                             reader of that language  ---------> A2 original, now
                             others: Translator::request ------> Backend::translate
                               (cache hit: no call)              (Claude: HTTPS, JSON)
                           each pass: chat_step                        |
                             poll answers  <---------------------------+
                             per reader, in order:
                               translation        -------------> A2 "[ru] ..."
                               timeout or error   -------------> A2 original

## The backend: compared

The scale: the repo has no chat counts. Assumed: **up to 5 000 lines a day**, 40 characters each,
read in **2 other languages** on average: 12 M characters a month to translate, 150 000 calls.

The host: `DEPLOY.md` gives **1 CPU and 1 GB RAM**, srp's container 512 MB (`SRP26_CPUS`,
`SRP26_MEMORY`), shared with the matches. A model on that core competes with them.

Local, measured 2026-10-23 on a Ryzen 9 5900X pinned to 2 cores, one short line at a time, warm,
on 40 real-style chat lines with slang (a VPS vCPU is likely 1.5-3 times slower):

| Local | Pairs | Disk | RAM (RSS) | ms a line (median / p95) | Quality on chat | Licence |
|---|---|---|---|---|---|---|
| opus-mt (Marian, CTranslate2 int8) | a model a pair: into English direct for all four; ru, de and pl among themselves through English (2 hops, about 130 ms) | about 80 MB a pair | 420 MB the 4 into English; 780 MB with 8 | 49 / 78 | fluent on plain lines; slang fails: "изи катка" "Lola, Ice skating", "слились" "merged", "low hp" horsepower, "wtf" "if", "camper" "Wohnmobil" | CC-BY 4.0 |
| Argos / LibreTranslate (CTranslate2 int8) | en to and from de, pl, ru, uk; the rest through English | 1.1 GB | 1.2 GB | 68 / 154 | about opus-mt's (several packages are opus-mt); ru to uk through English garbled | MIT code; models CC-BY or unstated |
| NLLB-200 distilled 600M (CTranslate2 int8) | all 20 directions | 600 MB | 730 MB | 484 / 688 | hallucinates: "лол, изи катка" "Oh, my God.", "аптечка" "pharmacy", "не стреляй по своим" "Don't shoot yourself" | **CC-BY-NC 4.0: not for commercial use** |
| M2M100 418M (CTranslate2 int8) | all 20 | 470 MB | 570 MB | 340 / 483 | worst: "gg ez" "Gdańsk", "snajper" "helicopter", random capitals | MIT |
| Qwen2.5-1.5B-Instruct Q4_K_M (llama.cpp) | all, one prompt | 940 MB | 1.1-1.7 GB | 375 / 624 | best into English ("i'm at zero", "good game guys"); poor into ru, de, pl ("anyone want to squad up?" "want to eat?"), Cyrillic in Polish | Apache-2.0 |
| Gemma-3-1B-it Q4_K_M (llama.cpp) | all, one prompt | 770 MB | 0.9-1.0 GB | 1293 / 1451 | unusable: wrong language, invented text, added swearing | Gemma terms |

No local model got "изи катка" (an easy match) or "слились" (gave up) right. None of them fits
srp's 512 MB beside the game for all directions; only opus-mt into English fits 1 GB.

| Cloud | Price | A month at our scale | Latency | Glossary | Notes |
|---|---|---|---|---|---|
| DeepL API Free | 0 for 500 000 characters a month | not enough: about 6 000 lines a month | about 0.3-0.6 s | yes (native) | key per account; texts may be used to improve DeepL on Free |
| DeepL API Pro | about $5.49 a month + $25 a million characters (2026 plans may differ: about EUR 22-27.50 a million) | **about $300** | about 0.3-0.6 s | yes (native) | best plain MT quality for DE/PL; no slang awareness beyond the glossary |
| Google Cloud Translation (NMT) | $20 a million characters, the first 500 000 free | **about $230** | about 0.2-0.5 s | Advanced edition | literal on slang |
| **Claude Haiku 5.5** (`claude-haiku-5-5`) | $0.10 / $0.50 a million input / output tokens | **about $10-15** (about 600 tokens in, 70 out a call) | about 0.6-1.2 s (not measured: no key here) | in the prompt (the line's terms) | understands slang, typos, transliteration and tone; one call for all languages; says the source language |

Prices are the vendors' public ones as of 2026-10 (DeepL's changed in 2026: check at sign-up).

## Recommendation

**Claude Haiku 5.5 (built).** About **$10-15 a month** at 5 000 lines a day (a few dollars at
1 500), a line typically **under about 1 s** later for a reader of another language (capped at
2 s), and no CPU or RAM on the game host. Reasons:

- The chat is slang, typos and transliteration; an LLM handles them where MT models translate
  word for word (the local results above).
- It is 20 times cheaper than DeepL or Google at this volume.
- Local models do not fit the host: 1 vCPU already carrying the matches, 512 MB for srp. A
  local backend would need a host of its own (the trait takes one without other changes).

## The glossary

- **The file:** `servers/lobby-server/chat-glossary.tsv`, built into srp. One row a term:
  `kind`, `en`, `ru`, `uk`, `de`, `pl`, `note`. In a cell, `|` between the ways players write it
  (the first is what a translation uses); a way ending in `*` matches every word it begins
  (`мосин*`: "мосинка", "мосинку").
- **Seeded only with the game's own names** (items, artefacts, maps, factions) in its five
  localizations: `srp-chat-glossary seed <folder>` (the five `localization/<lang>/localization`
  files from `srp-game-data get --dir <game folder>`, names from `SRP_GAME_DATA`): 363 rows.
- **Slang comes from real chat, not guesses** (the owner's): rows are added by hand after the
  sample below.
- **How each backend uses it:**
  - an LLM (built): the rows the line contains (at most 12), with their words in the languages
    asked, go with the line; the instructions say to use them;
  - an MT model (not built): each term found is swapped for a placeholder (`__T1__`) before and
    for the target's word after;
  - DeepL: its own glossaries (one per language pair, made from the file); Google: Advanced
    glossaries.
- **Fine-tuning:** not now. A small LoRA on a local model only makes sense with a local model and
  a few thousand corrected lines; later, if ever.

## Collecting slang

- **Today nothing keeps chat text** (Facts above).
- **The sample** (`chat_sample.rs`), **off by default**, on only on the owner's word:
  - `SRP_CHAT_SAMPLE_DIR` on srp's data volume (never a repository, never `private/` on a desk);
  - the public channels only: general, match, team, room. **Never private, squad, clan or clan
    team lines**;
  - a row: the UTC hour, the writer's game language, the guessed language, the channel, the text.
    **No account, name, id or address**;
  - a file a UTC day; files older than `SRP_CHAT_SAMPLE_KEEP_DAYS` (14) are deleted.
- **What players are told before it is on** (the operator's text, `A5`, or the site): "Public chat
  is kept for 14 days without names to improve translation."
- **The tool:** `srp-chat-glossary slang <files> --dict ru=ru_RU.dic --dict en=en_US.dic ...`
  lists, per writer's language, the words used most that are neither in that language's word list
  (a hunspell `.dic`) nor in the glossary. A person picks the rows to add.
- **The benchmark with and without the glossary** waits for this data: the slang-heavy lines will
  be taken from the sample, translated with the glossary's slang rows and without, and compared.

## Privacy

- **With the Claude backend the text of every translated line leaves the server** for Anthropic's
  API (in the US). Only the text goes, with the guessed language and the glossary's terms: no
  name, account or address. Under Anthropic's commercial API terms inputs are not used for
  training; they are kept a limited time for abuse checks.
- Lines read only by readers of their own language are not sent; "gg" is not sent.
- **What players are told** (before it is turned on; one line, on the site and the operator's
  text): "Chat is machine-translated: each line's text (never your name) goes to a translation
  service."

## Voice later: speech to speech (design only)

The owner's choice: **cascaded**, speech to text, then this text path (with the glossary), then
text to speech. Direct speech-to-speech models (SeamlessM4T v2, SeamlessStreaming, the cloud
real-time speech translators): **not chosen** (GPU-only or per-minute priced, no glossary, no
control of what is said).

- **The hook is the voice relay's** (`feat/voice-chat`, `voice-chat.md` "The relay"): the relay
  calls an `UtteranceTap` for each utterance (`started`, `audio` with the Opus frames, `ended`)
  and takes back `RelayOut::text` (wire type 85 TEXT: subtitles) and an audio stream (types
  86-8F). A translation tap needs nothing else from the wire.
- **Utterances are the push-to-talk presses:** a hold of P is one sentence or two, a natural unit
  for speech to text.
- **Nothing waits for it:** the original audio is relayed at once, as without the tap. The
  translation follows:
  1. the text, as a subtitle (85) for every teammate, in its language (this path: one call, the
     glossary, the cache), and into the team chat as a line if the owner wants;
  2. then, for a teammate of another language, the spoken translation (86-8F), played after the
     original.

      talker --Opus--> relay --------------------------------> teammates (original, now)
                         |
                         +--tap: buffer the utterance's frames
                              ended -> decode Opus 16 kHz -> speech to text
                                     -> text -> translate (glossary, cache)
                                     -> 85 TEXT to each teammate (subtitle, its language)
                                     -> text to speech -> Opus -> 86-8F to readers of
                                        another language

- **Speech to text:**
  - local: whisper.cpp or faster-whisper. `base` (74 M) runs about 3-10 times real time on 2 CPU
    cores (a 2 s callout in about 0.3-0.7 s) but is weak on Ukrainian, Polish and noisy speech;
    `small` (244 M) is good enough, about real time on 2 cores (about 2 s), 0.1-0.2 s on a GPU;
    `large-v3-turbo` needs a GPU;
  - cloud: OpenAI `gpt-4o-mini-transcribe` (about $0.003 a minute), Deepgram Nova (about $0.004-
    0.008 a minute), Google (about $0.016 a minute); about 0.3-0.8 s for a short clip.
- **Text to speech:**
  - local: **Piper** (MIT; voices for en, ru, uk, de, pl; about 0.1-0.3 s a sentence on one CPU
    core, 20-60 MB a voice);
  - cloud: about $15 a million characters (Google, Azure, OpenAI); no gain over Piper for short
    callouts.
- **The voice and the radio effect:** Piper speaks in its own voice, not the talker's (cloning,
  XTTS, needs a GPU and the speaker's consent: not planned). The radio effect is the listener's
  client's, after decoding (`voice-chat.md`), so it applies to the translated audio too.
- **Latency, from the release of P:** speech to text 0.3-0.8 s (cloud) + translation 0.6-1.2 s +
  Piper 0.2 s + the relay: **subtitle about 1-2 s, translated audio about 1.5-2.5 s** after the
  talker stops; the utterance itself came first. A 2 s callout is heard translated 3.5-4.5 s after
  it began: too late for "sniper on the roof" as audio, in time as a subtitle beside the original.
- **Cost per hour of speech** (one talker talking for an hour; a player talks a few minutes an
  hour): speech to text $0.18-0.96 (cloud), translation about $0.02 (Haiku: about 50 000
  characters), Piper 0: **about $0.2-1 an hour of speech**; with cloud speech instead of Piper,
  about $1-2 more.
- **Where it runs:** not on the game host (1 vCPU). The text path stays in srp. Speech to text in
  the cloud to start; Piper on a small box of its own, or the GPU box if whisper goes local later.
- **Privacy:** voice audio leaving the server is more than chat text: the players are told before
  any tap is on ("Voice is transcribed by a service to subtitle and translate it"); nothing is
  kept.
- **The first step:** subtitles only. A tap that sends each utterance to cloud speech to text and
  the text through this path to 85 TEXT, one language a teammate. It shows whether the text is
  good enough before any voice is synthesized.

## Tests

- `translate/` and `chat_out.rs` unit tests; `tests/core_tests/chat_translate_tests.rs` with fake
  backends: each reader in its language and the same-language reader the original; shorthand
  never sent; a failing backend gives the original at once; a stuck one holds a reader no longer
  than the timeout, in order; the server's room lines are not translated.
- The Claude request and answer are checked as JSON; **no call was made** (no key here).

## In-game test (needs the owner's key)

srp with `SRP_CHAT_TRANSLATE=claude` and `ANTHROPIC_API_KEY` set; two local games, one with
`-localization=russian`, one `-localization=german`:

1. The Russian one says "го на базу" in the general chat: the German one shows `[ru] ...` in
   German, under 2 s later.
2. The German one says "gg": the Russian one shows "gg" at once.
3. The same in a room's chat and in a match's team chat.
4. A line of 120 Cyrillic letters translated into German, and an English one into Russian: cut
   with `...`, no garbled letter.
5. German and Polish letters (ä, ß, ł, ż) show in the lobby and in the match HUD.
