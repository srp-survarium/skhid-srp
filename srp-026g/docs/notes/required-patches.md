# The game requires its release's patches (patch 49)

The rule after the crashes of 2026-10-21. Code:
`client-patches/mount_patches_026g.asm` (README patch 49, `client_patch.md` section 3.46);
the release: `release/build-zip`, `release/lib.sh` (`exe_required_patches`,
`check_exe_requires`).

- **Why:** players whose updater could not download patch_26/27 started the revision 27 exe
  without the launcher; the exe skipped the missing patches and the game crashed in matches
  (unknown item ids in profiles, other animation ids in world snapshots). One supported client
  version: never a game on older content than its server.
- **The rule:** each release's exe requires every patch of its release's offered-patch list
  (`release/patches.tsv` with the release's new patch; the test environment's
  `patches-test.tsv`). Built in at build time; nothing read at run time.
- **What a player sees:** a required `resources.db.patch_<n>` missing, the game stops at its
  start, before the menus: "Cannot open resources.db.patch_<n> (error 2). Please run the Skhid
  launcher again." The lowest missing n. With the launcher nothing changes: its updater
  fetches the patches before PLAY is on.
- **Not required:** a patch no longer listed, or any patch of a build without
  `--required-patches=` (local and test builds): skipped when missing, mounted when there.
- **The release:** one command as before. `release.sh` passes its list to `build-zip`
  (`--patches`), which builds both exes requiring its patches and reads them back;
  `release.sh` and `deploy` refuse a ZIP whose exe requires other patches than the server data
  offers. Every patch a release offers must be downloadable by every player: add the Drive
  mirror (`release/add-mirror`) when it is published.
- **Not a sign-in check:** the server does not ask what was mounted; the exe's own start does
  (the revision byte already refuses an older exe at sign-in).
