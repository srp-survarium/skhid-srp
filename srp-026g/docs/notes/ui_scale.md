# The UI at high resolutions (4K)

How the game's Flash UI follows the screen size, and what changes at 3840 x 2160. Found in the
movies' source (`srp-026g/flash`) and the engine (2026-10-21); not yet seen in a game at 4K.

- **The owner (2026-10-21):** "make ui usable in 4k"; "for 4k it is mostly important to center
  things". Then, from players on revision 27: the Esc menu (Back, Options, Quit to Windows) "is
  small", "or the settings themselves are small": the whole `main_menu` and every window it
  opens take the lobby's scale, and so do the cursor, the replay's controls and what the lobby
  puts outside its layout (tooltips, context menus, dragged icons, the sync clip). Nothing
  changes at 1080p.
- **The engine decides nothing.** Every movie gets the whole window as its viewport with
  Scaleform's `SM_NoScale` (Vostok's `flash_movie::SetViewScaleMode` ignores its argument;
  `flash_renderer::present` re-sets the viewport when the window size changes): `stage.stageWidth`
  and `stage.stageHeight` are the window's pixels, and each movie scales itself.
- **The scale:** a 1920 x 1080 design, by the width below an aspect of 1.85, by the height above:
  1 at 1920 x 1080 and 1920 x 1200, 1.33 at 2560 x 1440, 2560 x 1600 and 3440 x 1440, 2 at
  3840 x 2160.
  - `lobby_menu` (`LobbyMenu.onStageResize`), `hud` (`HUD.onResize`), `login_menu` (by the
    height, in steps of 0.01) take it as it is, below 1 too (0.67 at 1280 x 720).
  - `main_menu` (`MainMenu.onStageResize`), `replay_player` (`MainTimeline.onStageResize`),
    `cursor` (`MainTimeline.onStageResize`) and the lobby's stage-level parts
    (`MainScene.overlayScale`) take it never below 1: drawn 1:1 at 1080p and below, as before.
  - Bitmaps (icons, backgrounds, the cursor) are drawn at the scale (twice their size at 4K);
    text is expected to stay sharp (Scaleform draws glyphs at the size on the screen), not yet
    seen.
- **How each part follows it:**
  - `main_menu`: the movie is scaled and its `Layout` centres the Esc menu and Settings in the
    scaled stage; message boxes are scaled by `Message.setScaleFactor`. Our FOV slider is placed
    from gamma inside Settings, so it moves with it.
  - Dropdown lists (Settings' monitor list, the lobby's): `PopUpManager.show` puts them on the
    stage; its canvas now takes the scale its owner is drawn at, never below 1 (main_menu and lobby
    trees).
  - `replay_player`: scaled; the layout anchors the controls to the scaled stage's edges, the
    free-view button to its right edge.
  - `cursor`: the symbol is scaled about its origin, the hot spot (about 23 x 30 px at 1:1).
  - Lobby: the tooltips' container and the drag canvas are scaled (`ToolTip`, `DragManager` place
    in their container's units); the context menu (`SrvContextMenu`, every right-click menu:
    inventory, friends, squad, chat, clan, results, rooms) is scaled, placed from the mouse in its
    own units, its submenu goes left at the screen's right edge, and the menu is kept 4 units in
    from every edge; the sync clip is scaled and centred.
- **Still 1:1:** `player_icons` (markers and names over players): the engine gives the position in
  pixels (`base_game_scene::point_to_screen` 0x23DEC0) and a scale of 1 to 3 by distance
  (`game_world_ui::update_player_icons` 0x23AA10), never by the screen size; a fix is in the exe.
  The HUD's crosshair, on purpose (`1 / scaleFactor`: the spread is in pixels).
- **Fixed (`feat/ui-4k`):**
  - The HUD's hit-direction marks circled the stage's half size in HUD units: the screen's
    centre at 1080p only, the bottom right corner at 4K (off centre at 1440p and 720p too). They
    now circle the centre in the marks' container's own units (`DamageIndicator.screenCentre`).
  - The crosshair's half-pixel offset was in HUD units: one pixel right and down of 1080p's
    place at 4K. It is now in pixels.
- **Not a 4K problem:** the lobby's fade (`fade_in`/`fade_out`, a 2048 px square),
  `show_loading_wheels` and Settings' brightness screen (`showScreenAdjust`) are never called (no
  name of them in the exe or the movies).
