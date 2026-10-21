# The UI at high resolutions (4K)

How the game's Flash UI follows the screen size, and what changes at 3840 x 2160. Found in the
movies' source (`srp-026g/flash`) and the engine (2026-10-21); not yet seen in a game at 4K.

- **The owner (2026-10-21):** "make ui usable in 4k"; "for 4k it is mostly important to center
  things". Centring first; a bigger scale only where the UI is too small, and the owner chooses it.
- **The engine decides nothing.** Every movie gets the whole window as its viewport with
  Scaleform's `SM_NoScale` (Vostok's `flash_movie::SetViewScaleMode` ignores its argument;
  `flash_renderer::present` re-sets the viewport when the window size changes): `stage.stageWidth`
  and `stage.stageHeight` are the window's pixels, and each movie scales itself.
- **Movies that scale themselves** to a 1920 x 1080 design (by the width below an aspect of 1.85,
  by the height above): `lobby_menu` (`LobbyMenu.onStageResize`), `hud` (`HUD.onResize`),
  `login_menu` (by the height, in steps of 0.01). At 4K the factor is 2 and their layout is
  1080p's, twice as big. Bitmaps (icons, backgrounds) are drawn at twice their size; text is
  expected to stay sharp (Scaleform draws glyphs at the size on the screen), not yet seen.
- **Drawn 1:1 at every resolution** (placed right, half as big on the screen at 4K as at 1080p):
  - `main_menu` (Esc menu, Settings; centred by its `Layout`): Settings is about 650 x 565 px,
    17 % x 26 % of a 4K screen (34 % x 52 % at 1080p).
  - `replay_player` (the replay's controls, anchored to the edges).
  - `player_icons` (markers and names over players): the engine gives the position in pixels
    (`base_game_scene::point_to_screen` 0x23DEC0) and a scale of 1 to 3 by distance
    (`game_world_ui::update_player_icons` 0x23AA10), never by the screen size.
  - `cursor` (about 18 x 25 px: 1.2 % of a 4K screen's height).
  - In the lobby, what it puts on the stage itself: tooltips, context menus, dragged icons, the
    sync and loading-wheel clips. Message boxes are scaled (`Message.setScaleFactor`).
  - The HUD's crosshair, on purpose (`1 / scaleFactor`: the spread is in pixels).
- **Fixed (`feat/ui-4k`):**
  - The HUD's hit-direction marks circled the stage's half size in HUD units: the screen's
    centre at 1080p only, the bottom right corner at 4K (off centre at 1440p and 720p too). They
    now circle the centre in the marks' container's own units (`DamageIndicator.screenCentre`).
  - The crosshair's half-pixel offset was in HUD units: one pixel right and down of 1080p's
    place at 4K. It is now in pixels.
- **Not a 4K problem:** the lobby's fade (`fade_in`/`fade_out`, a 2048 px square) and
  `show_loading_wheels` are never called (no name of them in the exe or the movies).
