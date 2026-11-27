# Play from Steam's "Play" button (Steam mode)

This page is for players who have **Survarium in their Steam library** and want Steam's normal **Play** button to start
the game on a **Skhid (Схід)** server, a private server for Survarium 0.69d. This is called **Steam mode**: you get the
Steam overlay, "playing Survarium" on your Steam profile, and Steam achievements on your real Steam account.

You do not need Steam mode to play: the start file of the [player guide](../guide-player.md#without-steam-the-start-file) works without
Steam. Server operators: see [For server operators](#for-server-operators) at the end.

> Steps marked **(to be confirmed)** have not yet been tried on a real PC from start to end. Please tell your server
> operator what you see.

## What you need

* Survarium **in your Steam library**, installed from Steam. Survarium is no longer on the Steam store (since June 2022):
  only Steam accounts that added it before still have it. Without it Steam mode cannot start.
* Steam **running and signed in** to that Steam account.
* From your server operator:
  * the **server address** (for example `203.0.113.5` or `play.example.org`), and the ports if they are not the usual
    ones (login port 65000, update check port 65001);
  * the four setup files: `steam-play-setup.cmd`, `steam-play-setup.ps1`, `steam-play-undo.cmd` and
    `steam-play-undo.ps1` (in the server's `scripts/steam-play` folder). Put them together in one folder.
* An account name and password for this Skhid server (a new name is normally created at your first sign-in). Use a
  password you use nowhere else ([Good to know](#good-to-know)).

**What you will end up with:** Steam's Play button opens the Survarium launcher, its Play button turns on, and the game
starts on the Skhid server. Nothing in the game folder is changed: the setup script writes one small file in your own
Windows profile, and the undo script removes it.

## Set it up (once)

1. Double-click **`steam-play-setup.cmd`**.
   If Windows shows "Windows protected your PC", click **More info** -> **Run anyway**. (The scripts are plain text;
   you can open them in Notepad and read them.)
2. A window asks:

   ```
   Server address (from your server operator, e.g. 203.0.113.5):
   ```

   Type the server address and press **Enter**.
3. You should see `Written: ...\Survarium-Steam\launcher.cfg` followed by the lines it wrote, and then one of:
   * `Server check: the update check at <server address>:65001 answers. Good.` You are done.
   * `WARNING: Server check: no answer from ...` The file is written, but the launcher's Play button will stay off
     until the server answers. Check the address, then ask your operator whether the server runs and whether port
     65001 is open.
4. Press any key to close the window.

**Language:** the game uses the language set in Steam (right-click Survarium -> **Properties** -> **General** ->
**Language**). You do not need to set it again in the setup.

**Another language or other ports:** open the folder with the scripts, click the address bar, type `cmd` and press
Enter. In the black window type, for example:

```
steam-play-setup.cmd -Server 203.0.113.5 -Language russian
```

Use `-Language` only if you want the game in a different language than Steam's. The game languages are `english`,
`russian`, `ukrainian`, `german`, `french`, `polish`, `spanish`, `portuguese`, `turkish` and `chinese`. To go back to
Steam's language, run the setup again without `-Language`. Add `-Port <number>` or `-UpdatePort <number>` only if your
operator gave you other port numbers. Running the setup again replaces its own file.

## Play

1. In Steam, open your library, select **Survarium**, press **Play**.
2. The Survarium launcher window opens. If Windows asks for permission for the Survarium updater, click **Yes**
   ([Troubleshooting](#troubleshooting)).
3. After a few seconds the launcher shows that you have the **latest version** and its **Play** button turns on.
4. Press the launcher's **Play**. The launcher closes and the game starts.
5. The game shows the **Steam login panel** with your Steam name. Press **Login**.
6. The **Import** panel opens. For about 4 seconds it shows "invalid user or password": that is normal, ignore it.
7. Type **your Skhid account name and password** (not your Steam password) and press **Import**.
8. You are in the lobby. A new account first gets the nickname screen and the tutorial, as usual.

Every start asks for the password again: Steam mode has no "remember me".

## Undo

Double-click **`steam-play-undo.cmd`**. You should see `Deleted: ...\Survarium-Steam\launcher.cfg`. It deletes only that
file, and only if the setup script wrote it; your game settings in the same folder stay. After that, Steam's Play button
starts the game as it came from Steam (which cannot reach any server today).

## Troubleshooting

| What you see | Why | What to do |
|---|---|---|
| The launcher's **Play button stays off** (it keeps checking) | The launcher cannot reach the server's update check (port 65001) | Run `steam-play-setup.cmd` again and read its "Server check" line. Check the address and port, your firewall and any proxy. Your operator can see in the server log whether your check arrived. |
| A Windows permission prompt for the Survarium updater at every start | The update service that Steam installs with the game is missing on this PC | Click **Yes**. Or let Steam reinstall the game, or use [Launch Options](#other-ways-to-start-in-steam-mode). If you click **No**, the updater does not run and Play stays off. |
| "**Steam initialization failed.**" or "User is not logged in" | Steam is not running, not signed in, or this Steam account does not have Survarium | Start Steam, sign in, try again. Steam accounts without Survarium cannot use Steam mode: use the [start file](../guide-player.md#without-steam-the-start-file). |
| "**invalid user or password**" for about 4 seconds when the Import panel opens | Normal: the panel always opens with this text | Type your name and password and press Import. |
| "invalid user or password" after you pressed **Import** | Wrong name or password | Try again (the password field is cleared). After 5 wrong tries in a minute the server ignores that name from your PC for a minute. Forgot it? Ask your operator to set a new one. |
| The setup says `NOT DONE: ... exists and was not written by this script` | Another tool already wrote that file | Look at the file first; to replace it, run `steam-play-setup.cmd -Force` from a black window (see "Another language" above). |
| The game starts in another language than Steam's | The setup was run with `-Language` (that language wins over Steam's) | Run `steam-play-setup.cmd -Server <server address>` again without `-Language`, or set the language in Steam: Survarium -> **Properties** -> **General** -> **Language**. |
| A hardware warning from the launcher | The launcher's own system check | You can continue. |
| The launcher's news area is empty or shows an error | The official news site is gone | Nothing; it does not block Play. |
| You used Steam's **Verify integrity of game files** | | Nothing to redo: the setup file is outside the game folder. |

## Good to know

* **Your password travels in clear text** from the Import panel to the server (the game sends it that way in Steam
  mode; the normal start sends it protected). Use a password you use nowhere else.
* **No "remember me"** in Steam mode: you type the password at every start.
* **Only Steam accounts that already have Survarium** can use Steam mode; it can no longer be added from the store.
* Steam achievements unlock on your real Steam account.

More limits that apply to every Skhid player: [player guide, Known limits](../guide-player.md#known-limits).

## Other ways to start in Steam mode

These two need no setup script and no update check, and skip the launcher window. Replace `<server address>` with the
server's address, and the folder with your own game folder if Steam installed it elsewhere.

**Steam Launch Options** (keeps Steam's Play button): in Steam, right-click Survarium -> **Properties** -> **General**
-> **Launch Options**, and enter:

```
"C:\Program Files (x86)\Steam\steamapps\common\Survarium\game\binaries\x64\survarium.exe" -steam -product_name=Survarium-Steam -server=<server address>:65000 -localization=english %command%
```

Then press Play: the game starts directly at the Steam login panel ([Play](#play), step 5). Delete the line to undo.
**(to be confirmed: that Steam starts this program in place of the launcher)**

**Desktop shortcut:** right-click the desktop -> **New** -> **Shortcut**, and enter as the location:

```
"C:\Program Files (x86)\Steam\steamapps\common\Survarium\game\binaries\x64\survarium.exe" -steam -product_name=Survarium-Steam -server=<server address>:65000 -localization=english
```

Then right-click the shortcut -> **Properties** and set **Start in** to the `game\binaries\x64` folder. Steam must be
running and signed in. Do not add the game to Steam as a "Non-Steam Game": Steam would then treat it as another game.

These two ways start the game without the launcher, so Steam's language setting does not reach the game: change
`english` after `-localization=` to your language (see [Set it up](#set-it-up-once) for the list).

These lines have no login key: the 0.69d game has no `-login_public_key` argument (it stops at once with "Command line
argument 'login_public_key' has not been registered"), and Steam mode does not use a key.

---

## For server operators

* **Setting:** `[network] update` in `srp.toml` ([config/srp.example.toml](../../config/srp.example.toml)). The default
  `""` means the IP of `network.login` with port **65001**. It is always on. For players on other PCs it must listen
  on a reachable address (`0.0.0.0`, which `srp setup --address` writes) and the **TCP** port must be open in the
  firewall and forwarded like the login port ([server guide, Letting players in](../guide-server.md#7-letting-players-in)).
* **What to give players:** the four `scripts/steam-play` files and the line `srp check` prints, for example:

  ```
  update     tcp 0.0.0.0:65001 (the Steam launcher's update check: "latest version", so its Play button turns on)
  steam      players run: steam-play-setup.cmd -Server 203.0.113.5   (scripts/steam-play; the game takes the language set in Steam, -Language russian etc. overrides it)
  ```

  The address shown is the first of `network.login`, `network.lobby_public` and `network.match_public` that is not
  `0.0.0.0`. When you changed the ports, the line has `-Port` / `-UpdatePort` too.
* **The log:** at start `update check on tcp ...`. When a player's launcher checks (at most once per address every 10
  minutes at the normal log level): `update check from <address>:<port>: download_type.php -> "http"` and
  `update check from <address>:<port>: update version "..." (http) -> "<you use the latest version!>" ...`. The launcher
  asks again every 30 seconds while its window is open.
* **Port:** any port works; the launcher takes `host:port`. A port below 1024 on Linux or WSL 2 needs the right to bind
  it: `sudo setcap cap_net_bind_service=+ep <path to srp>`.
* **WSL 2:** in WSL's default network mode the `172.x.x.x` address is reachable from that Windows PC only. The update
  check is TCP, so `netsh interface portproxy` could forward it, but the lobby and matches use UDP: use WSL's mirrored
  network mode ([server guide](../guide-server.md#wsl-and-other-pcs-to-be-confirmed)).

## How it works (technical)

Exe addresses are from the 0.69d Steam build (`survarium_launcher.exe`, `survarium_updater.exe`, image base 0x200000000).

* Steam starts `game\binaries\x64\survarium_launcher.exe` without arguments. The launcher reads `<game>\launcher.cfg`, then
  `%LOCALAPPDATA%\<product_name>\launcher.cfg` with the same key callback (0x20001487C, 0x200014490): every key in the
  second file replaces that key only. Lines are `key value`, read with `fgetws` from a file opened in binary mode, so
  the file is UTF-16LE (a leading BOM is skipped, 0x200014239). A line is read into 260 characters: a longer line is cut.
  A line without a space ends the reading of that file. Unknown keys are ignored (the setup's `srp_steam_play 1` marks
  the file as its own).
* The setup script writes (UTF-16 LE, no BOM, CR LF):

  ```
  update_server <server>:<update port>
  file_to_execute game\binaries\x64\survarium.exe
  arguments -product_name=Survarium-Steam -server=<server>:<port>
  steam 1
  srp_steam_play 1
  ```

  With `-Language <x>` the `arguments` line ends in ` -localization=<x>`.

* The launcher's Play button turns on when the updater reports "latest version". The updater asks `update_server`:
  `GET /download_type.php` (answered `http`) and `GET /?update_type=update&version=...&download_type=http&public_key`
  (answered `<you use the latest version!>`), HTTP/1.0. `update_server` is split at its last `:` into host and port.
  srp's responder: `servers/srp/src/update.rs`. The repeats every 30 s are logged at debug level.
* The updater needs administrator rights: the launcher starts the "Survarium-Steam Update Service (64-bit)" that Steam's
  install script installs, else it asks Windows to run the updater elevated (UAC) (0x20000594E, 0x20000A3C6, 0x20000A5A1).
* `file_to_execute` skips the BattlEye launcher, so `-exe` is not passed. The launcher adds
  ` -localization=<word> -steam -use_render_device_thread=1` after `arguments` (0x20000F104). The word is the launcher's
  own language (0x20000F247), chosen at its start from its `-language=` argument (0x20001C1C0), else Steam's game
  language (`ISteamApps::GetCurrentGameLanguage`, only with `steam 1`, 0x20001C490), else the `language` number in its cfg.
* The game keeps the first value of a repeated flag (setter 0x200089A80 returns when the flag is already set; the
  language is read from that flag, 0x200091550). So the setup writes no `-localization` by default and Steam's language
  decides; `-Language <x>` puts ` -localization=<x>` in `arguments`, ahead of the launcher's, and wins. Seen in a live
  test: with `-localization=english` in `arguments` the game started in English although Steam and the launcher were set
  to Ukrainian; with `-localization=ukrainian` it started in Ukrainian.
* No login key: the 0.69d game has no `-login_public_key` argument (it stops at an unknown argument), and Steam mode
  does not use a key (the server never asks a Steam-mode client for encrypted data; the Import panel sends the password
  itself). Seen working in a live test.
* The sign-in then shows the Steam-mode lines of [login.md](login.md) in the server log (`steam sign-in (method 2) ...`,
  `steam import sub 2 ...`).
