# Agent instructions

## Rust client transition

- For networking and client-runtime migration work, read
  `srp-026g/docs/PLAN-RUST-CLIENT.md` and the latest entry in
  `srp-026g/docs/RUST-CLIENT-WORK.md` first. Update the status and handoff with
  exact evidence; distinguish implemented, tested, played and released.
- This transition uses the existing Rust arena framework (`rust_patches.md`),
  generated bindings and hooks. Rust logic belongs in safe modules; native ABI
  glue remains generated NASM. The arena placement and lock rules below apply.

## Commits and pull requests

- Do not add `Co-Authored-By: Claude ...` (or any other AI co-author trailer) to commit messages.
- Do not add "Generated with Claude Code" or similar AI attribution lines to pull request descriptions.
- Commits and PRs are authored by the git user only.

## Releases and the patch revision

- A feature PR never changes `REVISION` in `srp-026g/client-patches/patch_026g.py` and never adds a
  row to the revision table of `srp-026g/client-patches/README.md`, even when it changes the exe.
- The release sets them: each release of the players' download is one new revision (the live one + 1),
  recorded on `main` with its hashes and tagged `026g-rev<n>` on the commit it shipped. A revision that
  was not released does not exist; nothing reserves a number in advance.
- Releases go through `release/release.sh` and the `skhid-release` skill
  (`.claude/skills/skhid-release/SKILL.md`). Nothing players feel (a maintenance, a restart, the swap)
  happens without the owner's go at that moment.
- A PR that changes what a match computes (replays of older games would play out otherwise) adds the
  row `next<TAB><what>` to `srp-026g/crates/session/src/v026g/simulation_changes.tsv`; the release
  that ships it makes `next` its own revision, and replays recorded before it are refused.
- New game content ships as `resources.db.patch_<revision>` files listed in `release/patches.tsv`; the
  server reads everything about maps and items from the game data it is given
  (`server-game-data.db`), never from tables kept in code.

## Game content (resources.db)

`release/content.tsv` lists everything that should be live in players' `resources.db`: the published
base patches made outside this repo (maps, ported weapons and items), the Flash movies
(`srp-026g/flash/<movie>`), resource-porter content packs, and the edit lists
(`srp-026g/scripts/content-edit/*.json`, applied in order from the base). Its header says the format.

- A content change is a change of its source in the PR: an edit list (new, or a retune of one), a
  movie's ActionScript, a pack's new version. A new edit list or pack also adds its row to
  `release/content.tsv` in the same PR; a movie edit or a retune needs no new row.
- The release builds the patch: `release/build-content <revision> --full` builds everything the manifest
  lists, compares it with what players have (the stock db and every listed patch), and packs only the
  files that differ, printing each one and why; anything live that the manifest does not make stops
  it, save what an earlier version of a listed pack added and its new version no longer makes (left
  as it is, listed). Then `release/release.sh --resources <that patch>`. Without `--full` it is a quick
  build for local tests (no check of players' files with the patch, no item tables, `QUICK-BUILD` in its
  folder), which `release.sh` refuses.
- Never assemble a patch folder by hand, and never build a patch from the server game data or from
  one release's files: a file left out, or taken from the wrong revision, reverts what players have.

## Game symbols

Names of the 0.26g executable's functions, for reading and patching it, are in
`tools/re-kit/v026g/` (tab-separated; `README.txt` there explains the columns):

- `functions.tsv`: reviewed names (`reviewed`, `verified`, `observed`);
- `inferred.tsv`: unreviewed names (`inferred-high`, `inferred-medium`): check the code before patching;
- `abi.tsv`, `abi_inputs.tsv`: what is known of their calling conventions;
- `identity.json`: the exact executable's hashes, image bases and sections;
- `lookup.py`: `python tools/re-kit/v026g/lookup.py <name or address>`.

Earlier builds have their own folders (`tools/re-kit/v021d/`, `tools/re-kit/v0100b/`,
`tools/re-kit/launcher/`); shared tools are in `tools/re-kit/common/`. The kit is meant to grow: when
work identifies a function that is not there, or identifies one with more confidence, add a row to the
TSV (one line per name, no bulk JSON). The client patches' own addresses and reasoning are in
`srp-026g/docs/notes/client_patch.md` and `srp-026g/client-patches/README.md`.

- Add re-kit rows (names, `abi.tsv` / `abi_inputs.tsv`) as soon as a finding is made, in the same PR.
- Client patches call engine functions through the engine-function library
  (`srp-026g/client-patches/engine_026g.tsv`, `client_patch.md` §0.5): add a row there for a new
  function instead of a hand-written call. Only varargs, XMM-argument calls and tail jumps stay hand-written.
- New client-patch code goes in the `.srpx` arena as plain C or NASM, linked by
  `srp-026g/client-patches/link_026g.py` (`client_patch.md` §0.3): one `ARENA` row and its sources, hooks
  by symbol, no fixed addresses; run `python link_026g.py --write` and commit `arena_026g.lock.json`.

## Types and states

- Make illegal states unrepresentable. A state is one enum whose variants carry exactly the data valid
  in that state: no independent boolean flags, no sentinel values (`u32::MAX` or `-1` for "none" is an
  `Option`), no fields that mean nothing in some states. A machine's transitions are one function from
  (state, event) to the new state and its actions, total over the events.
- Prefer state transitions, or a clear separation into objects (a type per thing, each owning its
  data), to boolean flags. A flag that changes what other fields mean is a state the type should name.
- Parse, don't validate. Turn raw input (packets, game memory, files, settings) into typed values once,
  at the boundary, and let the code inside take only those types and never check them again. Distinct
  units get newtypes (game time, ids, addresses), with their arithmetic (wrap-around) defined once.
- The same holds for new client-patch C code: a tagged union and one transition function, raw memory
  read once into it.

## The world format and its check

Every world a game uploads is checked before another game gets it: by the match server, and by each
receiving game itself (patch 87), with the same code (`srp-validate`'s typed views over `world-frame`,
`srp-026g/docs/notes/packet-validation.md`). That parser is the world format's one definition.

- A change to what a game writes into a world changes that parser in the same PR. That covers an exe
  patch that saves or loads state, a new item class, weapon state, map object or anomaly kind, and a
  record or snapshot version. Each new field gets its view, its real name and its domain in the parse
  (a closed set of values is an enum, a bool is the game's 0x00/0xFF, an index is bounded by its table).
- Check it against real worlds. `tools/check` runs `world-check` (the server's check over the private
  corpus of live recordings and local test matches) and `world-drift` (the game's own check against
  the server's) for every change their inputs cover. Check the whole corpus: on 2026-11-07, 176 real
  worlds that a sample of a third of the recordings passed were refused.
- A change to what games write also needs worlds from a game that has it: play a local test match
  (local runs record into the corpus) and check it before the PR is merged.
- Never loosen a rule only to make a real world pass. Find what the game writes and why it reads it
  safely (its reader in the exe), as #1434 did. A game bug behind it gets a board ticket.
- Merging PRs that touch the same code: build and test the combined tree (main with each of them)
  before the second merge. Two PRs that each pass can break main together (#1463).

## Rust changes

- Design for a **pit of success**: the simplest supported use of an API should be correct by
  construction. Use ownership, lifetimes, types and explicit state transitions to enforce its
  invariants, rather than requiring callers to remember special ordering or cleanup rules.
- Give each piece of state a clear owner and a deliberate mutation API. Avoid overlapping setters,
  callbacks or wrappers that independently control the same value. When multiple mechanisms must
  coexist, encapsulate their coordination and ordering. At engine/FFI boundaries, keep raw access
  behind that owner instead of exposing competing ways to bypass it. Verify initialization,
  transitions and teardown as well as the settled state; Rust's type safety alone cannot enforce
  the lifecycle rules of an external engine.
- For Rust changes, test and lint affected crates under `nix develop` (`tools/check` runs the
  `cargo:<crate>` checks of every crate your change reaches); use workspace Clippy only when
  shared APIs, configuration or dependencies affect multiple crates.
- A PR adds no clippy or rustc warnings: fix the ones your change causes. A lint that is noise for this
  code is allowed once in `[workspace.lints.clippy]` (`srp-026g/Cargo.toml`) with a comment why.

## Checks while iterating and before a PR

- Run `tools/check`, also before pushing or opening a PR: it picks the checks your change affects
  (the rows of `tools/checks.tsv` whose inputs differ from the merge-base with origin/main) and
  skips the ones that already passed on the same inputs (a cache shared by every worktree), so a
  clean merge from main or a trivial edit reruns nothing it does not touch. `tools/check <name>`
  runs one check (still cached), `tools/check --list` shows what your change selects.
- Don't run checks by hand except to debug one. When you add a check, add its row to
  `tools/checks.tsv` with every input it reads; a changed file it reports as `not covered` has no
  check yet (docs need none).
- Where it has no row yet, use the patch's own self-tests and Unicorn check or the movie's
  Ruffle harness (`selftest_026g.py` runs at a release, in `release/release.sh`: not during
  development). Shell/docs/page-copy changes do not need game builds, Unicorn or unrelated
  server tests. World save/load changes also need `check_world_roundtrip_026g.py`. Reserve full
  suites for shared infrastructure changes that affect them or release validation.
- Broaden checks only for a concrete dependency or failure; do not repeat passing checks without
  relevant changes. Report validation limits and pre-existing failures without expanding the PR.
