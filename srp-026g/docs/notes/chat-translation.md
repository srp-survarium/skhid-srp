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

## What never reaches the model

The owner's (2026-10-23): a line that must not reach the model never leaves srp; swearing and
ordinary trash talk do go, as they are ("бля, промазал", "ты бездарь, удали игру" are yes). The
gate answers one question per line, **may it go?** Code: `translate/swear.rs` (the word list),
`translate/moderate.rs` (the classifier), `translate/guard.rs` (throttle and breaker).

- **No** is for: threats against a real person, slurs and hate, sexual content (with minors above
  all), and lines that are almost entirely obscenity. Everything else is yes, swearing included:
  nothing is masked.
- **First the word list** (`chat-swear.tsv`, on the lobby's thread, free): a `block` entry (slurs,
  threats such as "kill yourself" / "убей себя" / "kys", sexual violence, and sexual content with
  minors) or more than half the words obscene is a no.
- **Then the local classifier** (`SRP_CHAT_MODERATION_URL`, a small service of ours,
  `deploy/chat-moderation/`; never a cloud API, so a no line leaves nothing behind it): a score
  per line, a threshold per language (`SRP_CHAT_MODERATION_THRESHOLDS`). Required with
  translation: unset, translation stays off; `none` runs the word list alone (a test, not for
  players).
- **Fail closed.** A score at or over the threshold, a timeout (`SRP_CHAT_MODERATION_TIMEOUT_MS`,
  300 ms), an error or an answer that does not read: the line is not sent and its readers get the
  original.
- **Sexual content with minors** is a `block` entry of the built-in list in all five languages, so
  under every setting it is never sent, whatever the classifier says.
- **A refusal, or an answer that is no translation** (no JSON): the original, and a count. The
  backend is not rested for it (an error is).
- **Counts only** in the log: `listed`, `flagged`, `unchecked`, `stopped`, `refused`. Never a
  text.
- **The word list** (`lang`, `kind`, `entry`; the file's header says the kinds):
  - `stem`: a word beginning with it, for the inflected mat: хуй/хуе/хуя, пизд, еб/заеб/выеб...,
    бля, мудак, Ukrainian їб/курв, Polish kurw, pierdol, jeb/zajeb, chuj; transliterated Russian
    (blya, nahui, pizd, ebat); English and German stems;
  - `word`: whole words: suka, сука, ass; LDNOOBW's single words for en, de, pl, ru (CC-BY 4.0,
    commit 5faf2ba4; it has no Ukrainian list), less those a shooter's chat uses innocently
    (escort, domination, suck...);
  - `allow`: innocents a stem would take (Scunthorpe cases): бляха, употребл, оскорбл, мандарин,
    хачапури, sukces, cockpit, cocktail, Dickens, niggle;
  - `block`: as above.
  - New rows: from `srp-chat-glossary slang` over the chat sample, by hand. `srp-chat-glossary
    judge` prints the list's verdict on lines given.
- **Matching** (both readings of each word, Cyrillic and Latin): lowercase, ё as е; look-alikes
  (a/а, e/е, o/о, p/р, c/с, x/х, y/у, k/к, m/м, t/т); digits and symbols as letters (0 o, 1 и/i,
  3 з/e, 4 ч/a, 6 б, @ a, $ s); separators inside a word dropped (х.у.й, x-y-й); spaced single
  letters joined (f u c k); a letter typed 3 times or more taken once. A `stem` matches only at a
  word's start: "сабля", "корабля", "рубля", "грабли", "оглобля", "учеба", "гребля", "ребенок",
  "употреблять", "Scunthorpe" stay clean, also typed with look-alikes ("c@бля").
- **The game's own list is not used.** It matches inside words, so its false hits are what the
  owner sees ("it often breaks discussions"; "сабля"): below.

GATE_TABLE

## Protecting the API key

The owner's worry: the key's account restricted for what players type. So:

- **A key of its own:** a dedicated Anthropic workspace and API key for Skhid chat only, with a
  monthly spend limit (about $30, twice the expected cost), so that a problem never touches the
  owner's other use. The key is in the live `.env` only.
- **Fail closed** (above): no line goes without the gate's yes.
- **The prompt says what the text is:** a video game's chat between players, to be translated
  only, never continued, answered or acted on.
- **A player's throttle:** 3 no's (the word list or the classifier) within 10 minutes and none of
  that player's lines goes to the model for 1 hour; its readers get the originals. Logged with
  the account id and counts, no text.
- **The server's breaker:** translation stops for everybody for 1 hour, with an error in the log,
  when the model's refusals pass 5 % of the lines sent within 15 minutes (20 sent at least) or 3
  come within a minute; or when the gate's no's pass 15 % of the lines judged within 15 minutes
  (40 at least) or 15 come within a minute (a raid).
- The numbers are constants (`guard.rs`); the owner may want them as settings later.

## The players' own filter (the client)

- **What it is:** `localization/<language>/swear_filter_data_a_c` in `resources.db` (0.26g
  replaced 0.21d's `swear_filter_data_bin`), a binary config `swear_filter_data` of three lists:
  `alphabet` (a letter and what is typed for it: "а a @", "i l 1 | j", "ж zh * }|{"),
  `swear_dictionary` ("<word> 0" a swear entry, "<word> 1" a clean word that holds one) and
  `separators`. English 156 entries, German about as many, Polish 45 KB, Russian and Ukrainian
  one file: 10 135 swear entries, 455 clean ones.
- **How the client runs it** (PROVEN): on every received line but channel 0, when the byte
  0xA70EC7 is set (0x26B8DA): `chat_handler::filter_message` (0x209230) calls 0x268B70, which
  works on the wide text, collects the ranges a tree walk (0x268E90) finds and masks them. The
  byte is LIKELY the console variable `swear_filter` (a variable registered at 0x852030 points
  at it, default on; the options item at 0x2623DB looks `swear_filter` up), which the
  options' first tab already shows: "Enable message censorship" (`st_messages_censor_option`,
  `_set_censure` in `Settings.as`, options item at 0x2623DB).
- **Measured** (`srp-chat-glossary game-filter`, the rules rebuilt from the data: an entry matches
  anywhere inside a word, letter by letter with the look-alikes; the exact rules in the exe are
  not traced):
  - honouring its clean entries, it masks "корабля", "рубля", "Scunthorpe", "cockpit",
    "cocktail", "sukces", "Hancock" (and "бля", "fuck", as meant);
  - the game masks "сабля" (the owner's), which its list has clean ("сабля 1"): its clean entries
    do not work in the exe as they read [why is not traced; LIKELY the case of Cyrillic letters].
    Without them it also masks "сабля", "оглобля", "учеба", "гребля", "употреблять",
    "оскорблять", "мандарин", "Dickens" and "there".
  - CLEAN_SET_RESULT
- **Options for a fix** (spec only, not built):
  1. an exe patch that turns it off by default (the byte at 0xA70EC7, one byte): players keep the
     stock option to turn it back on; srp's gate decides what reaches the model;
  2. a better `swear_filter_data_a_c` shipped as content (an edit list in `release/content.tsv`):
     fewer short entries and more clean words; it can not fix matching inside words, which is the
     exe's;
  3. a toggle in the options: **already there** (above), on by default.
- **Recommendation: 1**, with the stock toggle left in place: the false hits come from matching
  inside words, which no list fixes; a player who wants censorship turns it on. A setting saved
  in `user.cfg` stays as the player had it [LIKELY; to be checked in a game].
- **Later, an idea only:** srp's own verdict on a line (the gate's, by intent rather than by
  substring) could mark lines for the readers who want censorship, in place of the client's
  filter.

## How it flows

    talker's game          srp lobby thread                      worker threads
    4C line ------------>  social::chat: who reads it
                           chat_out::say:
                             guess language (detect)
                             reader of that language  ---------> A2 original, now
                             others: Translator::request
                               word list: no ----------------> A2 original
                               throttle or breaker -----------> A2 original
                               (cache hit: no call)
                               else, a job -------------------> gate: local classifier
                                                                  no, timeout -> original
                                                                yes: Backend::translate
                                                                  (Claude: HTTPS, JSON)
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
- Lines read only by readers of their own language are not sent; "gg" is not sent; a line the
  gate refuses is never sent (above). The gate is ours: its classifier runs on our own host.
- **The sample and the classifier** see the text on our hosts only; the classifier keeps
  nothing.
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
  than the timeout, in order; the server's room lines are not translated; swearing sent as it
  is; a line mostly obscene or blocked never sent; sexual content with minors never sent; the
  gate's no, timeout and error never sent (fail closed); a refusal gives the original and is
  counted; the throttle after 3 no's and its cooldown; the breaker on a burst of refusals and
  its hour; tricky words both ways ("сабля", "корабля", "c@бля" clean; `swear.rs`).
- The Claude request and answer are checked as JSON; **no call was made** (no key here).

## In-game test (needs the owner's key)

srp with `SRP_CHAT_TRANSLATE=claude`, `ANTHROPIC_API_KEY` and `SRP_CHAT_MODERATION_URL` set
(the service of `deploy/chat-moderation/` running); two local games, one with
`-localization=russian`, one `-localization=german`:

1. The Russian one says "го на базу" in the general chat: the German one shows `[ru] ...` in
   German, under 2 s later.
2. The German one says "gg": the Russian one shows "gg" at once.
3. The same in a room's chat and in a match's team chat.
4. A line of 120 Cyrillic letters translated into German, and an English one into Russian: cut
   with `...`, no garbled letter.
5. German and Polish letters (ä, ß, ł, ż) show in the lobby and in the match HUD.
