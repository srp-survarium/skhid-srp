# Screenshot keys (0.26g)

The owner, 2026-11-16: "two buttons to the game, one to add screenshot and another without ui
screenshot", "f11, f12", "stores in game folder somewhere".

## The keys

- **F11 saves the picture on the screen**, interface and all.
- **F12 saves the same frame without the interface**: the world as drawn, before the hud or the
  lobby's panels go over it. Nothing is hidden on the screen; only the file is without it.
- **Options, Controls**: two rows of the game keys, "Screenshot" and "Screenshot without UI",
  rebound like any other key (`bind screenshot <key>`, `bind screenshot_no_ui <key>` in
  `user.cfg`).
- **Everywhere the game draws**: the login screen, the lobby, loading, a match, the death screen,
  a replay; with a chat or text box open too (a letter bound to them takes pictures while typing).
- **The key goes on to the game** as before (F11 and F12 do nothing else).

## The files

- **Where:** the folder `screenshots` in the game's folder (beside `game` and the launcher), made
  at the first picture.
- **Name:** the local date and time, `2026-11-16_21-03-55.png`; a further picture of the same
  second is `..._2.png` up to `_9`.
- **Format:** PNG, 24-bit, the window's size in pixels, lossless. About half the raw size: 3 to
  4 MB at 1920x1080.
- **Never written over**, never deleted by the game.

## What a player notices

- **A short stop** while the file is written (the frame waits for it): 20 to 190 ms at 1280x720 in
  the test runs, more at a larger window.
- **Nothing on the screen** says a picture was taken (a message would be in the next picture).
  The run's `srp_patches.<date>.log` has a line a picture: its name, size and time, or why none
  was taken.

## Hide interface moved to F9

- F11 was "Hide interface" (`hide-ui-key.md`). **Its default is F9 now**, for every player: the
  action is saved under a new name (`hide_interface`), so the F11 an older build saved for it is
  dropped and the three defaults stand. A player who had put it on another key gets F9 too.
- In a replay the same key opens the replay's controls: the lobby's replay texts say F9.

## Without the interface, what is in the picture

- **In:** the 3D world with its post-effects (the pain's desaturation, the scope's view), the
  weapon in hands.
- **Out:** everything the interface's movies draw: the hud, the crosshair, the minimap, the
  markers over heads, chat, the lobby's panels, the cursor, the hud's own screen effects (blood
  drips, red edges, the grenade's flash), and the fps line.

## Not done

- No picture when the back buffer is not four bytes a pixel (an HDR or 10-bit swap chain) or
  wider than 8192: the log says so. The game's own is R8G8B8A8.
- No sound or message for a picture taken.
- No JPEG, no choice of folder.

## How

- Exe patch 152 (`client-patches/rust/patches/src/screenshot.rs`, README "Two keys save a
  screenshot"): the keys' presses from every poll of the keyboard, the back buffer read back
  before the interface is drawn (F12) or before the frame is presented (F11), the PNG written by
  the patch's own encoder (`screenshot/png.rs`).
- The keys are the action table's two spare entries (the game's `torch` and `night_vision`,
  which nothing reads), named `screenshot` and `screenshot_no_ui`.
- No simulation change, no server change. The two Controls labels and the replay texts are
  `client-patches/localization/*.tsv`.
