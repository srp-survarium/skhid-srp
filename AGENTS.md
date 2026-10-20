# Agent instructions

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

## Rust changes

- Before opening a PR, run `cargo clippy --workspace --all-targets` (in `srp-026g`, under `nix develop`)
  and the tests of what you changed.
- A PR adds no clippy or rustc warnings: fix the ones your change causes. A lint that is noise for this
  code is allowed once in `[workspace.lints.clippy]` (`srp-026g/Cargo.toml`) with a comment why.
